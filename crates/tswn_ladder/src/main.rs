use std::collections::{HashMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::{Local, Utc};
use cron::Schedule;
use tokio::io::AsyncWriteExt;
use tokio_postgres::{Client, NoTls};
use tswn_core::Runner;
use tswn_core::player::PlrId;

const ELO_SCALE_FACTOR: f64 = 50_600.0;
const HANDICAP_SCALE: f64 = 88.3;
const BASE_K_FACTOR: f64 = 80.0;
const K_HANDICAP: f64 = 6.0;
const HANDICAP_DECAY: f64 = 0.97;
const MATCHCOUNT_DECAY: f64 = 0.9999;
const PERTURBATION_RANGE: f64 = 40.0;
const DROPRATE: f64 = 0.9996;
const MEMORY_LIMIT_MB: u64 = 256;
const HANDICAP_PRECISION: u32 = 2;
const BATTLE_ERROR_LOG_LIMIT: usize = 3;

const MAX_ACTIVE_MATCHES: usize = 7;
const MAX_PASSIVE_MATCHES: usize = 8;
const MAX_MATCHES: usize = 11;
const SUPPLEMENT_RESERVED_SLOTS: usize = 2;
const PRE_SA_MAX_MATCHES: usize = MAX_MATCHES - SUPPLEMENT_RESERVED_SLOTS;

const SECOND_ACTIVE_MATCHES: usize = 5;
const SECOND_PASSIVE_MATCHES: usize = 9;
const GLOBAL_EXPLORATION_RATIO: f64 = 0.2;
const GLOBAL_EXPLORATION_MIN: usize = 2;
const HANDICAP_ZERO_EPSILON: f64 = 1e-9;
const HANDICAP_SPARSE_PERSIST: bool = true;

const DEFAULT_GROUPS: &[&str] = &["1a", "1b", "2a", "2b", "3a"];

#[derive(Debug, Clone)]
struct Config {
    database_url: String,
    cron_schedule: String,
    groups: Vec<String>,
    run_once: bool,
    rank_log_dir: PathBuf,
}

impl Config {
    fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        let cron_schedule = env::var("LADDER_CRON_SCHEDULE").unwrap_or_else(|_| "* * * * *".to_string());
        let groups = env::var("LADDER_GROUPS")
            .ok()
            .map(|value| {
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .filter(|groups| !groups.is_empty())
            .unwrap_or_else(|| DEFAULT_GROUPS.iter().map(|x| x.to_string()).collect());
        let run_once = read_env_bool("LADDER_RUN_ONCE", false);
        let rank_log_dir = env::var("LADDER_RANK_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        Ok(Self {
            database_url,
            cron_schedule,
            groups,
            run_once,
            rank_log_dir,
        })
    }
}

#[derive(Debug, Clone)]
struct PlayerRow {
    id: i32,
    name: String,
    loaded_score: f64,
    score: i32,
    original_score: i32,
    initial_rank: i32,
    rank: i32,
    total_matches: usize,
    base_matches: usize,
    second_matches: usize,
    supplement_matches: usize,
}

#[derive(Debug, Clone)]
struct HandicapData {
    basic_value: f64,
    disturbed_value: f64,
    count: f64,
}

#[derive(Debug, Clone)]
struct HistoryRecord {
    win_or_lose: &'static str,
    from_member_id: i32,
    to_member_id: i32,
    current_score: i32,
    from_rank: i32,
    to_rank: i32,
    from_name: String,
    to_name: String,
    runs: String,
    number: String,
    expected_win_prob: f64,
    actual_result: f64,
}

#[derive(Debug, Clone)]
struct MatchStats {
    min_matches: usize,
    max_matches: usize,
    avg_matches: f64,
    final_squared_loss: f64,
}

#[derive(Debug, Clone)]
struct HandicapStats {
    avg_match_count: f64,
    avg_handicap: f64,
}

#[derive(Debug, Clone)]
struct UpdateResults {
    history_records: Vec<HistoryRecord>,
    changed_handicaps: HashSet<PairKey>,
    avg_delta: f64,
    match_quality: f64,
    failed_matches: usize,
    executed_matches: usize,
    skip_persist: bool,
    handicap_stats: HandicapStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PairKey(i32, i32);

impl PairKey {
    fn new(a: i32, b: i32) -> Self { if a < b { Self(a, b) } else { Self(b, a) } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MatchPair(usize, usize);

impl MatchPair {
    fn key(self) -> (usize, usize) { if self.0 < self.1 { (self.0, self.1) } else { (self.1, self.0) } }
}

struct Updator {
    client: Client,
    config: Config,
    rng: fastrand::Rng,
    battle_error_count: usize,
    battle_error_suppressed: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_env()?;
    let (client, connection) = tokio_postgres::connect(&config.database_url, NoTls).await.context("connect PostgreSQL")?;

    tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("PostgreSQL连接错误: {err}");
        }
    });

    eprintln!("[db] PostgreSQL连接池已创建");

    let mut updator = Updator::new(client, config);
    if updator.config.run_once {
        updator.run_round().await?;
        return Ok(());
    }

    updator.run_forever().await
}

impl Updator {
    fn new(client: Client, config: Config) -> Self {
        eprintln!("[start] Updator已启动");
        Self {
            client,
            config,
            rng: fastrand::Rng::new(),
            battle_error_count: 0,
            battle_error_suppressed: 0,
        }
    }

    async fn run_forever(&mut self) -> Result<()> {
        let schedule = parse_cron_schedule(&self.config.cron_schedule)?;
        eprintln!("[cron] schedule={}", self.config.cron_schedule);

        loop {
            let now = Utc::now();
            let Some(next) = schedule.upcoming(Utc).find(|instant| *instant > now) else {
                bail!("cron schedule has no upcoming instant");
            };
            let wait = (next - now).to_std().unwrap_or_else(|_| Duration::from_secs(0));

            tokio::select! {
                _ = tokio::time::sleep(wait) => {
                    if let Err(err) = self.run_round().await {
                        eprintln!("[tick] 天梯匹配轮次异常: {err:#}");
                    }
                }
                signal = shutdown_signal() => {
                    signal?;
                    eprintln!("[shutdown] 收到退出信号");
                    return Ok(());
                }
            }
        }
    }

    async fn run_round(&mut self) -> Result<()> {
        eprintln!("[tick] 天梯匹配轮次开始 {}", Utc::now().to_rfc3339());
        for group in self.config.groups.clone() {
            eprintln!("[group:{group}] 开始");
            if let Err(err) = self.process_all_players(&group).await {
                eprintln!("[group:{group}] 处理异常: {err:#}");
            }
        }
        self.check_memory_usage();
        eprintln!("[tick] 天梯匹配轮次结束");
        Ok(())
    }

    fn check_memory_usage(&self) {
        if let Some(used_mb) = current_rss_mb() {
            eprintln!("[mem] {used_mb}MB");
            if used_mb > MEMORY_LIMIT_MB {
                eprintln!("内存超限 ({used_mb}MB)，准备退出程序...");
                std::process::exit(1);
            }
        }
    }

    async fn process_all_players(&mut self, group: &str) -> Result<()> {
        let mut players = self.load_players(group).await?;
        if players.len() < 2 {
            eprintln!("[group:{group}] 活跃玩家不足2人，跳过");
            return Ok(());
        }

        eprintln!("[group:{group}] 玩家={}", players.len());

        for player in &mut players {
            player.score = self.probabilistic_round(player.loaded_score * DROPRATE);
            player.original_score = player.score;
            player.total_matches = 0;
            player.base_matches = 0;
            player.second_matches = 0;
            player.supplement_matches = 0;
        }

        players.sort_by(|a, b| b.score.cmp(&a.score));
        for (idx, player) in players.iter_mut().enumerate() {
            player.initial_rank = (idx + 1) as i32;
            player.original_score = player.score;
        }

        let mut handicap_map = self.load_handicap_data(&players, group).await?;
        let first_matches = self.generate_base_matches(&mut players, &handicap_map);
        let base_matches = self.generate_second_matches(&mut players, &handicap_map, &first_matches);
        let supplement_matches: Vec<MatchPair> = Vec::new();
        let matches = base_matches.iter().copied().chain(supplement_matches.iter().copied()).collect::<Vec<_>>();

        let match_stats = self.analyze_match_distribution(&matches, &players, base_matches.len(), &handicap_map);
        eprintln!(
            "[group:{group}] 对局={} (base={}, extra={}) | 场次范围={}-{} | 均值={:.1} | loss={:.2}",
            matches.len(),
            base_matches.len(),
            supplement_matches.len(),
            match_stats.min_matches,
            match_stats.max_matches,
            match_stats.avg_matches,
            match_stats.final_squared_loss
        );

        let update_results = self
            .execute_matches_with_calibrated_elo(&mut players, &matches, &mut handicap_map, group)
            .await?;

        if update_results.skip_persist {
            eprintln!(
                "[group:{group}] 写入已跳过 | success={} failed={}",
                update_results.executed_matches, update_results.failed_matches
            );
            return Ok(());
        }

        self.persist_results(&mut players, &mut handicap_map, &update_results.changed_handicaps, group)
            .await?;
        self.update_ranks_and_logs(&mut players, update_results.history_records, group).await?;

        eprintln!(
            "[group:{group}] 完成 | avgDelta={:.1} | quality={:.1}% | success={} failed={} | handicapAvgCount={:.1} | handicapAvgValue={:.1}",
            update_results.avg_delta,
            update_results.match_quality,
            update_results.executed_matches,
            update_results.failed_matches,
            update_results.handicap_stats.avg_match_count,
            update_results.handicap_stats.avg_handicap
        );

        Ok(())
    }

    async fn load_players(&self, group: &str) -> Result<Vec<PlayerRow>> {
        let rows = self
            .client
            .query(
                r#"
                SELECT id, name, score
                FROM member
                WHERE number = $1 AND is_active = true
                ORDER BY score DESC
                "#,
                &[&group],
            )
            .await
            .context("load active players")?;

        Ok(rows
            .into_iter()
            .map(|row| PlayerRow {
                id: row.get::<_, i32>(0),
                name: row.get::<_, String>(1),
                loaded_score: numeric_to_f64(&row, 2),
                score: 0,
                original_score: 0,
                initial_rank: 0,
                rank: 0,
                total_matches: 0,
                base_matches: 0,
                second_matches: 0,
                supplement_matches: 0,
            })
            .collect())
    }

    async fn load_handicap_data(&mut self, players: &[PlayerRow], group: &str) -> Result<HashMap<PairKey, HandicapData>> {
        let rows = self
            .client
            .query(
                r#"
                SELECT player1_id, player2_id, value, match_count
                FROM handicap
                WHERE number = $1
                  AND (match_count <> 0 OR ABS(value::double precision) > $2::double precision)
                "#,
                &[&group, &HANDICAP_ZERO_EPSILON],
            )
            .await
            .context("load handicap rows")?;

        let mut map = HashMap::with_capacity(players.len().saturating_mul(players.len().saturating_sub(1)) / 2);
        for i in 0..players.len() {
            for j in (i + 1)..players.len() {
                map.insert(
                    PairKey::new(players[i].id, players[j].id),
                    HandicapData {
                        basic_value: 0.0,
                        disturbed_value: self.random_in_range(1.0) * PERTURBATION_RANGE,
                        count: 0.0,
                    },
                );
            }
        }

        for row in rows {
            let id1 = row.get::<_, i32>(0);
            let id2 = row.get::<_, i32>(1);
            let value = numeric_to_f64(&row, 2);
            let match_count = numeric_to_f64(&row, 3);
            let key = PairKey::new(id1, id2);
            if let Some(data) = map.get_mut(&key) {
                data.basic_value = finite_or(value, 0.0);
                data.count = finite_or(match_count, 0.0);
                data.disturbed_value = if match_count > 100.0 {
                    finite_or(value, 0.0)
                } else {
                    finite_or(
                        value + self.random_in_range((100.0 - match_count) / 100.0) * PERTURBATION_RANGE,
                        0.0,
                    )
                };
            }
        }

        Ok(map)
    }

    fn generate_base_matches(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &HashMap<PairKey, HandicapData>,
    ) -> Vec<MatchPair> {
        let n = players.len();
        let mut matches = Vec::new();
        let mut matched_pairs = HashSet::new();
        let mut order = (0..n).collect::<Vec<_>>();
        self.shuffle(&mut order);

        for i in order {
            let candidates = self.build_candidate_list(i, n, &matched_pairs, |a, b| {
                let win_prob = expected_win_prob_with_handicap(a, b, players, handicap_map, true);
                (0.45..=0.55).contains(&win_prob)
                    && players[a].base_matches < MAX_ACTIVE_MATCHES
                    && players[b].base_matches < MAX_PASSIVE_MATCHES
                    && players[a].total_matches < PRE_SA_MAX_MATCHES
                    && players[b].total_matches < PRE_SA_MAX_MATCHES
            });

            for j in candidates {
                if players[i].base_matches >= MAX_ACTIVE_MATCHES || players[i].total_matches >= PRE_SA_MAX_MATCHES {
                    break;
                }
                if players[j].base_matches >= MAX_PASSIVE_MATCHES || players[j].total_matches >= PRE_SA_MAX_MATCHES {
                    continue;
                }

                let pair = MatchPair(i, j);
                if !matched_pairs.insert(pair.key()) {
                    continue;
                }

                matches.push(pair);
                players[i].total_matches += 1;
                players[j].total_matches += 1;
                players[i].base_matches += 1;
                players[j].base_matches += 1;
            }
        }

        matches
    }

    fn generate_second_matches(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &HashMap<PairKey, HandicapData>,
        first_matches: &[MatchPair],
    ) -> Vec<MatchPair> {
        let n = players.len();
        let mut matches = first_matches.to_vec();
        let mut matched_pairs = first_matches.iter().map(|pair| pair.key()).collect::<HashSet<_>>();
        let mut order = (0..n).collect::<Vec<_>>();
        self.shuffle(&mut order);

        for i in order {
            let candidates = self.build_candidate_list(i, n, &matched_pairs, |a, b| {
                let win_prob = expected_win_prob_with_handicap(a, b, players, handicap_map, true);
                (0.40..=0.60).contains(&win_prob)
                    && players[a].second_matches < SECOND_ACTIVE_MATCHES
                    && players[b].second_matches < SECOND_PASSIVE_MATCHES
                    && players[a].total_matches < PRE_SA_MAX_MATCHES
                    && players[b].total_matches < PRE_SA_MAX_MATCHES
            });

            for j in candidates {
                if players[i].second_matches >= SECOND_ACTIVE_MATCHES || players[i].total_matches >= PRE_SA_MAX_MATCHES {
                    break;
                }
                if players[j].second_matches >= SECOND_PASSIVE_MATCHES || players[j].total_matches >= PRE_SA_MAX_MATCHES {
                    continue;
                }

                let pair = MatchPair(i, j);
                if !matched_pairs.insert(pair.key()) {
                    continue;
                }

                matches.push(pair);
                players[i].total_matches += 1;
                players[j].total_matches += 1;
                players[i].base_matches += 1;
                players[j].base_matches += 1;
                players[i].second_matches += 1;
                players[j].second_matches += 1;
            }
        }

        matches
    }

    fn build_candidate_list<F>(
        &mut self,
        i: usize,
        n: usize,
        matched_pairs: &HashSet<(usize, usize)>,
        is_eligible_match: F,
    ) -> Vec<usize>
    where
        F: Fn(usize, usize) -> bool,
    {
        let mut local_candidates = Vec::new();
        let mut candidate_set = HashSet::new();
        let search_radius = if n < 30 { 25.min(n.saturating_sub(1)) } else { 40 };

        for offset in 1..=search_radius {
            let j1 = (i + offset) % n;
            if i != j1 {
                let pair_key = ordered_usize_pair(i, j1);
                if !matched_pairs.contains(&pair_key) && !candidate_set.contains(&j1) && is_eligible_match(i, j1) {
                    local_candidates.push(j1);
                    candidate_set.insert(j1);
                }
            }

            let j2 = (i + n - (offset % n)) % n;
            if i != j2 {
                let pair_key = ordered_usize_pair(i, j2);
                if !matched_pairs.contains(&pair_key) && !candidate_set.contains(&j2) && is_eligible_match(i, j2) {
                    local_candidates.push(j2);
                    candidate_set.insert(j2);
                }
            }
        }

        let exploration_quota = GLOBAL_EXPLORATION_MIN
            .max((local_candidates.len() as f64 * GLOBAL_EXPLORATION_RATIO).ceil() as usize)
            .min(n.saturating_sub(1).saturating_sub(candidate_set.len()));
        let mut random_candidates = Vec::new();
        let max_attempts = 20.max(exploration_quota * 8);

        for _ in 0..max_attempts {
            if random_candidates.len() >= exploration_quota {
                break;
            }
            let j = self.rng.usize(..n);
            if j == i || candidate_set.contains(&j) {
                continue;
            }
            let pair_key = ordered_usize_pair(i, j);
            if matched_pairs.contains(&pair_key) || !is_eligible_match(i, j) {
                continue;
            }
            random_candidates.push(j);
            candidate_set.insert(j);
        }

        let mut merged = self.blend_candidates(&local_candidates, &random_candidates, GLOBAL_EXPLORATION_RATIO);
        self.shuffle(&mut merged);
        merged
    }

    fn blend_candidates(
        &mut self,
        local_candidates: &[usize],
        random_candidates: &[usize],
        exploration_ratio: f64,
    ) -> Vec<usize> {
        let mut merged = Vec::with_capacity(local_candidates.len() + random_candidates.len());
        let mut local_idx = 0usize;
        let mut random_idx = 0usize;

        while local_idx < local_candidates.len() || random_idx < random_candidates.len() {
            let use_random = random_idx < random_candidates.len()
                && (local_idx >= local_candidates.len() || self.rng.f64() < exploration_ratio);

            if use_random {
                merged.push(random_candidates[random_idx]);
                random_idx += 1;
            } else if local_idx < local_candidates.len() {
                merged.push(local_candidates[local_idx]);
                local_idx += 1;
            }
        }

        merged
    }

    fn analyze_match_distribution(
        &self,
        matches: &[MatchPair],
        players: &[PlayerRow],
        base_match_count: usize,
        handicap_map: &HashMap<PairKey, HandicapData>,
    ) -> MatchStats {
        let mut min_matches = usize::MAX;
        let mut max_matches = 0usize;
        let mut total_matches = 0usize;

        for player in players {
            min_matches = min_matches.min(player.total_matches);
            max_matches = max_matches.max(player.total_matches);
            total_matches += player.total_matches;
        }

        let base_only_matches = &matches[..base_match_count.min(matches.len())];
        let mut deltas = self.calculate_all_expected_deltas(players, handicap_map, base_only_matches);
        for pair in &matches[base_match_count.min(matches.len())..] {
            let delta_change = calculate_expected_delta(pair.0, pair.1, players, handicap_map);
            deltas[pair.0] += delta_change;
            deltas[pair.1] -= delta_change;
        }

        MatchStats {
            min_matches: if min_matches == usize::MAX { 0 } else { min_matches },
            max_matches,
            avg_matches: total_matches as f64 / players.len().max(1) as f64,
            final_squared_loss: calculate_total_squared_loss(&deltas),
        }
    }

    fn calculate_all_expected_deltas(
        &self,
        players: &[PlayerRow],
        handicap_map: &HashMap<PairKey, HandicapData>,
        matches: &[MatchPair],
    ) -> Vec<f64> {
        let mut deltas = vec![0.0; players.len()];
        for pair in matches {
            let delta_change = calculate_expected_delta(pair.0, pair.1, players, handicap_map);
            deltas[pair.0] += delta_change;
            deltas[pair.1] -= delta_change;
        }
        deltas
    }

    async fn execute_matches_with_calibrated_elo(
        &mut self,
        players: &mut [PlayerRow],
        matches: &[MatchPair],
        handicap_map: &mut HashMap<PairKey, HandicapData>,
        group: &str,
    ) -> Result<UpdateResults> {
        let seed = generate_unique_seed(&mut self.rng);
        let mut history_records = Vec::new();
        let mut changed_handicaps = HashSet::new();
        let mut total_delta = 0.0;
        let mut high_quality_matches = 0usize;
        let mut total_match_count = 0.0;
        let mut total_handicap = 0.0;
        let mut handicap_count = 0usize;
        let mut executed_matches = 0usize;
        let mut failed_matches = 0usize;

        let valid_matches = matches
            .iter()
            .copied()
            .filter(|pair| pair.0 < players.len() && pair.1 < players.len() && pair.0 != pair.1)
            .collect::<Vec<_>>();

        if valid_matches.len() < matches.len() {
            eprintln!("组别 {group}: 过滤 {} 个无效匹配对", matches.len() - valid_matches.len());
        }

        for (match_index, pair) in valid_matches.iter().enumerate() {
            let i = pair.0;
            let j = pair.1;
            let handicap_direction = if players[i].id < players[j].id { 1.0 } else { -1.0 };
            let key = PairKey::new(players[i].id, players[j].id);
            let handicap = handicap_map.get(&key).map(|data| data.basic_value).unwrap_or(0.0);

            let score_diff_with_handicap = (players[i].score - players[j].score) as f64
                + if players[i].id < players[j].id {
                    handicap * HANDICAP_SCALE
                } else {
                    -handicap * HANDICAP_SCALE
                };
            let expected_with_handicap = logistic_expected(score_diff_with_handicap);

            let score_diff_base = (players[i].score - players[j].score) as f64;
            let expected_base = logistic_expected(score_diff_base);

            if (0.45..=0.55).contains(&expected_with_handicap) {
                high_quality_matches += 1;
            }

            let keep_plus = group == "diy";
            let result = self.get_winner(
                &players[i].name,
                &players[j].name,
                &seed,
                keep_plus,
                group,
                match_index,
                valid_matches.len(),
            );

            let result = match result {
                Some(result) => result,
                None => {
                    failed_matches += 1;
                    continue;
                }
            };
            executed_matches += 1;

            let actual = if result == 1 { 1.0 } else { 0.0 };
            let delta = BASE_K_FACTOR * (actual - expected_base);
            let p1_new_score = self.probabilistic_round(players[i].score as f64 + delta);
            let p2_new_score = self.probabilistic_round(players[j].score as f64 - delta);
            let actual_delta = p1_new_score - players[i].score;

            players[i].score = p1_new_score;
            players[j].score = p2_new_score;

            if actual > 0.0 {
                players[i].score += 1;
            } else {
                players[j].score += 1;
            }

            total_delta += (actual_delta as f64).abs();

            if let Some(data) = handicap_map.get_mut(&key) {
                let delta_h = K_HANDICAP * (actual - expected_with_handicap) * handicap_direction;
                data.basic_value += delta_h;
                data.count = 100.0 - (100.0 - data.count) * 0.1;

                total_match_count += data.count;
                total_handicap += data.basic_value.abs();
                handicap_count += 1;
                changed_handicaps.insert(key);
            } else if (actual - expected_with_handicap).abs() > 0.2 {
                let initial_handicap = K_HANDICAP * (actual - expected_with_handicap) * handicap_direction;
                handicap_map.insert(
                    key,
                    HandicapData {
                        disturbed_value: initial_handicap,
                        basic_value: initial_handicap,
                        count: 1.0,
                    },
                );

                total_match_count += 1.0;
                total_handicap += initial_handicap.abs();
                handicap_count += 1;
                changed_handicaps.insert(key);
            }

            let p1_name_for_runs = if keep_plus {
                players[i].name.clone()
            } else {
                players[i].name.replace('+', "\n")
            };
            let p2_name_for_runs = if keep_plus {
                players[j].name.clone()
            } else {
                players[j].name.replace('+', "\n")
            };
            let runs = format!("{p1_name_for_runs}\n\n{p2_name_for_runs}\nseed:{seed}@!");

            let p1_won = result == 1;
            history_records.push(HistoryRecord {
                win_or_lose: "win",
                from_member_id: if p1_won { players[i].id } else { players[j].id },
                to_member_id: if p1_won { players[j].id } else { players[i].id },
                current_score: 0,
                from_rank: if p1_won {
                    players[i].initial_rank
                } else {
                    players[j].initial_rank
                },
                to_rank: if p1_won {
                    players[j].initial_rank
                } else {
                    players[i].initial_rank
                },
                from_name: if p1_won {
                    players[i].name.clone()
                } else {
                    players[j].name.clone()
                },
                to_name: if p1_won {
                    players[j].name.clone()
                } else {
                    players[i].name.clone()
                },
                runs: runs.clone(),
                number: group.to_string(),
                expected_win_prob: expected_with_handicap,
                actual_result: actual,
            });

            history_records.push(HistoryRecord {
                win_or_lose: "lose",
                from_member_id: if p1_won { players[j].id } else { players[i].id },
                to_member_id: if p1_won { players[i].id } else { players[j].id },
                current_score: 0,
                from_rank: if p1_won {
                    players[j].initial_rank
                } else {
                    players[i].initial_rank
                },
                to_rank: if p1_won {
                    players[i].initial_rank
                } else {
                    players[j].initial_rank
                },
                from_name: if p1_won {
                    players[j].name.clone()
                } else {
                    players[i].name.clone()
                },
                to_name: if p1_won {
                    players[i].name.clone()
                } else {
                    players[j].name.clone()
                },
                runs,
                number: group.to_string(),
                expected_win_prob: 1.0 - expected_with_handicap,
                actual_result: 1.0 - actual,
            });
        }

        let match_quality = if valid_matches.is_empty() {
            0.0
        } else {
            high_quality_matches as f64 / valid_matches.len() as f64 * 100.0
        };

        if !valid_matches.is_empty() && executed_matches == 0 {
            eprintln!(
                "[group:{group}] 本轮 {} 场对战全部失败，跳过分数/克制/历史写入",
                valid_matches.len()
            );
            return Ok(UpdateResults {
                history_records: Vec::new(),
                changed_handicaps: HashSet::new(),
                avg_delta: 0.0,
                match_quality,
                failed_matches,
                executed_matches,
                skip_persist: true,
                handicap_stats: HandicapStats {
                    avg_match_count: 0.0,
                    avg_handicap: 0.0,
                },
            });
        }

        let avg_match_count = if handicap_count > 0 {
            total_match_count / handicap_count as f64
        } else {
            0.0
        };
        let avg_handicap = if handicap_count > 0 {
            total_handicap / handicap_count as f64
        } else {
            0.0
        };

        Ok(UpdateResults {
            history_records,
            changed_handicaps,
            avg_delta: total_delta / executed_matches.max(1) as f64,
            match_quality,
            failed_matches,
            executed_matches,
            skip_persist: false,
            handicap_stats: HandicapStats {
                avg_match_count,
                avg_handicap,
            },
        })
    }

    fn get_winner(
        &mut self,
        player1_str: &str,
        player2_str: &str,
        seed: &str,
        keep_plus: bool,
        group: &str,
        match_index: usize,
        match_total: usize,
    ) -> Option<i32> {
        let player1 = if keep_plus {
            player1_str.to_string()
        } else {
            player1_str.replace('+', "\n")
        };
        let player2 = if keep_plus {
            player2_str.to_string()
        } else {
            player2_str.replace('+', "\n")
        };
        let runs = format!("{player1}\n\n{player2}\nseed:{seed}@!");

        match run_battle_winner(&runs, &player1, &player2) {
            Ok(winner) => winner,
            Err(err) => {
                self.log_battle_error(&err, group, match_index, match_total, seed, player1_str, player2_str);
                None
            }
        }
    }

    fn log_battle_error(
        &mut self,
        err: &anyhow::Error,
        group: &str,
        match_index: usize,
        match_total: usize,
        seed: &str,
        p1_name: &str,
        p2_name: &str,
    ) {
        self.battle_error_count += 1;
        let message = format!(
            "[battle] 对战执行错误 #{} group={} match={}/{} seed={}: {} {} vs {}",
            self.battle_error_count,
            group,
            match_index + 1,
            match_total,
            seed,
            err,
            p1_name,
            p2_name
        );

        if self.battle_error_count <= BATTLE_ERROR_LOG_LIMIT {
            eprintln!("{message}");
            return;
        }

        self.battle_error_suppressed += 1;
        if self.battle_error_suppressed == 1 || self.battle_error_suppressed % 100 == 0 {
            eprintln!(
                "[battle] 已抑制 {} 条重复对战错误日志，最近错误: {}",
                self.battle_error_suppressed, err
            );
        }
    }

    async fn persist_results(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &mut HashMap<PairKey, HandicapData>,
        changed_handicap_keys: &HashSet<PairKey>,
        group: &str,
    ) -> Result<()> {
        for batch in players.chunks(200) {
            let ids = batch.iter().map(|p| p.id).collect::<Vec<i32>>();
            let scores = batch.iter().map(|p| p.score).collect::<Vec<i32>>();
            self.client
                .execute(
                    r#"
                    UPDATE member AS m
                    SET score = u.score
                    FROM UNNEST($1::int[], $2::int[]) AS u(id, score)
                    WHERE m.id = u.id
                    "#,
                    &[&ids, &scores],
                )
                .await
                .context("update member scores")?;
        }

        let mut handicap_rows = Vec::new();
        for (key, data) in handicap_map.iter_mut() {
            let raw_value = if data.basic_value.is_finite() {
                data.basic_value
            } else {
                data.disturbed_value
            };
            let decayed_value = if changed_handicap_keys.contains(key) {
                raw_value * HANDICAP_DECAY
            } else {
                raw_value
            };
            let persisted_value = self.round_handicap(decayed_value);
            data.disturbed_value = persisted_value;
            data.basic_value = persisted_value;

            let mut count = self.round_handicap(data.count * MATCHCOUNT_DECAY);
            if count > 100.0 {
                count = 100.0;
            }

            let has_signal = persisted_value.abs() > HANDICAP_ZERO_EPSILON || count > 0.0;
            let should_persist = !HANDICAP_SPARSE_PERSIST || has_signal || changed_handicap_keys.contains(key);
            if !should_persist {
                continue;
            }

            handicap_rows.push((group.to_string(), key.0, key.1, persisted_value, count));
        }

        for batch in handicap_rows.chunks(300) {
            let numbers = batch.iter().map(|r| r.0.clone()).collect::<Vec<String>>();
            let player1_ids = batch.iter().map(|r| r.1).collect::<Vec<i32>>();
            let player2_ids = batch.iter().map(|r| r.2).collect::<Vec<i32>>();
            let values = batch.iter().map(|r| r.3).collect::<Vec<f64>>();
            let counts = batch.iter().map(|r| r.4).collect::<Vec<f64>>();

            self.client
                .execute(
                    r#"
                    INSERT INTO handicap (number, player1_id, player2_id, value, match_count)
                    SELECT u.number, u.player1_id, u.player2_id, u.value, u.match_count
                    FROM UNNEST(
                        $1::text[],
                        $2::int[],
                        $3::int[],
                        $4::double precision[],
                        $5::double precision[]
                    ) AS u(number, player1_id, player2_id, value, match_count)
                    ON CONFLICT (number, player1_id, player2_id)
                    DO UPDATE SET
                        value = EXCLUDED.value,
                        match_count = EXCLUDED.match_count
                    "#,
                    &[&numbers, &player1_ids, &player2_ids, &values, &counts],
                )
                .await
                .context("upsert handicap rows")?;
        }

        if HANDICAP_SPARSE_PERSIST {
            self.client
                .execute(
                    r#"
                    DELETE FROM handicap
                    WHERE number = $1
                      AND match_count = 0
                      AND ABS(value::double precision) <= $2::double precision
                    "#,
                    &[&group, &HANDICAP_ZERO_EPSILON],
                )
                .await
                .context("delete sparse handicap rows")?;
        }

        Ok(())
    }

    async fn update_ranks_and_logs(
        &mut self,
        players: &mut [PlayerRow],
        mut history_records: Vec<HistoryRecord>,
        group: &str,
    ) -> Result<()> {
        players.sort_by(|a, b| b.score.cmp(&a.score));
        let final_scores = players.iter().map(|p| (p.id, p.score)).collect::<HashMap<_, _>>();

        for rec in &mut history_records {
            rec.current_score = final_scores.get(&rec.from_member_id).copied().unwrap_or(0);
        }

        for (idx, player) in players.iter_mut().enumerate() {
            player.rank = (idx + 1) as i32;
        }

        let rank_ids = players.iter().map(|p| p.id).collect::<Vec<i32>>();
        let rank_values = players.iter().map(|p| p.rank).collect::<Vec<i32>>();
        self.client
            .execute(
                r#"
                UPDATE member AS m
                SET member_rank = u.member_rank
                FROM UNNEST($1::int[], $2::int[]) AS u(id, member_rank)
                WHERE m.id = u.id
                "#,
                &[&rank_ids, &rank_values],
            )
            .await
            .context("update member ranks")?;

        if let Some(sample) = history_records.first() {
            eprintln!(
                "[db] 验证历史记录: {}#{} vs {}#{}",
                sample.from_name, sample.from_rank, sample.to_name, sample.to_rank
            );
            eprintln!(
                "   预期胜率: {:.2}, 实际结果: {}",
                sample.expected_win_prob, sample.actual_result
            );
        }

        for batch in history_records.chunks(300) {
            let win_or_lose = batch.iter().map(|r| r.win_or_lose).collect::<Vec<&str>>();
            let from_member_ids = batch.iter().map(|r| r.from_member_id).collect::<Vec<i32>>();
            let to_member_ids = batch.iter().map(|r| r.to_member_id).collect::<Vec<i32>>();
            let current_scores = batch.iter().map(|r| r.current_score).collect::<Vec<i32>>();
            let from_ranks = batch.iter().map(|r| r.from_rank).collect::<Vec<i32>>();
            let to_ranks = batch.iter().map(|r| r.to_rank).collect::<Vec<i32>>();
            let from_names = batch.iter().map(|r| r.from_name.clone()).collect::<Vec<String>>();
            let to_names = batch.iter().map(|r| r.to_name.clone()).collect::<Vec<String>>();
            let runs = batch.iter().map(|r| r.runs.clone()).collect::<Vec<String>>();
            let numbers = batch.iter().map(|r| r.number.clone()).collect::<Vec<String>>();
            let expected_win_probs = batch.iter().map(|r| r.expected_win_prob).collect::<Vec<f64>>();
            let actual_results = batch.iter().map(|r| r.actual_result).collect::<Vec<f64>>();

            self.client
                .execute(
                    r#"
                    INSERT INTO history (
                        win_or_lose, from_member_id, to_member_id, created_at,
                        current_score, from_rank, to_rank, from_name, to_name, runs, number,
                        expected_win_prob, actual_result
                    )
                    SELECT
                        u.win_or_lose, u.from_member_id, u.to_member_id, NOW(),
                        u.current_score, u.from_rank, u.to_rank, u.from_name, u.to_name, u.runs, u.number,
                        u.expected_win_prob, u.actual_result
                    FROM UNNEST(
                        $1::text[],
                        $2::int[],
                        $3::int[],
                        $4::int[],
                        $5::int[],
                        $6::int[],
                        $7::text[],
                        $8::text[],
                        $9::text[],
                        $10::text[],
                        $11::double precision[],
                        $12::double precision[]
                    ) AS u(
                        win_or_lose,
                        from_member_id,
                        to_member_id,
                        current_score,
                        from_rank,
                        to_rank,
                        from_name,
                        to_name,
                        runs,
                        number,
                        expected_win_prob,
                        actual_result
                    )
                    "#,
                    &[
                        &win_or_lose,
                        &from_member_ids,
                        &to_member_ids,
                        &current_scores,
                        &from_ranks,
                        &to_ranks,
                        &from_names,
                        &to_names,
                        &runs,
                        &numbers,
                        &expected_win_probs,
                        &actual_results,
                    ],
                )
                .await
                .context("insert history rows")?;
        }

        let rank_list = players.iter().map(|p| p.id.to_string()).collect::<Vec<_>>().join(" ");
        tokio::fs::create_dir_all(&self.config.rank_log_dir)
            .await
            .with_context(|| format!("create rank log dir {}", self.config.rank_log_dir.display()))?;
        let path = self.config.rank_log_dir.join(format!("rank_{group}.txt"));
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .with_context(|| format!("open rank log {}", path.display()))?
            .write_all(format!("{rank_list}\n").as_bytes())
            .await
            .with_context(|| format!("append rank log {}", path.display()))?;

        Ok(())
    }

    fn probabilistic_round(&mut self, value: f64) -> i32 {
        let floor = value.floor();
        if self.rng.f64() < value - floor {
            floor as i32 + 1
        } else {
            floor as i32
        }
    }

    fn round_handicap(&mut self, value: f64) -> f64 {
        let scale = 10_f64.powi(HANDICAP_PRECISION as i32);
        self.probabilistic_round(value * scale) as f64 / scale
    }

    fn random_in_range(&mut self, a: f64) -> f64 { self.rng.f64() * 2.0 * a - a }

    fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.rng.usize(..=i);
            values.swap(i, j);
        }
    }
}

fn parse_cron_schedule(value: &str) -> Result<Schedule> {
    let parts = value.split_whitespace().count();
    if parts == 5 {
        let with_seconds = format!("0 {value}");
        return Schedule::from_str(&with_seconds).with_context(|| format!("parse cron schedule {value:?} / {with_seconds:?}"));
    }

    Schedule::from_str(value).with_context(|| format!("parse cron schedule {value:?}"))
}

fn read_env_bool(name: &str, default: bool) -> bool {
    match env::var(name).ok().map(|x| x.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on") => true,
        Some(value) if matches!(value.as_str(), "0" | "false" | "no" | "n" | "off") => false,
        _ => default,
    }
}

async fn shutdown_signal() -> Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut terminate = signal(SignalKind::terminate()).context("listen for SIGTERM")?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("listen for ctrl-c")?;
            }
            _ = terminate.recv() => {}
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.context("listen for ctrl-c")?;
    }

    Ok(())
}

fn ordered_usize_pair(a: usize, b: usize) -> (usize, usize) { if a < b { (a, b) } else { (b, a) } }

fn expected_win_prob_with_handicap(
    i: usize,
    j: usize,
    players: &[PlayerRow],
    handicap_map: &HashMap<PairKey, HandicapData>,
    disturbed: bool,
) -> f64 {
    let p1 = &players[i];
    let p2 = &players[j];
    let key = PairKey::new(p1.id, p2.id);
    let handicap = handicap_map
        .get(&key)
        .map(|data| if disturbed { data.disturbed_value } else { data.basic_value })
        .unwrap_or(0.0);
    let score_diff = (p1.score - p2.score) as f64
        + if p1.id < p2.id {
            handicap * HANDICAP_SCALE
        } else {
            -handicap * HANDICAP_SCALE
        };
    logistic_expected(score_diff)
}

fn calculate_expected_delta(i: usize, j: usize, players: &[PlayerRow], handicap_map: &HashMap<PairKey, HandicapData>) -> f64 {
    let p1 = &players[i];
    let p2 = &players[j];
    let expected_with_handicap = expected_win_prob_with_handicap(i, j, players, handicap_map, true);
    let expected_base = logistic_expected((p1.score - p2.score) as f64);

    BASE_K_FACTOR * ((1.0 - expected_base) * expected_with_handicap - expected_base * (1.0 - expected_with_handicap))
}

fn calculate_total_squared_loss(deltas: &[f64]) -> f64 { deltas.iter().map(|delta| delta * delta).sum() }

fn logistic_expected(score_diff: f64) -> f64 { 1.0 / (1.0 + 10_f64.powf(-score_diff / ELO_SCALE_FACTOR)) }

fn generate_unique_seed(rng: &mut fastrand::Rng) -> String {
    let basic_seed = format!("{:06}", rng.u32(..1_000_000));
    let seed = Local::now().format("%Y-%m-%d %H:%M").to_string();
    format!("{seed} #{basic_seed}")
}

fn count_input_players(player_text: &str) -> usize {
    player_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("seed:"))
        .count()
}

fn run_battle_winner(runs: &str, player1: &str, player2: &str) -> Result<Option<i32>> {
    let player1_count = count_input_players(player1);
    let player2_count = count_input_players(player2);
    let input_player_count = player1_count + player2_count;

    let mut runner = Runner::new_from_namerena_raw(runs.to_string()).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    let input_groups = runner.input_groups.clone();
    let finished = runner.run_to_completion();
    if !finished {
        return Ok(None);
    }

    let Some(winner_ids) = runner.world.winner.clone() else {
        return Ok(None);
    };
    if winner_ids.is_empty() {
        return Ok(None);
    }

    let winner_ids = winner_ids.into_iter().collect::<HashSet<PlrId>>();
    let mut winner_sides = HashSet::new();
    let mut team_index_to_side = HashMap::new();

    for (input_team_idx, group) in input_groups.iter().enumerate() {
        let side = match input_team_idx {
            0 => 1,
            1 => 0,
            _ => continue,
        };
        for player_id in group {
            if let Some(team_idx) = runner.world.team_index_of(*player_id) {
                team_index_to_side.insert(team_idx, side);
            }
        }
    }

    for player_id in runner.storage.all_player_ids() {
        if !winner_ids.contains(&player_id) {
            continue;
        }

        if player_id < player1_count {
            winner_sides.insert(1);
        } else if player_id < input_player_count {
            winner_sides.insert(0);
        } else if let Some(team_idx) = runner.world.team_index_of(player_id)
            && let Some(side) = team_index_to_side.get(&team_idx)
        {
            winner_sides.insert(*side);
        }
    }

    if winner_sides.contains(&1) && !winner_sides.contains(&0) {
        Ok(Some(1))
    } else if winner_sides.contains(&0) && !winner_sides.contains(&1) {
        Ok(Some(0))
    } else {
        Ok(None)
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 { if value.is_finite() { value } else { fallback } }

fn current_rss_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

fn numeric_to_f64(row: &tokio_postgres::Row, idx: usize) -> f64 {
    if let Ok(value) = row.try_get::<_, f64>(idx) {
        return value;
    }
    if let Ok(value) = row.try_get::<_, f32>(idx) {
        return value as f64;
    }
    if let Ok(value) = row.try_get::<_, i32>(idx) {
        return value as f64;
    }
    if let Ok(value) = row.try_get::<_, i64>(idx) {
        return value as f64;
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_five_field_cron_like_node_cron() {
        assert!(parse_cron_schedule("* * * * *").is_ok());
    }

    #[test]
    fn battle_winner_maps_first_input_side_to_one() {
        let winner = run_battle_winner("aaaaa\n\nbbbbb\nseed:test@!", "aaaaa", "bbbbb").expect("battle should run");
        assert!(matches!(winner, Some(0) | Some(1) | None));
    }

    #[test]
    fn pair_key_is_ordered() {
        assert_eq!(PairKey::new(5, 2), PairKey(2, 5));
    }
}

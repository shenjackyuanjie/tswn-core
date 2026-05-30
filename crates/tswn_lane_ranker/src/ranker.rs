use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Instant;

use crate::db::Db;
use crate::model::{GroupId, LaneResultRow, RankNode, StoredGroup};
use crate::team::TeamDsu;
use crate::winrate::compute_rate_without_db;

pub const STANDARD_SIZE: usize = 50;
pub const TEAM_LIMIT: usize = 5;
pub const DEFAULT_WARMUP_ROUNDS: usize = 10_000;
pub const DEFAULT_TOTAL_ROUNDS: usize = 1_000_000;

/// 默认 1000% 精度，即 100000 场。
pub const DEFAULT_WIN_RATE_SAMPLES: usize = 100_000;

/// 胜率补算支持浏览器传入两层 worker：
///
/// - outer_workers = 0：外层按系统可用并行度自动开 worker，并用动态队列分发 pair。
/// - outer_workers > 0：外层使用指定 worker 数，并按静态区间分块。
/// - inner_workers = 0：传给 tswn_core::win_rate::groups_win_rate 的 thread=0，让核心库自动决定线程策略。
/// - inner_workers > 0：传给 tswn_core 指定内层线程数。

/// 默认粘性：单人组 10，双人组 20，x 人组 10 * x。
pub const DEFAULT_STICKINESS_PER_MEMBER: usize = 10;

/// Odds/Golden 从第 10000 轮后开始累计，与 warmup_rounds 解耦。
pub const ODDS_START_ROUND: usize = 10_000;

/// 100000 轮后开始检查三位小数平均 CQD 是否稳定。
pub const EARLY_STOP_START_ROUND: usize = 100_000;
pub const EARLY_STOP_STABLE_ROUNDS: usize = 100;

pub const KICK_AVG_CQD_THRESHOLD: f64 = 45.0;

/// 跑完后自动封存 avg cqd 未达到 48 的组合。
pub const ARCHIVE_AVG_CQD_THRESHOLD: f64 = 48.0;

/// 和浏览器折叠 UI 保持一致：每个主行的折叠内部只保留前 20 个子项；超出的子项会被封存。
pub const FOLDED_CHILD_KEEP_LIMIT: usize = 20;

#[derive(Debug, Clone)]
pub struct RankerConfig {
    pub warmup_rounds: usize,
    pub total_rounds: usize,
    pub win_rate_samples: usize,
    pub stickiness: Option<usize>,
    pub outer_workers: usize,
    pub inner_workers: u32,
    pub skip_archived: bool,
}

impl RankerConfig {
    pub fn from_env() -> Self {
        Self {
            warmup_rounds: read_env_usize("LANE_WARMUP_ROUNDS", DEFAULT_WARMUP_ROUNDS),
            total_rounds: read_env_usize("LANE_TOTAL_ROUNDS", DEFAULT_TOTAL_ROUNDS),
            win_rate_samples: read_env_usize("LANE_WIN_RATE_SAMPLES", DEFAULT_WIN_RATE_SAMPLES),
            stickiness: read_env_optional_usize("LANE_STICKINESS"),
            outer_workers: read_env_usize("LANE_OUTER_WORKERS", 0),
            inner_workers: read_env_u32("LANE_INNER_WORKERS", 0),
            skip_archived: read_env_bool("LANE_SKIP_ARCHIVED", true),
        }
    }

    pub fn effective_stickiness(&self, lane_size: usize) -> usize {
        self.stickiness.unwrap_or(DEFAULT_STICKINESS_PER_MEMBER * lane_size.max(1)).max(1)
    }
}

fn read_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|x| x.parse().ok()).unwrap_or(default)
}

fn read_env_u32(name: &str, default: u32) -> u32 { std::env::var(name).ok().and_then(|x| x.parse().ok()).unwrap_or(default) }

fn read_env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name).ok().map(|x| x.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on") => true,
        Some(value) if matches!(value.as_str(), "0" | "false" | "no" | "n" | "off") => false,
        _ => default,
    }
}

fn read_env_optional_usize(name: &str) -> Option<usize> {
    std::env::var(name).ok().and_then(|x| x.parse::<usize>().ok()).filter(|&x| x > 0)
}

fn resolve_rate_pair_workers(requested_outer_workers: usize, total: usize) -> usize {
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }

    let available = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    available.max(1).min(total.max(1))
}

fn format_outer_workers(requested_outer_workers: usize, actual_workers: usize) -> String {
    if requested_outer_workers == 0 {
        format!("auto({actual_workers})")
    } else {
        format!("static({actual_workers})")
    }
}

fn format_inner_workers(inner_workers: u32) -> String {
    if inner_workers == 0 {
        "auto(0)".to_string()
    } else {
        inner_workers.to_string()
    }
}

pub fn recompute_lane_until_stable(db: &Db, lane_size: usize, config: &RankerConfig) -> anyhow::Result<()> {
    let mut recompute_pass = 0usize;

    loop {
        recompute_pass += 1;
        let groups = db.load_groups_by_lane_for_run(lane_size, config.skip_archived)?;

        if groups.len() < STANDARD_SIZE {
            db.clear_lane_results(lane_size)?;
            db.set_lane_status(lane_size, "waiting", groups.len())?;
            db.set_lane_progress(
                lane_size,
                "waiting",
                0,
                config.total_rounds,
                0,
                0,
                0,
                &format!("waiting: {}/{} groups", groups.len(), STANDARD_SIZE),
            )?;
            return Ok(());
        }

        db.set_lane_status(lane_size, "running", groups.len())?;
        db.set_lane_progress(
            lane_size,
            "loading",
            0,
            config.total_rounds,
            0,
            0,
            0,
            &format!(
                "loading lane {lane_size}, pass {recompute_pass}, stickiness={}, outer_workers={}, inner_threads={}, skip_archived={}",
                config.effective_stickiness(lane_size),
                if config.outer_workers == 0 { "dynamic_auto".to_string() } else { format!("static({})", config.outer_workers) },
                format_inner_workers(config.inner_workers),
                config.skip_archived
            ),
        )?;

        let dsu = db.load_team_dsu()?;
        let core_result = run_core_algorithm(db, lane_size, groups, dsu, config)?;
        let final_round = core_result.final_round;
        let early_stopped = core_result.early_stopped;
        let result = core_result.rows;

        if result.is_empty() {
            db.clear_lane_results(lane_size)?;
            db.set_lane_status(lane_size, "waiting", 0)?;
            db.set_lane_progress(
                lane_size,
                "waiting",
                0,
                config.total_rounds,
                0,
                0,
                0,
                "cannot build a 50-group standard list under current constraints",
            )?;
            return Ok(());
        }

        let archive_candidates = core_result.archive_candidates;

        if archive_candidates.is_empty() {
            db.save_lane_results(lane_size, &result)?;
            db.set_lane_status(lane_size, "ready", result.len())?;
            let ready_message = if early_stopped {
                format!(
                    "done: early stopped at round {} after avg cqd stayed unchanged for {} rounds at 3 decimals; skip_archived={}",
                    final_round, EARLY_STOP_STABLE_ROUNDS, config.skip_archived
                )
            } else {
                format!("done; skip_archived={}", config.skip_archived)
            };
            db.set_lane_progress(lane_size, "ready", final_round, config.total_rounds, 0, 0, 0, &ready_message)?;
            return Ok(());
        }

        let archive_rows: Vec<(GroupId, String, f64)> = archive_candidates
            .iter()
            .map(|candidate| (candidate.group_id, candidate.reason.clone(), candidate.average_cqd))
            .collect();
        let archived_count = db.archive_group_combinations(&archive_rows)?;

        if !config.skip_archived {
            db.save_lane_results(lane_size, &result)?;
            db.set_lane_status(lane_size, "ready", result.len())?;
            db.set_lane_progress(
                lane_size,
                "ready",
                final_round,
                config.total_rounds,
                0,
                0,
                archived_count,
                &format!(
                    "done; archived/updated {} combinations but kept them in this run because skip_archived=false",
                    archived_count
                ),
            )?;
            return Ok(());
        }

        db.set_lane_progress(
            lane_size,
            "archiving",
            final_round,
            config.total_rounds,
            0,
            0,
            archived_count,
            &format!(
                "archived {} combinations: avg cqd < {} or folded child outside top {}; rerunning with archived combinations skipped",
                archived_count,
                ARCHIVE_AVG_CQD_THRESHOLD,
                FOLDED_CHILD_KEEP_LIMIT
            ),
        )?;

        // 封存后当前有效组、标准榜、胜率需求和平均 CQD 都可能变化；默认继续重算，直到不再产生新封存组合。
    }
}

#[derive(Debug)]
struct CoreAlgorithmResult {
    rows: Vec<LaneResultRow>,
    final_round: usize,
    early_stopped: bool,
    archive_candidates: Vec<ArchiveCandidate>,
}

#[derive(Debug, Clone)]
struct ArchiveCandidate {
    group_id: GroupId,
    reason: String,
    average_cqd: f64,
}

impl CoreAlgorithmResult {
    fn empty(final_round: usize) -> Self {
        Self {
            rows: Vec::new(),
            final_round,
            early_stopped: false,
            archive_candidates: Vec::new(),
        }
    }
}

fn run_core_algorithm(
    db: &Db,
    lane_size: usize,
    groups: Vec<StoredGroup>,
    mut dsu: TeamDsu,
    config: &RankerConfig,
) -> anyhow::Result<CoreAlgorithmResult> {
    let mut nodes: Vec<RankNode> = groups.into_iter().map(RankNode::new).collect();

    // 初始靶子优先使用数据库中上一轮结果的 Score 前 50。
    let (mut standard, smooth_from_start) = make_initial_standard_indices(db, lane_size, &nodes, &mut dsu)?;
    if standard.len() < STANDARD_SIZE {
        return Ok(CoreAlgorithmResult::empty(0));
    }

    db.set_lane_progress(
        lane_size,
        "initial_target",
        0,
        config.total_rounds,
        standard.len(),
        STANDARD_SIZE,
        0,
        if smooth_from_start {
            "initial target selected from database Score order with standard constraints; smoothing from round 1"
        } else {
            "initial target selected by current make logic; warmup remains unsmoothed"
        },
    )?;

    // v13 关键优化：
    // 这里不再全量预计算整个赛道的两两胜率。
    // RateMatrix 只懒加载当前 standard 需要的列；核心热循环仍然只读内存。
    let mut rate_matrix = RateMatrix::new(nodes.len());

    let core_started = Instant::now();
    let stickiness = config.effective_stickiness(lane_size);
    let mut last_rounded_avg_cqd = vec![i64::MIN; nodes.len()];
    let mut stable_avg_rounds = 0usize;
    let mut final_round = config.total_rounds;
    let mut early_stopped = false;

    for round in 1..=config.total_rounds {
        rate_matrix.ensure_rates_for_standard(db, lane_size, round, &nodes, &standard, config)?;

        fight_in_memory(&mut nodes, &standard, &rate_matrix);

        data(&mut nodes, round, config.warmup_rounds, stickiness, smooth_from_start);

        standard = make_standard_indices(&nodes, &mut dsu);
        if standard.len() < STANDARD_SIZE {
            return Ok(CoreAlgorithmResult::empty(round));
        }

        if round >= ODDS_START_ROUND {
            let selected_ids: HashSet<usize> = standard.iter().copied().collect();
            for (idx, node) in nodes.iter_mut().enumerate() {
                node.odds_n += 1;
                if selected_ids.contains(&idx) {
                    node.bz += 1;
                }
            }
        }

        if round >= EARLY_STOP_START_ROUND {
            if avg_cqd_rounded_unchanged(&nodes, &mut last_rounded_avg_cqd) {
                stable_avg_rounds += 1;
            } else {
                stable_avg_rounds = 0;
            }

            if stable_avg_rounds >= EARLY_STOP_STABLE_ROUNDS {
                final_round = round;
                early_stopped = true;

                db.set_lane_progress(
                    lane_size,
                    "early_stop",
                    round,
                    config.total_rounds,
                    rate_matrix.loaded_pair_count(),
                    0,
                    0,
                    &format!(
                        "early stop at round {round}: avg cqd unchanged for {EARLY_STOP_STABLE_ROUNDS} consecutive rounds at 3 decimals, loaded_pairs={}, stickiness={stickiness}",
                        rate_matrix.loaded_pair_count()
                    ),
                )?;
                break;
            }
        }

        if should_report_round(round, config.total_rounds) {
            let elapsed = core_started.elapsed().as_secs_f64().max(0.001);
            let rounds_per_sec = round as f64 / elapsed;
            let eta_sec = if rounds_per_sec > 0.0 {
                (config.total_rounds.saturating_sub(round)) as f64 / rounds_per_sec
            } else {
                0.0
            };

            db.set_lane_progress(
                lane_size,
                "iterating",
                round,
                config.total_rounds,
                rate_matrix.loaded_pair_count(),
                0,
                0,
                &format!(
                    "core algorithm round {}/{}, {:.2} rounds/s, elapsed {}, eta {}, loaded_pairs={}, stickiness={}, mode=lazy_rate_matrix",
                    round,
                    config.total_rounds,
                    rounds_per_sec,
                    format_duration(elapsed),
                    format_duration(eta_sec),
                    rate_matrix.loaded_pair_count(),
                    stickiness
                ),
            )?;
        }
    }

    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| nodes[b].cqds.total_cmp(&nodes[a].cqds));

    let rows: Vec<LaneResultRow> = order
        .into_iter()
        .enumerate()
        .map(|(rank_idx, node_idx)| {
            let node = &nodes[node_idx];

            LaneResultRow {
                lane_size,
                group_id: node.group.id,
                rank: rank_idx + 1,
                canonical: node.group.canonical.clone(),
                average_cqd: node.avg_cqd(),
                min_cqd: if node.cqdmin.is_finite() { node.cqdmin } else { 0.0 },
                max_cqd: if node.cqdmax.is_finite() { node.cqdmax } else { 0.0 },
                variance_cqd: node.variance_cqd(),
                golden_rate: node.golden_rate(),
            }
        })
        .collect();

    let archive_candidates = find_archive_candidates(&rows, &nodes);

    Ok(CoreAlgorithmResult {
        rows,
        final_round,
        early_stopped,
        archive_candidates,
    })
}

fn find_archive_candidates(rows: &[LaneResultRow], nodes: &[RankNode]) -> Vec<ArchiveCandidate> {
    let members_by_group_id: HashMap<GroupId, Vec<String>> =
        nodes.iter().map(|node| (node.group.id, node.group.members.clone())).collect();

    let mut by_group_id: HashMap<GroupId, ArchiveCandidate> = HashMap::new();

    for row in rows {
        if row.average_cqd < KICK_AVG_CQD_THRESHOLD {
            push_archive_reason(
                &mut by_group_id,
                row.group_id,
                row.average_cqd,
                format!("avg_cqd_below_kick_threshold_{KICK_AVG_CQD_THRESHOLD:.3}"),
            );
        } else if row.average_cqd < ARCHIVE_AVG_CQD_THRESHOLD {
            push_archive_reason(
                &mut by_group_id,
                row.group_id,
                row.average_cqd,
                format!("avg_cqd_below_archive_threshold_{ARCHIVE_AVG_CQD_THRESHOLD:.3}"),
            );
        }
    }

    for group_id in folded_children_outside_keep_limit(rows, &members_by_group_id, FOLDED_CHILD_KEEP_LIMIT) {
        if let Some(row) = rows.iter().find(|row| row.group_id == group_id) {
            push_archive_reason(
                &mut by_group_id,
                row.group_id,
                row.average_cqd,
                format!("folded_child_outside_top_{FOLDED_CHILD_KEEP_LIMIT}"),
            );
        }
    }

    let mut out: Vec<_> = by_group_id.into_values().collect();
    out.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    out
}

fn push_archive_reason(
    by_group_id: &mut HashMap<GroupId, ArchiveCandidate>,
    group_id: GroupId,
    average_cqd: f64,
    reason: String,
) {
    by_group_id
        .entry(group_id)
        .and_modify(|candidate| {
            if !candidate.reason.split(';').any(|part| part == reason.as_str()) {
                candidate.reason.push(';');
                candidate.reason.push_str(&reason);
            }
            candidate.average_cqd = average_cqd;
        })
        .or_insert(ArchiveCandidate {
            group_id,
            reason,
            average_cqd,
        });
}

fn folded_children_outside_keep_limit(
    rows: &[LaneResultRow],
    members_by_group_id: &HashMap<GroupId, Vec<String>>,
    keep_limit: usize,
) -> Vec<GroupId> {
    let mut consumed = vec![false; rows.len()];
    let mut archived = Vec::new();

    for i in 0..rows.len() {
        if consumed[i] {
            continue;
        }

        consumed[i] = true;
        let Some(parent_members) = members_by_group_id.get(&rows[i].group_id) else {
            continue;
        };

        let mut child_count = 0usize;
        for j in (i + 1)..rows.len() {
            if consumed[j] {
                continue;
            }

            let Some(child_members) = members_by_group_id.get(&rows[j].group_id) else {
                continue;
            };

            if has_member_overlap(parent_members, child_members) {
                consumed[j] = true;
                child_count += 1;
                if child_count > keep_limit {
                    archived.push(rows[j].group_id);
                }
            }
        }
    }

    archived
}

fn has_member_overlap(a: &[String], b: &[String]) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }

    let seen: HashSet<&str> = a.iter().map(String::as_str).collect();
    b.iter().any(|member| seen.contains(member.as_str()))
}

struct RateMatrix {
    rates: Vec<Vec<Option<f64>>>,
    covered_standard_indices: HashSet<usize>,
    loaded_pairs: usize,
}

impl RateMatrix {
    fn new(size: usize) -> Self {
        let mut rates = vec![vec![None; size]; size];
        for i in 0..size {
            rates[i][i] = Some(50.0);
        }

        Self {
            rates,
            covered_standard_indices: HashSet::new(),
            loaded_pairs: 0,
        }
    }

    fn get(&self, i: usize, j: usize) -> f64 { self.rates[i][j].expect("rate should be loaded before fight") }

    fn loaded_pair_count(&self) -> usize { self.loaded_pairs }

    fn set_pair(&mut self, i: usize, j: usize, rate_i_to_j: f64) {
        if i == j {
            self.rates[i][j] = Some(50.0);
            return;
        }

        if self.rates[i][j].is_none() {
            self.loaded_pairs += 1;
        }

        self.rates[i][j] = Some(rate_i_to_j);
        self.rates[j][i] = Some(100.0 - rate_i_to_j);
    }

    fn ensure_rates_for_standard(
        &mut self,
        db: &Db,
        lane_size: usize,
        round: usize,
        nodes: &[RankNode],
        standard: &[usize],
        config: &RankerConfig,
    ) -> anyhow::Result<()> {
        let new_standard: Vec<usize> = standard
            .iter()
            .copied()
            .filter(|idx| !self.covered_standard_indices.contains(idx))
            .collect();

        if new_standard.is_empty() {
            return Ok(());
        }

        let mut missing_pairs = Vec::<(usize, usize, StoredGroup, StoredGroup)>::new();
        let mut seen_pairs = HashSet::<(usize, usize)>::new();

        for &std_idx in &new_standard {
            for i in 0..nodes.len() {
                if i == std_idx {
                    continue;
                }

                if self.rates[i][std_idx].is_some() {
                    continue;
                }

                let (a_idx, b_idx) = ordered_index_pair(i, std_idx);
                if !seen_pairs.insert((a_idx, b_idx)) {
                    continue;
                }

                if let Some(rate) = db.get_rate(nodes[i].group.id, nodes[std_idx].group.id)? {
                    self.set_pair(i, std_idx, rate);
                    continue;
                }

                if let Some(reverse_rate) = db.get_rate(nodes[std_idx].group.id, nodes[i].group.id)? {
                    self.set_pair(i, std_idx, 100.0 - reverse_rate);
                    continue;
                }

                missing_pairs.push((i, std_idx, nodes[i].group.clone(), nodes[std_idx].group.clone()));
            }
        }

        if missing_pairs.is_empty() {
            for idx in new_standard {
                self.covered_standard_indices.insert(idx);
            }

            db.set_lane_progress(
                lane_size,
                "rates_ready",
                round,
                config.total_rounds,
                self.loaded_pair_count(),
                0,
                0,
                &format!(
                    "rates for current target already cached, loaded_pairs={}, mode=lazy_rate_matrix_dynamic_workers",
                    self.loaded_pair_count()
                ),
            )?;

            return Ok(());
        }

        let total = missing_pairs.len();
        let requested_outer_workers = config.outer_workers;
        let inner_workers = config.inner_workers;
        let workers = resolve_rate_pair_workers(requested_outer_workers, total);
        let outer_label = format_outer_workers(requested_outer_workers, workers);
        let inner_label = format_inner_workers(inner_workers);
        let mode = if requested_outer_workers == 0 {
            "dynamic_queue"
        } else {
            "static_chunks"
        };
        let done = Arc::new(AtomicUsize::new(0));
        let rate_started = Arc::new(Instant::now());

        db.set_lane_progress(
            lane_size,
            "computing_rates",
            round,
            config.total_rounds,
            0,
            total,
            0,
            &format!(
                "computing missing rates for current target 0/{total}, 0.00 pair/s, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"
            ),
        )?;

        let missing_pairs = Arc::new(missing_pairs);
        let mut handles = Vec::with_capacity(workers);

        if requested_outer_workers == 0 {
            let next_pair = Arc::new(AtomicUsize::new(0));

            for _worker_id in 0..workers {
                let db = db.clone();
                let missing_pairs = Arc::clone(&missing_pairs);
                let next_pair = Arc::clone(&next_pair);
                let done = Arc::clone(&done);
                let rate_started = Arc::clone(&rate_started);
                let samples = config.win_rate_samples;
                let total_rounds = config.total_rounds;
                let outer_label = outer_label.clone();
                let inner_label = inner_label.clone();

                handles.push(thread::spawn(move || -> anyhow::Result<Vec<(usize, usize, GroupId, GroupId, f64)>> {
                    let mut computed = Vec::new();

                    loop {
                        let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                        let Some((i, j, a, b)) = missing_pairs.get(pair_idx) else {
                            break;
                        };

                        let rate = compute_rate_without_db(a, b, samples, inner_workers)?;
                        computed.push((*i, *j, a.id, b.id, rate));

                        let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if should_report_rate_progress(current, total) {
                            let elapsed = rate_started.elapsed().as_secs_f64().max(0.001);
                            let pairs_per_sec = current as f64 / elapsed;
                            let eta_sec = if pairs_per_sec > 0.0 {
                                total.saturating_sub(current) as f64 / pairs_per_sec
                            } else {
                                0.0
                            };

                            db.set_lane_progress(
                                lane_size,
                                "computing_rates",
                                round,
                                total_rounds,
                                current,
                                total,
                                0,
                                &format!(
                                    "computing missing rates for current target {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_label}, mode=dynamic_queue",
                                    pairs_per_sec,
                                    format_duration(elapsed),
                                    format_duration(eta_sec)
                                ),
                            )?;
                        }
                    }

                    Ok(computed)
                }));
            }
        } else {
            for worker_id in 0..workers {
                let db = db.clone();
                let missing_pairs = Arc::clone(&missing_pairs);
                let done = Arc::clone(&done);
                let rate_started = Arc::clone(&rate_started);
                let samples = config.win_rate_samples;
                let total_rounds = config.total_rounds;
                let outer_label = outer_label.clone();
                let inner_label = inner_label.clone();
                let start = total * worker_id / workers;
                let end = total * (worker_id + 1) / workers;

                handles.push(thread::spawn(move || -> anyhow::Result<Vec<(usize, usize, GroupId, GroupId, f64)>> {
                    let mut computed = Vec::with_capacity(end.saturating_sub(start));

                    for pair_idx in start..end {
                        let Some((i, j, a, b)) = missing_pairs.get(pair_idx) else {
                            break;
                        };

                        let rate = compute_rate_without_db(a, b, samples, inner_workers)?;
                        computed.push((*i, *j, a.id, b.id, rate));

                        let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if should_report_rate_progress(current, total) {
                            let elapsed = rate_started.elapsed().as_secs_f64().max(0.001);
                            let pairs_per_sec = current as f64 / elapsed;
                            let eta_sec = if pairs_per_sec > 0.0 {
                                total.saturating_sub(current) as f64 / pairs_per_sec
                            } else {
                                0.0
                            };

                            db.set_lane_progress(
                                lane_size,
                                "computing_rates",
                                round,
                                total_rounds,
                                current,
                                total,
                                0,
                                &format!(
                                    "computing missing rates for current target {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_label}, mode=static_chunks",
                                    pairs_per_sec,
                                    format_duration(elapsed),
                                    format_duration(eta_sec)
                                ),
                            )?;
                        }
                    }

                    Ok(computed)
                }));
            }
        }

        let mut db_rates = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
        let mut matrix_rates = Vec::<(usize, usize, f64)>::with_capacity(total);

        for handle in handles {
            let part = handle.join().expect("rate pair worker thread panicked")?;
            for (i, j, a_id, b_id, rate) in part {
                db_rates.push((a_id, b_id, rate));
                matrix_rates.push((i, j, rate));
            }
        }

        db.set_lane_progress(
            lane_size,
            "saving_rates",
            round,
            config.total_rounds,
            total,
            total,
            0,
            &format!("saving {total} computed current-target rates in one transaction, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"),
        )?;

        db.save_rate_pairs_bulk(&db_rates, config.win_rate_samples)?;

        for (i, j, rate) in matrix_rates {
            self.set_pair(i, j, rate);
        }

        for idx in new_standard {
            self.covered_standard_indices.insert(idx);
        }

        let elapsed = rate_started.elapsed().as_secs_f64().max(0.001);
        let pairs_per_sec = total as f64 / elapsed;

        db.set_lane_progress(
            lane_size,
            "rates_ready",
            round,
            config.total_rounds,
            total,
            total,
            0,
            &format!(
                "computed and saved current-target rates {total}/{total}, {:.2} pair/s, elapsed {}, loaded_pairs={}, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}",
                pairs_per_sec,
                format_duration(elapsed),
                self.loaded_pair_count()
            ),
        )?;

        Ok(())
    }
}

fn fight_in_memory(nodes: &mut [RankNode], standard: &[usize], rate_matrix: &RateMatrix) {
    let standard_len = standard.len() as f64;

    for (idx, node) in nodes.iter_mut().enumerate() {
        let mut score = 0.0;
        for &bz_idx in standard {
            score += rate_matrix.get(idx, bz_idx);
        }
        node.cqd = score / standard_len;
    }
}

fn make_initial_standard_indices(
    db: &Db,
    lane_size: usize,
    nodes: &[RankNode],
    dsu: &mut TeamDsu,
) -> anyhow::Result<(Vec<usize>, bool)> {
    // 返回值第二项 used_history_target：
    // - true：确实从数据库历史 Score 结果中选入了至少一个初始靶子；
    //         此时神秘系数从第 1 轮开始直接平滑学习。
    // - false：没有可用历史靶子，只能走当前 make 逻辑；
    //          此时保持原算法，热身期前 shenmixishu = cqd，不平滑。
    //
    // 注意：不能直接把数据库 Score 前 50 搬过来。
    // 初始靶子也必须满足 make 的基本条件：
    // 1. 同一个合并后战队最多 TEAM_LIMIT 个
    // 2. 同一个 member 在靶子中只能出现一次
    let top_ids = db.lane_top_score_group_ids(lane_size, nodes.len())?;

    let id_to_idx: HashMap<GroupId, usize> = nodes.iter().enumerate().map(|(idx, node)| (node.group.id, idx)).collect();

    let mut selected = Vec::with_capacity(STANDARD_SIZE);
    let mut selected_set = HashSet::<usize>::new();
    let mut team_count: HashMap<String, usize> = HashMap::new();
    let mut used_members: HashSet<String> = HashSet::new();
    let mut used_history_target = false;

    for group_id in top_ids {
        let Some(&idx) = id_to_idx.get(&group_id) else {
            continue;
        };

        if try_select_standard_index(
            idx,
            nodes,
            dsu,
            &mut selected,
            &mut selected_set,
            &mut team_count,
            &mut used_members,
        ) {
            used_history_target = true;
        }

        if selected.len() >= STANDARD_SIZE {
            return Ok((selected, used_history_target));
        }
    }

    // 如果历史结果不足 50 个，或者历史高分里有大量冲突项，
    // 则按当前 make 逻辑继续补足，但仍然沿用已选靶子的 team/member 占用状态。
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| nodes[b].shenmixishu.total_cmp(&nodes[a].shenmixishu));

    for idx in order {
        let _ = try_select_standard_index(
            idx,
            nodes,
            dsu,
            &mut selected,
            &mut selected_set,
            &mut team_count,
            &mut used_members,
        );

        if selected.len() >= STANDARD_SIZE {
            break;
        }
    }

    Ok((selected, used_history_target))
}
fn make_standard_indices(nodes: &[RankNode], dsu: &mut TeamDsu) -> Vec<usize> {
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| nodes[b].shenmixishu.total_cmp(&nodes[a].shenmixishu));

    let mut selected = Vec::with_capacity(STANDARD_SIZE);
    let mut selected_set = HashSet::<usize>::new();
    let mut team_count: HashMap<String, usize> = HashMap::new();
    let mut used_members: HashSet<String> = HashSet::new();

    for idx in order {
        try_select_standard_index(
            idx,
            nodes,
            dsu,
            &mut selected,
            &mut selected_set,
            &mut team_count,
            &mut used_members,
        );

        if selected.len() >= STANDARD_SIZE {
            break;
        }
    }

    selected
}

fn try_select_standard_index(
    idx: usize,
    nodes: &[RankNode],
    dsu: &mut TeamDsu,
    selected: &mut Vec<usize>,
    selected_set: &mut HashSet<usize>,
    team_count: &mut HashMap<String, usize>,
    used_members: &mut HashSet<String>,
) -> bool {
    if selected.len() >= STANDARD_SIZE {
        return false;
    }

    if !selected_set.insert(idx) {
        return false;
    }

    let group = &nodes[idx].group;
    let root_team = dsu.find(&group.team_name);

    if team_count.get(&root_team).copied().unwrap_or(0) >= TEAM_LIMIT {
        selected_set.remove(&idx);
        return false;
    }

    if group.members.iter().any(|member| used_members.contains(member)) {
        selected_set.remove(&idx);
        return false;
    }

    selected.push(idx);
    *team_count.entry(root_team).or_insert(0) += 1;

    for member in &group.members {
        used_members.insert(member.clone());
    }

    true
}
fn data(nodes: &mut [RankNode], round: usize, warmup_rounds: usize, stickiness: usize, smooth_from_start: bool) {
    // 粘性就是神秘系数平滑分母：alpha = 1 / stickiness。
    // 默认情况下：单人组 stickiness=10，双人组 stickiness=20，x 人组 stickiness=10*x。
    //
    // 关键尺度约定：
    // 一旦进入“平滑学习”阶段，shenmixishu 必须始终保持 cqd * 100 的尺度。
    // 因此进入平滑的第一轮，不从 0 开始平滑，而是先初始化为当前 cqd * 100。
    //
    // 最终 Score 的统计窗口不变，仍然只在 warmup_rounds 后累计。
    let alpha = 1.0 / stickiness.max(1) as f64;
    let should_smooth = smooth_from_start || round >= warmup_rounds;
    let starts_smoothing_this_round = (smooth_from_start && round == 1) || (!smooth_from_start && round == warmup_rounds);

    for node in nodes.iter_mut() {
        if round >= warmup_rounds {
            node.n += 1;
            node.cqds += node.cqd;
            node.cqdss += node.cqd * node.cqd;
            node.cqdmin = node.cqdmin.min(node.cqd);
            node.cqdmax = node.cqdmax.max(node.cqd);
        }

        if should_smooth {
            if starts_smoothing_this_round {
                node.shenmixishu = node.cqd * 100.0;
            } else {
                node.shenmixishu = (1.0 - alpha) * node.shenmixishu + (100.0 * alpha) * node.cqd;
            }
        } else {
            node.shenmixishu = node.cqd;
        }
    }
}

fn avg_cqd_rounded_unchanged(nodes: &[RankNode], last: &mut [i64]) -> bool {
    let mut changed = false;

    for (idx, node) in nodes.iter().enumerate() {
        let value = round_to_3_int(node.avg_cqd());
        if last[idx] != value {
            last[idx] = value;
            changed = true;
        }
    }

    !changed
}

#[allow(dead_code)]
fn round_to_3(value: f64) -> f64 { round_to_3_int(value) as f64 / 1000.0 }

fn round_to_3_int(value: f64) -> i64 { (value * 1000.0).round() as i64 }

fn ordered_index_pair(a: usize, b: usize) -> (usize, usize) { if a <= b { (a, b) } else { (b, a) } }

fn should_report_rate_progress(done: usize, total: usize) -> bool { done == total || done % 10 == 0 }

fn should_report_round(round: usize, total_rounds: usize) -> bool { round == 1 || round == total_rounds || round % 1000 == 0 }

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

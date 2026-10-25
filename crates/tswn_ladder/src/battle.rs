//! 战斗执行、ELO/handicap 校准与错误记录。
//!
//! 一轮匹配共用带上海时间戳的 seed，每场调用 core 最多推进 20,000 轮。
//! 只有输入双方之一成为唯一胜者才更新分数、handicap 和双向历史；无胜者、解析失败
//! 或运行失败均记为失败场次。全部计划对局失败时返回跳过持久化的标记，不回写评分衰减。

use super::*;

impl Updator {
    pub(super) async fn execute_matches_with_calibrated_elo(
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

        // 本轮开始时的分数与 K：所有对局的期望都按这份快照计算，结算统一放在循环之后。
        // 旧实现每打完一场就改 `players[].score`，同一轮里后打的场次看到的是已经被前几场
        // 改过的分数——同一批对局换个顺序会算出不同的分数。
        let start_scores = players.iter().map(|p| p.score).collect::<Vec<i32>>();
        let k_factors = players.iter().map(|p| self.k_factor(p.id)).collect::<Vec<f64>>();
        let mut executed_pairs = Vec::with_capacity(valid_matches.len());
        let mut outcomes = Vec::with_capacity(valid_matches.len());

        for (match_index, pair) in valid_matches.iter().enumerate() {
            let i = pair.0;
            let j = pair.1;
            let handicap_direction = if players[i].id < players[j].id { 1.0 } else { -1.0 };
            let key = PairKey::new(players[i].id, players[j].id);
            let handicap = handicap_map.get(&key).map(|data| data.basic_value).unwrap_or(0.0);

            let score_diff_with_handicap = start_scores[i] as f64 - start_scores[j] as f64
                + if players[i].id < players[j].id {
                    handicap * HANDICAP_SCALE
                } else {
                    -handicap * HANDICAP_SCALE
                };
            let expected_with_handicap = logistic_expected(score_diff_with_handicap);

            let score_diff_base = start_scores[i] as f64 - start_scores[j] as f64;
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
            executed_pairs.push((i, j));
            outcomes.push(result == 1);

            let actual = if result == 1 { 1.0 } else { 0.0 };
            // 每人用自己的 K：新玩家用满 K 快速就位，打了几千场的老玩家用收缩后的 K，
            // 稳态误差随之下降，名次不再每分钟大幅互穿。
            total_delta += (k_factors[i] * (actual - expected_base)).abs();

            let played = self.member_state.entry(players[i].id).or_default();
            played.matches += 1.0;
            let played = self.member_state.entry(players[j].id).or_default();
            played.matches += 1.0;

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
                        persisted_value: 0.0,
                        persisted_count: 0.0,
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

        // 统一结算：本轮所有对局都按轮初分数算期望，累加后再一次性落到 players 上。
        // 这样同一批对局换个顺序得到的分数完全一致。
        let settled = settle_round(&start_scores, &executed_pairs, &outcomes, &k_factors);
        for (idx, target) in settled.iter().enumerate() {
            players[idx].score = self.probabilistic_round(*target);
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

    pub(super) fn get_winner(
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

    pub(super) fn log_battle_error(
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
}

/// 一轮的分数结算（纯函数，便于回归测试）。
///
/// `scores` 必须是**轮初**分数：每场对局的期望胜率都按它计算，因此结算结果与对局顺序
/// 无关。这正是旧实现的 bug——它每打完一场就改 `players[].score`，同一轮里后打的场次
/// 看到的是已经被前几场改过的分数，同一批对局换个顺序会算出不同的分数，而且给后打的
/// 场次掺进了一个与对手无关的偏置。
///
/// `outcomes[k] == true` 表示 `matches[k]` 的左侧（`i`）获胜；`k_factors` 是每人本轮
/// 使用的 K。返回值是每人轮末的**未取整**目标分（已含胜场 +1），由调用方做概率取整。
pub(super) fn settle_round(scores: &[i32], matches: &[(usize, usize)], outcomes: &[bool], k_factors: &[f64]) -> Vec<f64> {
    let mut targets = scores.iter().map(|score| *score as f64).collect::<Vec<f64>>();
    for (idx, &(i, j)) in matches.iter().enumerate() {
        let Some(winner_left) = outcomes.get(idx).copied() else {
            continue;
        };
        if i >= targets.len() || j >= targets.len() || i == j {
            continue;
        }

        let actual = if winner_left { 1.0 } else { 0.0 };
        let expected = logistic_expected(scores[i] as f64 - scores[j] as f64);
        targets[i] += k_factors[i] * (actual - expected);
        targets[j] -= k_factors[j] * (actual - expected);
        // 胜场 +1：整除性上等价于取整后再 +1，放在这里让「一轮的总增量」只有一个来源。
        if winner_left {
            targets[i] += 1.0;
        } else {
            targets[j] += 1.0;
        }
    }
    targets
}

pub(super) fn generate_unique_seed(rng: &mut fastrand::Rng) -> String {
    let basic_seed = format!("{:06}", rng.u32(..1_000_000));
    let seed = format_seed_timestamp(Utc::now());
    format!("{seed} #{basic_seed}")
}

/// 将调用方给定的 UTC 时刻格式化为上海时间，精度到分钟。
///
/// 这里只生成 seed 的时间前缀，不读取系统时钟或随机源；不同主机时区不会改变相同时刻的输出。
pub(super) fn format_seed_timestamp(now: DateTime<Utc>) -> String {
    now.with_timezone(&Shanghai).format("%Y-%m-%d %H:%M").to_string()
}

pub(super) fn run_battle_winner(runs: &str, _player1: &str, _player2: &str) -> Result<Option<i32>> {
    let mut runner = Runner::new_from_namerena_raw(runs.to_string()).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    if runner.run_to_completion(20_000).winner_team.is_none() {
        return Ok(None);
    }
    match runner.winner_team_indices().as_slice() {
        [0] => Ok(Some(1)),
        [1] => Ok(Some(0)),
        _ => Ok(None),
    }
}

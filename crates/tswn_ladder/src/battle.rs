//! 战斗执行、ELO/handicap 校准与错误记录。

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
            // 每人用自己的 K：新玩家用满 K 快速就位，打了几千场的老玩家用收缩后的 K，
            // 稳态误差随之下降，名次不再每分钟大幅互穿。
            let k_i = self.k_factor(players[i].id);
            let k_j = self.k_factor(players[j].id);
            let delta_i = k_i * (actual - expected_base);
            let delta_j = k_j * (actual - expected_base);
            let p1_new_score = self.probabilistic_round(players[i].score as f64 + delta_i);
            let p2_new_score = self.probabilistic_round(players[j].score as f64 - delta_j);
            let actual_delta = p1_new_score - players[i].score;

            players[i].score = p1_new_score;
            players[j].score = p2_new_score;

            if actual > 0.0 {
                players[i].score += 1;
            } else {
                players[j].score += 1;
            }

            *self.match_counts.entry(players[i].id).or_insert(0.0) += 1.0;
            *self.match_counts.entry(players[j].id).or_insert(0.0) += 1.0;

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

pub(super) fn generate_unique_seed(rng: &mut fastrand::Rng) -> String {
    let basic_seed = format!("{:06}", rng.u32(..1_000_000));
    let seed = format_seed_timestamp(Utc::now());
    format!("{seed} #{basic_seed}")
}

/// seed 时间戳固定用 `Asia/Shanghai`，不随部署主机时区变化。
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

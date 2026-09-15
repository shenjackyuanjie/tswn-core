//! 配队矩阵的有界窗口执行与稳定汇总。
//!
//! 并行单元横跨选手、队友和靶子，不能只把同一个选手的队友留在串行外层。
//! 窗口只限制在途请求，不改变原始汇总顺序；未完整完成的选手不会输出半成品排名。

use std::sync::atomic::{AtomicBool, Ordering};

use tswn_core::runtime::{RuntimeCqpMatchup, runtime_cqp_matchups};

use super::parse::{first_duplicate_name_in_matchup, groups_have_same_players};

pub(super) struct PairMatrixInput<'a> {
    pub players: &'a [String],
    pub teammates: &'a [String],
    pub targets: &'a [String],
    pub target_factors: &'a [f64],
    pub teammate_factors: &'a [f64],
    pub target_factored: bool,
    pub teammate_factored: bool,
    pub n: usize,
    pub eval_rq: f64,
    pub threads: u32,
    pub cancel: &'a AtomicBool,
}

#[derive(Default)]
struct PairAccumulator {
    weighted_rate: f64,
    factor: f64,
    valid: usize,
}

enum Slot {
    Mirror,
    Skip,
    Request(usize),
}

pub(super) fn run_pair_matrix(
    input: &PairMatrixInput<'_>,
    on_progress: impl FnMut(usize),
    on_player: impl FnMut(usize, Vec<(f64, usize)>) -> Result<(), String>,
) -> Result<(), String> {
    run_pair_matrix_windowed(input, 4096, on_progress, on_player)
}

fn run_pair_matrix_windowed(
    input: &PairMatrixInput<'_>,
    window_size: usize,
    mut on_progress: impl FnMut(usize),
    mut on_player: impl FnMut(usize, Vec<(f64, usize)>) -> Result<(), String>,
) -> Result<(), String> {
    let target_count = input.targets.len();
    let teammate_count = input.teammates.len();
    if target_count == 0 || teammate_count == 0 || input.players.is_empty() || window_size == 0 {
        return Err("pair: 配队矩阵或窗口为空。".to_owned());
    }
    if input.target_factors.len() != target_count || input.teammate_factors.len() != teammate_count {
        return Err("pair: 权重数量与输入数量不一致。".to_owned());
    }
    let total = input
        .players
        .len()
        .checked_mul(teammate_count)
        .and_then(|n| n.checked_mul(target_count))
        .ok_or_else(|| "pair: 配队矩阵大小溢出。".to_owned())?;
    let mut done = 0;
    let mut accumulated = PairAccumulator::default();
    let mut rates = Vec::new();
    for offset in (0..total).step_by(window_size) {
        if input.cancel.load(Ordering::Relaxed) {
            break;
        }
        let end = offset.saturating_add(window_size).min(total);
        let mut requests = Vec::new();
        let mut slots = Vec::with_capacity(end - offset);
        for flat in offset..end {
            let pair = flat / target_count;
            let player_index = pair / teammate_count;
            let teammate_index = pair % teammate_count;
            let target = &input.targets[flat % target_count];
            let team = format!("{}\n{}", input.players[player_index], input.teammates[teammate_index]);
            if input.target_factored && groups_have_same_players(&team, target) {
                slots.push(Slot::Mirror);
                done += 1;
            } else if !input.target_factored && first_duplicate_name_in_matchup(&[&team, target]).is_some() {
                slots.push(Slot::Skip);
                done += 1;
            } else {
                slots.push(Slot::Request(requests.len()));
                let lines = |group: &str| group.lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
                requests.push(RuntimeCqpMatchup::new(vec![lines(&team), lines(target)]));
            }
        }
        on_progress(done);
        let mut matrix = runtime_cqp_matchups(&requests, input.n, input.eval_rq, input.threads, input.cancel, || {
            done += 1;
            on_progress(done);
        })
        .map_err(|err| format!("pair 执行失败: {err}"))?;

        // 浮点加权必须按原始 target 顺序进行，不能按 worker 的完成顺序累加。
        for (slot_offset, slot) in slots.into_iter().enumerate() {
            let flat = offset + slot_offset;
            let target_index = flat % target_count;
            let pair = flat / target_count;
            let teammate_index = pair % teammate_count;
            let player_index = pair / teammate_count;
            let rate = match slot {
                Slot::Mirror => Some(50.0),
                Slot::Skip => None,
                Slot::Request(index) => {
                    let Some(outcome) = matrix.matchups[index].take() else {
                        // 取消后不能把尚未完成的 matchup 当成有效的零胜率。
                        return Ok(());
                    };
                    outcome.summary.ok().map(|summary| summary.win_rate_percent())
                }
            };
            if let Some(rate) = rate {
                let factor = if input.target_factored {
                    input.target_factors[target_index]
                } else {
                    1.0
                };
                accumulated.weighted_rate += rate * factor;
                accumulated.factor += factor;
                accumulated.valid += 1;
            }
            if target_index + 1 == target_count {
                if accumulated.valid > 0 {
                    let avg = if accumulated.factor > 0.0 {
                        accumulated.weighted_rate / accumulated.factor
                    } else {
                        0.0
                    };
                    let factor = if input.teammate_factored {
                        input.teammate_factors[teammate_index]
                    } else {
                        1.0
                    };
                    rates.push((avg * factor, teammate_index));
                }
                accumulated = PairAccumulator::default();
                if teammate_index + 1 == teammate_count {
                    on_player(player_index, std::mem::take(&mut rates))?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::score::bench_batch_rate_for_group;

    #[test]
    fn matrix_matches_legacy_weights_mirrors_duplicates_and_window_boundaries() {
        let players = vec!["+ol:alpha".to_owned(), "+ol:beta".to_owned()];
        let teammates = vec!["mate@red".to_owned(), "mate2@green".to_owned(), "mate@red".to_owned()];
        let targets = vec![
            "+ol:alpha\nmate@red".to_owned(),
            "target@blue".to_owned(),
            "target2@red".to_owned(),
        ];
        let factors = [0.5, 1.25, 3.0];
        let mate_factors = [0.25, 2.0, 1.0];
        for target_factored in [false, true] {
            for teammate_factored in [false, true] {
                let cancel = AtomicBool::new(false);
                let mut expected = Vec::new();
                for (player_index, player) in players.iter().enumerate() {
                    let mut rates = Vec::new();
                    for (index, mate) in teammates.iter().enumerate() {
                        let summary = bench_batch_rate_for_group(
                            &format!("{player}\n{mate}"),
                            &targets,
                            target_factored.then_some(&factors[..]),
                            24,
                            Some(1),
                            6.0,
                            false,
                            &mut String::new(),
                            &cancel,
                            |_, _, _, _| {},
                        );
                        if summary.valid_matchups > 0 {
                            rates.push((summary.avg * if teammate_factored { mate_factors[index] } else { 1.0 }, index));
                        }
                    }
                    expected.push((player_index, rates));
                }
                for threads in [1, 4] {
                    for window in [1, 7, 4096] {
                        let input = PairMatrixInput {
                            players: &players,
                            teammates: &teammates,
                            targets: &targets,
                            target_factors: &factors,
                            teammate_factors: &mate_factors,
                            target_factored,
                            teammate_factored,
                            n: 24,
                            eval_rq: 6.0,
                            threads,
                            cancel: &cancel,
                        };
                        let mut actual = Vec::new();
                        let mut progress = Vec::new();
                        run_pair_matrix_windowed(
                            &input,
                            window,
                            |done| progress.push(done),
                            |index, rates| {
                                actual.push((index, rates));
                                Ok(())
                            },
                        )
                        .unwrap();
                        assert_eq!(actual, expected, "threads={threads}, window={window}");
                        assert_eq!(progress.last(), Some(&18));
                        assert!(progress.windows(2).all(|pair| pair[0] <= pair[1]));
                    }
                }
            }
        }
    }

    #[test]
    fn cancellation_does_not_emit_partial_player() {
        let players = vec!["+ol:alpha".to_owned()];
        let teammates = vec!["mate@red".to_owned()];
        let targets = vec!["target@blue".to_owned(), "target2@red".to_owned()];
        let cancel = AtomicBool::new(false);
        let input = PairMatrixInput {
            players: &players,
            teammates: &teammates,
            targets: &targets,
            target_factors: &[1.0, 1.0],
            teammate_factors: &[1.0],
            target_factored: false,
            teammate_factored: false,
            n: 24,
            eval_rq: 6.0,
            threads: 1,
            cancel: &cancel,
        };
        let mut emitted = 0;
        run_pair_matrix_windowed(
            &input,
            1,
            |done| {
                if done == 1 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            |_, _| {
                emitted += 1;
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(emitted, 0);
    }
}

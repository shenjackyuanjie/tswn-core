//! 批量胜率的有界请求执行与稳定汇总。

use std::fs::File;
use std::io::Write as _;
use std::sync::atomic::Ordering;

use tswn_core::runtime::{RuntimeCqpMatchup, runtime_cqp_matchups_observed};

use super::format::should_highlight;
use super::format::{format_batch_file_record, format_batch_screen_log};
use super::live::{ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::{create_output_file, finalize_sorted_output_file};
use super::parse::{
    first_duplicate_name_in_matchup, normalized_group_players, parse_factored_target_groups, parse_player_groups_with_labels,
    parse_target_groups,
};
use super::score::BatchRateSummary;
use super::score::{eval_rq, outer_thread_spec};
use super::types::{BatchRateInput, ProgressEvent};

const MATCHUP_WINDOW: usize = 4096;

pub fn run_batch_rate(input: BatchRateInput, send: impl Fn(ProgressEvent)) { run_batch_rate_observed(input, send, None); }

/// GUI 可选增量观察接口；None 保留原来的输出契约。
pub fn run_batch_rate_observed(input: BatchRateInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
    run_batch_rate_windowed(input, send, observer, MATCHUP_WINDOW);
}

fn run_batch_rate_windowed(input: BatchRateInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>, window: usize) {
    let (target_groups, target_factors) = if input.target_factor_enabled {
        match parse_factored_target_groups(&input.target_text) {
            Ok(targets) => targets,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        }
    } else {
        let groups = parse_target_groups(&input.target_text, input.target_double_plus);
        let factors = vec![1.0; groups.len()];
        (groups, factors)
    };
    let (player_groups, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("batch-rate: 靶子列表为空。".to_string())));
        return;
    }
    if player_groups.is_empty() {
        send(ProgressEvent::Done(Err("batch-rate: 选手列表为空。".to_string())));
        return;
    }

    let mut output = match input.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    if output.is_none() {
        send(ProgressEvent::Log("未选择输出文件，本次只输出到日志。".to_string()));
    }

    let n = input.options.count.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let Some(total) = player_groups.len().checked_mul(target_groups.len()) else {
        send(ProgressEvent::Done(Err("cqd/cqp: 对局矩阵大小溢出。".into())));
        return;
    };
    let mut done = 0usize;

    let mut results = player_labels
        .iter()
        .map(|label| BatchRateJobResult {
            label: label.clone(),
            summary: BatchRateSummary {
                avg: 0.0,
                wins: 0,
                total: 0,
                valid_matchups: 0,
                skipped_matchups: 0,
            },
            accumulated_factor: 0.0,
            detail_rates: Vec::new(),
        })
        .collect::<Vec<_>>();
    // 输入身份只解析一次，镜像检测及实时最终汇总复用相同判定。
    let identities = input.target_factor_enabled.then(|| {
        (
            player_groups.iter().map(|group| normalized_group_players(group)).collect::<Vec<_>>(),
            target_groups.iter().map(|group| normalized_group_players(group)).collect::<Vec<_>>(),
        )
    });
    let is_mirror =
        |player: usize, target: usize| identities.as_ref().is_some_and(|(players, targets)| players[player] == targets[target]);
    // 旧格式先累加所有镜像，再按输入顺序累加实际对局；窗口不能改变浮点运算顺序。
    if input.target_factor_enabled {
        for (player, result) in results.iter_mut().enumerate() {
            if input.cancel.load(Ordering::Relaxed) {
                send(ProgressEvent::Done(Ok("已停止。".into())));
                return;
            }
            for (target, group) in target_groups.iter().enumerate() {
                if is_mirror(player, target) {
                    result.summary.avg += 50.0 * target_factors[target];
                    result.summary.wins += 1;
                    result.summary.total += 2;
                    result.summary.valid_matchups += 1;
                    result.accumulated_factor += target_factors[target];
                    if input.show_matchups && observer.is_none() {
                        result.detail_rates.push((50.0, group.clone()));
                    }
                }
            }
        }
    }
    let mut live_rates = std::collections::HashMap::<usize, (usize, Vec<Option<f64>>)>::new();
    let mut record_live = |player: usize, target: usize, rate: Option<f64>| {
        let Some(observer) = observer else { return };
        let entry = live_rates.entry(player).or_insert_with(|| (0, vec![None; target_groups.len()]));
        entry.0 += 1;
        entry.1[target] = rate;
        let mut update = ResultUpdate::new(player, &player_labels[player], ResultKind::Rate, precision);
        if input.show_matchups {
            let mut detail = ResultEntry::number(
                target,
                super::format::clean_group_label(&target_groups[target]),
                rate.unwrap_or(0.0),
            );
            if rate.is_none() {
                detail.value = None;
                detail.text = "跳过或计算失败".into();
            }
            update.entries.push(detail);
        }
        if entry.0 == target_groups.len() {
            let (_, rates) = live_rates.remove(&player).unwrap();
            let mut sum = 0.0;
            let mut weight = 0.0;
            // 兼容原输出：镜像项先累计，随后按原靶子顺序累计实际对局。
            for mirrors in [true, false] {
                for (index, rate) in rates.iter().enumerate() {
                    let mirror = is_mirror(player, index);
                    if mirror == mirrors
                        && let Some(rate) = rate
                    {
                        sum += rate * target_factors[index];
                        weight += target_factors[index];
                    }
                }
            }
            let score = if weight > 0.0 { sum / weight } else { 0.0 };
            update.finish = Some(ResultFinish {
                score: Some(score),
                visible: input.options.min_screen.is_none_or(|min| score >= min),
                highlight: should_highlight(score, input.options.min_screen, input.highlight_delta),
            });
        }
        if !update.entries.is_empty() || update.finish.is_some() {
            observer(update);
        }
    };
    for offset in (0..total).step_by(window) {
        if input.cancel.load(Ordering::Relaxed) {
            break;
        }
        let end = offset.saturating_add(window).min(total);
        let mut requests = Vec::with_capacity(end - offset);
        let mut request_slots = Vec::with_capacity(end - offset);
        for flat in offset..end {
            if input.cancel.load(Ordering::Relaxed) {
                break;
            }
            let player_index = flat / target_groups.len();
            let target_index = flat % target_groups.len();
            let player = &player_groups[player_index];
            let target = &target_groups[target_index];
            if is_mirror(player_index, target_index) {
                record_live(player_index, target_index, Some(50.0));
                done += 1;
                send(ProgressEvent::Progress { done, total });
            } else if !input.target_factor_enabled && first_duplicate_name_in_matchup(&[player, target]).is_some() {
                record_live(player_index, target_index, None);
                results[player_index].summary.skipped_matchups += 1;
                done += 1;
                send(ProgressEvent::Progress { done, total });
            } else {
                requests.push(RuntimeCqpMatchup::new(vec![group_lines(player), group_lines(target)]));
                request_slots.push((player_index, target_index));
            }
        }

        let matrix = match runtime_cqp_matchups_observed(
            &requests,
            n,
            eval_rq,
            outer_thread_spec(input.options.threads),
            &input.cancel,
            |index, result| {
                let (player, target) = request_slots[index];
                record_live(
                    player,
                    target,
                    result.summary.as_ref().ok().map(|summary| summary.win_rate_percent()),
                );
                done += 1;
                send(ProgressEvent::Progress { done, total });
            },
        ) {
            Ok(matrix) => matrix,
            Err(err) => {
                send(ProgressEvent::Done(Err(format!("cqd/cqp 执行失败: {err}"))));
                return;
            }
        };

        for ((player_index, target_index), outcome) in request_slots.into_iter().zip(matrix.matchups) {
            let Some(outcome) = outcome else {
                continue;
            };
            let result = &mut results[player_index];
            match outcome.summary {
                Ok(summary) => {
                    let percent = summary.win_rate_percent();
                    let factor = target_factors[target_index];
                    result.summary.avg += percent * factor;
                    result.summary.wins += summary.wins;
                    result.summary.total += summary.total;
                    result.summary.valid_matchups += 1;
                    result.accumulated_factor += factor;
                    if input.show_matchups && observer.is_none() {
                        result.detail_rates.push((percent, target_groups[target_index].clone()));
                    }
                }
                Err(_) => result.summary.skipped_matchups += 1,
            }
        }
    }

    for result in &mut results {
        result.summary.avg = if result.accumulated_factor > 0.0 {
            result.summary.avg / result.accumulated_factor
        } else {
            0.0
        };
        if input.cancel.load(Ordering::Relaxed) && result.summary.valid_matchups == 0 && result.summary.skipped_matchups == 0 {
            continue;
        }
        if let Err(err) = emit_batch_rate_result(
            result,
            &input,
            &mut output,
            precision,
            observer.is_none().then_some(&send as &dyn Fn(ProgressEvent)),
        ) {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    }

    if input.cancel.load(Ordering::Relaxed) {
        send(ProgressEvent::Done(Ok("已停止。".to_string())));
        return;
    }

    if let Err(err) = finalize_sorted_output_file(output.take(), input.output_file.as_deref(), input.output_mode) {
        send(ProgressEvent::Done(Err(err)));
        return;
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
}

struct BatchRateJobResult {
    label: String,
    summary: BatchRateSummary,
    accumulated_factor: f64,
    detail_rates: Vec<(f64, String)>,
}

fn group_lines(group: &str) -> Vec<String> {
    group.lines().map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned).collect()
}

fn emit_batch_rate_result(
    result: &BatchRateJobResult,
    input: &BatchRateInput,
    output: &mut Option<File>,
    precision: usize,
    send: Option<&dyn Fn(ProgressEvent)>,
) -> Result<(), String> {
    if input.options.min_file.is_none_or(|limit| result.summary.avg >= limit)
        && let Some(output) = output.as_mut()
    {
        let line = format_batch_file_record(input.output_mode, &result.label, result.summary.avg, precision);
        if let Err(err) = writeln!(output, "{line}") {
            return Err(format!("写入输出文件失败: {err}"));
        }
    }

    if let Some(send) = send.filter(|_| input.options.min_screen.is_none_or(|limit| result.summary.avg >= limit)) {
        let detail_rates = if input.show_matchups {
            result.detail_rates.as_slice()
        } else {
            &[]
        };
        let log = format_batch_screen_log(&result.label, result.summary.avg, detail_rates, precision);
        if should_highlight(result.summary.avg, input.options.min_screen, input.highlight_delta) {
            send(ProgressEvent::HighlightLog(log));
        } else {
            send(ProgressEvent::Log(log));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::score::bench_batch_rate_for_group;
    use crate::backend::{CommonBenchOptions, OutputMode};
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex, atomic::AtomicBool};
    use tswn_core::namerena::eval_name::DEFAULT_EVAL_RQ;

    #[test]
    fn windows_preserve_text_and_live_scores_with_mirrors_weights_and_duplicates() {
        for factored in [false, true] {
            let input = BatchRateInput {
                target_text: if factored {
                    "[[targets]]\nfactor=0.1\nplayers=[\"alpha\"]\n[[targets]]\nfactor=3.7\nplayers=[\"target\"]\n[[targets]]\nfactor=0.9\nplayers=[\"beta\"]".into()
                } else {
                    "alpha\ntarget\nbeta".into()
                },
                player_text: "alpha\nbeta\nalpha".into(),
                target_factor_enabled: factored,
                target_double_plus: false,
                player_double_plus: false,
                show_matchups: true,
                highlight_delta: None,
                output_mode: OutputMode::Log,
                output_file: None,
                options: CommonBenchOptions {
                    count: 12,
                    threads: Some(4),
                    keep_rq: true,
                    verbose: false,
                    min_screen: None,
                    min_file: None,
                    wr_precision: 9,
                },
                cancel: Arc::new(AtomicBool::new(false)),
            };
            let mut baseline = None;
            let mut live_baseline = None;
            for window in [4096, 1, 7] {
                let log = RefCell::new(Vec::new());
                run_batch_rate_windowed(
                    input.clone(),
                    |event| match event {
                        ProgressEvent::Log(line) | ProgressEvent::HighlightLog(line) => log.borrow_mut().push(line),
                        ProgressEvent::Done(result) => assert!(result.is_ok()),
                        _ => {}
                    },
                    None,
                    window,
                );
                let log = log.into_inner();
                if let Some(expected) = &baseline {
                    assert_eq!(&log, expected);
                } else {
                    baseline = Some(log);
                }
                let finishes = Mutex::new(vec![None; 3]);
                run_batch_rate_windowed(
                    input.clone(),
                    |_| {},
                    Some(&|update| {
                        if let Some(finish) = update.finish {
                            finishes.lock().unwrap()[update.group] = finish.score;
                        }
                    }),
                    window,
                );
                let scores = finishes.into_inner().unwrap();
                assert!(scores.iter().all(Option::is_some));
                if let Some(expected) = &live_baseline {
                    assert_eq!(&scores, expected);
                } else {
                    live_baseline = Some(scores);
                }
            }
        }
    }
    #[test]
    fn batch_rate_runtime_matrix_matches_legacy_summary_and_order() {
        let players = ["alpha@red", "beta@blue"];
        let targets = ["gamma@green", "delta@yellow"];
        let mut expected = Vec::new();
        for player in players {
            let mut verbose = String::new();
            let cancel = AtomicBool::new(false);
            let summary = bench_batch_rate_for_group(
                player,
                &targets.map(str::to_owned),
                None,
                24,
                Some(1),
                DEFAULT_EVAL_RQ,
                false,
                &mut verbose,
                &cancel,
                |_, _, _, _| {},
            );
            expected.push(format_batch_screen_log(player, summary.avg, &[], 9));
        }

        let events = RefCell::new(Vec::new());
        run_batch_rate(
            BatchRateInput {
                target_text: targets.join("\n"),
                player_text: players.join("\n"),
                target_factor_enabled: false,
                target_double_plus: false,
                player_double_plus: false,
                show_matchups: false,
                highlight_delta: None,
                output_mode: OutputMode::Log,
                output_file: None,
                options: CommonBenchOptions {
                    count: 24,
                    threads: Some(4),
                    keep_rq: true,
                    verbose: false,
                    min_screen: None,
                    min_file: None,
                    wr_precision: 9,
                },
                cancel: Arc::new(AtomicBool::new(false)),
            },
            |event| events.borrow_mut().push(event),
        );

        let events = events.into_inner();
        let actual = events
            .iter()
            .filter_map(|event| match event {
                ProgressEvent::Log(line) if players.iter().any(|player| line.contains(player)) => Some(line.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Progress { done: 4, total: 4 })));
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Done(Ok(_)))));
    }

    #[test]
    fn factored_mirror_match_is_weighted_as_fifty_percent() {
        let events = RefCell::new(Vec::new());
        run_batch_rate(
            BatchRateInput {
                target_text: "[[targets]]\nfactor = 2.5\nplayers = [\"mario\", \"luigi\"]".to_string(),
                player_text: "mario+luigi".to_string(),
                target_factor_enabled: true,
                target_double_plus: false,
                player_double_plus: false,
                show_matchups: true,
                highlight_delta: None,
                output_mode: OutputMode::Log,
                output_file: None,
                options: CommonBenchOptions {
                    count: 1,
                    threads: Some(1),
                    keep_rq: true,
                    verbose: false,
                    min_screen: None,
                    min_file: None,
                    wr_precision: 9,
                },
                cancel: Arc::new(AtomicBool::new(false)),
            },
            |event| events.borrow_mut().push(event),
        );

        let events = events.into_inner();
        assert!(
            events
                .iter()
                .any(|event| matches!(event, ProgressEvent::Log(log) if log.contains("50.000000000")))
        );
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Progress { done: 1, total: 1 })));
        assert!(events.iter().any(|event| matches!(event, ProgressEvent::Done(Ok(_)))));
    }
}

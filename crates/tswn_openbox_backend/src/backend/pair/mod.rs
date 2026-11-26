//! 配队任务：输入准备、队友明细与最终排名。
//!
//! 选手和队友按完整队伍交给矩阵窗口执行；未完成选手不发布最终排名。
//! 增量搭档仅是预览，文件与终态由本模块统一收口，错误或取消不回滚已有输出。

/// 选手×队友×靶子矩阵遍历。
mod matrix;

use super::format::{format_pair_file_record, format_pair_screen_log, should_highlight};
use super::live::{EntryKind, ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::{create_output_file, finalize_sorted_output_file};
use super::parse::{parse_factored_target_groups, parse_player_groups_with_labels, parse_target_groups};
use super::score::{eval_rq, outer_thread_spec};
use super::types::{PairInput, ProgressEvent};
use matrix::{PairMatrixInput, run_pair_matrix, run_pair_matrix_observed};
use std::io::Write as _;
use std::sync::atomic::Ordering;
use std::time::Instant;

/// 同步执行配队矩阵与头部排名，借 send 传回日志、进度和 Done(Result)。
/// 输入按值移交，取消通过 input.cancel 请求；回调失败或 panic 不在此入口捕获。
pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) { run_pair_observed(input, send, None); }

/// 在原任务事件之外发布结构化增量；observer=None 保留传统文本输出。
///
/// 观察者仅在本次同步调用期间借用，可能在计算 worker 并发执行；应把自有更新
/// 转交收件箱，不直接访问 UI。完成前的明细只是预览，最终状态以 Done 为准。
pub fn run_pair_observed(input: PairInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
    let PreparedPairGroups {
        target_groups,
        target_factors,
        players,
        player_labels,
        teammates,
        teammate_labels,
        teammate_factors,
    } = match prepare_pair_groups(&input) {
        Ok(prepared) => prepared,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };

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
    let head = input.head.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let Some(total) = players
        .len()
        .checked_mul(teammates.len())
        .and_then(|n| n.checked_mul(target_groups.len()))
    else {
        send(ProgressEvent::Done(Err("pair: 配队矩阵大小溢出。".to_owned())));
        return;
    };
    let matrix_input = PairMatrixInput {
        players: &players,
        teammates: &teammates,
        targets: &target_groups,
        target_factors: &target_factors,
        teammate_factors: &teammate_factors,
        target_factored: input.target_factor_enabled,
        teammate_factored: input.teammate_factor_enabled,
        n,
        eval_rq,
        threads: outer_thread_spec(input.options.threads),
        cancel: &input.cancel,
    };
    let mut last_progress = Instant::now();
    send(ProgressEvent::Progress { done: 0, total });
    let on_progress = |done| {
        // 短 matchup 可能每秒完成数万次；不让 GUI 消息积压反过来限制计算吞吐。
        if done == total || last_progress.elapsed() >= std::time::Duration::from_millis(40) {
            send(ProgressEvent::Progress { done, total });
            last_progress = Instant::now();
        }
    };
    let on_player = |player_index: usize, mut rates: Vec<(f64, usize)>| {
        // 相同胜率保留输入顺序，文件和 observed 日志复用同一次排序、求和。
        rates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let selected_count = head.min(rates.len());
        let final_score = rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let player_label = &player_labels[player_index];
        let visible = input.options.min_screen.is_none_or(|limit| final_score >= limit);
        let highlight = should_highlight(final_score, input.options.min_screen, input.highlight_delta);
        let file_visible = output.is_some() && input.options.min_file.is_none_or(|limit| final_score >= limit);
        let pair_rates = if visible || file_visible {
            rates
                .into_iter()
                .map(|(rate, index)| (rate, teammate_labels[index].clone()))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if file_visible && let Some(output) = output.as_mut() {
            let line = format_pair_file_record(
                input.output_mode,
                player_label,
                final_score,
                selected_count,
                head,
                &pair_rates,
                precision,
            );
            writeln!(output, "{line}").map_err(|err| format!("写入输出文件失败: {err}"))?;
        }
        let legacy_log = visible.then(|| {
            (
                format_pair_screen_log(
                    player_label,
                    final_score,
                    selected_count,
                    &pair_rates,
                    input.detail_mode,
                    input.detail_min,
                    precision,
                ),
                if highlight { EntryKind::Highlight } else { EntryKind::Plain },
            )
        });
        if let Some(observer) = observer {
            let mut update = ResultUpdate::new_with_legacy(player_index, player_label, ResultKind::Pair, precision);
            update.top = (input.detail_mode == super::types::PairDetailMode::Top).then_some(head);
            update.finish = Some(ResultFinish {
                score: Some(final_score),
                visible,
                highlight,
            });
            update.legacy_log = legacy_log;
            observer(update);
        } else if let Some((log, kind)) = legacy_log {
            if kind == EntryKind::Highlight {
                send(ProgressEvent::HighlightLog(log));
            } else {
                send(ProgressEvent::Log(log));
            }
        }
        Ok(())
    };
    let result = if let Some(observer) = observer {
        run_pair_matrix_observed(&matrix_input, on_progress, on_player, &mut |player, teammate, rate| {
            let mut update = ResultUpdate::new_with_legacy(player, &player_labels[player], ResultKind::Pair, precision);
            update.top = (input.detail_mode == super::types::PairDetailMode::Top).then_some(head);
            if let Some(rate) = rate {
                let visible = match input.detail_mode {
                    super::types::PairDetailMode::None => false,
                    super::types::PairDetailMode::Top => true,
                    super::types::PairDetailMode::Every => input.detail_min.is_none_or(|min| rate >= min),
                };
                if visible {
                    update
                        .entries
                        .push(ResultEntry::number(teammate, teammate_labels[teammate].clone(), rate));
                }
            }
            if !update.entries.is_empty() {
                observer(update);
            }
        })
    } else {
        run_pair_matrix(&matrix_input, on_progress, on_player)
    };
    if let Err(err) = result {
        send(ProgressEvent::Done(Err(err)));
        return;
    }
    if input.cancel.load(Ordering::Relaxed) {
        send(ProgressEvent::Done(Ok("已停止。".to_owned())));
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

/// 解析靶子分组：带权走 TOML，否则按普通分组解析并补 1.0 权重。
fn parse_pair_target_groups(content: &str, factor_enabled: bool) -> Result<(Vec<String>, Vec<f64>), String> {
    if factor_enabled {
        parse_factored_target_groups(content)
    } else {
        let groups = parse_target_groups(content, false);
        let factors = vec![1.0; groups.len()];
        Ok((groups, factors))
    }
}

/// 队友解析结果：分组、展示标签与权重。
type ParsedTeammates = (Vec<String>, Vec<String>, Vec<f64>);

/// 解析队友分组：带权走 TOML，否则按 ++ 语义解析并补 1.0 权重。
fn parse_pair_teammate_groups(content: &str, double_plus: bool, factor_enabled: bool) -> Result<ParsedTeammates, String> {
    if factor_enabled {
        let (groups, factors) = parse_factored_target_groups(content)?;
        let labels = groups.iter().map(|group| group.lines().collect::<Vec<_>>().join("+")).collect();
        Ok((groups, labels, factors))
    } else {
        let (groups, labels) = parse_player_groups_with_labels(content, double_plus);
        let factors = vec![1.0; groups.len()];
        Ok((groups, labels, factors))
    }
}

/// pair 执行前的全部分组与权重快照。
struct PreparedPairGroups {
    target_groups: Vec<String>,
    target_factors: Vec<f64>,
    players: Vec<String>,
    player_labels: Vec<String>,
    teammates: Vec<String>,
    teammate_labels: Vec<String>,
    teammate_factors: Vec<f64>,
}

/// 保留成员原文，交给 core 按完整队伍计算组队加成及 DIY 覆盖。
/// 汇总靶子/选手/队友分组并校验非空，保留成员原文交给 core。
fn prepare_pair_groups(input: &PairInput) -> Result<PreparedPairGroups, String> {
    let (target_groups, target_factors) = parse_pair_target_groups(&input.target_text, input.target_factor_enabled)?;
    let (players, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    let (teammates, teammate_labels, teammate_factors) =
        parse_pair_teammate_groups(&input.teammate_text, input.teammate_double_plus, input.teammate_factor_enabled)?;
    if target_groups.is_empty() {
        return Err("pair: 靶子列表为空。".to_string());
    }
    if players.is_empty() {
        return Err("pair: 选手列表为空。".to_string());
    }
    if teammates.is_empty() {
        return Err("pair: 队友列表为空。".to_string());
    }
    Ok(PreparedPairGroups {
        target_groups,
        target_factors,
        players,
        player_labels,
        teammates,
        teammate_labels,
        teammate_factors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::types::{CommonBenchOptions, OutputMode, PairDetailMode};
    use crate::backend::{BatchRateInput, run_batch_rate_observed, run_to_diy};
    use std::sync::atomic::AtomicBool;
    use std::sync::{Arc, Mutex};

    #[test]
    fn pair_parses_factored_teammates_with_labels_and_weights() {
        let raw = "[[targets]]\nfactor = 2\nplayers = [\"mario\", \"luigi\"]";
        let (groups, labels, factors) = parse_pair_teammate_groups(raw, true, true).expect("valid teammates");
        assert_eq!(groups, vec!["mario\nluigi"]);
        assert_eq!(labels, vec!["mario+luigi"]);
        assert_eq!(factors, vec![2.0]);
    }

    #[test]
    fn pair_cqp_matches_direct_three_player_team_with_or_without_diy() {
        const FIRST: &str = "gpt-5-pro #A8wcy9ZY7@Shabby_fish";
        const SECOND: &str = "Diffindo #3WzDmhsqL@Shabby_fish";
        const MATE: &str = "九十九八桥 6G6GIGLI@Squall";
        const FACTOR: f64 = 0.99044987;
        let names = format!("{FIRST}+{SECOND}");
        let mut pair = PairInput {
            target_text: include_str!("../../../assets/targets/target3.txt").into(),
            target_factor_enabled: false,
            player_text: names.clone(),
            player_double_plus: false,
            teammate_text: format!("[[targets]]\nfactor = {FACTOR}\nplayers = [\"{MATE}\"]"),
            teammate_double_plus: true,
            teammate_factor_enabled: true,
            head: 1,
            detail_mode: PairDetailMode::Every,
            detail_min: None,
            highlight_delta: None,
            output_mode: OutputMode::Log,
            output_file: None,
            options: CommonBenchOptions {
                count: 100,
                threads: Some(1),
                keep_rq: true,
                verbose: false,
                min_screen: None,
                min_file: None,
                wr_precision: 3,
            },
            cancel: Arc::new(AtomicBool::new(false)),
        };
        let cqp = Mutex::new(None);
        run_batch_rate_observed(
            BatchRateInput {
                target_text: pair.target_text.clone(),
                player_text: format!("{names}+{MATE}"),
                target_factor_enabled: false,
                target_double_plus: false,
                player_double_plus: false,
                show_matchups: false,
                highlight_delta: None,
                output_file: None,
                output_mode: OutputMode::Log,
                options: pair.options.clone(),
                cancel: pair.cancel.clone(),
            },
            |_| {},
            Some(&|update| *cqp.lock().unwrap() = update.finish.and_then(|finish| finish.score)),
        );
        let cqp = cqp.into_inner().unwrap().unwrap();
        let score = cqp * FACTOR;
        let expected_detail = format!("  {score:.3} {MATE}");

        let diy = run_to_diy(&names, false, false, false, None, &pair.cancel).unwrap();
        assert!(diy.contains(&format!("}}++{SECOND}+ol:")));
        for (input, double_plus) in [(&names, false), (&diy, true)] {
            pair.player_text.clone_from(input);
            pair.player_double_plus = double_plus;
            let updates = Mutex::new(Vec::new());
            run_pair_observed(pair.clone(), |_| {}, Some(&|update| updates.lock().unwrap().push(update)));
            let updates = updates.into_inner().unwrap();
            let detail = updates.iter().flat_map(|update| &update.entries).next().unwrap();
            assert_eq!(detail.label, MATE);
            assert_eq!(detail.value, Some(score));
            let finish = updates.iter().find(|update| update.finish.is_some()).unwrap();
            assert_eq!(finish.finish.as_ref().unwrap().score, Some(score));
            let log = &finish.legacy_log.as_ref().unwrap().0;
            assert!(log.lines().any(|line| line == expected_detail));
        }
    }
}

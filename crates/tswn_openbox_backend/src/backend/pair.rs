//! 配队任务：输入准备、队友明细与最终排名。

mod matrix;

use super::format::{format_pair_file_record, format_pair_screen_log, should_highlight};
use super::live::{ResultEntry, ResultFinish, ResultKind, ResultObserver, ResultUpdate};
use super::output::{create_output_file, finalize_sorted_output_file};
use super::parse::{parse_factored_target_groups, parse_player_groups_with_labels, parse_target_groups};
use super::score::{eval_rq, outer_thread_spec};
use super::types::{PairInput, ProgressEvent};
use matrix::{PairMatrixInput, run_pair_matrix, run_pair_matrix_observed};
use std::io::Write as _;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tswn_core::cli_api;

pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) { run_pair_observed(input, send, None); }

/// GUI 可选增量观察接口；None 保留原来的输出契约。
pub fn run_pair_observed(input: PairInput, send: impl Fn(ProgressEvent), observer: ResultObserver<'_>) {
    let (target_groups, target_factors) = match parse_pair_target_groups(&input.target_text, input.target_factor_enabled) {
        Ok(targets) => targets,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };
    let (player_groups, player_labels) = parse_player_groups_with_labels(&input.player_text, input.player_double_plus);
    let (teammate_groups, teammate_labels, teammate_factors) =
        match parse_pair_teammate_groups(&input.teammate_text, input.teammate_double_plus, input.teammate_factor_enabled) {
            Ok(value) => value,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        };
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 靶子列表为空。".to_string())));
        return;
    }
    if player_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 选手列表为空。".to_string())));
        return;
    }
    if teammate_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 队友列表为空。".to_string())));
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
    let head = input.head.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let Some(total) = player_groups
        .len()
        .checked_mul(teammate_groups.len())
        .and_then(|n| n.checked_mul(target_groups.len()))
    else {
        send(ProgressEvent::Done(Err("pair: 配队矩阵大小溢出。".to_owned())));
        return;
    };
    let converted_players = match player_groups
        .iter()
        .map(|player| player_group_to_ol(player))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(players) => players,
        Err(err) => {
            send(ProgressEvent::Done(Err(err)));
            return;
        }
    };
    let matrix_input = PairMatrixInput {
        players: &converted_players,
        teammates: &teammate_groups,
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
    let on_player = |player_index: usize, rates: Vec<(f64, usize)>| {
        if observer.is_some() && output.is_none() {
            return Ok(());
        }
        let player_label = &player_labels[player_index];
        let mut pair_rates = rates
            .into_iter()
            .map(|(rate, index)| (rate, teammate_labels[index].clone()))
            .collect::<Vec<_>>();
        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        if input.options.min_file.is_none_or(|limit| final_score >= limit)
            && let Some(output) = output.as_mut()
        {
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
        if observer.is_none() && input.options.min_screen.is_none_or(|limit| final_score >= limit) {
            let log = format_pair_screen_log(
                player_label,
                final_score,
                selected_count,
                &pair_rates,
                input.detail_mode,
                input.detail_min,
                precision,
            );
            if should_highlight(final_score, input.options.min_screen, input.highlight_delta) {
                send(ProgressEvent::HighlightLog(log));
            } else {
                send(ProgressEvent::Log(log));
            }
        }
        Ok(())
    };
    let result = if let Some(observer) = observer {
        let mut pending = std::collections::HashMap::<usize, (usize, Vec<(f64, usize)>)>::new();
        run_pair_matrix_observed(&matrix_input, on_progress, on_player, &mut |player, teammate, rate| {
            let entry = pending.entry(player).or_default();
            entry.0 += 1;
            if let Some(rate) = rate {
                entry.1.push((rate, teammate));
            }
            let mut update = ResultUpdate::new(player, &player_labels[player], ResultKind::Pair, precision);
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
            if entry.0 == teammate_groups.len() {
                let (_, mut rates) = pending.remove(&player).unwrap();
                rates.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                let score = rates.iter().take(head).map(|(rate, _)| rate).sum();
                update.finish = Some(ResultFinish {
                    score: Some(score),
                    visible: input.options.min_screen.is_none_or(|min| score >= min),
                    highlight: should_highlight(score, input.options.min_screen, input.highlight_delta),
                });
            }
            if !update.entries.is_empty() || update.finish.is_some() {
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

fn parse_pair_target_groups(content: &str, factor_enabled: bool) -> Result<(Vec<String>, Vec<f64>), String> {
    if factor_enabled {
        parse_factored_target_groups(content)
    } else {
        let groups = parse_target_groups(content, false);
        let factors = vec![1.0; groups.len()];
        Ok((groups, factors))
    }
}

type ParsedTeammates = (Vec<String>, Vec<String>, Vec<f64>);

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

fn player_to_ol(raw: &str) -> Result<String, String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    cli_api::to_diy(raw, false, false).map_err(|err| format!("转换 player-list 名字为 +ol 失败: {raw}: {err}"))
}

fn player_group_to_ol(group: &str) -> Result<String, String> {
    group
        .lines()
        .map(player_to_ol)
        .collect::<Result<Vec<_>, _>>()
        .map(|players| players.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pair_keeps_each_member_when_converting_a_multi_player_input_group() {
        let group = "+ol:player-a\n+ol:player-b";
        assert_eq!(super::player_group_to_ol(group).unwrap(), group);
    }

    #[test]
    fn pair_parses_factored_teammates_with_labels_and_weights() {
        let raw = "[[targets]]\nfactor = 2\nplayers = [\"mario\", \"luigi\"]";
        let (groups, labels, factors) = parse_pair_teammate_groups(raw, true, true).expect("valid teammates");
        assert_eq!(groups, vec!["mario\nluigi"]);
        assert_eq!(labels, vec!["mario+luigi"]);
        assert_eq!(factors, vec![2.0]);
    }
}

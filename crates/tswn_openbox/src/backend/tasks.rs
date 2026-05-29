//! 后端任务实现。
//!
//! 实现各工具的核心计算逻辑（`run_to_diy`、`run_namer_pf`、`run_batch_rate`、`run_pair`），
//! 通过 `Sender<ProgressEvent>` 向 GUI 线程实时推送进度日志和最终结果。

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use tswn_core::engine::storage::Storage;
use tswn_core::player::{Player, eval_name::WIN_RATE_EVAL_RQ};

use super::format::{format_batch_file_record, format_batch_screen_log, format_pair_file_record, format_pair_screen_log};
use super::parse::{parse_line_list, parse_namer_pf_groups, parse_player_groups_with_labels, parse_plus_separated_groups};
use super::score::{bench_batch_rate_for_group, namer_pf_score};
use super::skill_board::{SkillBoardConfig, evaluate_skill_board};
use super::types::{BatchRateInput, NamerPfInput, NamerPfMetric, PairInput, ProgressEvent};

pub fn run_to_diy(
    raw: &str,
    old: bool,
    minions: bool,
    details: bool,
    output_file: Option<PathBuf>,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<String, String> {
    let names = parse_line_list(raw);
    if names.is_empty() {
        return Err("请输入至少一个名字。".to_string());
    }

    let storage = Storage::new_arc();
    let mut out = String::new();
    for name in &names {
        if cancel.load(Ordering::Relaxed) {
            return Ok("已停止。".to_string());
        }
        let mut player =
            Player::new_from_namerena_raw(name.clone(), storage.clone()).map_err(|err| format!("构建玩家失败: {name}: {err}"))?;
        player.build();
        let export = if old {
            player.to_diy_compact()
        } else if minions {
            player.to_ol_json_with_minions()
        } else {
            player.to_ol_json()
        };
        let _ = writeln!(out, "{export}");

        if details && names.len() == 1 {
            let status = player.get_status();
            let _ = writeln!(out);
            let _ = writeln!(out, "=== 原始信息 ===");
            let _ = writeln!(out, "名字: {}", player.id_name());
            let _ = writeln!(out, "队伍: {}", player.clan_name());
            let _ = writeln!(
                out,
                "八围: atk={} def={} spd={} agi={} mag={} res={} wis={} maxhp={}",
                status.attack,
                status.defense,
                status.speed,
                status.agility,
                status.magic,
                status.resistance,
                status.wisdom,
                status.max_hp,
            );
            let _ = writeln!(out, "name_factor: {:.6}", player.get_name_factor());
        }
    }

    finish_output(output_file.as_deref(), out)
}

pub fn run_namer_pf(input: NamerPfInput, send: impl Fn(ProgressEvent)) {
    let groups = parse_namer_pf_groups(&input.raw);
    if groups.is_empty() {
        send(ProgressEvent::Done(Err("namer-pf: 输入为空或无有效玩家。".to_string())));
        return;
    }
    if input.metrics.iter().all(|metric| !metric.screen && metric.output_file.is_none())
        && !input.skill_board.screen
        && input.skill_board.output_file.is_none()
    {
        send(ProgressEvent::Done(Err(
            "namer-pf: 请至少选择一个屏幕输出或输出文件。".to_string()
        )));
        return;
    }

    let mut outputs = Vec::with_capacity(input.metrics.len());
    for metric in &input.metrics {
        let output = match metric.output_file.as_deref() {
            Some(path) => match create_output_file(path) {
                Ok(file) => Some(file),
                Err(err) => {
                    send(ProgressEvent::Done(Err(err)));
                    return;
                }
            },
            None => None,
        };
        outputs.push(output);
    }
    let mut skill_board_output = match input.skill_board.output_file.as_deref() {
        Some(path) => match create_output_file(path) {
            Ok(file) => Some(file),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        },
        None => None,
    };
    let skill_board_config = if input.skill_board.screen || skill_board_output.is_some() {
        match SkillBoardConfig::load_default() {
            Ok(config) => Some(config),
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        }
    } else {
        None
    };

    let n = input.count.max(1);
    let eval_rq = eval_rq(input.keep_rq);
    let total = groups.len();
    let needs_skill_board_scores = skill_board_config.is_some();
    let needs_sum = metric_enabled(&input.metrics, NamerPfMetric::Sum);
    let needs_all_scores = needs_skill_board_scores || needs_sum;
    let needs_pp = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Pp);
    let needs_pd = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Pd);
    let needs_qp = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Qp);
    let needs_qd = needs_all_scores || metric_enabled(&input.metrics, NamerPfMetric::Qd);
    for (index, group) in groups.iter().enumerate() {
        if input.cancel.load(Ordering::Relaxed) {
            send(ProgressEvent::Done(Ok("已停止。".to_string())));
            return;
        }
        let pp = if needs_pp {
            namer_pf_score(group, "\u{0002}", false, n, input.threads, eval_rq)
        } else {
            0
        };
        let pd = if needs_pd {
            namer_pf_score(group, "\u{0002}", true, n, input.threads, eval_rq)
        } else {
            0
        };
        let qp = if needs_qp {
            namer_pf_score(group, "!", false, n, input.threads, eval_rq)
        } else {
            0
        };
        let qd = if needs_qd {
            namer_pf_score(group, "!", true, n, input.threads, eval_rq)
        } else {
            0
        };
        let scores = NamerPfScores {
            pp,
            pd,
            qp,
            qd,
            sum: pp + pd + qp + qd,
        };
        let label = group.join("+");

        for (metric, output) in input.metrics.iter().zip(outputs.iter_mut()) {
            let score = scores.get(metric.metric);
            if metric.min_screen.is_none_or(|limit| score as f64 >= limit) && metric.screen {
                let line = format!("{} {}:{}", label, metric.metric.label(), score);
                if should_highlight(score as f64, metric.min_screen, metric.highlight_delta) {
                    send(ProgressEvent::HighlightLog(line));
                } else {
                    send(ProgressEvent::Log(line));
                }
            }
            if metric.min_file.is_none_or(|limit| score as f64 >= limit)
                && let Some(output) = output.as_mut()
                && let Err(err) = writeln!(output, "{} {}", score, label)
            {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }
        if let Some(config) = &skill_board_config {
            for line in evaluate_skill_board(group, &scores, config) {
                if input.skill_board.screen {
                    send(ProgressEvent::SkillBoardLog(format!("{} {} {}", line.title, line.score, label)));
                }
                if let Some(output) = skill_board_output.as_mut()
                    && let Err(err) = writeln!(output, "{} {} {}", line.title, line.score, label)
                {
                    send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                    return;
                }
            }
        }
        send(ProgressEvent::Progress { done: index + 1, total });
    }

    let mut written = input
        .metrics
        .iter()
        .filter_map(|metric| {
            metric
                .output_file
                .as_ref()
                .map(|path| format!("{} -> {}", metric.metric.label(), path.display()))
        })
        .collect::<Vec<_>>();
    if let Some(path) = input.skill_board.output_file.as_ref() {
        written.push(format!("技能榜 -> {}", path.display()));
    }
    let message = if written.is_empty() {
        "完成。".to_string()
    } else {
        format!("完成，结果已写入: {}", written.join("; "))
    };
    send(ProgressEvent::Done(Ok(message)));
}

fn metric_enabled(metrics: &[super::types::NamerPfMetricOptions], metric: NamerPfMetric) -> bool {
    metrics
        .iter()
        .any(|options| options.metric == metric && (options.screen || options.output_file.is_some()))
}

pub struct NamerPfScores {
    pub pp: u64,
    pub pd: u64,
    pub qp: u64,
    pub qd: u64,
    pub sum: u64,
}

impl NamerPfScores {
    fn get(&self, metric: NamerPfMetric) -> u64 {
        match metric {
            NamerPfMetric::Pp => self.pp,
            NamerPfMetric::Pd => self.pd,
            NamerPfMetric::Qp => self.qp,
            NamerPfMetric::Qd => self.qd,
            NamerPfMetric::Sum => self.sum,
        }
    }
}

pub fn run_batch_rate(input: BatchRateInput, send: impl Fn(ProgressEvent)) {
    let target_groups = parse_plus_separated_groups(&input.target_text);
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

    let n = input.options.count.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let total = player_groups.len() * target_groups.len() * if input.show_matchups { 2 } else { 1 };
    let mut done = 0usize;

    for (player, label) in player_groups.iter().zip(player_labels.iter()) {
        let mut verbose = String::new();
        let summary = bench_batch_rate_for_group(
            player,
            &target_groups,
            n,
            input.options.threads,
            eval_rq,
            input.options.verbose,
            &mut verbose,
            &input.cancel,
            |_, _| {
                done += 1;
                send(ProgressEvent::Progress { done, total });
            },
        );

        if input.options.min_file.is_none_or(|limit| summary.avg >= limit)
            && let Some(output) = output.as_mut()
        {
            let line = format_batch_file_record(input.output_mode, label, summary.avg, precision);
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| summary.avg >= limit) {
            let matchup_rates = if input.show_matchups {
                batch_matchup_rates(
                    player,
                    &target_groups,
                    n,
                    input.options.threads,
                    eval_rq,
                    &input.cancel,
                    &mut done,
                    total,
                    &send,
                )
            } else {
                Vec::new()
            };
            let log = format_batch_screen_log(label, summary.avg, &matchup_rates, input.show_matchups, precision);
            if should_highlight(summary.avg, input.options.min_screen, input.highlight_delta) {
                send(ProgressEvent::HighlightLog(log));
            } else {
                send(ProgressEvent::Log(log));
            }
        }
        if input.cancel.load(Ordering::Relaxed) {
            send(ProgressEvent::Done(Ok("已停止。".to_string())));
            return;
        }
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
}

pub fn run_pair(input: PairInput, send: impl Fn(ProgressEvent)) {
    let target_groups = parse_plus_separated_groups(&input.target_text);
    let players = parse_line_list(&input.player_text);
    let teammates = parse_line_list(&input.teammate_text);
    if target_groups.is_empty() {
        send(ProgressEvent::Done(Err("pair: 靶子列表为空。".to_string())));
        return;
    }
    if players.is_empty() {
        send(ProgressEvent::Done(Err("pair: 选手列表为空。".to_string())));
        return;
    }
    if teammates.is_empty() {
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

    let n = input.options.count.max(1);
    let head = input.head.max(1);
    let eval_rq = eval_rq(input.options.keep_rq);
    let precision = input.options.wr_precision.min(9);
    let total = players.len() * teammates.len() * target_groups.len();
    let mut done = 0usize;

    for player in &players {
        let started = Instant::now();
        let converted_player = match player_to_ol(player) {
            Ok(value) => value,
            Err(err) => {
                send(ProgressEvent::Done(Err(err)));
                return;
            }
        };
        let mut pair_rates = Vec::with_capacity(teammates.len());
        let mut total_wins = 0usize;
        let mut total_battles = 0usize;
        let mut _total_valid_matchups = 0usize;
        let mut _total_skipped_matchups = 0usize;
        let mut verbose = String::new();

        for teammate in &teammates {
            let pair_group = format!("{converted_player}\n{teammate}");
            if input.options.verbose {
                let _ = writeln!(verbose, "teammate: {teammate}");
            }
            let summary = bench_batch_rate_for_group(
                &pair_group,
                &target_groups,
                n,
                input.options.threads,
                eval_rq,
                input.options.verbose,
                &mut verbose,
                &input.cancel,
                |_, _| {
                    done += 1;
                    send(ProgressEvent::Progress { done, total });
                },
            );
            if summary.valid_matchups > 0 {
                pair_rates.push((summary.avg, teammate.clone()));
            }
            total_wins += summary.wins;
            total_battles += summary.total;
            _total_valid_matchups += summary.valid_matchups;
            _total_skipped_matchups += summary.skipped_matchups;
            if input.cancel.load(Ordering::Relaxed) {
                break;
            }
        }

        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let _elapsed = started.elapsed();
        let _aggregate_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;

        if input.options.min_file.is_none_or(|limit| final_score >= limit)
            && let Some(output) = output.as_mut()
        {
            let line = format_pair_file_record(
                input.output_mode,
                player,
                final_score,
                selected_count,
                head,
                &pair_rates,
                precision,
            );
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| final_score >= limit) {
            let log = format_pair_screen_log(
                player,
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
        if input.cancel.load(Ordering::Relaxed) {
            send(ProgressEvent::Done(Ok("已停止。".to_string())));
            return;
        }
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
}

fn should_highlight(score: f64, min_screen: Option<f64>, highlight_delta: Option<f64>) -> bool {
    highlight_delta.is_some_and(|delta| score >= min_screen.unwrap_or(0.0) + delta)
}

fn batch_matchup_rates(
    player: &str,
    target_groups: &[String],
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    cancel: &std::sync::atomic::AtomicBool,
    done: &mut usize,
    total: usize,
    send: &impl Fn(ProgressEvent),
) -> Vec<(f64, String)> {
    let mut rates = Vec::new();
    for target in target_groups {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut verbose = String::new();
        let summary = bench_batch_rate_for_group(
            player,
            std::slice::from_ref(target),
            n,
            threads,
            eval_rq,
            false,
            &mut verbose,
            cancel,
            |_, _| {
                *done += 1;
                send(ProgressEvent::Progress { done: *done, total });
            },
        );
        if summary.valid_matchups > 0 {
            rates.push((summary.avg, target.clone()));
        }
    }
    rates
}

fn finish_output(output_file: Option<&Path>, out: String) -> Result<String, String> {
    match output_file {
        Some(path) => {
            let mut file = create_output_file(path)?;
            file.write_all(out.as_bytes())
                .and_then(|_| file.flush())
                .map_err(|err| format!("写入输出文件失败: {}: {err}", path.display()))?;
            Ok(format!("完成，结果已写入: {}", path.display()))
        }
        None => Ok(out),
    }
}

fn create_output_file(path: &Path) -> Result<File, String> {
    if path.file_name().is_none() {
        return Err(format!("输出路径必须包含文件名: {}", path.display()));
    }
    if path.exists() && path.is_dir() {
        return Err(format!("输出路径不能是目录: {}", path.display()));
    }
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty())
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("创建输出目录失败: {}: {err}", parent.display()))?;
    }
    File::create(path).map_err(|err| format!("打开输出文件失败: {}: {err}", path.display()))
}

fn eval_rq(keep_rq: bool) -> f64 {
    if keep_rq {
        tswn_core::player::eval_name::DEFAULT_EVAL_RQ
    } else {
        WIN_RATE_EVAL_RQ
    }
}

fn player_to_ol(raw: &str) -> Result<String, String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw(raw.to_string(), storage)
        .map_err(|err| format!("转换 player-list 名字为 +ol 失败: {raw}: {err}"))?;
    player.build();
    Ok(player.to_ol_json())
}

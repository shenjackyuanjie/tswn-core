use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use tswn_core::engine::storage::Storage;
use tswn_core::player::{Player, eval_name::WIN_RATE_EVAL_RQ};
use tswn_core::win_rate::WinRateTiming;

use super::format::{format_batch_file_record, format_batch_screen_log, format_pair_file_record, format_pair_screen_log};
use super::parse::{parse_line_list, parse_namer_pf_groups, parse_player_groups_with_labels, parse_plus_separated_groups};
use super::score::{bench_batch_rate_for_group, namer_pf_score};
use super::types::{BatchRateInput, PairInput, ProgressEvent};

pub fn run_to_diy(raw: &str, old: bool, details: bool, output_file: Option<PathBuf>) -> Result<String, String> {
    let names = parse_line_list(raw);
    if names.is_empty() {
        return Err("请输入至少一个名字。".to_string());
    }

    let storage = Storage::new_arc();
    let mut out = String::new();
    for name in &names {
        let mut player =
            Player::new_from_namerena_raw(name.clone(), storage.clone()).map_err(|err| format!("构建玩家失败: {name}: {err}"))?;
        player.build();
        let export = if old { player.to_diy_compact() } else { player.to_ol_json() };
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

pub fn run_namer_pf(raw: &str, n: usize, threads: Option<usize>, output_file: Option<PathBuf>, send: impl Fn(ProgressEvent)) {
    let groups = parse_namer_pf_groups(raw);
    if groups.is_empty() {
        send(ProgressEvent::Done(Err("namer-pf: 输入为空或无有效玩家。".to_string())));
        return;
    }

    let mut out = String::from("pp|pd|qp|qd|sum\n");
    let total = groups.len();
    for (index, group) in groups.iter().enumerate() {
        let pp = namer_pf_score(group, "\u{0002}", false, n, threads);
        let pd = namer_pf_score(group, "\u{0002}", true, n, threads);
        let qp = namer_pf_score(group, "!", false, n, threads);
        let qd = namer_pf_score(group, "!", true, n, threads);
        let sum = pp + pd + qp + qd;
        let _ = writeln!(out, "{pp}|{pd}|{qp}|{qd}|{sum}");
        send(ProgressEvent::Progress { done: index + 1, total });
    }

    send(ProgressEvent::Done(finish_output(output_file.as_deref(), out)));
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
    let total = player_groups.len() * target_groups.len();
    let mut done = 0usize;

    for (index, (player, label)) in player_groups.iter().zip(player_labels.iter()).enumerate() {
        let mut verbose = String::new();
        let summary = bench_batch_rate_for_group(
            player,
            &target_groups,
            n,
            input.options.threads,
            eval_rq,
            input.options.verbose,
            &mut verbose,
            |_, _| {
                done += 1;
                send(ProgressEvent::Progress { done, total });
            },
        );

        if input.options.min_file.is_none_or(|limit| summary.avg >= limit)
            && let Some(output) = output.as_mut()
        {
            let line = format_batch_file_record(
                input.output_mode,
                label,
                summary.avg,
                summary.aggregate_rate,
                summary.wins,
                summary.total,
                summary.valid_matchups,
                summary.skipped_matchups,
                summary.elapsed,
                precision,
            );
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| summary.avg >= limit) {
            let log = format_batch_screen_log(
                index + 1,
                player_groups.len(),
                label,
                summary.avg,
                summary.aggregate_rate,
                summary.wins,
                summary.total,
                summary.valid_matchups,
                summary.skipped_matchups,
                summary.elapsed,
                summary.timing,
                precision,
                input.options.verbose,
                &verbose,
                input.options.perf,
            );
            send(ProgressEvent::Log(log));
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

    for (index, player) in players.iter().enumerate() {
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
        let mut total_valid_matchups = 0usize;
        let mut total_skipped_matchups = 0usize;
        let mut total_timing = WinRateTiming::default();
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
            total_valid_matchups += summary.valid_matchups;
            total_skipped_matchups += summary.skipped_matchups;
            total_timing.merge(summary.timing);
        }

        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let elapsed = started.elapsed();
        let aggregate_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;

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
                aggregate_rate,
                total_wins,
                total_battles,
                total_valid_matchups,
                total_skipped_matchups,
                elapsed,
                precision,
            );
            if let Err(err) = writeln!(output, "{line}") {
                send(ProgressEvent::Done(Err(format!("写入输出文件失败: {err}"))));
                return;
            }
        }

        if input.options.min_screen.is_none_or(|limit| final_score >= limit) {
            let log = format_pair_screen_log(
                index + 1,
                players.len(),
                player,
                final_score,
                selected_count,
                head,
                &pair_rates,
                aggregate_rate,
                total_wins,
                total_battles,
                total_valid_matchups,
                total_skipped_matchups,
                elapsed,
                total_timing,
                precision,
                input.options.verbose,
                &verbose,
                input.options.perf,
            );
            send(ProgressEvent::Log(log));
        }
    }

    let final_message = if let Some(path) = input.output_file.as_deref() {
        format!("完成，结果已写入: {}", path.display())
    } else {
        "完成。".to_string()
    };
    send(ProgressEvent::Done(Ok(final_message)));
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

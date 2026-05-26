//! 批量 benchmark 与 pair 评估。
//!
//! 这部分的特点不是“单次对局逻辑复杂”，而是外层循环很多：
//! - 一个 player group 需要对多个 target group 重复跑；
//! - pair 模式还要在 player × teammate 上再套一层组合循环；
//! - 同时还要维护进度条、阈值过滤和结果落盘。
//!
//! 因此这里把批量控制流、进度条和文件输出调度集中在一起，避免和底层单场 benchmark 细节混杂。

use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write as _};
use std::path::Path;
use std::time::{Duration, Instant};

use tswn_core::win_rate::WinRateTiming;

use crate::args::BenchThreadMode;

use super::common::format_duration;
use super::output::{
    display_group, first_duplicate_name_in_matchup, format_batch_rate_log_record, format_batch_rate_pure_record,
    format_batch_rate_record, format_pair_rate_record, format_rate, open_batch_rate_output, player_to_ol_or_exit,
    print_perf_lines, write_batch_rate_record,
};
use super::winrate::bench_winrate_summary;

const PROGRESS_BAR_WIDTH: usize = 30;
const SLIDING_WINDOW: usize = 5;

/// 文件输出模式。
#[derive(Debug, Clone, Copy)]
enum BatchFileOutputMode {
    Log,
    Json,
    Pure,
}

/// 批量胜率测试的终端进度条。
///
/// 以“对局”（player × target）为最小进度单位，但 ETA 以“已完成 player 的平均耗时”估算，
/// 这样既能在细粒度上持续动起来，又不会被单个 matchup 的抖动放大。
struct BatchProgress {
    enabled: bool,
    total_players: usize,
    completed_players: usize,
    targets_per_player: usize,
    completed_targets_in_current: usize,
    player_durations: Vec<Duration>,
    started_at: Instant,
}

impl BatchProgress {
    fn new(total_players: usize, targets_per_player: usize) -> Self {
        Self {
            enabled: io::stderr().is_terminal(),
            total_players,
            completed_players: 0,
            targets_per_player,
            completed_targets_in_current: 0,
            player_durations: Vec::with_capacity(total_players),
            started_at: Instant::now(),
        }
    }

    /// 完成当前选手的一个 target 对局后刷新进度条。
    fn tick_target(&mut self) {
        self.completed_targets_in_current += 1;
        self.draw();
    }

    /// 完成一个 player 的全部 matchups，并记录耗时用于 ETA。
    fn complete_player(&mut self, duration: Duration) {
        self.completed_players += 1;
        self.completed_targets_in_current = 0;
        self.player_durations.push(duration);
    }

    /// 清除当前进度行，给详细输出腾地方。
    fn clear(&self) {
        if !self.enabled {
            return;
        }
        eprint!("\r\x1b[K");
        let _ = io::stderr().flush();
    }

    /// 绘制 / 刷新进度条。
    fn draw(&self) {
        if !self.enabled {
            return;
        }
        let total_matchups = self.total_players * self.targets_per_player;
        if total_matchups == 0 {
            return;
        }

        let done_matchups = self.completed_players * self.targets_per_player + self.completed_targets_in_current;
        let frac = done_matchups as f64 / total_matchups as f64;
        let filled = (frac * PROGRESS_BAR_WIDTH as f64) as usize;
        let empty = PROGRESS_BAR_WIDTH.saturating_sub(filled);
        let remaining_players = self.total_players - self.completed_players;

        let total_eta = if self.completed_players > 0 {
            let elapsed = self.started_at.elapsed().as_secs_f64();
            let avg_per_player = elapsed / self.completed_players as f64;
            format_duration(avg_per_player * remaining_players as f64)
        } else {
            "--".to_string()
        };

        let sliding_eta = if self.completed_players > 0 {
            let window_start = self.player_durations.len().saturating_sub(SLIDING_WINDOW);
            let window = &self.player_durations[window_start..];
            let window_sum: f64 = window.iter().map(|d| d.as_secs_f64()).sum();
            let avg_per_player = window_sum / window.len() as f64;
            format_duration(avg_per_player * remaining_players as f64)
        } else {
            "--".to_string()
        };

        let bar_filled: String = "█".repeat(filled);
        let bar_empty: String = "░".repeat(empty);
        eprint!(
            "\r进度 [{bar_filled}{bar_empty}] {done}/{total} ({pct:.1}%) | 预计: {total_eta} | 滑动: {sliding_eta}\x1b[K",
            done = done_matchups,
            total = total_matchups,
            pct = frac * 100.0,
        );
        let _ = io::stderr().flush();
    }

    /// 全部完成时打印汇总信息。
    fn finish(&self) {
        if !self.enabled {
            return;
        }
        self.clear();
        let elapsed = self.started_at.elapsed();
        eprintln!(
            "完成: {}/{} 组选手, 总用时: {}",
            self.completed_players,
            self.total_players,
            format_duration(elapsed.as_secs_f64()),
        );
    }
}

/// 单个 player group 批量测试后的汇总。
#[derive(Debug)]
struct BatchRateSummary {
    avg: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    timing: WinRateTiming,
    elapsed: Duration,
    valid_matchups: usize,
    skipped_matchups: usize,
}

impl BatchRateSummary {
    /// 每秒完成多少场 battle。
    fn throughput(&self) -> f64 {
        let elapsed_secs = self.elapsed.as_secs_f64();
        if elapsed_secs > 0.0 {
            self.total as f64 / elapsed_secs
        } else {
            0.0
        }
    }
}

/// `bench batch-rate` / `bench cqp` 入口。
#[allow(clippy::too_many_arguments)]
pub fn run_bench_batch_rate(
    target_groups: &[String],
    player_groups: &[String],
    player_labels: &[String],
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    perf: bool,
    out_file: Option<&Path>,
    force: bool,
    log: bool,
    pure: bool,
    min_screen: Option<f64>,
    min_file: Option<f64>,
    wr_precision: usize,
) {
    let file_mode = if pure {
        BatchFileOutputMode::Pure
    } else if log {
        BatchFileOutputMode::Json
    } else {
        BatchFileOutputMode::Log
    };

    let mut out_file = match out_file {
        Some(path) => match open_batch_rate_output(path, force) {
            Ok(file) => Some(file),
            Err(err) => {
                eprintln!("打开批量结果输出文件失败: {err}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    println!(
        "=== 批量组胜率测试 ({n} 场/对局, {} 选手组, {} 靶子组) ===",
        player_groups.len(),
        target_groups.len()
    );
    if let Some(threshold) = min_screen {
        println!("终端最低胜率阈值: {:.2}%", threshold);
    }
    if out_file.is_some()
        && let Some(threshold) = min_file
    {
        println!("文件最低胜率阈值: {:.2}%", threshold);
    }

    let mut progress = BatchProgress::new(player_groups.len(), target_groups.len());
    progress.draw();

    for (pi, (player, label)) in player_groups.iter().zip(player_labels.iter()).enumerate() {
        // verbose 模式先把逐靶输出缓冲起来，等阈值判断通过后再整体打印，避免刷屏。
        let mut verbose_buf = String::new();
        if verbose {
            let _ = writeln!(&mut verbose_buf);
            let _ = writeln!(&mut verbose_buf, "━━━ [{}/{}] {} ━━━", pi + 1, player_groups.len(), label);
        }

        let summary = bench_batch_rate_for_group(
            player,
            target_groups,
            n,
            mode,
            threads,
            eval_rq,
            verbose,
            &mut verbose_buf,
            |_, _| progress.tick_target(),
        );
        let avg = summary.avg;
        let aggregate_rate = summary.aggregate_rate;
        let elapsed = summary.elapsed;
        let elapsed_secs = elapsed.as_secs_f64();
        let throughput = summary.throughput();
        let summary_json = format_batch_rate_record(
            label,
            avg,
            aggregate_rate,
            summary.wins,
            summary.total,
            elapsed,
            throughput,
            summary.valid_matchups,
            summary.skipped_matchups,
            wr_precision,
        );
        let summary_log = format_batch_rate_log_record(label, avg, wr_precision);
        let summary_pure = format_batch_rate_pure_record(label);

        progress.complete_player(elapsed);

        let passes_screen = min_screen.is_none_or(|t| avg >= t);
        let passes_file = min_file.is_none_or(|t| avg >= t);

        if passes_screen {
            if verbose {
                progress.clear();
                print!("{verbose_buf}");
                println!(
                    "平均胜率: {}%  (有效 {} 组靶子，跳过 {} 场重复号)",
                    format_rate(avg, wr_precision),
                    summary.valid_matchups,
                    summary.skipped_matchups
                );
                println!(
                    "汇总胜率: {}%  ({}/{})",
                    format_rate(aggregate_rate, wr_precision),
                    summary.wins,
                    summary.total
                );
                println!(
                    "用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                    elapsed_secs,
                    elapsed.as_micros() as f64 / summary.total.max(1) as f64,
                    throughput
                );
            } else {
                progress.clear();
                println!(
                    "{}\t平均胜率: {}%\t有效: {}\t跳过重复: {}\t用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                    label,
                    format_rate(avg, wr_precision),
                    summary.valid_matchups,
                    summary.skipped_matchups,
                    elapsed_secs,
                    elapsed.as_micros() as f64 / summary.total.max(1) as f64,
                    throughput
                );
            }
        }

        if passes_file && let Some(file) = out_file.as_mut() {
            let line = match file_mode {
                BatchFileOutputMode::Log => &summary_log,
                BatchFileOutputMode::Json => &summary_json,
                BatchFileOutputMode::Pure => &summary_pure,
            };
            if let Err(err) = write_batch_rate_record(file, line) {
                eprintln!("写入批量结果输出文件失败: {err}");
                std::process::exit(1);
            }
        }

        if perf && passes_screen {
            progress.clear();
            print_perf_lines(elapsed, summary.timing, summary.total);
        }

        progress.draw();
    }

    progress.finish();
}

/// 计算单个 player group 对整个 target 列表的平均胜率。
#[allow(clippy::too_many_arguments)]
fn bench_batch_rate_for_group(
    player: &str,
    target_groups: &[String],
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    verbose_buf: &mut String,
    mut tick_target: impl FnMut(usize, &str),
) -> BatchRateSummary {
    let overall_started = Instant::now();
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut accumulated_timing = WinRateTiming::default();
    let mut valid_matchups = 0usize;
    let mut skipped_matchups = 0usize;

    for (ti, target) in target_groups.iter().enumerate() {
        if let Some(duplicate) = first_duplicate_name_in_matchup(&[player, target.as_str()]) {
            skipped_matchups += 1;
            if verbose {
                let _ = writeln!(
                    verbose_buf,
                    "  [{}/{}] vs {}  =>  SKIP duplicate name: {}",
                    ti + 1,
                    target_groups.len(),
                    display_group(target),
                    duplicate
                );
            }
            tick_target(ti, target);
            continue;
        }

        let raw = format!("{player}\n\n{target}");
        let summary = bench_winrate_summary(&raw, n, mode, threads, eval_rq);
        if verbose {
            let _ = writeln!(
                verbose_buf,
                "  [{}/{}] vs {}  =>  {:.2}%  ({}/{})",
                ti + 1,
                target_groups.len(),
                display_group(target),
                summary.win_rate_percent(),
                summary.wins,
                summary.total
            );
        }
        accumulated_rate += summary.win_rate_percent();
        accumulated_wins += summary.wins;
        accumulated_total += summary.total;
        accumulated_timing.merge(summary.timing);
        valid_matchups += 1;
        tick_target(ti, target);
    }

    let avg = if valid_matchups > 0 {
        accumulated_rate / valid_matchups as f64
    } else {
        0.0
    };
    let aggregate_rate = accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64;
    BatchRateSummary {
        avg,
        aggregate_rate,
        wins: accumulated_wins,
        total: accumulated_total,
        timing: accumulated_timing,
        elapsed: overall_started.elapsed(),
        valid_matchups,
        skipped_matchups,
    }
}

/// `bench pair` 入口。
#[allow(clippy::too_many_arguments)]
pub fn run_bench_pair(
    target_groups: &[String],
    players: &[String],
    teammates: &[String],
    head: usize,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    perf: bool,
    out_file: Option<&Path>,
    force: bool,
    log: bool,
    pure: bool,
    min_screen: Option<f64>,
    min_file: Option<f64>,
    wr_precision: usize,
) {
    let file_mode = if pure {
        BatchFileOutputMode::Pure
    } else if log {
        BatchFileOutputMode::Json
    } else {
        BatchFileOutputMode::Log
    };

    let mut out_file = match out_file {
        Some(path) => match open_batch_rate_output(path, force) {
            Ok(file) => Some(file),
            Err(err) => {
                eprintln!("打开 pair 结果输出文件失败: {err}");
                std::process::exit(1);
            }
        },
        None => None,
    };

    println!(
        "=== 二人组 batch rate ({n} 场/对局, {} 选手, {} 队友, {} 靶子组, head={head}) ===",
        players.len(),
        teammates.len(),
        target_groups.len()
    );
    if let Some(threshold) = min_screen {
        println!("终端最低最终分数阈值: {}", format_rate(threshold, wr_precision));
    }
    if out_file.is_some()
        && let Some(threshold) = min_file
    {
        println!("文件最低最终分数阈值: {}", format_rate(threshold, wr_precision));
    }

    let total_matchups_per_player = teammates.len().saturating_mul(target_groups.len());
    let mut progress = BatchProgress::new(players.len(), total_matchups_per_player);
    progress.draw();

    for (pi, player) in players.iter().enumerate() {
        let overall_started = Instant::now();
        let converted_player = player_to_ol_or_exit(player);
        let mut pair_rates = Vec::with_capacity(teammates.len());
        let mut total_wins = 0usize;
        let mut total_battles = 0usize;
        let mut total_valid_matchups = 0usize;
        let mut total_skipped_matchups = 0usize;
        let mut total_timing = WinRateTiming::default();
        let mut verbose_buf = String::new();

        if verbose {
            let _ = writeln!(&mut verbose_buf);
            let _ = writeln!(&mut verbose_buf, "━━━━━━━━ [{}/{}] {} ━━━━━━━━", pi + 1, players.len(), player);
        }

        for teammate in teammates {
            let pair_group = format!("{converted_player}\n{teammate}");
            if verbose {
                let _ = writeln!(&mut verbose_buf, "  teammate: {teammate}");
            }
            let summary = bench_batch_rate_for_group(
                &pair_group,
                target_groups,
                n,
                mode,
                threads,
                eval_rq,
                verbose,
                &mut verbose_buf,
                |_, _| progress.tick_target(),
            );
            if summary.valid_matchups > 0 {
                pair_rates.push((summary.avg, teammate.clone()));
            }
            total_wins += summary.wins;
            total_battles += summary.total;
            total_valid_matchups += summary.valid_matchups;
            total_skipped_matchups += summary.skipped_matchups;
            total_timing.merge(summary.timing);
            if verbose {
                let _ = writeln!(
                    &mut verbose_buf,
                    "  teammate avg: {}%  (有效 {}, 跳过 {})",
                    format_rate(summary.avg, wr_precision),
                    summary.valid_matchups,
                    summary.skipped_matchups
                );
            }
        }

        pair_rates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let selected_count = head.min(pair_rates.len());
        let final_score = pair_rates.iter().take(selected_count).map(|(rate, _)| *rate).sum::<f64>();
        let elapsed = overall_started.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let throughput = if elapsed_secs > 0.0 {
            total_battles as f64 / elapsed_secs
        } else {
            0.0
        };
        let aggregate_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;
        let summary_json = format_pair_rate_record(
            player,
            final_score,
            selected_count,
            head,
            &pair_rates,
            aggregate_rate,
            total_wins,
            total_battles,
            elapsed,
            throughput,
            total_valid_matchups,
            total_skipped_matchups,
            wr_precision,
        );
        let summary_log = format_batch_rate_log_record(player, final_score, wr_precision);
        let summary_pure = format_batch_rate_pure_record(player);

        progress.complete_player(elapsed);

        let passes_screen = min_screen.is_none_or(|t| final_score >= t);
        let passes_file = min_file.is_none_or(|t| final_score >= t);

        if passes_screen {
            progress.clear();
            if verbose {
                print!("{verbose_buf}");
                println!("top {}:", selected_count);
                for (index, (rate, teammate)) in pair_rates.iter().take(selected_count).enumerate() {
                    println!("  #{} {}% {}", index + 1, format_rate(*rate, wr_precision), teammate);
                }
                println!(
                    "最终分数: {}  (head={}, 有效组合 {}, 有效靶子 {}, 跳过 {} 场重复号)",
                    format_rate(final_score, wr_precision),
                    head,
                    pair_rates.len(),
                    total_valid_matchups,
                    total_skipped_matchups
                );
                println!(
                    "汇总胜率: {}%  ({}/{})",
                    format_rate(aggregate_rate, wr_precision),
                    total_wins,
                    total_battles
                );
                println!(
                    "用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                    elapsed_secs,
                    elapsed.as_micros() as f64 / total_battles.max(1) as f64,
                    throughput
                );
            } else {
                println!(
                    "{}\t最终分数: {}\ttop: {}/{}\t有效靶子: {}\t跳过重复: {}\t用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                    player,
                    format_rate(final_score, wr_precision),
                    selected_count,
                    head,
                    total_valid_matchups,
                    total_skipped_matchups,
                    elapsed_secs,
                    elapsed.as_micros() as f64 / total_battles.max(1) as f64,
                    throughput
                );
            }
        }

        if passes_file && let Some(file) = out_file.as_mut() {
            let line = match file_mode {
                BatchFileOutputMode::Log => &summary_log,
                BatchFileOutputMode::Json => &summary_json,
                BatchFileOutputMode::Pure => &summary_pure,
            };
            if let Err(err) = write_batch_rate_record(file, line) {
                eprintln!("写入 pair 结果输出文件失败: {err}");
                std::process::exit(1);
            }
        }

        if perf && passes_screen {
            progress.clear();
            print_perf_lines(elapsed, total_timing, total_battles);
        }

        progress.draw();
    }

    progress.finish();
}

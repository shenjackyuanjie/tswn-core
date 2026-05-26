use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, IsTerminal, Write as _};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tswn_core::{
    Runner,
    engine::storage::Storage,
    player::{Player, eval_name::WIN_RATE_EVAL_RQ, overlay::PlayerOverlay},
    win_rate::{
        WinRateTiming, prepared_win_rate, resolve_win_rate_workers, run_prepared_win_rate_range, use_js_profile_seed_schedule,
    },
};

use crate::{BENCH_PARALLEL_THRESHOLD, args::BenchThreadMode};

const PROGRESS_BAR_WIDTH: usize = 30;
const SLIDING_WINDOW: usize = 5;

#[derive(Debug, Clone, Copy)]
pub enum BatchFileOutputMode {
    Log,
    Json,
    Pure,
}

/// 批量胜率测试的终端进度条。
///
/// 以「对局」(matchup = 选手×靶子) 粒度推进进度条动画，
/// 以「选手」粒度计算总体预计剩余时间和滑动窗口预计时间。
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

    /// 完成当前选手的一个靶子对局，刷新进度条。
    fn tick_target(&mut self) {
        self.completed_targets_in_current += 1;
        self.draw();
    }

    /// 完成一个选手组的全部对局，记录耗时用于 ETA 计算。
    fn complete_player(&mut self, duration: Duration) {
        self.completed_players += 1;
        self.completed_targets_in_current = 0;
        self.player_durations.push(duration);
    }

    /// 清除当前进度行（用于在进度条上方插入结果输出）。
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

        // 总体预计：已完成选手的平均耗时 × 剩余选手数
        let total_eta = if self.completed_players > 0 {
            let elapsed = self.started_at.elapsed().as_secs_f64();
            let avg_per_player = elapsed / self.completed_players as f64;
            format_duration(avg_per_player * remaining_players as f64)
        } else {
            "--".to_string()
        };

        // 滑动预计：最近 N 个选手的平均耗时 × 剩余选手数
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

    /// 全部完成时清除进度条并输出汇总行。
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

/// 将秒数格式化成人类可读的时间字符串。
fn format_duration(secs: f64) -> String {
    if secs < 0.0 || secs.is_nan() || secs.is_infinite() {
        return "--".to_string();
    }
    let s = secs.round() as u64;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m{}s", s / 60, s % 60)
    } else {
        format!("{}h{}m{}s", s / 3600, (s % 3600) / 60, s % 60)
    }
}

#[derive(Debug, Clone, Copy)]
struct BenchSummary {
    wins: usize,
    total: usize,
    timing: WinRateTiming,
    elapsed: Duration,
}

impl BenchSummary {
    fn win_rate_percent(self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }
}

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
    fn throughput(&self) -> f64 {
        let elapsed_secs = self.elapsed.as_secs_f64();
        if elapsed_secs > 0.0 {
            self.total as f64 / elapsed_secs
        } else {
            0.0
        }
    }
}

#[derive(Debug)]
struct BenchmarkInput {
    groups: Vec<Vec<String>>,
    score_modifier: Option<&'static str>,
}

fn parse_benchmark_input(raw: &str) -> BenchmarkInput {
    let (mut groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let mut score_modifier = None;

    if groups.first().and_then(|group| group.first()).is_some_and(|name| name == "!test!") {
        let marker_group = groups.remove(0);
        score_modifier = Some(if marker_group.get(1).is_some_and(|name| name == "!") {
            "!"
        } else {
            "\u{0002}"
        });
    }

    BenchmarkInput { groups, score_modifier }
}

fn groups_to_raw(groups: &[Vec<String>]) -> String {
    groups
        .iter()
        .filter(|group| !group.is_empty())
        .map(|group| group.join("\n"))
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn run_benchmark(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let raw = raw.trim();
    let BenchmarkInput { groups, score_modifier } = parse_benchmark_input(raw);
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("benchmark: 输入为空或无有效玩家"),
        1 => {
            if let Some(modifier) = score_modifier {
                run_bench_score_with_modifier(&groups, modifier, n, mode, threads, perf, buckets_step);
            } else {
                run_bench_score(&groups_to_raw(&groups), n, mode, threads, perf, buckets_step);
            }
        }
        _ => run_bench_winrate(
            &groups_to_raw(&groups),
            n,
            mode,
            threads,
            tswn_core::player::eval_name::WIN_RATE_EVAL_RQ,
            perf,
            buckets_step,
        ),
    }
}

pub fn run_bench_winrate(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    perf: bool,
    buckets_step: Option<usize>,
) {
    println!("=== 对战胜率测试 ({n} 场) ===");

    if let Some(step) = buckets_step {
        let summary = bench_winrate_with_buckets(raw, n, step, eval_rq);
        print_bench_winrate_summary(summary, perf);
    } else {
        let summary = bench_winrate_summary(raw, n, mode, threads, eval_rq);
        print_bench_winrate_summary(summary, perf);
    }
}

pub fn run_bench_group_win_rate(
    target: &str,
    against: &[String],
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    eval_rq: f64,
    perf: bool,
) {
    println!("=== 对组列表胜率测试 ({n} 场) ===");
    println!("target: {}", display_group(target));

    let overall_started = Instant::now();
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut accumulated_timing = WinRateTiming::default();

    for (index, opponent) in against.iter().enumerate() {
        println!();
        println!("[{}/{}] vs {}", index + 1, against.len(), display_group(opponent));
        let raw = format!("{target}\n\n{opponent}");
        let summary = bench_winrate_summary(&raw, n, mode, threads, eval_rq);
        println!("胜率: {:.2}%  ({}/{})", summary.win_rate_percent(), summary.wins, summary.total);
        if perf {
            print_perf_lines(summary.elapsed, summary.timing, summary.total);
        }
        accumulated_rate += summary.win_rate_percent();
        accumulated_wins += summary.wins;
        accumulated_total += summary.total;
        accumulated_timing.merge(summary.timing);
    }

    println!();
    println!("平均胜率: {:.2}%", accumulated_rate / against.len().max(1) as f64);
    println!(
        "汇总胜率: {:.2}%  ({}/{})",
        accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64,
        accumulated_wins,
        accumulated_total
    );
    if perf {
        print_perf_lines(overall_started.elapsed(), accumulated_timing, accumulated_total);
    }
}

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
        // 在 verbose 模式下，先缓冲逐靶输出，待阈值判定后再决定是否打印。
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

        // 终端阈值过滤：avg 是百分比 (0-100)。
        let passes_screen = min_screen.is_none_or(|t| avg >= t);
        // 文件阈值过滤：avg 是百分比 (0-100)。
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

fn bench_winrate_summary(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, eval_rq: f64) -> BenchSummary {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    // 这里的模板只服务当前这一个 matchup；`batch-rate` / `bench pair` 的外层循环
    // 会不断传入新的 `raw`，几乎没有缓存复用价值。改走 uncached 后，当前 matchup
    // 跑完即可释放模板，不会把大量一次性对阵长期压在全局缓存里。
    let prepared = match Runner::prepare_groups_with_eval_rq_uncached(&groups, eval_rq) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: Duration::default(),
            };
        }
    };
    let started_at = Instant::now();

    let thread = match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0),
    };
    let summary = match prepared_win_rate(&prepared, n, eval_rq, thread) {
        Ok(summary) => summary,
        Err(err) => {
            eprintln!("执行胜率测试失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: Duration::default(),
            };
        }
    };

    let mut summary = BenchSummary {
        wins: summary.wins,
        total: summary.total,
        timing: summary.timing,
        elapsed: Duration::default(),
    };

    summary.elapsed = started_at.elapsed();
    summary
}

/// 分段累积胜率测试。按 `step` 将 `n` 场分块，每块结束后输出一次累积胜率。
/// 强制单线程以保证顺序正确。
fn bench_winrate_with_buckets(raw: &str, n: usize, step: usize, eval_rq: f64) -> BenchSummary {
    let step = step.max(1);
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    // 分段输出和普通 win-rate 一样，只消费当前这一份模板；这里没有必要把模板写入
    // 全局缓存，否则批量分析多个输入时缓存会只增不减。
    let prepared = match Runner::prepare_groups_with_eval_rq_uncached(&groups, eval_rq) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: WinRateTiming::default(),
                elapsed: Duration::default(),
            };
        }
    };

    let started_at = Instant::now();
    let use_profile_seed = use_js_profile_seed_schedule(eval_rq);
    let mut cumulative_wins = 0usize;
    let mut cumulative_total = 0usize;
    let mut cumulative_timing = WinRateTiming::default();

    let mut offset = 0usize;
    while offset < n {
        let chunk_end = (offset + step).min(n);
        let chunk = match run_prepared_win_rate_range(&prepared, offset, chunk_end, use_profile_seed) {
            Ok(chunk) => chunk,
            Err(err) => {
                eprintln!("分段 [{offset}, {chunk_end}) 胜率测试失败: {err}");
                break;
            }
        };
        cumulative_wins += chunk.wins;
        cumulative_total += chunk.total;
        cumulative_timing.merge(chunk.timing);
        println!(
            "胜率(分段): {:.2}%  ({}/{})",
            cumulative_wins as f64 * 100.0 / cumulative_total.max(1) as f64,
            cumulative_wins,
            cumulative_total,
        );
        offset = chunk_end;
    }

    let mut summary = BenchSummary {
        wins: cumulative_wins,
        total: cumulative_total,
        timing: cumulative_timing,
        elapsed: Duration::default(),
    };
    summary.elapsed = started_at.elapsed();
    summary
}

fn print_bench_winrate_summary(summary: BenchSummary, perf: bool) {
    let elapsed_secs = summary.elapsed.as_secs_f64();
    let throughput = if elapsed_secs > 0.0 {
        summary.total as f64 / elapsed_secs
    } else {
        0.0
    };
    println!("胜率: {:.2}%  ({}/{})", summary.win_rate_percent(), summary.wins, summary.total);
    println!(
        "耗时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
        elapsed_secs,
        summary.elapsed.as_micros() as f64 / summary.total.max(1) as f64,
        throughput
    );
    if perf {
        print_perf_lines(summary.elapsed, summary.timing, summary.total);
    }
}

fn resolve_bench_workers(mode: BenchThreadMode, threads: Option<usize>, total: usize) -> usize {
    match mode {
        BenchThreadMode::SingleThread => 1,
        BenchThreadMode::Parallel => resolve_win_rate_workers(threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0), total),
    }
}

fn js_score_targets_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else {
        target_group.len()
    }
}

fn js_score_profiles_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else if target_group.len() == 1 {
        3
    } else {
        target_group.len()
    }
}

fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, bench_input: &mut String) {
    bench_input.clear();

    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = tswn_core::engine::PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        bench_input.push_str(&target_group[0]);
        bench_input.push('\n');
        let _ = write!(bench_input, "{}@{modifier}", profile_base);
        bench_input.push_str("\n\n");
        let _ = write!(bench_input, "{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2);
        return;
    }

    for (idx, name) in target_group.iter().take(tracked_targets).enumerate() {
        if idx > 0 {
            bench_input.push('\n');
        }
        bench_input.push_str(name);
    }
    bench_input.push_str("\n\n");
    for offset in 0..profile_count {
        if offset > 0 {
            bench_input.push('\n');
        }
        let _ = write!(bench_input, "{}@{modifier}", profile_base + offset);
    }
}

fn run_bench_score_with_modifier(
    groups: &[Vec<String>],
    modifier: &'static str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let target_group = groups.first().cloned().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }
    let label = if modifier == "!" { "!评分" } else { "普通评分" };

    println!("=== 实力评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    let summary = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, modifier, n, step)
    } else {
        run_bench_score_inner(&target_group, modifier, n, mode, threads, true)
    };
    let score = summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64;
    println!("{label}: {:.0} / 10000  ({}/{})", score, summary.wins, summary.total);
    if perf {
        print_perf_lines(summary.elapsed, summary.timing, summary.total);
    }
}

fn run_bench_score(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, perf: bool, buckets_step: Option<usize>) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let target_group = groups.into_iter().next().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }

    println!("=== 实力评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    print!("[普通评分] ");
    let normal = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, "\u{0002}", n, step)
    } else {
        run_bench_score_inner(&target_group, "\u{0002}", n, mode, threads, true)
    };
    let ns = normal.wins as f64 * 10_000.0 / normal.total.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", ns, normal.wins, normal.total);
    if perf {
        print_perf_lines(normal.elapsed, normal.timing, normal.total);
    }

    print!("[!评分]    ");
    let bang = if let Some(step) = buckets_step.filter(|step| *step > 0) {
        run_bench_score_with_bucket_output(&target_group, "!", n, step)
    } else {
        run_bench_score_inner(&target_group, "!", n, mode, threads, true)
    };
    let bs = bang.wins as f64 * 10_000.0 / bang.total.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", bs, bang.wins, bang.total);
    if perf {
        print_perf_lines(bang.elapsed, bang.timing, bang.total);
    }
}

fn run_bench_score_with_bucket_output(target_group: &[String], modifier: &str, n: usize, step: usize) -> BenchSummary {
    let started_at = Instant::now();
    let (wins, total, timing) = run_bench_score_range_with_bucket_output(target_group, modifier, 0, n, step);
    BenchSummary {
        wins,
        total,
        timing,
        elapsed: started_at.elapsed(),
    }
}

fn run_bench_score_range_with_bucket_output(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    step: usize,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        // score 路径每轮都会把 round 编进 profile 名字里；从缓存视角看，这几乎等价于
        // “每一局都是一组全新的 players key”。继续走 cached 构造只会不断制造新缓存项，
        // 命中率极低，却会让内存随场数线性增长。
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }

        if total % step == 0 || i + 1 == end {
            let score = wins as f64 * 10_000.0 / total.max(1) as f64;
            println!("评分(分段): {:.0} / 10000  ({wins}/{total})", score);
        }
    }

    (wins, total, timing)
}

fn run_bench_score_inner(
    target_group: &[String],
    modifier: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    show_progress: bool,
) -> BenchSummary {
    let workers = resolve_bench_workers(mode, threads, n);
    let started_at = Instant::now();

    let mut summary = if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        let (wins, total, timing) = run_bench_score_range(target_group, modifier, 0, n, show_progress);
        BenchSummary {
            wins,
            total,
            timing,
            elapsed: Duration::default(),
        }
    } else {
        let next = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let target_group = target_group.to_vec();
            let modifier = modifier.to_string();
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_score_worker(&target_group, modifier.as_str(), next.as_ref(), n)
            }));
        }

        let mut merged = BenchSummary {
            wins: 0,
            total: 0,
            timing: WinRateTiming::default(),
            elapsed: Duration::default(),
        };
        for handle in handles {
            let (wins, total, timing) = handle.join().expect("score worker thread panicked");
            merged.wins += wins;
            merged.total += total;
            merged.timing.merge(timing);
        }
        merged
    };

    summary.elapsed = started_at.elapsed();
    summary
}

pub fn run_namer_pf(raw: &str, n: usize, threads: Option<usize>) {
    let groups = parse_plus_separated_groups(raw);
    if groups.is_empty() {
        eprintln!("namer-pf: 输入为空或无有效玩家");
        return;
    }

    println!("pp|pd|qp|qd");
    for group in groups {
        let pp = namer_pf_score(&group, "\u{0002}", false, n, threads);
        let pd = namer_pf_score(&group, "\u{0002}", true, n, threads);
        let qp = namer_pf_score(&group, "!", false, n, threads);
        let qd = namer_pf_score(&group, "!", true, n, threads);
        let sum = pp + pd + qp + qd;
        println!("{pp}|{pd}|{qp}|{qd}|{sum}");
    }
}

fn parse_plus_separated_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_namer_pf_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

fn parse_namer_pf_group_line(line: &str) -> Vec<String> {
    let mut group: Vec<String> = Vec::new();
    for segment in split_plus_outside_quotes(line) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if PlayerOverlay::parse_inline(segment).is_some()
            && let Some(previous) = group.last_mut()
        {
            previous.push('+');
            previous.push_str(segment);
            continue;
        }
        group.push(segment.to_string());
    }
    group
}

fn split_plus_outside_quotes(raw: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in raw.chars() {
        if in_string {
            current.push(ch);
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
        } else if ch == '+' {
            segments.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
            if ch == '"' {
                in_string = true;
            }
        }
    }
    segments.push(current);
    segments
}

fn namer_pf_score(base_group: &[String], modifier: &str, duplicate: bool, n: usize, threads: Option<usize>) -> u64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }

    let summary = run_bench_score_inner(&target_group, modifier, n, BenchThreadMode::Parallel, threads, false);
    (summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64).round() as u64
}

fn run_bench_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    show_progress: bool,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut progress_printed = false;
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        // 并行 worker 与单线程路径是同一个问题：profile 名字持续变化，缓存几乎不会命中。
        // 这里必须走 uncached，避免多个 worker 一起向全局缓存灌入只用一次的模板。
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
        if show_progress && (i + 1) % 100 == 0 {
            print!("\r  进度: {}/{}  ", i + 1, end);
            progress_printed = true;
        }
    }
    if progress_printed {
        println!();
    }
    (wins, total, timing)
}

fn run_bench_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);

        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, WIN_RATE_EVAL_RQ) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

fn print_perf_lines(total_elapsed: Duration, timing: WinRateTiming, total: usize) {
    let total_f = total.max(1) as f64;
    let total_secs = total_elapsed.as_secs_f64();
    let throughput = if total_secs > 0.0 { total_f / total_secs } else { 0.0 };
    println!("─────────────────────────────────");
    println!(
        "total :  {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
        total_secs,
        total_elapsed.as_micros() as f64 / total_f,
        throughput
    );
    println!(
        "init  :  {:.3}s  ({:.1}µs/场)",
        timing.init_nanos as f64 / 1e9,
        timing.init_nanos as f64 / 1e3 / total_f
    );
    println!(
        "fight :  {:.3}s  ({:.1}µs/场)",
        timing.fight_nanos as f64 / 1e9,
        timing.fight_nanos as f64 / 1e3 / total_f
    );
}

fn display_group(raw: &str) -> String {
    raw.lines().map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join(", ")
}

fn first_duplicate_name_in_matchup(groups: &[&str]) -> Option<String> {
    let mut seen = HashSet::new();
    for group in groups {
        for name in group.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let id_name = Player::raw_namerena_to_idname(name);
            if !seen.insert(id_name.clone()) {
                return Some(id_name);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_js_default_score_marker() {
        let parsed = parse_benchmark_input("!test!\n\naaaa\nbbbb");
        assert_eq!(parsed.score_modifier, Some("\u{0002}"));
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string(), "bbbb".to_string()]]);
    }

    #[test]
    fn parses_js_bang_score_marker() {
        let parsed = parse_benchmark_input("!test!\n!\n\naaaa\nbbbb");
        assert_eq!(parsed.score_modifier, Some("!"));
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string(), "bbbb".to_string()]]);
    }

    #[test]
    fn parses_js_win_rate_marker() {
        let parsed = parse_benchmark_input("!test!\n\naaaa\n\nbbbb@!");
        assert_eq!(parsed.score_modifier, Some("\u{0002}"));
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string()], vec!["bbbb@!".to_string()]]);
        assert_eq!(groups_to_raw(&parsed.groups), "aaaa\n\nbbbb@!");
    }

    #[test]
    fn leaves_non_marker_input_unchanged() {
        let parsed = parse_benchmark_input("aaaa\n\nbbbb@!");
        assert_eq!(parsed.score_modifier, None);
        assert_eq!(parsed.groups, vec![vec!["aaaa".to_string()], vec!["bbbb@!".to_string()]]);
    }

    #[test]
    fn duplicate_name_check_detects_cross_group_duplicate() {
        assert_eq!(
            first_duplicate_name_in_matchup(&["alice\nbob", "carol\nalice"]),
            Some("alice".to_string())
        );
    }

    #[test]
    fn duplicate_name_check_ignores_overlay_suffix() {
        let base = "涵虚不等式 PFVKEUPBU@TigerStar";
        let overlay = r#"涵虚不等式 PFVKEUPBU@TigerStar+ol:{"attrs":[89,85,88,77,48,96,97,327]}"#;
        assert_eq!(first_duplicate_name_in_matchup(&[base, overlay]), Some(base.to_string()));
    }

    #[test]
    fn duplicate_name_check_allows_distinct_names() {
        assert_eq!(first_duplicate_name_in_matchup(&["alice\nbob", "carol\ndave"]), None);
    }

    #[test]
    fn builds_single_target_score_match_like_js() {
        let single = ["aaaaa".to_string()];
        let mut bench_input = String::new();
        build_js_score_match_input(&single, "!", 0, &mut bench_input);
        assert_eq!(js_score_targets_per_round(&single), 1);
        assert_eq!(js_score_profiles_per_round(&single), 3);
        assert_eq!(bench_input, "aaaaa\n33554431@!\n\n33554432@!\n33554433@!");
    }

    #[test]
    fn collapses_duplicate_single_target_like_js() {
        let duplicate = ["aaaaa".to_string(), "aaaaa".to_string()];
        let mut bench_input = String::new();
        build_js_score_match_input(&duplicate, "!", 0, &mut bench_input);
        assert_eq!(js_score_targets_per_round(&duplicate), 1);
        assert_eq!(js_score_profiles_per_round(&duplicate), 1);
        assert_eq!(bench_input, "aaaaa\n\n33554431@!");
    }

    #[test]
    fn namer_pf_parser_accepts_plus_groups() {
        assert_eq!(
            parse_plus_separated_groups("aaaaa+bbbbb\nccccc\n\n"),
            vec![vec!["aaaaa".to_string(), "bbbbb".to_string()], vec!["ccccc".to_string()],]
        );
    }

    #[test]
    fn namer_pf_parser_keeps_diy_overlay_with_player() {
        let diy = r#"aaaaa+diy[58,87,82,78,89,93,99,343]{"skldefend":13,"sklassassinate":"2*46","sklheal":"40+30"}"#;
        let raw = format!("{diy}+bbbbb");

        assert_eq!(
            parse_plus_separated_groups(&raw),
            vec![vec![diy.to_string(), "bbbbb".to_string(),]]
        );
    }

    #[test]
    fn namer_pf_parser_keeps_ol_overlay_with_player() {
        let ol = r#"aaaaa+ol:{"attrs":[58,87,82,78,89,93,99,343],"skills":{"skldefend":13,"sklheal":"40+30"},"name_factor_enabled":true}"#;
        let raw = format!("{ol}+bbbbb");

        assert_eq!(
            parse_plus_separated_groups(&raw),
            vec![vec![ol.to_string(), "bbbbb".to_string(),]]
        );
    }
}

enum ExistingFileAction {
    Overwrite,
    Append,
}

fn open_batch_rate_output(path: &Path, force: bool) -> io::Result<File> {
    if path.file_name().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出路径必须包含文件名: {}", path.display()),
        ));
    }
    if path.exists() && path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出路径不能是目录: {}", path.display()),
        ));
    }

    ensure_batch_rate_output_parent(path)?;

    if path.exists() {
        match resolve_existing_file_action(path, force)? {
            ExistingFileAction::Overwrite => OpenOptions::new().write(true).truncate(true).open(path),
            ExistingFileAction::Append => OpenOptions::new().append(true).open(path),
        }
    } else {
        OpenOptions::new().write(true).create_new(true).open(path)
    }
}

fn ensure_batch_rate_output_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("输出目录不存在: {}", parent.display()),
        ));
    }
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("输出目录不是目录: {}", parent.display()),
        ));
    }
    Ok(())
}

fn resolve_existing_file_action(path: &Path, force: bool) -> io::Result<ExistingFileAction> {
    if force {
        return Ok(ExistingFileAction::Overwrite);
    }
    if prompt_yes_no(&format!("输出文件已存在: {}\n是否覆盖? [y/N]: ", path.display()))? {
        return Ok(ExistingFileAction::Overwrite);
    }
    if prompt_yes_no("是否追加? [y/N]: ")? {
        return Ok(ExistingFileAction::Append);
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!("输出文件已存在，且未选择覆盖或追加: {}", path.display()),
    ))
}

fn prompt_yes_no(prompt: &str) -> io::Result<bool> {
    let mut stderr = io::stderr().lock();
    stderr.write_all(prompt.as_bytes())?;
    stderr.flush()?;

    let mut line = String::new();
    open_prompt_reader()?.read_line(&mut line)?;
    let answer = line.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

fn open_prompt_reader() -> io::Result<BufReader<File>> {
    open_tty_for_read().map(BufReader::new).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("输出文件已存在，但当前无法交互确认: {err}；请改用 --force/-f"),
        )
    })
}

#[cfg(windows)]
fn open_tty_for_read() -> io::Result<File> { File::open("CONIN$") }

#[cfg(not(windows))]
fn open_tty_for_read() -> io::Result<File> { File::open("/dev/tty") }

fn write_batch_rate_record(file: &mut File, line: &str) -> io::Result<()> {
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    file.flush()
}

fn format_batch_rate_record(
    label: &str,
    avg_rate: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    elapsed: Duration,
    throughput: f64,
    valid_matchups: usize,
    skipped_matchups: usize,
    wr_precision: usize,
) -> String {
    format!(
        "{{\"label\":\"{}\",\"avg_win_rate\":{},\"aggregate_win_rate\":{},\"wins\":{wins},\"total\":{total},\"valid_matchups\":{valid_matchups},\"skipped_matchups\":{skipped_matchups},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{throughput:.0}}}",
        escape_json_string(label),
        format_rate(avg_rate, wr_precision),
        format_rate(aggregate_rate, wr_precision),
        elapsed.as_secs_f64(),
        elapsed.as_micros() as f64 / total.max(1) as f64,
    )
}

fn format_batch_rate_log_record(label: &str, avg_rate: f64, wr_precision: usize) -> String {
    format!("{} {label}", format_rate(avg_rate, wr_precision))
}

fn format_batch_rate_pure_record(label: &str) -> String { label.to_string() }

#[allow(clippy::too_many_arguments)]
fn format_pair_rate_record(
    label: &str,
    final_score: f64,
    selected_count: usize,
    head: usize,
    pair_rates: &[(f64, String)],
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    elapsed: Duration,
    throughput: f64,
    valid_matchups: usize,
    skipped_matchups: usize,
    wr_precision: usize,
) -> String {
    let top_pairs = pair_rates
        .iter()
        .take(selected_count)
        .map(|(rate, teammate)| {
            format!(
                "{{\"teammate\":\"{}\",\"batch_rate\":{}}}",
                escape_json_string(teammate),
                format_rate(*rate, wr_precision)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"label\":\"{}\",\"score\":{},\"head\":{head},\"selected\":{selected_count},\"top_pairs\":[{top_pairs}],\"aggregate_win_rate\":{},\"wins\":{wins},\"total\":{total},\"valid_matchups\":{valid_matchups},\"skipped_matchups\":{skipped_matchups},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{throughput:.0}}}",
        escape_json_string(label),
        format_rate(final_score, wr_precision),
        format_rate(aggregate_rate, wr_precision),
        elapsed.as_secs_f64(),
        elapsed.as_micros() as f64 / total.max(1) as f64,
    )
}

fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

fn player_to_ol_or_exit(raw: &str) -> String {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return raw.to_string();
    }
    let storage = Storage::new_arc();
    let mut player = match Player::new_from_namerena_raw(raw.to_string(), storage) {
        Ok(player) => player,
        Err(err) => {
            eprintln!("转换 player-list 名字为 +ol 失败: {raw}: {err}");
            std::process::exit(1);
        }
    };
    player.build();
    player.to_ol_json()
}

fn escape_json_string(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

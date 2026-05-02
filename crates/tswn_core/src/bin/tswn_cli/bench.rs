use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, IsTerminal, Write as _};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tswn_core::{
    Runner,
    win_rate::{
        WinRateTiming, prepared_win_rate, resolve_win_rate_workers, run_prepared_win_rate_range, use_js_profile_seed_schedule,
    },
};

use crate::{BENCH_PARALLEL_THRESHOLD, args::BenchThreadMode};

const PROGRESS_BAR_WIDTH: usize = 30;
const SLIDING_WINDOW: usize = 5;

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

pub fn run_benchmark(
    raw: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    perf: bool,
    buckets_step: Option<usize>,
) {
    let raw = raw.trim();
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("benchmark: 输入为空或无有效玩家"),
        1 => run_bench_score(raw, n, mode, threads, perf),
        _ => run_bench_winrate(
            raw,
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
    min_wr: Option<u16>,
) {
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
    if let Some(threshold) = min_wr {
        println!("最低胜率阈值: {threshold}/10000 ({:.2}%)", threshold as f64 / 100.0);
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

        let overall_started = Instant::now();
        let mut accumulated_rate = 0.0;
        let mut accumulated_wins = 0usize;
        let mut accumulated_total = 0usize;
        let mut accumulated_timing = WinRateTiming::default();

        for (ti, target) in target_groups.iter().enumerate() {
            let raw = format!("{player}\n\n{target}");
            let summary = bench_winrate_summary(&raw, n, mode, threads, eval_rq);
            if verbose {
                let _ = writeln!(
                    &mut verbose_buf,
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
            progress.tick_target();
        }

        let avg = accumulated_rate / target_groups.len().max(1) as f64;
        let elapsed = overall_started.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        let throughput = if elapsed_secs > 0.0 {
            accumulated_total as f64 / elapsed_secs
        } else {
            0.0
        };
        let aggregate_rate = accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64;
        let summary_line = format_batch_rate_record(
            label,
            avg,
            aggregate_rate,
            accumulated_wins,
            accumulated_total,
            elapsed,
            throughput,
        );

        progress.complete_player(elapsed);

        // 阈值过滤：avg 是百分比 (0-100), min_wr 是万分比 (0-10000)
        let passes = min_wr.is_none_or(|t| avg * 100.0 >= t as f64);

        if passes {
            if verbose {
                progress.clear();
                print!("{verbose_buf}");
                println!("平均胜率: {:.2}%  (对 {} 组靶子)", avg, target_groups.len());
                println!("汇总胜率: {:.2}%  ({}/{})", aggregate_rate, accumulated_wins, accumulated_total);
                println!(
                    "用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                    elapsed_secs,
                    elapsed.as_micros() as f64 / accumulated_total.max(1) as f64,
                    throughput
                );
            } else if out_file.is_none() {
                progress.clear();
                println!(
                    "{}\t平均胜率: {:.2}%\t用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                    label,
                    avg,
                    elapsed_secs,
                    elapsed.as_micros() as f64 / accumulated_total.max(1) as f64,
                    throughput
                );
            }
        }

        // 文件输出不受阈值影响，始终写入。
        if let Some(file) = out_file.as_mut()
            && let Err(err) = write_batch_rate_record(file, &summary_line)
        {
            eprintln!("写入批量结果输出文件失败: {err}");
            std::process::exit(1);
        }

        if perf && passes {
            progress.clear();
            print_perf_lines(elapsed, accumulated_timing, accumulated_total);
        }

        progress.draw();
    }

    progress.finish();
}

fn bench_winrate_summary(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, eval_rq: f64) -> BenchSummary {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let prepared = match Runner::prepare_groups_with_eval_rq(&groups, eval_rq) {
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
    let prepared = match Runner::prepare_groups_with_eval_rq(&groups, eval_rq) {
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

fn run_bench_score(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, perf: bool) {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let target_group = groups.into_iter().next().unwrap_or_default();
    let target_count = target_group.len();
    if target_count == 0 {
        eprintln!("评分: 无目标玩家");
        return;
    }
    let target_str = target_group.join("\n");

    println!("=== 实力评分测试 ({n} 场) ===");
    println!("目标: {}", target_group.join(", "));
    println!("info: {target_count}");

    print!("[普通评分] ");
    let normal = run_bench_score_inner(&target_str, target_count, "\u{0002}", n, mode, threads, true);
    let ns = normal.wins as f64 * 10_000.0 / normal.total.max(1) as f64;
    println!("普通评分: {:.0} / 10000  ({}/{})", ns, normal.wins, normal.total);
    if perf {
        print_perf_lines(normal.elapsed, normal.timing, normal.total);
    }

    print!("[!评分]    ");
    let bang = run_bench_score_inner(&target_str, target_count, "!", n, mode, threads, true);
    let bs = bang.wins as f64 * 10_000.0 / bang.total.max(1) as f64;
    println!("!评分:     {:.0} / 10000  ({}/{})", bs, bang.wins, bang.total);
    if perf {
        print_perf_lines(bang.elapsed, bang.timing, bang.total);
    }
}

fn run_bench_score_inner(
    target_str: &str,
    target_count: usize,
    modifier: &str,
    n: usize,
    mode: BenchThreadMode,
    threads: Option<usize>,
    show_progress: bool,
) -> BenchSummary {
    let workers = resolve_bench_workers(mode, threads, n);
    let started_at = Instant::now();

    let mut summary = if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        let (wins, total, timing) = run_bench_score_range(target_str, target_count, modifier, 0, n, show_progress);
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
            let target_str = target_str.to_string();
            let modifier = modifier.to_string();
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_score_worker(target_str.as_str(), target_count, modifier.as_str(), next.as_ref(), n)
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

fn run_bench_score_range(
    target_str: &str,
    target_count: usize,
    modifier: &str,
    start: usize,
    end: usize,
    show_progress: bool,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut progress_printed = false;
    let mut targets = String::with_capacity(target_count.saturating_mul(24));
    let mut bench_input = String::with_capacity(target_str.len() + target_count.saturating_mul(24) + 3);

    for i in start..end {
        targets.clear();
        let base = tswn_core::engine::PROFILE_START as usize + i * target_count;
        for offset in 0..target_count {
            if offset > 0 {
                targets.push('\n');
            }
            let _ = write!(&mut targets, "{}@{modifier}", base + offset);
        }

        bench_input.clear();
        bench_input.push_str(target_str);
        bench_input.push_str("\n\n");
        bench_input.push_str(&targets);

        let t_init = Instant::now();
        let mut runner = match Runner::new_from_namerena_raw(bench_input.clone()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_roster: Vec<usize> = runner.input_groups.first().cloned().unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|w| team0_roster.contains(w))
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
    target_str: &str,
    target_count: usize,
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut targets = String::with_capacity(target_count.saturating_mul(24));
    let mut bench_input = String::with_capacity(target_str.len() + target_count.saturating_mul(24) + 3);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        targets.clear();
        let base = tswn_core::engine::PROFILE_START as usize + i * target_count;
        for offset in 0..target_count {
            if offset > 0 {
                targets.push('\n');
            }
            let _ = write!(&mut targets, "{}@{modifier}", base + offset);
        }

        bench_input.clear();
        bench_input.push_str(target_str);
        bench_input.push_str("\n\n");
        bench_input.push_str(&targets);

        let t_init = Instant::now();
        let mut runner = match Runner::new_from_namerena_raw(bench_input.clone()) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let team0_roster: Vec<usize> = runner.input_groups.first().cloned().unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(ref winners) = runner.world.winner
            && winners.iter().any(|w| team0_roster.contains(w))
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
) -> String {
    format!(
        "{{\"label\":\"{}\",\"avg_win_rate\":{avg_rate:.2},\"aggregate_win_rate\":{aggregate_rate:.2},\"wins\":{wins},\"total\":{total},\"elapsed_s\":{:.3},\"us_per_battle\":{:.1},\"battles_per_s\":{throughput:.0}}}",
        escape_json_string(label),
        elapsed.as_secs_f64(),
        elapsed.as_micros() as f64 / total.max(1) as f64,
    )
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

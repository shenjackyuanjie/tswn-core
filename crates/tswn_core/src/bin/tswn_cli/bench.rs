use std::fmt::Write as _;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write as _};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tswn_core::{
    Runner,
    win_rate::{WinRateTiming, prepared_win_rate, resolve_win_rate_workers},
};

use crate::args::BenchThreadMode;

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

pub fn run_benchmark(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, perf: bool) {
    let raw = raw.trim();
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    match group_count {
        0 => eprintln!("benchmark: 输入为空或无有效玩家"),
        1 => run_bench_score(raw, n, mode, threads, perf),
        _ => run_bench_winrate(raw, n, mode, threads, tswn_core::player::eval_name::WIN_RATE_EVAL_RQ, perf),
    }
}

pub fn run_bench_winrate(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, eval_rq: f64, perf: bool) {
    println!("=== 对战胜率测试 ({n} 场) ===");
    let summary = bench_winrate_summary(raw, n, mode, threads, eval_rq);
    print_bench_winrate_summary(summary, perf);
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

    for (pi, (player, label)) in player_groups.iter().zip(player_labels.iter()).enumerate() {
        if verbose {
            println!();
            println!("━━━ [{}/{}] {} ━━━", pi + 1, player_groups.len(), label);
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
                println!(
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

        if verbose {
            println!("平均胜率: {:.2}%  (对 {} 组靶子)", avg, target_groups.len());
            println!("汇总胜率: {:.2}%  ({}/{})", aggregate_rate, accumulated_wins, accumulated_total);
            println!(
                "用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                elapsed_secs,
                elapsed.as_micros() as f64 / accumulated_total.max(1) as f64,
                throughput
            );
        } else if out_file.is_none() {
            println!(
                "{}\t平均胜率: {:.2}%\t用时: {:.3}s  ({:.1}µs/场, {:.0} 场/s)",
                label,
                avg,
                elapsed_secs,
                elapsed.as_micros() as f64 / accumulated_total.max(1) as f64,
                throughput
            );
        }

        if let Some(file) = out_file.as_mut()
            && let Err(err) = write_batch_rate_record(file, &summary_line)
        {
            eprintln!("写入批量结果输出文件失败: {err}");
            std::process::exit(1);
        }

        if perf {
            print_perf_lines(elapsed, accumulated_timing, accumulated_total);
        }
    }
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

    let mut summary = if workers <= 1 || n < 2000 {
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

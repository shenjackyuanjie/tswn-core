use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use tswn_core::{PreparedRunner, Runner};

use crate::args::BenchThreadMode;

const PROFILE_WINRATE_SEED_START: usize = 33_554_431;

fn use_js_profile_seed_schedule(eval_rq: f64) -> bool { eval_rq == tswn_core::player::eval_name::WIN_RATE_EVAL_RQ }

#[derive(Debug, Clone, Copy, Default)]
struct TimingParts {
    init_nanos: u128,
    fight_nanos: u128,
}

impl TimingParts {
    fn merge(&mut self, other: Self) {
        self.init_nanos += other.init_nanos;
        self.fight_nanos += other.fight_nanos;
    }
}

#[derive(Debug, Clone, Copy)]
struct BenchSummary {
    wins: usize,
    total: usize,
    timing: TimingParts,
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
    let mut accumulated_timing = TimingParts::default();

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

fn bench_winrate_summary(raw: &str, n: usize, mode: BenchThreadMode, threads: Option<usize>, eval_rq: f64) -> BenchSummary {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let team0_count = groups
        .first()
        .map(|group| group.iter().filter(|name| !tswn_core::player::Player::check_is_seed(name)).count())
        .unwrap_or(0);
    let prepared = match Runner::prepare_groups_with_eval_rq(&groups, eval_rq) {
        Ok(prepared) => prepared,
        Err(err) => {
            eprintln!("构建胜率模板失败: {err}");
            return BenchSummary {
                wins: 0,
                total: 0,
                timing: TimingParts::default(),
                elapsed: Duration::default(),
            };
        }
    };
    let prepared = Arc::new(prepared);
    let workers = resolve_bench_workers(mode, threads, n);
    let use_profile_seed = use_js_profile_seed_schedule(eval_rq);
    let started_at = Instant::now();

    let mut summary = if workers <= 1 || n < 2000 {
        let (wins, total, timing) = run_bench_winrate_range(prepared.as_ref(), team0_count, 0, n, use_profile_seed);
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
            let prepared = Arc::clone(&prepared);
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_winrate_worker(prepared.as_ref(), team0_count, next.as_ref(), n, use_profile_seed)
            }));
        }
        let mut merged = BenchSummary {
            wins: 0,
            total: 0,
            timing: TimingParts::default(),
            elapsed: Duration::default(),
        };
        for handle in handles {
            let (wins, total, timing) = handle.join().expect("winrate worker thread panicked");
            merged.wins += wins;
            merged.total += total;
            merged.timing.merge(timing);
        }
        merged
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
        BenchThreadMode::Parallel => threads
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|x| x.get().saturating_mul(5).div_ceil(4))
                    .unwrap_or(1)
            })
            .min(total.max(1)),
    }
}

fn run_bench_winrate_range(
    prepared: &PreparedRunner,
    _team0_count: usize,
    start: usize,
    end: usize,
    use_profile_seed: bool,
) -> (usize, usize, TimingParts) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut seed = String::with_capacity(24);
    let mut timing = TimingParts::default();

    for i in start..end {
        let seed_ref: &[String] = if use_profile_seed {
            if i == 0 {
                &[]
            } else {
                seed.clear();
                let _ = write!(&mut seed, "seed:{}@!", PROFILE_WINRATE_SEED_START + i);
                std::slice::from_ref(&seed)
            }
        } else {
            seed.clear();
            let _ = write!(&mut seed, "seed:{i}@!");
            std::slice::from_ref(&seed)
        };
        let t_init = Instant::now();
        let mut runner = match Runner::new_from_prepared_with_seed(prepared, seed_ref) {
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
            && winners.iter().any(|winner| team0_roster.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

fn run_bench_winrate_worker(
    prepared: &PreparedRunner,
    _team0_count: usize,
    next: &AtomicUsize,
    end: usize,
    use_profile_seed: bool,
) -> (usize, usize, TimingParts) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut seed = String::with_capacity(24);
    let mut timing = TimingParts::default();

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        let seed_ref: &[String] = if use_profile_seed {
            if i == 0 {
                &[]
            } else {
                seed.clear();
                let _ = write!(&mut seed, "seed:{}@!", PROFILE_WINRATE_SEED_START + i);
                std::slice::from_ref(&seed)
            }
        } else {
            seed.clear();
            let _ = write!(&mut seed, "seed:{i}@!");
            std::slice::from_ref(&seed)
        };
        let t_init = Instant::now();
        let mut runner = match Runner::new_from_prepared_with_seed(prepared, seed_ref) {
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
            && winners.iter().any(|winner| team0_roster.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
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
            timing: TimingParts::default(),
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
) -> (usize, usize, TimingParts) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = TimingParts::default();
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
) -> (usize, usize, TimingParts) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = TimingParts::default();
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

fn print_perf_lines(total_elapsed: Duration, timing: TimingParts, total: usize) {
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

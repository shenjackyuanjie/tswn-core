use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::error::runner::RunnerResult;
use crate::{PreparedRunner, Runner};

const PREPARED_WIN_RATE_PARALLEL_THRESHOLD: usize = 100;

#[cfg(target_family = "wasm")]
fn platform_default_win_rate_workers() -> usize { 1 }

#[cfg(not(target_family = "wasm"))]
fn platform_default_win_rate_workers() -> usize {
    std::thread::available_parallelism()
        .map(|x| x.get().saturating_mul(5).div_ceil(4))
        .unwrap_or(1)
}

#[cfg(target_family = "wasm")]
fn platform_limit_win_rate_workers(_workers: usize) -> usize {
    // Browser wasm does not expose std thread spawning by default.
    1
}

#[cfg(not(target_family = "wasm"))]
fn platform_limit_win_rate_workers(workers: usize) -> usize { workers.max(1) }

#[derive(Debug, Clone, Copy, Default)]
pub struct WinRateTiming {
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

impl WinRateTiming {
    pub fn merge(&mut self, other: Self) {
        self.init_nanos += other.init_nanos;
        self.fight_nanos += other.fight_nanos;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WinRateSummary {
    pub wins: usize,
    pub total: usize,
    pub timing: WinRateTiming,
}

impl WinRateSummary {
    pub fn win_rate_percent(self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }
}

pub fn use_js_profile_seed_schedule(eval_rq: f64) -> bool { eval_rq == crate::player::eval_name::WIN_RATE_EVAL_RQ }

pub fn resolve_win_rate_workers(thread: u32, total: usize) -> usize {
    let workers = match thread {
        0 => platform_default_win_rate_workers(),
        1 => 1,
        n => n as usize,
    };
    platform_limit_win_rate_workers(workers).min(total.max(1))
}

pub fn prepared_win_rate(prepared: &PreparedRunner, n: usize, eval_rq: f64, thread: u32) -> RunnerResult<WinRateSummary> {
    let workers = resolve_win_rate_workers(thread, n);
    let use_profile_seed = use_js_profile_seed_schedule(eval_rq);

    if !should_parallelize_prepared_win_rate(workers, n) {
        return run_prepared_win_rate_range(prepared, 0, n, use_profile_seed);
    }

    let prepared = Arc::new(prepared.clone());
    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let prepared = Arc::clone(&prepared);
        let next = Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_prepared_win_rate_worker(prepared.as_ref(), next.as_ref(), n, use_profile_seed)
        }));
    }

    let mut merged = WinRateSummary::default();
    for handle in handles {
        let part = handle.join().expect("win-rate worker thread panicked")?;
        merged.wins += part.wins;
        merged.total += part.total;
        merged.timing.merge(part.timing);
    }
    Ok(merged)
}

pub fn groups_win_rate(groups: &[Vec<String>], n: usize, eval_rq: f64, thread: u32) -> RunnerResult<WinRateSummary> {
    let prepared = Runner::prepare_groups_with_eval_rq(groups, eval_rq)?;
    prepared_win_rate(&prepared, n, eval_rq, thread)
}

fn should_parallelize_prepared_win_rate(workers: usize, n: usize) -> bool {
    workers > 1 && n >= PREPARED_WIN_RATE_PARALLEL_THRESHOLD
}

fn run_prepared_win_rate_range(
    prepared: &PreparedRunner,
    start: usize,
    end: usize,
    use_profile_seed: bool,
) -> RunnerResult<WinRateSummary> {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut seed = String::with_capacity(24);
    let mut timing = WinRateTiming::default();

    for i in start..end {
        let seed_ref: &[String] = if use_profile_seed {
            if i == 0 {
                &[]
            } else {
                seed.clear();
                let _ = write!(&mut seed, "seed:{}@!", crate::engine::PROFILE_START as usize + i);
                std::slice::from_ref(&seed)
            }
        } else {
            seed.clear();
            let _ = write!(&mut seed, "seed:{i}@!");
            std::slice::from_ref(&seed)
        };

        let t_init = std::time::Instant::now();
        let mut runner = Runner::new_from_prepared_with_seed(prepared, seed_ref)?;
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = std::time::Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(winners) = runner.world.winner.as_ref()
            && let Some(team0) = runner.input_groups.first()
            && winners.iter().any(|winner| team0.contains(winner))
        {
            wins += 1;
        }
    }

    Ok(WinRateSummary { wins, total, timing })
}

fn run_prepared_win_rate_worker(
    prepared: &PreparedRunner,
    next: &AtomicUsize,
    end: usize,
    use_profile_seed: bool,
) -> RunnerResult<WinRateSummary> {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut seed = String::with_capacity(24);
    let mut timing = WinRateTiming::default();

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
                let _ = write!(&mut seed, "seed:{}@!", crate::engine::PROFILE_START as usize + i);
                std::slice::from_ref(&seed)
            }
        } else {
            seed.clear();
            let _ = write!(&mut seed, "seed:{i}@!");
            std::slice::from_ref(&seed)
        };

        let t_init = std::time::Instant::now();
        let mut runner = Runner::new_from_prepared_with_seed(prepared, seed_ref)?;
        timing.init_nanos += t_init.elapsed().as_nanos();

        let t_fight = std::time::Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(winners) = runner.world.winner.as_ref()
            && let Some(team0) = runner.input_groups.first()
            && winners.iter().any(|winner| team0.contains(winner))
        {
            wins += 1;
        }
    }

    Ok(WinRateSummary { wins, total, timing })
}

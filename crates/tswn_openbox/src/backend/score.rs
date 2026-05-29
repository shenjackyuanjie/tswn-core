//! 基准测试摘要与胜率汇总。
//!
//! 提供 [`BenchSummary`]（单组基准测试结果汇总）及 [`BatchRateSummary`]（多组胜率统计），
//! 封装批量对战计算、结果汇总与文本格式化。

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use tswn_core::win_rate::{WinRateTiming, prepared_win_rate, resolve_win_rate_workers};
use tswn_core::{PreparedRunner, Runner};

use super::format::display_group;
use super::parse::first_duplicate_name_in_matchup;

const BENCH_PARALLEL_THRESHOLD: usize = 100;

#[derive(Debug, Clone)]
pub struct BenchSummary {
    pub wins: usize,
    pub total: usize,
    pub timing: WinRateTiming,
}

impl BenchSummary {
    pub fn win_rate_percent(&self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }
}

#[derive(Debug, Clone)]
pub struct BatchRateSummary {
    pub avg: f64,
    pub wins: usize,
    pub total: usize,
    pub valid_matchups: usize,
    pub skipped_matchups: usize,
}

impl BatchRateSummary {}

pub fn bench_batch_rate_for_group(
    player: &str,
    target_groups: &[String],
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    verbose_buf: &mut String,
    cancel: &AtomicBool,
    mut tick_target: impl FnMut(usize, &str),
) -> BatchRateSummary {
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut _accumulated_timing = WinRateTiming::default();
    let mut valid_matchups = 0usize;
    let mut skipped_matchups = 0usize;

    for (index, target) in target_groups.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if let Some(duplicate) = first_duplicate_name_in_matchup(&[player, target.as_str()]) {
            skipped_matchups += 1;
            if verbose {
                let _ = writeln!(
                    verbose_buf,
                    "  [{}/{}] vs {} => SKIP duplicate name: {}",
                    index + 1,
                    target_groups.len(),
                    display_group(target),
                    duplicate
                );
            }
            tick_target(index, target);
            continue;
        }

        let raw = format!("{player}\n\n{target}");
        match bench_winrate_summary(&raw, n, threads, eval_rq) {
            Ok(summary) => {
                if verbose {
                    let _ = writeln!(
                        verbose_buf,
                        "  [{}/{}] vs {} => {:.2}% ({}/{})",
                        index + 1,
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
                _accumulated_timing.merge(summary.timing);
                valid_matchups += 1;
            }
            Err(err) => {
                skipped_matchups += 1;
                if verbose {
                    let _ = writeln!(
                        verbose_buf,
                        "  [{}/{}] vs {} => ERROR: {err}",
                        index + 1,
                        target_groups.len(),
                        display_group(target)
                    );
                }
            }
        }
        tick_target(index, target);
    }

    let avg = if valid_matchups > 0 {
        accumulated_rate / valid_matchups as f64
    } else {
        0.0
    };
    BatchRateSummary {
        avg,
        wins: accumulated_wins,
        total: accumulated_total,
        valid_matchups,
        skipped_matchups,
    }
}

pub fn namer_pf_score(
    base_group: &[String],
    modifier: &str,
    duplicate: bool,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
) -> f64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }
    let summary = run_bench_score_inner(&target_group, modifier, n, threads, eval_rq);
    summary.wins as f64 * 10_000.0 / summary.total.max(1) as f64
}

fn bench_winrate_summary(raw: &str, n: usize, threads: Option<usize>, eval_rq: f64) -> Result<BenchSummary, String> {
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_string());
    let prepared = Runner::prepare_groups_with_eval_rq(&groups, eval_rq).map_err(|err| format!("{err}"))?;
    bench_prepared_summary(&prepared, n, threads, eval_rq)
}

fn bench_prepared_summary(
    prepared: &PreparedRunner,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
) -> Result<BenchSummary, String> {
    let thread = threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0);
    let summary = prepared_win_rate(prepared, n, eval_rq, thread).map_err(|err| format!("{err}"))?;
    Ok(BenchSummary {
        wins: summary.wins,
        total: summary.total,
        timing: summary.timing,
    })
}

fn run_bench_score_inner(
    target_group: &[String],
    modifier: &str,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
) -> BenchSummary {
    let workers = resolve_win_rate_workers(threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0), n);
    let (wins, total, timing) = if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        run_bench_score_range(target_group, modifier, 0, n, eval_rq)
    } else {
        let next = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            let target_group = target_group.to_vec();
            let modifier = modifier.to_string();
            let next = Arc::clone(&next);
            handles.push(std::thread::spawn(move || {
                run_bench_score_worker(&target_group, &modifier, &next, n, eval_rq)
            }));
        }
        let mut wins = 0usize;
        let mut total = 0usize;
        let mut timing = WinRateTiming::default();
        for handle in handles {
            let (part_wins, part_total, part_timing) = handle.join().expect("score worker thread panicked");
            wins += part_wins;
            total += part_total;
            timing.merge(part_timing);
        }
        (wins, total, timing)
    };
    BenchSummary { wins, total, timing }
}

fn run_bench_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        build_js_score_match_input(target_group, modifier, i, &mut bench_input);
        let t_init = Instant::now();
        let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, eval_rq) {
            Ok(runner) => runner,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();
        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(winners) = runner.world.winner.as_ref()
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

fn run_bench_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
    eval_rq: f64,
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
        let mut runner = match Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, eval_rq) {
            Ok(runner) => runner,
            Err(_) => continue,
        };
        let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
        timing.init_nanos += t_init.elapsed().as_nanos();
        let t_fight = Instant::now();
        runner.run_to_completion();
        timing.fight_nanos += t_fight.elapsed().as_nanos();
        total += 1;
        if let Some(winners) = runner.world.winner.as_ref()
            && winners.first().is_some_and(|winner| target_team.contains(winner))
        {
            wins += 1;
        }
    }
    (wins, total, timing)
}

fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, out: &mut String) {
    out.clear();
    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = tswn_core::engine::PROFILE_START as usize + round * profile_count;

    if target_group.len() == 1 {
        out.push_str(&target_group[0]);
        out.push('\n');
        let _ = write!(out, "{}@{modifier}", profile_base);
        out.push_str("\n\n");
        let _ = write!(out, "{}@{modifier}\n{}@{modifier}", profile_base + 1, profile_base + 2);
        return;
    }

    for (index, name) in target_group.iter().take(tracked_targets).enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(name);
    }
    out.push_str("\n\n");
    for offset in 0..profile_count {
        if offset > 0 {
            out.push('\n');
        }
        let _ = write!(out, "{}@{modifier}", profile_base + offset);
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

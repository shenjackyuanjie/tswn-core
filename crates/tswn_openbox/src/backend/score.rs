//! 基准测试摘要与胜率汇总。

use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};

use tswn_core::runtime::{RuntimeRunner, runtime_groups_win_rate_timed, runtime_score_timed};
use tswn_core::win_rate::WinRateTiming;

use super::format::display_group;
use super::parse::{first_duplicate_name_in_matchup, groups_have_same_players};

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

#[derive(Debug, Clone)]
pub enum BatchTargetOutcome {
    Rate,
    Skipped,
}

#[allow(clippy::too_many_arguments)]
pub fn bench_batch_rate_for_group(
    player: &str,
    target_groups: &[String],
    target_factors: Option<&[f64]>,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
    verbose: bool,
    verbose_buf: &mut String,
    cancel: &AtomicBool,
    mut tick_target: impl FnMut(usize, usize, &str, BatchTargetOutcome),
) -> BatchRateSummary {
    let mut accumulated_rate = 0.0;
    let mut accumulated_factor = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut valid_matchups = 0usize;
    let mut skipped_matchups = 0usize;

    for (index, target) in target_groups.iter().enumerate() {
        let target_total = target_groups.len();
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let factor = target_factors.map_or(1.0, |factors| factors[index]);
        if target_factors.is_some() && groups_have_same_players(player, target) {
            accumulated_rate += 50.0 * factor;
            accumulated_factor += factor;
            accumulated_wins += 1;
            accumulated_total += 2;
            valid_matchups += 1;
            tick_target(index, target_total, target, BatchTargetOutcome::Rate);
            continue;
        }
        if target_factors.is_none() && let Some(duplicate) = first_duplicate_name_in_matchup(&[player, target]) {
            skipped_matchups += 1;
            if verbose {
                let _ = writeln!(
                    verbose_buf,
                    "  [{}/{}] vs {} => SKIP duplicate name: {}",
                    index + 1,
                    target_total,
                    display_group(target),
                    duplicate
                );
            }
            tick_target(index, target_total, target, BatchTargetOutcome::Skipped);
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
                        target_total,
                        display_group(target),
                        summary.win_rate_percent(),
                        summary.wins,
                        summary.total
                    );
                }
                accumulated_rate += summary.win_rate_percent();
                accumulated_factor += factor;
                accumulated_wins += summary.wins;
                accumulated_total += summary.total;
                valid_matchups += 1;
                tick_target(index, target_total, target, BatchTargetOutcome::Rate);
            }
            Err(err) => {
                skipped_matchups += 1;
                if verbose {
                    let _ = writeln!(
                        verbose_buf,
                        "  [{}/{}] vs {} => ERROR: {err}",
                        index + 1,
                        target_total,
                        display_group(target)
                    );
                }
                tick_target(index, target_total, target, BatchTargetOutcome::Skipped);
            }
        }
    }

    BatchRateSummary {
        avg: if valid_matchups > 0 {
            if target_factors.is_some() { accumulated_rate / accumulated_factor.max(f64::MIN_POSITIVE) } else { accumulated_rate / valid_matchups as f64 }
        } else {
            0.0
        },
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
    let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
    let thread = threads.and_then(|value| u32::try_from(value).ok()).unwrap_or(0);
    runtime_groups_win_rate_timed(&groups, n, eval_rq, thread)
        .map(|summary| BenchSummary {
            wins: summary.wins,
            total: summary.total,
            timing: WinRateTiming {
                init_nanos: summary.timing.init_nanos,
                fight_nanos: summary.timing.fight_nanos,
            },
        })
        .map_err(|error| error.to_string())
}

fn run_bench_score_inner(
    target_group: &[String],
    modifier: &str,
    n: usize,
    threads: Option<usize>,
    eval_rq: f64,
) -> BenchSummary {
    let thread = threads.and_then(|value| u32::try_from(value).ok()).unwrap_or(0);
    match runtime_score_timed(target_group, modifier, n, eval_rq, thread) {
        Ok(summary) => BenchSummary {
            wins: summary.wins,
            total: summary.total,
            timing: WinRateTiming {
                init_nanos: summary.timing.init_nanos,
                fight_nanos: summary.timing.fight_nanos,
            },
        },
        Err(_) => BenchSummary {
            wins: 0,
            total: 0,
            timing: WinRateTiming::default(),
        },
    }
}

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use crate::Runner;
use crate::player::Player;
use crate::win_rate::{WinRateTiming, resolve_win_rate_workers};

use super::{BatchSummary, BenchSummary, CliApiResult, PairRateEntry, PairRateResult};

const BENCH_PARALLEL_THRESHOLD: usize = 64;

pub(super) fn run_score_inner(target_group: &[String], modifier: &str, n: usize, eval_rq: f64, thread: u32) -> BenchSummary {
    let workers = resolve_win_rate_workers(thread, n);
    if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        let (wins, total, errors, timing) = run_score_range(target_group, modifier, 0, n, eval_rq);
        return BenchSummary {
            wins,
            total,
            errors,
            timing,
        };
    }

    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let target_group = target_group.to_vec();
        let modifier = modifier.to_string();
        let next = Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_score_worker(&target_group, &modifier, next.as_ref(), n, eval_rq)
        }));
    }

    let mut merged = BenchSummary::default();
    for handle in handles {
        let (wins, total, errors, timing) = handle.join().expect("score worker thread panicked");
        merged.wins += wins;
        merged.total += total;
        merged.errors += errors;
        merged.timing.merge(timing);
    }
    merged
}

pub(super) fn namer_pf_score(base_group: &[String], modifier: &str, duplicate: bool, n: usize, thread: u32, eval_rq: f64) -> f64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }

    run_score_inner(&target_group, modifier, n, eval_rq, thread).score_10000()
}

pub(super) fn batch_rate_for_group(
    player: &str,
    target_groups: &[String],
    n: usize,
    thread: u32,
    eval_rq: f64,
) -> CliApiResult<BatchSummary> {
    let mut accumulated_rate = 0.0;
    let mut accumulated_wins = 0usize;
    let mut accumulated_total = 0usize;
    let mut accumulated_timing = WinRateTiming::default();
    let mut valid_matchups = 0usize;
    let mut skipped_matchups = 0usize;

    for target in target_groups {
        if first_duplicate_name_in_matchup(&[player, target.as_str()]).is_some() {
            skipped_matchups += 1;
            continue;
        }

        let raw = format!("{player}\n\n{target}");
        let summary = super::win_rate_summary(&raw, n, Some(eval_rq), thread)?;
        accumulated_rate += summary.win_rate;
        accumulated_wins += summary.wins;
        accumulated_total += summary.total;
        accumulated_timing.merge(WinRateTiming {
            init_nanos: summary.init_nanos,
            fight_nanos: summary.fight_nanos,
        });
        valid_matchups += 1;
    }

    let avg = if valid_matchups > 0 {
        accumulated_rate / valid_matchups as f64
    } else {
        0.0
    };
    let aggregate_rate = accumulated_wins as f64 * 100.0 / accumulated_total.max(1) as f64;
    Ok(BatchSummary {
        avg,
        aggregate_rate,
        wins: accumulated_wins,
        total: accumulated_total,
        timing: accumulated_timing,
        valid_matchups,
        skipped_matchups,
    })
}

pub(super) fn pair_rate_for_player(
    player: &str,
    target_groups: &[String],
    teammates: &[String],
    head: usize,
    n: usize,
    thread: u32,
    eval_rq: f64,
) -> CliApiResult<PairRateResult> {
    let converted_player = player_to_ol(player)?;
    let mut pair_rates = Vec::with_capacity(teammates.len());
    let mut total_wins = 0usize;
    let mut total_battles = 0usize;
    let mut total_valid_matchups = 0usize;
    let mut total_skipped_matchups = 0usize;
    let mut total_timing = WinRateTiming::default();

    for teammate in teammates {
        let pair_group = format!("{converted_player}\n{teammate}");
        let summary = batch_rate_for_group(&pair_group, target_groups, n, thread, eval_rq)?;
        if summary.valid_matchups > 0 {
            pair_rates.push(PairRateEntry {
                name: teammate.clone(),
                rate: summary.avg,
            });
        }
        total_wins += summary.wins;
        total_battles += summary.total;
        total_valid_matchups += summary.valid_matchups;
        total_skipped_matchups += summary.skipped_matchups;
        total_timing.merge(summary.timing);
    }

    pair_rates.sort_by(|a, b| b.rate.total_cmp(&a.rate));
    let selected = head.min(pair_rates.len());
    let final_score = pair_rates.iter().take(selected).map(|pair| pair.rate).sum::<f64>();
    let aggregate_win_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;

    Ok(PairRateResult {
        label: player.to_string(),
        final_score,
        head,
        selected,
        top_pairs: pair_rates.into_iter().take(selected).collect(),
        aggregate_win_rate,
        wins: total_wins,
        total: total_battles,
        valid_matchups: total_valid_matchups,
        skipped_matchups: total_skipped_matchups,
        init_nanos: total_timing.init_nanos,
        fight_nanos: total_timing.fight_nanos,
    })
}

fn run_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
    eval_rq: f64,
) -> (usize, usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut errors = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        run_score_round(target_group, modifier, i, eval_rq, &mut bench_input, &mut RoundAccum {
            wins: &mut wins,
            total: &mut total,
            errors: &mut errors,
            timing: &mut timing,
        });
    }

    (wins, total, errors, timing)
}

fn run_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> (usize, usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut errors = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        run_score_round(target_group, modifier, i, eval_rq, &mut bench_input, &mut RoundAccum {
            wins: &mut wins,
            total: &mut total,
            errors: &mut errors,
            timing: &mut timing,
        });
    }

    (wins, total, errors, timing)
}

struct RoundAccum<'a> {
    wins: &'a mut usize,
    total: &'a mut usize,
    errors: &'a mut usize,
    timing: &'a mut WinRateTiming,
}

fn run_score_round(
    target_group: &[String],
    modifier: &str,
    round: usize,
    eval_rq: f64,
    bench_input: &mut String,
    accum: &mut RoundAccum<'_>,
) {
    build_js_score_match_input(target_group, modifier, round, bench_input);

    let t_init = Instant::now();
    let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
    let Ok(mut runner) = Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, eval_rq) else {
        *accum.errors += 1;
        return;
    };
    let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
    accum.timing.init_nanos += t_init.elapsed().as_nanos();

    let t_fight = Instant::now();
    runner.run_to_completion();
    accum.timing.fight_nanos += t_fight.elapsed().as_nanos();
    *accum.total += 1;
    if let Some(ref winners) = runner.world.winner
        && winners.first().is_some_and(|winner| target_team.contains(winner))
    {
        *accum.wins += 1;
    }
}

// When the first two names in target_group are identical, namer-pf duplicate
// mode is active: one target, one profile per round (relying on +ol: cloning).
fn js_score_targets_per_round(target_group: &[String]) -> usize {
    if target_group.len() == 2 && target_group[0] == target_group[1] {
        1
    } else {
        target_group.len()
    }
}

// Duplicate mode: one profile. Single-player: three profiles (standard JS bench).
// Otherwise: one profile per distinct player in the group.
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
    let profile_base = crate::engine::PROFILE_START as usize + round * profile_count;

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

fn player_to_ol(raw: &str) -> CliApiResult<String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    super::parse::export_player(raw, false, false)
}

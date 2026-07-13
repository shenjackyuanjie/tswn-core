//! Runtime v2 批量评分与胜率执行层。
//!
//! 胜率路径复用 [`PreparedRuntimeV2Runner`]，每局只重建 seed 相关状态；评分路径的
//! profile 名字每轮都会变化，因此复用 registry 配置并按轮构造 roster。两条路径都
//! 使用不保留逐回合帧的 completion runner，避免 benchmark 热路径积累回放向量。

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use crate::win_rate::{WinRateTiming, resolve_win_rate_workers};

use super::{
    CustomRuntimeV2ImportError, DefaultCustomRuntimeV2ProfileError, PreparedRuntimeV2Runner, RuntimeV2Runner,
    default_custom_runtime_v2_import_config,
};

const BATCH_PARALLEL_THRESHOLD: usize = 100;
const BATCH_MAX_ROUNDS: usize = 100_000;

#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeV2BatchSummary {
    pub wins: usize,
    pub total: usize,
    pub errors: usize,
    pub guard_exhausted: usize,
    pub timing: WinRateTiming,
}

impl RuntimeV2BatchSummary {
    pub fn win_rate_percent(self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }

    pub fn score_10000(self) -> f64 { self.wins as f64 * 10_000.0 / self.total.max(1) as f64 }

    pub fn merge(&mut self, other: Self) {
        self.wins += other.wins;
        self.total += other.total;
        self.errors += other.errors;
        self.guard_exhausted += other.guard_exhausted;
        self.timing.merge(other.timing);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeV2BatchError {
    DefaultProfile(DefaultCustomRuntimeV2ProfileError),
    Import(CustomRuntimeV2ImportError),
}

impl std::fmt::Display for RuntimeV2BatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefaultProfile(error) => write!(f, "runtime v2 default profile failed: {error:?}"),
            Self::Import(error) => write!(f, "runtime v2 batch import failed: {error:?}"),
        }
    }
}

impl std::error::Error for RuntimeV2BatchError {}

impl From<DefaultCustomRuntimeV2ProfileError> for RuntimeV2BatchError {
    fn from(error: DefaultCustomRuntimeV2ProfileError) -> Self { Self::DefaultProfile(error) }
}

impl From<CustomRuntimeV2ImportError> for RuntimeV2BatchError {
    fn from(error: CustomRuntimeV2ImportError) -> Self { Self::Import(error) }
}

pub fn runtime_v2_groups_win_rate(
    groups: &[Vec<String>],
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeV2BatchSummary, RuntimeV2BatchError> {
    let config = default_custom_runtime_v2_import_config()?;
    let prepared = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(groups, eval_rq, config)?;
    prepared_runtime_v2_win_rate(&prepared, n, thread)
}

pub fn prepared_runtime_v2_win_rate(
    prepared: &PreparedRuntimeV2Runner,
    n: usize,
    thread: u32,
) -> Result<RuntimeV2BatchSummary, RuntimeV2BatchError> {
    let workers = resolve_win_rate_workers(thread, n);
    if workers <= 1 || n < BATCH_PARALLEL_THRESHOLD {
        return run_prepared_range(prepared, 0, n).map_err(Into::into);
    }

    let prepared = Arc::new(prepared.clone());
    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let prepared = Arc::clone(&prepared);
        let next = Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_prepared_worker(prepared.as_ref(), next.as_ref(), n)
        }));
    }

    let mut merged = RuntimeV2BatchSummary::default();
    for handle in handles {
        let part = handle.join().expect("runtime v2 win-rate worker thread panicked")?;
        merged.merge(part);
    }
    Ok(merged)
}

/// 对已经准备好的 Runtime v2 对局执行指定轮次区间。
pub fn prepared_runtime_v2_win_rate_range(
    prepared: &PreparedRuntimeV2Runner,
    start: usize,
    end: usize,
) -> Result<RuntimeV2BatchSummary, RuntimeV2BatchError> {
    run_prepared_range(prepared, start, end).map_err(Into::into)
}

pub fn runtime_v2_score(
    target_group: &[String],
    modifier: &str,
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeV2BatchSummary, RuntimeV2BatchError> {
    let first_groups = ScoreMatchGroups::new(target_group, modifier).groups;
    let config = default_custom_runtime_v2_import_config()?;
    let prepared = Arc::new(PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(
        &first_groups,
        eval_rq,
        config,
    )?);
    let workers = resolve_win_rate_workers(thread, n);
    if workers <= 1 || n < BATCH_PARALLEL_THRESHOLD {
        return Ok(run_score_range(target_group, modifier, 0, n, eval_rq, prepared.as_ref()));
    }

    let next = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let target_group = target_group.to_vec();
        let modifier = modifier.to_owned();
        let next = Arc::clone(&next);
        let prepared = Arc::clone(&prepared);
        handles.push(std::thread::spawn(move || {
            run_score_worker(&target_group, &modifier, next.as_ref(), n, eval_rq, prepared.as_ref())
        }));
    }

    let mut merged = RuntimeV2BatchSummary::default();
    for handle in handles {
        merged.merge(handle.join().expect("runtime v2 score worker thread panicked"));
    }
    Ok(merged)
}

/// 使用与完整评分相同的轮次编号执行一个评分区间。
pub fn runtime_v2_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> Result<RuntimeV2BatchSummary, RuntimeV2BatchError> {
    let first_groups = ScoreMatchGroups::new(target_group, modifier).groups;
    let config = default_custom_runtime_v2_import_config()?;
    let prepared = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(&first_groups, eval_rq, config)?;
    Ok(run_score_range(target_group, modifier, start, end, eval_rq, &prepared))
}

fn run_prepared_range(
    prepared: &PreparedRuntimeV2Runner,
    start: usize,
    end: usize,
) -> Result<RuntimeV2BatchSummary, CustomRuntimeV2ImportError> {
    let mut summary = RuntimeV2BatchSummary::default();
    let mut seed = String::with_capacity(24);
    let mut runner = prepared.new_reusable_runner();
    for round in start..end {
        run_prepared_round(prepared, &mut runner, profile_seed_for_round(&mut seed, round), &mut summary)?;
    }
    Ok(summary)
}

fn run_prepared_worker(
    prepared: &PreparedRuntimeV2Runner,
    next: &AtomicUsize,
    end: usize,
) -> Result<RuntimeV2BatchSummary, RuntimeV2BatchError> {
    let mut summary = RuntimeV2BatchSummary::default();
    let mut seed = String::with_capacity(24);
    let mut runner = prepared.new_reusable_runner();
    loop {
        let round = next.fetch_add(1, Ordering::Relaxed);
        if round >= end {
            break;
        }
        run_prepared_round(prepared, &mut runner, profile_seed_for_round(&mut seed, round), &mut summary)?;
    }
    Ok(summary)
}

fn run_prepared_round(
    prepared: &PreparedRuntimeV2Runner,
    runner: &mut RuntimeV2Runner,
    seed: &[String],
    summary: &mut RuntimeV2BatchSummary,
) -> Result<(), CustomRuntimeV2ImportError> {
    let init_started = Instant::now();
    prepared.reset_with_seed(runner, seed)?;
    summary.timing.init_nanos += init_started.elapsed().as_nanos();

    let fight_started = Instant::now();
    let completion = runner.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
    summary.timing.fight_nanos += fight_started.elapsed().as_nanos();
    summary.total += 1;
    summary.guard_exhausted += usize::from(completion.guard_exhausted);
    summary.wins += usize::from(runner.input_group_won(0));
    Ok(())
}

fn profile_seed_for_round(seed: &mut String, round: usize) -> &[String] {
    if round == 0 {
        &[]
    } else {
        seed.clear();
        let _ = write!(seed, "seed:{}@!", crate::engine::PROFILE_START as usize + round);
        std::slice::from_ref(seed)
    }
}

fn run_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
    prepared: &PreparedRuntimeV2Runner,
) -> RuntimeV2BatchSummary {
    let mut summary = RuntimeV2BatchSummary::default();
    let mut match_groups = ScoreMatchGroups::new(target_group, modifier);
    let mut runner = prepared.new_reusable_runner();
    for round in start..end {
        run_score_round(round, eval_rq, prepared, &mut runner, &mut match_groups, &mut summary);
    }
    summary
}

fn run_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
    eval_rq: f64,
    prepared: &PreparedRuntimeV2Runner,
) -> RuntimeV2BatchSummary {
    let mut summary = RuntimeV2BatchSummary::default();
    let mut match_groups = ScoreMatchGroups::new(target_group, modifier);
    let mut runner = prepared.new_reusable_runner();
    loop {
        let round = next.fetch_add(1, Ordering::Relaxed);
        if round >= end {
            break;
        }
        run_score_round(round, eval_rq, prepared, &mut runner, &mut match_groups, &mut summary);
    }
    summary
}

fn run_score_round(
    round: usize,
    eval_rq: f64,
    prepared: &PreparedRuntimeV2Runner,
    runner: &mut RuntimeV2Runner,
    match_groups: &mut ScoreMatchGroups,
    summary: &mut RuntimeV2BatchSummary,
) {
    match_groups.set_round(round);
    let init_started = Instant::now();
    if prepared
        .reset_from_groups_with_seed_and_eval_rq(runner, &match_groups.groups, &[], eval_rq)
        .is_err()
    {
        summary.errors += 1;
        return;
    }
    summary.timing.init_nanos += init_started.elapsed().as_nanos();

    let fight_started = Instant::now();
    let completion = runner.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
    summary.timing.fight_nanos += fight_started.elapsed().as_nanos();
    summary.total += 1;
    summary.guard_exhausted += usize::from(completion.guard_exhausted);
    summary.wins += usize::from(runner.input_group_won(0));
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

struct ScoreMatchGroups {
    groups: Vec<Vec<String>>,
    profile_slots: Vec<(usize, usize)>,
    modifier: String,
}

impl ScoreMatchGroups {
    fn new(target_group: &[String], modifier: &str) -> Self {
        let tracked_targets = js_score_targets_per_round(target_group);
        let profile_count = js_score_profiles_per_round(target_group);
        let mut groups = Vec::with_capacity(2);
        let mut profile_slots = Vec::with_capacity(profile_count);

        if !target_group.is_empty() {
            let mut target_team = target_group.iter().take(tracked_targets).cloned().collect::<Vec<_>>();
            if target_group.len() == 1 {
                target_team.push(String::with_capacity(modifier.len() + 24));
                profile_slots.push((0, target_team.len() - 1));
            }
            groups.push(target_team);

            let mut profile_team = Vec::with_capacity(profile_count - usize::from(target_group.len() == 1));
            for _ in profile_slots.len()..profile_count {
                profile_team.push(String::with_capacity(modifier.len() + 24));
                profile_slots.push((1, profile_team.len() - 1));
            }
            if !profile_team.is_empty() {
                groups.push(profile_team);
            }
        }

        let mut value = Self {
            groups,
            profile_slots,
            modifier: modifier.to_owned(),
        };
        value.set_round(0);
        value
    }

    fn set_round(&mut self, round: usize) {
        let profile_base = crate::engine::PROFILE_START as usize + round * self.profile_slots.len();
        for (offset, &(group, player)) in self.profile_slots.iter().enumerate() {
            let profile = &mut self.groups[group][player];
            profile.clear();
            let _ = write!(profile, "{}@{}", profile_base + offset, self.modifier);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_v2_prepared_win_rate_matches_legacy_seed_schedule() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let legacy = crate::win_rate::groups_win_rate(&groups, 24, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("legacy win rate should run");
        let v2 = runtime_v2_groups_win_rate(&groups, 24, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("runtime v2 win rate should run");

        assert_eq!(
            (v2.wins, v2.total, v2.errors, v2.guard_exhausted),
            (legacy.wins, legacy.total, 0, 0)
        );
    }

    #[test]
    fn runtime_v2_score_matches_legacy_profile_rounds() {
        let target_group = vec!["mario".to_owned()];
        let legacy = crate::cli_api::score("mario", 200, "normal", Some(crate::player::eval_name::WIN_RATE_EVAL_RQ), 1)
            .expect("legacy score should run");
        let v2 = runtime_v2_score(&target_group, "\u{0002}", 200, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("runtime v2 score should run");

        assert_eq!(
            (v2.wins, v2.total, v2.errors, v2.guard_exhausted),
            (legacy.wins, legacy.total, legacy.errors, 0)
        );
    }

    #[test]
    fn runtime_v2_parallel_score_and_win_rate_keep_deterministic_totals() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let legacy_win_rate = crate::win_rate::groups_win_rate(&groups, 128, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("legacy win rate should run");
        let v2_win_rate = runtime_v2_groups_win_rate(&groups, 128, crate::player::eval_name::WIN_RATE_EVAL_RQ, 4)
            .expect("parallel runtime v2 win rate should run");
        assert_eq!(
            (v2_win_rate.wins, v2_win_rate.total),
            (legacy_win_rate.wins, legacy_win_rate.total)
        );

        let target_group = vec!["mario".to_owned()];
        let legacy_score = crate::cli_api::score("mario", 128, "bang", Some(crate::player::eval_name::WIN_RATE_EVAL_RQ), 1)
            .expect("legacy score should run");
        let v2_score = runtime_v2_score(&target_group, "!", 128, crate::player::eval_name::WIN_RATE_EVAL_RQ, 4)
            .expect("parallel runtime v2 score should run");
        assert_eq!(
            (v2_score.wins, v2_score.total, v2_score.errors, v2_score.guard_exhausted),
            (legacy_score.wins, legacy_score.total, legacy_score.errors, 0)
        );
    }

    #[test]
    #[ignore = "手动定位批量评分的首个胜负分叉"]
    fn runtime_v2_score_first_winner_divergence() {
        let target_group = vec!["mario".to_owned()];
        let modifier = "!";
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut match_groups = ScoreMatchGroups::new(&target_group, modifier);
        let config = default_custom_runtime_v2_import_config().expect("默认 Runtime v2 配置应构建成功");
        let prepared = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(&match_groups.groups, eval_rq, config)
            .expect("Runtime v2 评分模板应构建成功");
        let mut v2 = prepared.new_reusable_runner();

        for round in 0..128 {
            match_groups.set_round(round);
            let mut legacy = crate::Runner::new_from_groups_with_seed_and_eval_rq_uncached(&match_groups.groups, &[], eval_rq)
                .expect("legacy 评分对局应构建成功");
            let target_team = legacy.input_groups[0].clone();
            legacy.run_to_completion();
            let legacy_won = legacy
                .world
                .winner
                .as_ref()
                .and_then(|winners| winners.first())
                .is_some_and(|winner| target_team.contains(winner));

            prepared
                .reset_from_groups_with_seed_and_eval_rq(&mut v2, &match_groups.groups, &[], eval_rq)
                .expect("Runtime v2 评分对局应复位成功");
            v2.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
            let v2_won = v2.input_group_won(0);
            if legacy_won != v2_won {
                panic!(
                    "评分首个胜负分叉：round={round} legacy={legacy_won} v2={v2_won} groups={:?}",
                    match_groups.groups
                );
            }
        }
    }

    #[test]
    fn runtime_v2_win_rate_charm_exchange_seed_matches_legacy() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let seed = vec!["seed:33554642@!".to_owned()];
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut legacy = crate::Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, eval_rq)
            .expect("legacy charm/exchange fixture should initialize");
        let expected = crate::runtime_v2::normalize_legacy_run(&mut legacy, BATCH_MAX_ROUNDS);

        let config = default_custom_runtime_v2_import_config().expect("runtime v2 profile should build");
        let mut v2 = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(&groups, eval_rq, config)
            .expect("runtime v2 charm/exchange fixture should initialize")
            .new_with_seed(&seed)
            .expect("runtime v2 charm/exchange seed should apply");
        let actual = v2.run_until_winner_normalized_rounds(BATCH_MAX_ROUNDS);

        assert_eq!(crate::runtime_v2::strict_diff_runs(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_v2_score_reflected_ice_fixture_matches_legacy() {
        let modifier = "\u{0002}";
        let profile_base = crate::engine::PROFILE_START as usize + 191 * 3;
        let groups = vec![
            vec!["mario".to_owned(), format!("{profile_base}@{modifier}")],
            vec![
                format!("{}@{modifier}", profile_base + 1),
                format!("{}@{modifier}", profile_base + 2),
            ],
        ];
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut legacy = crate::Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &[], eval_rq)
            .expect("legacy reflected-ice fixture should initialize");
        let expected = crate::runtime_v2::normalize_legacy_run(&mut legacy, 3);

        let config = default_custom_runtime_v2_import_config().expect("runtime v2 profile should build");
        let mut v2 = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(&groups, eval_rq, config)
            .expect("runtime v2 reflected-ice fixture should initialize")
            .new_with_seed(&[])
            .expect("runtime v2 reflected-ice fixture seed should apply");
        let actual = v2.run_until_winner_normalized_rounds(3);

        assert_eq!(crate::runtime_v2::strict_diff_runs(&expected, &actual), Ok(()));
    }

    #[test]
    fn score_match_builder_keeps_js_single_target_shape() {
        let mut groups = ScoreMatchGroups::new(&["mario".to_owned()], "!");
        groups.set_round(2);
        let base = crate::engine::PROFILE_START as usize + 6;
        assert_eq!(
            groups.groups,
            vec![
                vec!["mario".to_owned(), format!("{base}@!")],
                vec![format!("{}@!", base + 1), format!("{}@!", base + 2)]
            ]
        );
    }

    #[test]
    #[ignore = "manual runtime-v2/legacy batch timing probe"]
    fn runtime_v2_batch_perf_probe() {
        let n = std::env::var("TSWN_BATCH_PERF_N")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1_000);
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let target_group = vec!["mario".to_owned()];

        let started = Instant::now();
        let v2_score = runtime_v2_score(&target_group, "\u{0002}", n, eval_rq, 1).expect("runtime v2 score should run");
        let v2_score_wall = started.elapsed();
        let started = Instant::now();
        let legacy_score = crate::cli_api::score("mario", n, "normal", Some(eval_rq), 1).expect("legacy score should run");
        let legacy_score_wall = started.elapsed();

        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let started = Instant::now();
        let v2_rate = runtime_v2_groups_win_rate(&groups, n, eval_rq, 1).expect("runtime v2 win rate should run");
        let v2_rate_wall = started.elapsed();
        let started = Instant::now();
        let legacy_rate = crate::win_rate::groups_win_rate(&groups, n, eval_rq, 1).expect("legacy win rate should run");
        let legacy_rate_wall = started.elapsed();

        eprintln!(
            "score n={n}: v2 wall={v2_score_wall:?} init={}us fight={}us; legacy wall={legacy_score_wall:?} init={}us fight={}us",
            v2_score.timing.init_nanos / 1_000,
            v2_score.timing.fight_nanos / 1_000,
            legacy_score.init_nanos / 1_000,
            legacy_score.fight_nanos / 1_000,
        );
        eprintln!(
            "win-rate n={n}: v2 wall={v2_rate_wall:?} init={}us fight={}us; legacy wall={legacy_rate_wall:?} init={}us fight={}us",
            v2_rate.timing.init_nanos / 1_000,
            v2_rate.timing.fight_nanos / 1_000,
            legacy_rate.timing.init_nanos / 1_000,
            legacy_rate.timing.fight_nanos / 1_000,
        );

        assert_eq!((v2_score.wins, v2_score.total), (legacy_score.wins, legacy_score.total));
        assert_eq!((v2_rate.wins, v2_rate.total), (legacy_rate.wins, legacy_rate.total));
    }
}

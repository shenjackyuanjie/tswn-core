//! Runtime 批量评分与胜率执行层。
//!
//! 胜率路径复用 [`PreparedRuntimeRunner`]，每局只重建 seed 相关状态；评分路径的
//! profile 名字每轮都会变化，因此复用 registry 配置并按轮构造 roster。两条路径都
//! 使用不保留逐回合帧的 completion runner，避免 benchmark 热路径积累回放向量。

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use crate::win_rate::{WinRateTiming, resolve_win_rate_workers};

use super::{
    CustomRuntimeImportError, DefaultCustomRuntimeProfileError, PreparedRuntimeRunner, RuntimeRunner, ScoreIdentityBuffer,
    ScoreRosterBuffers, ScoreRoundScratch, SkillLoadout, default_custom_runtime_import_config,
};

const BATCH_PARALLEL_THRESHOLD: usize = 100;
const BATCH_MAX_ROUNDS: usize = 100_000;

#[derive(Debug, Clone, Copy, Default)]
pub struct RuntimeBatchSummary {
    pub wins: usize,
    pub total: usize,
    pub errors: usize,
    pub guard_exhausted: usize,
    pub timing: WinRateTiming,
}

impl RuntimeBatchSummary {
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
pub enum RuntimeBatchError {
    DefaultProfile(DefaultCustomRuntimeProfileError),
    Import(CustomRuntimeImportError),
}

impl std::fmt::Display for RuntimeBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DefaultProfile(error) => write!(f, "runtime default profile failed: {error:?}"),
            Self::Import(error) => write!(f, "runtime batch import failed: {error:?}"),
        }
    }
}

impl std::error::Error for RuntimeBatchError {}

impl From<DefaultCustomRuntimeProfileError> for RuntimeBatchError {
    fn from(error: DefaultCustomRuntimeProfileError) -> Self { Self::DefaultProfile(error) }
}

impl From<CustomRuntimeImportError> for RuntimeBatchError {
    fn from(error: CustomRuntimeImportError) -> Self { Self::Import(error) }
}

pub fn runtime_groups_win_rate(
    groups: &[Vec<String>],
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let config = default_custom_runtime_import_config()?;
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(groups, eval_rq, config)?;
    prepared_runtime_win_rate(&prepared, n, thread)
}

pub fn prepared_runtime_win_rate(
    prepared: &PreparedRuntimeRunner,
    n: usize,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
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

    let mut merged = RuntimeBatchSummary::default();
    for handle in handles {
        let part = handle.join().expect("runtime win-rate worker thread panicked")?;
        merged.merge(part);
    }
    Ok(merged)
}

/// 对已经准备好的 Runtime 对局执行指定轮次区间。
pub fn prepared_runtime_win_rate_range(
    prepared: &PreparedRuntimeRunner,
    start: usize,
    end: usize,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    run_prepared_range(prepared, start, end).map_err(Into::into)
}

pub fn runtime_score(
    target_group: &[String],
    modifier: &str,
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let first_groups = ScoreMatchGroups::new(target_group, modifier).groups;
    let config = default_custom_runtime_import_config()?;
    let prepared = Arc::new(PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(
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

    let mut merged = RuntimeBatchSummary::default();
    for handle in handles {
        merged.merge(handle.join().expect("runtime score worker thread panicked"));
    }
    Ok(merged)
}

/// 使用与完整评分相同的轮次编号执行一个评分区间。
pub fn runtime_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let first_groups = ScoreMatchGroups::new(target_group, modifier).groups;
    let config = default_custom_runtime_import_config()?;
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&first_groups, eval_rq, config)?;
    Ok(run_score_range(target_group, modifier, start, end, eval_rq, &prepared))
}

fn run_prepared_range(
    prepared: &PreparedRuntimeRunner,
    start: usize,
    end: usize,
) -> Result<RuntimeBatchSummary, CustomRuntimeImportError> {
    let mut summary = RuntimeBatchSummary::default();
    let mut seed = String::with_capacity(24);
    let mut runner = prepared.new_reusable_runner();
    for round in start..end {
        run_prepared_round(prepared, &mut runner, profile_seed_for_round(&mut seed, round), &mut summary)?;
    }
    Ok(summary)
}

fn run_prepared_worker(
    prepared: &PreparedRuntimeRunner,
    next: &AtomicUsize,
    end: usize,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let mut summary = RuntimeBatchSummary::default();
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
    prepared: &PreparedRuntimeRunner,
    runner: &mut RuntimeRunner,
    seed: &[String],
    summary: &mut RuntimeBatchSummary,
) -> Result<(), CustomRuntimeImportError> {
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
    prepared: &PreparedRuntimeRunner,
) -> RuntimeBatchSummary {
    let mut summary = RuntimeBatchSummary::default();
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
    prepared: &PreparedRuntimeRunner,
) -> RuntimeBatchSummary {
    let mut summary = RuntimeBatchSummary::default();
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
    prepared: &PreparedRuntimeRunner,
    runner: &mut RuntimeRunner,
    match_groups: &mut ScoreMatchGroups,
    summary: &mut RuntimeBatchSummary,
) {
    match_groups.set_round(round);
    let init_started = Instant::now();
    if prepared
        .reset_score_groups_with_seed_and_eval_rq(
            runner,
            &match_groups.groups,
            &match_groups.profile_player_ids,
            &match_groups.modifier,
            &match_groups.profile_team_rng,
            &mut match_groups.skill_buffers,
            &mut match_groups.identity_buffers,
            &mut match_groups.round_scratch,
            &mut match_groups.roster_buffers,
            &[],
            eval_rq,
        )
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
    profile_player_ids: Vec<crate::player::PlrId>,
    modifier: String,
    profile_team_rng: crate::rc4::RC4,
    skill_buffers: Vec<SkillLoadout>,
    identity_buffers: Vec<ScoreIdentityBuffer>,
    round_scratch: ScoreRoundScratch,
    roster_buffers: ScoreRosterBuffers,
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

        let mut group_offsets = Vec::with_capacity(groups.len());
        let mut offset = 0usize;
        for group in &groups {
            group_offsets.push(offset);
            offset += group.len();
        }
        let profile_player_ids = profile_slots.iter().map(|&(group, player)| group_offsets[group] + player).collect();

        let mut value = Self {
            groups,
            profile_slots,
            profile_player_ids,
            modifier: modifier.to_owned(),
            profile_team_rng: if modifier.len() <= crate::player::TEAM_MAX_LEN {
                crate::player::Player::score_profile_team_rng(modifier)
            } else {
                // 非法的超长 modifier 会在完整玩家构造路径返回原有错误；这里不能提前 panic。
                crate::rc4::RC4::default()
            },
            skill_buffers: vec![SkillLoadout::default(); profile_count],
            identity_buffers: (0..profile_count).map(|_| ScoreIdentityBuffer::default()).collect(),
            round_scratch: ScoreRoundScratch::default(),
            roster_buffers: ScoreRosterBuffers::default(),
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
    fn runtime_prepared_win_rate_matches_legacy_seed_schedule() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let legacy = crate::win_rate::groups_win_rate(&groups, 24, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("legacy win rate should run");
        let runtime = runtime_groups_win_rate(&groups, 24, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("runtime win rate should run");

        assert_eq!(
            (runtime.wins, runtime.total, runtime.errors, runtime.guard_exhausted),
            (legacy.wins, legacy.total, 0, 0)
        );
    }

    #[test]
    fn reusable_runner_reset_matches_fresh_runner_after_mutating_fights() {
        let groups = vec![
            vec!["Don't_Force_It #f4fMecHe1@Shabby_fish".to_owned()],
            vec!["涵虚不等式 PFVKEUPBU@TigerStar".to_owned()],
        ];
        let config = default_custom_runtime_import_config().expect("默认 Runtime 配置应构建成功");
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(
            &groups,
            crate::player::eval_name::DEFAULT_EVAL_RQ,
            config,
        )
        .expect("CQP 对局应准备成功");
        let mut reusable = prepared.new_reusable_runner();
        let mut seed = String::with_capacity(24);

        for round in 0..200 {
            let seed = profile_seed_for_round(&mut seed, round);
            prepared.reset_with_seed(&mut reusable, seed).expect("复用 runner 应复位成功");
            let reused = reusable.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);

            let mut fresh = prepared.new_with_seed(seed).expect("全新 runner 应构建成功");
            let expected = fresh.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
            assert_eq!(
                (reused.winner_team, reusable.input_group_won(0), reused.guard_exhausted),
                (expected.winner_team, fresh.input_group_won(0), expected.guard_exhausted),
                "round={round}"
            );
        }
    }

    #[test]
    fn runtime_score_matches_legacy_profile_rounds() {
        let target_group = vec!["mario".to_owned()];
        let legacy = crate::cli_api::score("mario", 200, "normal", Some(crate::player::eval_name::WIN_RATE_EVAL_RQ), 1)
            .expect("legacy score should run");
        let runtime = runtime_score(&target_group, "\u{0002}", 200, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("runtime score should run");

        assert_eq!(
            (runtime.wins, runtime.total, runtime.errors, runtime.guard_exhausted),
            (legacy.wins, legacy.total, legacy.errors, 0)
        );
    }

    #[test]
    fn runtime_lazy_score_round_42_matches_legacy() {
        let target_group = vec!["mario".to_owned()];
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut match_groups = ScoreMatchGroups::new(&target_group, "\u{0002}");
        let first_groups = match_groups.groups.clone();
        match_groups.set_round(42);

        let mut legacy = crate::LegacyRunner::new_from_groups_with_seed_and_eval_rq_uncached(&match_groups.groups, &[], eval_rq)
            .expect("legacy score round should initialize");
        let expected = crate::runtime::normalize_legacy_run(&mut legacy, BATCH_MAX_ROUNDS);

        let config = default_custom_runtime_import_config().expect("runtime profile should build");
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&first_groups, eval_rq, config)
            .expect("runtime score template should initialize");
        let mut actual = prepared.new_reusable_runner();
        prepared
            .reset_score_groups_with_seed_and_eval_rq(
                &mut actual,
                &match_groups.groups,
                &match_groups.profile_player_ids,
                &match_groups.modifier,
                &match_groups.profile_team_rng,
                &mut match_groups.skill_buffers,
                &mut match_groups.identity_buffers,
                &mut match_groups.round_scratch,
                &mut match_groups.roster_buffers,
                &[],
                eval_rq,
            )
            .expect("runtime lazy score round should initialize");
        let actual = actual.run_until_winner_normalized_rounds(BATCH_MAX_ROUNDS);

        assert_eq!(crate::runtime::strict_diff_runs(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_parallel_score_and_win_rate_keep_deterministic_totals() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let legacy_win_rate = crate::win_rate::groups_win_rate(&groups, 128, crate::player::eval_name::WIN_RATE_EVAL_RQ, 1)
            .expect("legacy win rate should run");
        let runtime_win_rate = runtime_groups_win_rate(&groups, 128, crate::player::eval_name::WIN_RATE_EVAL_RQ, 4)
            .expect("parallel runtime win rate should run");
        assert_eq!(
            (runtime_win_rate.wins, runtime_win_rate.total),
            (legacy_win_rate.wins, legacy_win_rate.total)
        );

        let target_group = vec!["mario".to_owned()];
        let legacy_score = crate::cli_api::score("mario", 128, "bang", Some(crate::player::eval_name::WIN_RATE_EVAL_RQ), 1)
            .expect("legacy score should run");
        let runtime_score = runtime_score(&target_group, "!", 128, crate::player::eval_name::WIN_RATE_EVAL_RQ, 4)
            .expect("parallel runtime score should run");
        assert_eq!(
            (
                runtime_score.wins,
                runtime_score.total,
                runtime_score.errors,
                runtime_score.guard_exhausted
            ),
            (legacy_score.wins, legacy_score.total, legacy_score.errors, 0)
        );
    }

    #[test]
    #[ignore = "手动定位批量评分的首个胜负分叉"]
    fn runtime_score_first_winner_divergence() {
        let target_group = vec!["mario".to_owned()];
        let modifier = "!";
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut match_groups = ScoreMatchGroups::new(&target_group, modifier);
        let config = default_custom_runtime_import_config().expect("默认 Runtime 配置应构建成功");
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&match_groups.groups, eval_rq, config)
            .expect("Runtime 评分模板应构建成功");
        let mut runtime = prepared.new_reusable_runner();

        for round in 0..128 {
            match_groups.set_round(round);
            let mut legacy =
                crate::LegacyRunner::new_from_groups_with_seed_and_eval_rq_uncached(&match_groups.groups, &[], eval_rq)
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
                .reset_from_groups_with_seed_and_eval_rq(&mut runtime, &match_groups.groups, &[], eval_rq)
                .expect("Runtime 评分对局应复位成功");
            runtime.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
            let runtime_won = runtime.input_group_won(0);
            if legacy_won != runtime_won {
                panic!(
                    "评分首个胜负分叉：round={round} legacy={legacy_won} runtime={runtime_won} groups={:?}",
                    match_groups.groups
                );
            }
        }
    }

    #[test]
    fn runtime_win_rate_charm_exchange_seed_matches_legacy() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let seed = vec!["seed:33554642@!".to_owned()];
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut legacy = crate::LegacyRunner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, eval_rq)
            .expect("legacy charm/exchange fixture should initialize");
        let expected = crate::runtime::normalize_legacy_run(&mut legacy, BATCH_MAX_ROUNDS);

        let config = default_custom_runtime_import_config().expect("runtime profile should build");
        let mut runtime = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&groups, eval_rq, config)
            .expect("runtime charm/exchange fixture should initialize")
            .new_with_seed(&seed)
            .expect("runtime charm/exchange seed should apply");
        let actual = runtime.run_until_winner_normalized_rounds(BATCH_MAX_ROUNDS);

        assert_eq!(crate::runtime::strict_diff_runs(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_score_reflected_ice_fixture_matches_legacy() {
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
        let mut legacy = crate::LegacyRunner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &[], eval_rq)
            .expect("legacy reflected-ice fixture should initialize");
        let expected = crate::runtime::normalize_legacy_run(&mut legacy, 3);

        let config = default_custom_runtime_import_config().expect("runtime profile should build");
        let mut runtime = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&groups, eval_rq, config)
            .expect("runtime reflected-ice fixture should initialize")
            .new_with_seed(&[])
            .expect("runtime reflected-ice fixture seed should apply");
        let actual = runtime.run_until_winner_normalized_rounds(3);

        assert_eq!(crate::runtime::strict_diff_runs(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_score_shabby_fish_round_185_matches_legacy() {
        let modifier = "\u{0002}";
        let profile_base = crate::engine::PROFILE_START as usize + 185 * 3;
        let groups = vec![
            vec!["11 #CxersT6Za@Shabby_fish".to_owned(), format!("{profile_base}@{modifier}")],
            vec![
                format!("{}@{modifier}", profile_base + 1),
                format!("{}@{modifier}", profile_base + 2),
            ],
        ];
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let mut legacy = crate::LegacyRunner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &[], eval_rq)
            .expect("legacy Shabby_fish 评分用例应初始化成功");
        let expected = crate::runtime::normalize_legacy_run(&mut legacy, BATCH_MAX_ROUNDS);

        let config = default_custom_runtime_import_config().expect("Runtime 默认配置应构建成功");
        let mut runtime = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&groups, eval_rq, config)
            .expect("Runtime Shabby_fish 评分用例应初始化成功")
            .new_with_seed(&[])
            .expect("Runtime Shabby_fish 评分用例 seed 应应用成功");
        let actual = runtime.run_until_winner_normalized_rounds(BATCH_MAX_ROUNDS);

        assert_eq!(crate::runtime::strict_diff_runs(&expected, &actual), Ok(()));
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
    #[ignore = "manual runtime/legacy batch timing probe"]
    fn runtime_batch_perf_probe() {
        let n = std::env::var("TSWN_BATCH_PERF_N")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1_000);
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let target_group = vec!["mario".to_owned()];

        let started = Instant::now();
        let runtime_score = runtime_score(&target_group, "\u{0002}", n, eval_rq, 1).expect("runtime score should run");
        let runtime_score_wall = started.elapsed();
        let started = Instant::now();
        let legacy_score = crate::cli_api::score("mario", n, "normal", Some(eval_rq), 1).expect("legacy score should run");
        let legacy_score_wall = started.elapsed();

        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let started = Instant::now();
        let runtime_rate = runtime_groups_win_rate(&groups, n, eval_rq, 1).expect("runtime win rate should run");
        let runtime_rate_wall = started.elapsed();
        let started = Instant::now();
        let legacy_rate = crate::win_rate::groups_win_rate(&groups, n, eval_rq, 1).expect("legacy win rate should run");
        let legacy_rate_wall = started.elapsed();

        eprintln!(
            "score n={n}: runtime wall={runtime_score_wall:?} init={}us fight={}us; legacy wall={legacy_score_wall:?} init={}us fight={}us",
            runtime_score.timing.init_nanos / 1_000,
            runtime_score.timing.fight_nanos / 1_000,
            legacy_score.init_nanos / 1_000,
            legacy_score.fight_nanos / 1_000,
        );
        eprintln!(
            "win-rate n={n}: runtime wall={runtime_rate_wall:?} init={}us fight={}us; legacy wall={legacy_rate_wall:?} init={}us fight={}us",
            runtime_rate.timing.init_nanos / 1_000,
            runtime_rate.timing.fight_nanos / 1_000,
            legacy_rate.timing.init_nanos / 1_000,
            legacy_rate.timing.fight_nanos / 1_000,
        );

        assert_eq!(
            (runtime_score.wins, runtime_score.total),
            (legacy_score.wins, legacy_score.total)
        );
        assert_eq!((runtime_rate.wins, runtime_rate.total), (legacy_rate.wins, legacy_rate.total));
    }
}

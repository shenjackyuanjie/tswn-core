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
        let _ = write!(seed, "seed:{}@!", crate::runtime::PROFILE_START as usize + round);
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
    profile_player_ids: Vec<crate::runtime::PlrId>,
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
            profile_team_rng: if modifier.len() <= crate::namerena::TEAM_MAX_LEN {
                crate::namerena::score_profile_team_rng(modifier)
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
        let profile_base = crate::runtime::PROFILE_START as usize + round * self.profile_slots.len();
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
    #[ignore]
    fn probe_score_outcomes_from_env() {
        let target = std::env::var("TSWN_SCORE_PROBE_TARGET").expect("set TSWN_SCORE_PROBE_TARGET");
        let modifier = std::env::var("TSWN_SCORE_PROBE_MODIFIER").unwrap_or_else(|_| "!".to_owned());
        let eval_rq = crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
        let target_group = vec![target];
        let first_groups = ScoreMatchGroups::new(&target_group, &modifier).groups;
        let config = default_custom_runtime_import_config().unwrap();
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&first_groups, eval_rq, config).unwrap();
        let mut match_groups = ScoreMatchGroups::new(&target_group, &modifier);
        let mut runner = prepared.new_reusable_runner();
        if let Ok(round_number) = std::env::var("TSWN_SCORE_PROBE_ROUND").map(|value| value.parse::<usize>().unwrap()) {
            match_groups.set_round(round_number - 1);
            prepared
                .reset_score_groups_with_seed_and_eval_rq(
                    &mut runner,
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
                .unwrap();
            for _ in 0..BATCH_MAX_ROUNDS {
                let outcome = runner.run_round();
                if let Some(frame) = outcome.frame {
                    for update in frame.updates.updates {
                        if matches!(update.update_type, crate::runtime::update::UpdateType::NextLine) {
                            continue;
                        }
                        let entity_name = |idx: usize| {
                            runner
                                .runtime()
                                .entities
                                .get(crate::runtime::EntityIdx(idx as u32))
                                .map(|entity| entity.template.display_name.clone())
                                .unwrap_or_else(|| format!("#{idx}"))
                        };
                        let caster = entity_name(update.caster);
                        let target = entity_name(update.target);
                        let mut message = update.message.replace("[0]", &caster).replace("[1]", &target);
                        let param = update.param.unwrap_or(update.score).to_string();
                        message = message.replace("[2]", &param);
                        println!("UPDATE={message}");
                    }
                }
                if outcome.winner_team.is_some() {
                    break;
                }
            }
            println!("OUTCOME={}", usize::from(runner.input_group_won(0)));
            return;
        }
        let mut outcomes = String::with_capacity(10_000);
        for round in 0..10_000 {
            match_groups.set_round(round);
            prepared
                .reset_score_groups_with_seed_and_eval_rq(
                    &mut runner,
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
                .unwrap();
            runner.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
            outcomes.push(if runner.input_group_won(0) { '1' } else { '0' });
        }
        println!("OUTCOMES={outcomes}");
    }

    fn score_round_target_won(target: &str, round_number: usize) -> bool {
        let target_group = vec![target.to_owned()];
        let modifier = "!";
        let eval_rq = crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
        let first_groups = ScoreMatchGroups::new(&target_group, modifier).groups;
        let config = default_custom_runtime_import_config().unwrap();
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&first_groups, eval_rq, config).unwrap();
        let mut match_groups = ScoreMatchGroups::new(&target_group, modifier);
        let mut runner = prepared.new_reusable_runner();
        match_groups.set_round(round_number - 1);
        prepared
            .reset_score_groups_with_seed_and_eval_rq(
                &mut runner,
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
            .unwrap();

        for _ in 0..BATCH_MAX_ROUNDS {
            let outcome = runner.run_round();
            if outcome.winner_team.is_some() {
                break;
            }
        }

        runner.input_group_won(0)
    }

    #[test]
    fn score_round_596_matches_legacy_clone_battle_outcome() {
        assert!(score_round_target_won("艾泽莉娅 #IAPBVEKLIO@无惨", 596));
    }

    #[test]
    fn score_round_9889_matches_legacy_heal_haste_outcome() {
        assert!(!score_round_target_won("龟钧募犀红@Hell", 9889));
    }

    #[test]
    fn reusable_runner_reset_matches_fresh_runner() {
        let groups = vec![
            vec!["Don't_Force_It #f4fMecHe1@Shabby_fish".to_owned()],
            vec!["涵虚不等式 PFVKEUPBU@TigerStar".to_owned()],
        ];
        let config = default_custom_runtime_import_config().unwrap();
        let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(
            &groups,
            crate::namerena::eval_name::DEFAULT_EVAL_RQ,
            config,
        )
        .unwrap();
        let mut reusable = prepared.new_reusable_runner();
        let mut seed_buffer = String::with_capacity(24);

        for round in 0..32 {
            let seed = profile_seed_for_round(&mut seed_buffer, round);
            prepared.reset_with_seed(&mut reusable, seed).unwrap();
            let reused = reusable.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
            let mut fresh = prepared.new_with_seed(seed).unwrap();
            let expected = fresh.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
            assert_eq!(reused, expected, "round={round}");
            assert_eq!(reusable.input_group_won(0), fresh.input_group_won(0), "round={round}");
        }
    }

    #[test]
    fn parallel_win_rate_and_score_are_deterministic() {
        let groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
        let single = runtime_groups_win_rate(&groups, 128, crate::namerena::eval_name::WIN_RATE_EVAL_RQ, 1).unwrap();
        let parallel = runtime_groups_win_rate(&groups, 128, crate::namerena::eval_name::WIN_RATE_EVAL_RQ, 4).unwrap();
        assert_eq!(
            (single.wins, single.total, single.errors, single.guard_exhausted),
            (parallel.wins, parallel.total, parallel.errors, parallel.guard_exhausted)
        );

        let targets = vec!["mario".to_owned()];
        let single = runtime_score(&targets, "!", 128, crate::namerena::eval_name::WIN_RATE_EVAL_RQ, 1).unwrap();
        let parallel = runtime_score(&targets, "!", 128, crate::namerena::eval_name::WIN_RATE_EVAL_RQ, 4).unwrap();
        assert_eq!(
            (single.wins, single.total, single.errors, single.guard_exhausted),
            (parallel.wins, parallel.total, parallel.errors, parallel.guard_exhausted)
        );
    }

    #[test]
    fn score_match_builder_keeps_single_target_shape() {
        let mut groups = ScoreMatchGroups::new(&["mario".to_owned()], "!");
        groups.set_round(2);
        let base = crate::runtime::PROFILE_START as usize + 6;
        assert_eq!(
            groups.groups,
            vec![
                vec!["mario".to_owned(), format!("{base}@!")],
                vec![format!("{}@!", base + 1), format!("{}@!", base + 2)]
            ]
        );
    }
}

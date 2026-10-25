//! Runtime 批量评分与胜率执行层。
//!
//! 胜率路径复用 [`PreparedRuntimeRunner`]，每局只重建 seed 相关状态；评分路径的
//! profile 名字每轮都会变化，因此复用 registry 配置并按轮构造 roster。两条路径都
//! 使用不保留逐回合帧的 completion runner，避免 benchmark 热路径积累回放向量。
//!
//! # 轮次、分片与统计
//!
//! 胜率轮次从 0 开始：第 0 轮不附加 seed，后续轮次使用
//! `seed:{PROFILE_START + round}@!`。区间入口按原始 `[start, end)` 编号执行，
//! 不会在每个分片重新从 0 编号；并行调度只影响完成顺序，不改变每轮输入。
//! 评分则以轮次推进数字 profile 名字，并使用空的额外 seed；不要混用这两种编号规则。
//!
//! 每局最多推进 100,000 回合。`total` 计入成功初始化的对局，达到保护上限的局
//! 仍计入 `total` 并累加 `guard_exhausted`；`wins` 按第一输入组与最终胜者 roster
//! 是否相交判定。评分的逐轮初始化失败累加 `errors`，不计入 `total`；
//! 固定阵容胜率的初始化错误直接返回 `Err`。普通入口不逐局计时，`timing` 保持零值。

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::time::Stopwatch;
use crate::win_rate::{WinRateTiming, resolve_win_rate_workers};

use super::{
    CustomRuntimeImportError, DefaultCustomRuntimeProfileError, PreparedRuntimeRunner, RuntimeRunner, ScoreIdentityBuffer,
    ScoreRosterBuffers, ScoreRoundScratch, SkillLoadout, default_custom_runtime_import_config,
};

const BATCH_PARALLEL_THRESHOLD: usize = 100;
const BATCH_MAX_ROUNDS: usize = 100_000;

/// 一批对局的累计统计；比例以 `total` 为分母，不包含初始化失败的轮次。
///
/// 空统计的胜率和万分制评分为 0；比例计算将分母至少按 1 处理。保护上限耗尽不属于初始化错误，
/// 可用 `guard_exhausted` 单独识别；合并统计时各计数与计时相加。
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

/// 预构建固定阵容并执行 `0..n` 轮胜率，统计第一输入组是否获胜。
///
/// `n == 0` 仍校验和构建阵容，但不推进对局；`thread` 的 worker 预算由
/// 胜率调度策略解析，小批量直接串行执行。构建或复位失败通过错误通道返回。
pub fn runtime_groups_win_rate(
    groups: &[Vec<String>],
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    runtime_groups_win_rate_with_timing::<false>(groups, n, eval_rq, thread)
}

/// 与 [`runtime_groups_win_rate`] 相同，但额外统计每场的 init / fight 耗时。
///
/// 计时覆盖每场初始化和战斗完成路径，不包含预构建阵容、线程创建或结果合并。
/// 多 worker 的时间直接求和，不等于整批的墙钟耗时；不需要计时的调用方使用普通入口。
pub fn runtime_groups_win_rate_timed(
    groups: &[Vec<String>],
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    runtime_groups_win_rate_with_timing::<true>(groups, n, eval_rq, thread)
}

fn runtime_groups_win_rate_with_timing<const TIMED: bool>(
    groups: &[Vec<String>],
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let config = default_custom_runtime_import_config()?;
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(groups, eval_rq, config)?;
    prepared_runtime_win_rate_with_timing::<TIMED>(&prepared, n, thread)
}

/// 对已有准备模板执行 `0..n` 轮胜率，各 worker 独占一个可复位的 runner。
///
/// 阵容、注册表和 `eval_rq` 来自 `prepared`；只有每轮 seed 和可变战斗状态重建。
/// `n == 0` 返回空统计。轮次 seed、保护上限和计数口径见本模块说明。
pub fn prepared_runtime_win_rate(
    prepared: &PreparedRuntimeRunner,
    n: usize,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    prepared_runtime_win_rate_with_timing::<false>(prepared, n, thread)
}

/// 与 [`prepared_runtime_win_rate`] 相同，但额外统计每场的 init / fight 耗时。
pub fn prepared_runtime_win_rate_timed(
    prepared: &PreparedRuntimeRunner,
    n: usize,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    prepared_runtime_win_rate_with_timing::<true>(prepared, n, thread)
}

fn prepared_runtime_win_rate_with_timing<const TIMED: bool>(
    prepared: &PreparedRuntimeRunner,
    n: usize,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let workers = resolve_win_rate_workers(thread, n);
    if workers <= 1 || n < BATCH_PARALLEL_THRESHOLD {
        return run_prepared_range::<TIMED>(prepared, 0, n).map_err(Into::into);
    }

    let prepared = Arc::new(prepared.clone());
    let next = Arc::new(AtomicUsize::new(0));
    // 小区间动态领取，减少逐场原子争用，同时为每个 worker 留出多次领取机会。
    let chunk_size = n.div_ceil(workers.saturating_mul(8)).clamp(1, 32);
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let prepared = Arc::clone(&prepared);
        let next = Arc::clone(&next);
        handles.push(std::thread::spawn(move || {
            run_prepared_worker::<TIMED>(prepared.as_ref(), next.as_ref(), n, chunk_size)
        }));
    }

    let mut merged = RuntimeBatchSummary::default();
    for handle in handles {
        let part = handle.join().expect("runtime win-rate worker thread panicked")?;
        merged.merge(part);
    }
    Ok(merged)
}

/// 串行执行准备模板的原始轮次区间 `[start, end)`，适合外部任务分片。
///
/// 使用与完整胜率入口相同的轮次 seed；`start >= end` 返回空统计。
/// 相邻不重叠区间的计数可以合并，重叠区间会重复计数。
pub fn prepared_runtime_win_rate_range(
    prepared: &PreparedRuntimeRunner,
    start: usize,
    end: usize,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    run_prepared_range::<false>(prepared, start, end).map_err(Into::into)
}

/// 与 [`prepared_runtime_win_rate_range`] 相同，但额外统计每场的 init / fight 耗时。
pub fn prepared_runtime_win_rate_range_timed(
    prepared: &PreparedRuntimeRunner,
    start: usize,
    end: usize,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    run_prepared_range::<true>(prepared, start, end).map_err(Into::into)
}

/// 按数字 profile 轮次执行目标组的万分制评分，`modifier` 参与 profile 阵营构建。
///
/// 单目标、重复二人组与普通多目标沿用各自的 profile 数量及组队布局；
/// 不把每个目标都独立重建成一场对局。初始配置错误返回 `Err`，逐轮构建失败
/// 记录在 `errors` 中；有效评分使用成功初始化的 `total` 作为分母。
pub fn runtime_score(
    target_group: &[String],
    modifier: &str,
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    runtime_score_with_timing::<false>(target_group, modifier, n, eval_rq, thread)
}

/// 与 [`runtime_score`] 相同，但额外统计每场的 init / fight 耗时。
pub fn runtime_score_timed(
    target_group: &[String],
    modifier: &str,
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    runtime_score_with_timing::<true>(target_group, modifier, n, eval_rq, thread)
}

fn runtime_score_with_timing<const TIMED: bool>(
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
        return Ok(run_score_range::<TIMED>(
            target_group,
            modifier,
            0,
            n,
            eval_rq,
            prepared.as_ref(),
        ));
    }

    let next = Arc::new(AtomicUsize::new(0));
    // 小区间动态领取，减少逐场原子争用，同时为每个 worker 留出多次领取机会。
    let chunk_size = n.div_ceil(workers.saturating_mul(8)).clamp(1, 32);
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let target_group = target_group.to_vec();
        let modifier = modifier.to_owned();
        let next = Arc::clone(&next);
        let prepared = Arc::clone(&prepared);
        handles.push(std::thread::spawn(move || {
            run_score_worker::<TIMED>(
                &target_group,
                &modifier,
                next.as_ref(),
                n,
                eval_rq,
                prepared.as_ref(),
                chunk_size,
            )
        }));
    }

    let mut merged = RuntimeBatchSummary::default();
    for handle in handles {
        merged.merge(handle.join().expect("runtime score worker thread panicked"));
    }
    Ok(merged)
}

/// 串行执行原始评分轮次区间 `[start, end)`，数字 profile 不因分片而重新编号。
///
/// `start >= end` 仍执行初始配置与阵容校验，但返回空统计；其他错误与计数口径
/// 与 [`runtime_score`] 一致。外部分片时应合并计数后再计算评分，不直接平均各分片比例。
pub fn runtime_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    runtime_score_range_with_timing::<false>(target_group, modifier, start, end, eval_rq)
}

/// 与 [`runtime_score_range`] 相同，但额外统计每场的 init / fight 耗时。
pub fn runtime_score_range_timed(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    runtime_score_range_with_timing::<true>(target_group, modifier, start, end, eval_rq)
}

fn runtime_score_range_with_timing<const TIMED: bool>(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let first_groups = ScoreMatchGroups::new(target_group, modifier).groups;
    let config = default_custom_runtime_import_config()?;
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&first_groups, eval_rq, config)?;
    Ok(run_score_range::<TIMED>(target_group, modifier, start, end, eval_rq, &prepared))
}

fn run_prepared_range<const TIMED: bool>(
    prepared: &PreparedRuntimeRunner,
    start: usize,
    end: usize,
) -> Result<RuntimeBatchSummary, CustomRuntimeImportError> {
    let mut summary = RuntimeBatchSummary::default();
    let mut seed = String::with_capacity(24);
    let mut runner = prepared.new_reusable_runner();
    for round in start..end {
        run_prepared_round::<TIMED>(prepared, &mut runner, profile_seed_for_round(&mut seed, round), &mut summary)?;
    }
    Ok(summary)
}

fn run_prepared_worker<const TIMED: bool>(
    prepared: &PreparedRuntimeRunner,
    next: &AtomicUsize,
    end: usize,
    chunk_size: usize,
) -> Result<RuntimeBatchSummary, RuntimeBatchError> {
    let mut summary = RuntimeBatchSummary::default();
    let mut seed = String::with_capacity(24);
    let mut runner = prepared.new_reusable_runner();
    loop {
        let start = next.fetch_add(chunk_size, Ordering::Relaxed);
        if start >= end {
            break;
        }
        for round in start..start.saturating_add(chunk_size).min(end) {
            run_prepared_round::<TIMED>(prepared, &mut runner, profile_seed_for_round(&mut seed, round), &mut summary)?;
        }
    }
    Ok(summary)
}

fn run_prepared_round<const TIMED: bool>(
    prepared: &PreparedRuntimeRunner,
    runner: &mut RuntimeRunner,
    seed: &[String],
    summary: &mut RuntimeBatchSummary,
) -> Result<(), CustomRuntimeImportError> {
    // 单场只有几微秒，逐场计时在 Windows 上是四次 QPC，占比可达 2%~3%；
    // 只有 `_timed` 入口会把 TIMED 打开，其余批量调用方完全不付这份开销。
    let init_started = TIMED.then(Stopwatch::now);
    prepared.reset_with_seed(runner, seed)?;
    if let Some(init_started) = init_started {
        summary.timing.init_nanos += init_started.elapsed_nanos();
    }

    let fight_started = TIMED.then(Stopwatch::now);
    let completion = runner.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
    if let Some(fight_started) = fight_started {
        summary.timing.fight_nanos += fight_started.elapsed_nanos();
    }
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

fn run_score_range<const TIMED: bool>(
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
        run_score_round::<TIMED>(round, eval_rq, prepared, &mut runner, &mut match_groups, &mut summary);
    }
    summary
}

fn run_score_worker<const TIMED: bool>(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
    eval_rq: f64,
    prepared: &PreparedRuntimeRunner,
    chunk_size: usize,
) -> RuntimeBatchSummary {
    let mut summary = RuntimeBatchSummary::default();
    let mut match_groups = ScoreMatchGroups::new(target_group, modifier);
    let mut runner = prepared.new_reusable_runner();
    loop {
        let start = next.fetch_add(chunk_size, Ordering::Relaxed);
        if start >= end {
            break;
        }
        for round in start..start.saturating_add(chunk_size).min(end) {
            run_score_round::<TIMED>(round, eval_rq, prepared, &mut runner, &mut match_groups, &mut summary);
        }
    }
    summary
}

fn run_score_round<const TIMED: bool>(
    round: usize,
    eval_rq: f64,
    prepared: &PreparedRuntimeRunner,
    runner: &mut RuntimeRunner,
    match_groups: &mut ScoreMatchGroups,
    summary: &mut RuntimeBatchSummary,
) {
    match_groups.set_round(round);
    let init_started = TIMED.then(Stopwatch::now);
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
    if let Some(init_started) = init_started {
        summary.timing.init_nanos += init_started.elapsed_nanos();
    }

    let fight_started = TIMED.then(Stopwatch::now);
    let completion = runner.run_to_completion_prevalidated(BATCH_MAX_ROUNDS);
    if let Some(fight_started) = fight_started {
        summary.timing.fight_nanos += fight_started.elapsed_nanos();
    }
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
    fn score_round_6539_matches_legacy_skill_only_merge_haste_outcome() {
        assert!(!score_round_target_won("琉音芽玲亚 #AIBSXOQHSK@无惨", 6539));
    }

    #[test]
    fn score_round_6670_matches_legacy_broken_iron_haste_outcome() {
        assert!(!score_round_target_won("雪萤23848163494498@四象柯", 6670));
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
    fn score_trailing_empty_team_matches_no_team() {
        let with_empty_team = runtime_score(
            &["宗铭丸 #DCVIDQJX@".to_owned()],
            "!",
            256,
            crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
            1,
        )
        .unwrap();
        let without_team = runtime_score(
            &["宗铭丸 #DCVIDQJX".to_owned()],
            "!",
            256,
            crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
            1,
        )
        .unwrap();
        assert_eq!(with_empty_team.wins, without_team.wins);
    }

    #[test]
    fn score_colon_team_suffix_matches_no_team() {
        let modifier = "\u{0002}";
        let eval_rq = crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
        let with_suffix = vec!["荧屏上的梦想 NWKm7L_6@dream_on_screen qp:6070".to_owned(); 2];
        let without_team = vec!["荧屏上的梦想 NWKm7L_6".to_owned(); 2];

        let with_suffix = runtime_score(&with_suffix, modifier, 256, eval_rq, 1).unwrap();
        let without_team = runtime_score(&without_team, modifier, 256, eval_rq, 1).unwrap();

        assert_eq!(with_suffix.wins, without_team.wins);
        assert_eq!(with_suffix.total, without_team.total);
        assert_eq!(with_suffix.errors, without_team.errors);
        assert_eq!(with_suffix.guard_exhausted, without_team.guard_exhausted);
    }

    #[test]
    fn score_target_sharing_profile_team_does_not_panic() {
        let target = vec!["33554433@\u{0002}".to_owned()];
        let summary = runtime_score(&target, "\u{0002}", 2, crate::namerena::eval_name::WIN_RATE_EVAL_RQ, 1).unwrap();

        assert_eq!(summary.total, 2);
        assert_eq!(summary.errors, 0);
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

#[cfg(test)]
mod chunk_regressions {
    use super::*;
    #[test]
    fn chunked_schedulers_preserve_counts_at_uneven_boundaries() {
        let groups = vec![vec!["alpha@red".to_owned()], vec!["beta@blue".to_owned()]];
        let targets = vec!["alpha@red".to_owned()];
        let eval = crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
        for n in [99, 100, 101, 257, 1025] {
            let expected_win = runtime_groups_win_rate(&groups, n, eval, 1).unwrap();
            let expected_score = runtime_score(&targets, "!", n, eval, 1).unwrap();
            for threads in [2, 4] {
                for timed in [false, true] {
                    let win = if timed {
                        runtime_groups_win_rate_timed(&groups, n, eval, threads)
                    } else {
                        runtime_groups_win_rate(&groups, n, eval, threads)
                    }
                    .unwrap();
                    let score = if timed {
                        runtime_score_timed(&targets, "!", n, eval, threads)
                    } else {
                        runtime_score(&targets, "!", n, eval, threads)
                    }
                    .unwrap();
                    for (actual, expected) in [(win, expected_win), (score, expected_score)] {
                        assert_eq!(
                            (actual.wins, actual.total, actual.errors, actual.guard_exhausted),
                            (expected.wins, expected.total, expected.errors, expected.guard_exhausted)
                        );
                    }
                }
            }
        }
    }
}

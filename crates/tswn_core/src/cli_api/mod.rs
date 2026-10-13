//! 应用接入门面：参数校验、规范 DTO 与 CLI 对齐的批量辅助函数。
//!
//! # 使用哪一部分
//!
//! - 对局播放：[`battle::BattleSession`]；完整收集：[`battle::battle_replay`]。
//! - 学习数据：[`battle::BattleModelSession`]，复用同一终止策略但不构造展示对象。
//! - 两队/多队胜率：[`win_rate_summary`]、[`team_win_rate_summary`]、[`group_win_rate_summary`]。
//! - 名字评分与组合评估：[`score`]、[`namer_pf`]、[`batch_rate`]、[`pair_rate`]。
//! - 格式转换与图标：[`to_diy`]、[`to_diy_batch`]、[`icon_info`]。
//!
//! [`CliApiError`] 提供可读说明，[`CliApiError::code`] 提供跨语言错误分类。
//! 错误码描述调用失败；[`battle::BattleStopReason`] 描述正常终止原因。
//! 胜率以百分比表示，`eval_rq` 指定名字构建口径。
//! 调用同步执行，包装层负责宿主调度、资源生命周期和 UI 更新。

pub mod battle;
mod bench;
mod parse;

#[allow(deprecated)]
pub use battle::{BattleOptions, BattleReplay, BattleReplayOptions, BattleResult, BattleSession, BattleStatus, BattleStopReason};

use crate::namerena::eval_name;
use crate::namerena::icon::icon_from_raw_name;
use crate::runtime::update::UpdateType;
use crate::runtime::{
    CustomRuntimeImportConfig, NormalizedOutcome, NormalizedUpdateFrame, RuntimeBatchSummary, RuntimeNormalizedRun,
    RuntimeRunner, default_custom_runtime_import_config, runtime_groups_win_rate_timed, runtime_score_timed,
};
use crate::win_rate::{WinRateSummary, WinRateTiming};

/// 高层调用结果，错误包含跨语言分类与可读说明。
pub type CliApiResult<T> = Result<T, CliApiError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
/// 跨语言错误分类；序列化使用 SCREAMING_SNAKE_CASE。
pub enum CliApiErrorCode {
    InvalidInput,
    InvalidArgument,
    UnsupportedOption,
    RunnerInitFailed,
    RuntimeFailed,
    InternalError,
}
impl CliApiErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidInput => "INVALID_INPUT",
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::UnsupportedOption => "UNSUPPORTED_OPTION",
            Self::RunnerInitFailed => "RUNNER_INIT_FAILED",
            Self::RuntimeFailed => "RUNTIME_FAILED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}
#[derive(Debug, Clone)]
/// 参数、初始化、机制执行或内部转换失败，保留可读错误说明。
pub enum CliApiError {
    InvalidInput(String),
    InvalidArgument(String),
    UnsupportedOption(String),
    RunnerInit(String),
    Runtime(String),
    Internal(String),
}
impl CliApiError {
    pub fn code(&self) -> CliApiErrorCode {
        match self {
            Self::InvalidInput(_) => CliApiErrorCode::InvalidInput,
            Self::InvalidArgument(_) => CliApiErrorCode::InvalidArgument,
            Self::UnsupportedOption(_) => CliApiErrorCode::UnsupportedOption,
            Self::RunnerInit(_) => CliApiErrorCode::RunnerInitFailed,
            Self::Runtime(_) => CliApiErrorCode::RuntimeFailed,
            Self::Internal(_) => CliApiErrorCode::InternalError,
        }
    }
}
impl std::fmt::Display for CliApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::InvalidArgument(message) => f.write_str(message),
            Self::UnsupportedOption(message) => f.write_str(message),
            Self::RunnerInit(message) => f.write_str(message),
            Self::Runtime(message) => f.write_str(message),
            Self::Internal(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for CliApiError {}

#[derive(Debug, Clone, PartialEq)]
/// 第一输入组获胜统计，win_rate 使用百分比，耗时字段使用纳秒。
pub struct WinRateResult {
    pub wins: usize,
    pub total: usize,
    pub win_rate: f64,
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

impl From<WinRateSummary> for WinRateResult {
    fn from(value: WinRateSummary) -> Self {
        Self {
            wins: value.wins,
            total: value.total,
            win_rate: value.win_rate_percent(),
            init_nanos: value.timing.init_nanos,
            fight_nanos: value.timing.fight_nanos,
        }
    }
}

impl From<RuntimeBatchSummary> for WinRateResult {
    fn from(value: RuntimeBatchSummary) -> Self {
        Self {
            wins: value.wins,
            total: value.total,
            win_rate: value.win_rate_percent(),
            init_nanos: value.timing.init_nanos,
            fight_nanos: value.timing.fight_nanos,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 一组目标对一个 opponent 的胜率结果。
pub struct GroupWinRateResult {
    pub opponent: String,
    pub result: WinRateResult,
}

#[derive(Debug, Clone, PartialEq)]
/// 数字 profile 靶子评分及成功/错误统计，score 使用万分制。
pub struct ScoreResult {
    pub score: f64,
    pub wins: usize,
    pub total: usize,
    pub errors: usize,
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeNormalizedRun {
    pub rounds: Vec<JsonRuntimeNormalizedOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
    pub total_score: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeNormalizedOutcome {
    pub winner_team: Option<usize>,
    pub round: u64,
    pub total_score: u64,
    pub rng_i: u32,
    pub rng_j: u32,
    pub entity_ids: Vec<usize>,
    pub teams: Vec<usize>,
    pub hp: Vec<i32>,
    pub magic_point: Vec<i32>,
    pub defense: Vec<i32>,
    pub resistance: Vec<i32>,
    pub alive: Vec<bool>,
    pub round_order: Vec<usize>,
    pub flat_alive: Vec<usize>,
    pub team_alive: Vec<Vec<usize>>,
    pub alive_group_count: usize,
    pub actions: Vec<JsonRuntimeActionBoundary>,
    pub frames: Vec<JsonRuntimeUpdateFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeActionBoundary {
    pub round: u64,
    pub actor: usize,
    pub target: usize,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeUpdateFrame {
    pub message: String,
    pub caster: usize,
    pub target: usize,
    pub targets: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub update_type: &'static str,
}

impl From<RuntimeNormalizedRun> for JsonRuntimeNormalizedRun {
    fn from(value: RuntimeNormalizedRun) -> Self {
        Self {
            rounds: value.rounds.into_iter().map(Into::into).collect(),
            winner_team: value.winner_team,
            guard_exhausted: value.guard_exhausted,
            total_score: value.total_score,
        }
    }
}

impl From<NormalizedOutcome> for JsonRuntimeNormalizedOutcome {
    fn from(value: NormalizedOutcome) -> Self {
        Self {
            winner_team: value.winner_team,
            round: value.round,
            total_score: value.total_score,
            rng_i: value.rng.i,
            rng_j: value.rng.j,
            entity_ids: value.entity_ids,
            teams: value.teams,
            hp: value.hp,
            magic_point: value.magic_point,
            defense: value.defense,
            resistance: value.resistance,
            alive: value.alive,
            round_order: value.round_order,
            flat_alive: value.flat_alive,
            team_alive: value.team_alive,
            alive_group_count: value.alive_group_count,
            actions: value
                .actions
                .into_iter()
                .map(|action| JsonRuntimeActionBoundary {
                    round: action.round,
                    actor: action.actor,
                    target: action.target,
                    amount: action.amount,
                })
                .collect(),
            frames: value.frames.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<NormalizedUpdateFrame> for JsonRuntimeUpdateFrame {
    fn from(value: NormalizedUpdateFrame) -> Self {
        Self {
            message: value.message,
            caster: value.caster,
            target: value.target,
            targets: value.targets,
            param: value.param,
            score: value.score,
            delay0: value.delay0,
            delay1: value.delay1,
            update_type: runtime_update_type_name(value.update_type),
        }
    }
}

pub fn runtime_update_type_name(value: UpdateType) -> &'static str {
    match value {
        UpdateType::Win => "win",
        UpdateType::None => "none",
        UpdateType::NextLine => "next_line",
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 一组名字的所选命配指标与合计。
pub struct NamerPfResult {
    pub group: Vec<String>,
    pub modes: Vec<String>,
    pub scores: Vec<f64>,
    pub total_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// 对靶子集合的平均与合计胜率、有效对抗数量及耗时。
pub struct BatchRateResult {
    pub label: String,
    pub avg_win_rate: f64,
    pub aggregate_win_rate: f64,
    pub wins: usize,
    pub total: usize,
    pub valid_matchups: usize,
    pub skipped_matchups: usize,
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairRateEntry {
    pub name: String,
    pub rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// 配对评估的最终分数、Top 明细和总体统计。
pub struct PairRateResult {
    pub label: String,
    pub final_score: f64,
    pub head: usize,
    pub selected: usize,
    pub top_pairs: Vec<PairRateEntry>,
    pub aggregate_win_rate: f64,
    pub wins: usize,
    pub total: usize,
    pub valid_matchups: usize,
    pub skipped_matchups: usize,
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

#[derive(Debug, Clone, PartialEq)]
/// 名字图标的边框、形状与调色板资料，供渲染或界面分析。
pub struct IconInfo {
    pub border_style: usize,
    pub shapes: Vec<usize>,
    pub bg_color_idx: usize,
    pub bg_color: [u8; 3],
    pub fg_color_indices: Vec<usize>,
    pub fg_colors: Vec<[u8; 3]>,
    pub colors_consumed: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ScoreMode {
    Normal,
    Bang,
}

impl ScoreMode {
    fn modifier(self) -> &'static str {
        match self {
            Self::Normal => "\u{0002}",
            Self::Bang => "!",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum NamerPfMode {
    Pp,
    Pd,
    Qp,
    Qd,
}

impl NamerPfMode {
    const ALL: [Self; 4] = [Self::Pp, Self::Pd, Self::Qp, Self::Qd];

    fn label(self) -> &'static str {
        match self {
            Self::Pp => "pp",
            Self::Pd => "pd",
            Self::Qp => "qp",
            Self::Qd => "qd",
        }
    }

    fn score_params(self) -> (&'static str, bool) {
        match self {
            Self::Pp => ("\u{0002}", false),
            Self::Pd => ("\u{0002}", true),
            Self::Qp => ("!", false),
            Self::Qd => ("!", true),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct BatchSummary {
    avg: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    timing: WinRateTiming,
    valid_matchups: usize,
    skipped_matchups: usize,
}

/// 计算第一输入组对其他组的胜率；默认 rq 为胜率口径，n 至少按 1 处理。
pub fn win_rate_summary(raw: &str, n: usize, eval_rq: Option<f64>, thread: u32) -> CliApiResult<WinRateResult> {
    let eval_rq = eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ);
    let groups = RuntimeRunner::split_namerena_into_groups(raw.to_owned()).0;
    ensure_win_rate_group_count(&groups)?;
    runtime_groups_win_rate_timed(&groups, n.max(1), eval_rq, thread)
        .map(Into::into)
        .map_err(runtime_batch_error)
}

/// 将两队文本按空行组合后计算规范胜率统计。
pub fn team_win_rate_summary(
    team1: &str,
    team2: &str,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> CliApiResult<WinRateResult> {
    win_rate_summary(&format!("{team1}\n\n{team2}"), n, eval_rq, thread)
}

/// 按 opponent 列表顺序逐项计算目标组的胜率。
pub fn group_win_rate_summary(
    target: &str,
    against: &[String],
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> CliApiResult<Vec<GroupWinRateResult>> {
    let eval_rq = eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ);
    against
        .iter()
        .map(|opponent| {
            let result = team_win_rate_summary(target, opponent, n, Some(eval_rq), thread)?;
            Ok(GroupWinRateResult {
                opponent: opponent.clone(),
                result,
            })
        })
        .collect()
}

/// 对第一组名字计算所选 profile 模式的万分制评分。
pub fn score(raw: &str, n: usize, mode: &str, eval_rq: Option<f64>, thread: u32) -> CliApiResult<ScoreResult> {
    let score_mode = parse_score_mode(mode)?;
    let (groups, _) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
    let target_group = groups.into_iter().next().unwrap_or_default();
    if target_group.is_empty() {
        return Err(invalid_input("score requires at least one player"));
    }
    let summary = runtime_score_timed(
        &target_group,
        score_mode.modifier(),
        n.max(1),
        eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ),
        thread,
    )
    .map_err(runtime_batch_error)?;
    Ok(ScoreResult {
        score: summary.score_10000(),
        wins: summary.wins,
        total: summary.total,
        errors: summary.errors + summary.guard_exhausted,
        init_nanos: summary.timing.init_nanos,
        fight_nanos: summary.timing.fight_nanos,
    })
}

/// 按命配组合格式解析名字并计算所选 pp/pd/qp/qd 指标；keep_rq 选择普通构建口径。
pub fn namer_pf(raw: &str, n: usize, modes: Option<Vec<String>>, keep_rq: bool, thread: u32) -> CliApiResult<Vec<NamerPfResult>> {
    let groups = parse::parse_plus_separated_groups(raw);
    if groups.is_empty() {
        return Err(invalid_input("namer_pf requires at least one non-empty group"));
    }
    let modes = normalize_namer_pf_modes(modes)?;
    let eval_rq = if keep_rq {
        eval_name::DEFAULT_EVAL_RQ
    } else {
        eval_name::WIN_RATE_EVAL_RQ
    };
    let labels = modes.iter().map(|mode| mode.label().to_string()).collect::<Vec<_>>();

    Ok(groups
        .into_iter()
        .map(|group| {
            let scores = modes
                .iter()
                .map(|mode| {
                    let (modifier, duplicate) = mode.score_params();
                    bench::namer_pf_score(&group, modifier, duplicate, n.max(1), thread, eval_rq)
                })
                .collect::<CliApiResult<Vec<_>>>()?;
            let total_score = scores.iter().sum();
            Ok(NamerPfResult {
                group,
                modes: labels.clone(),
                scores,
                total_score,
            })
        })
        .collect::<CliApiResult<Vec<_>>>()?)
}

/// 计算每组选手对靶子列表的平均/合计胜率；标签与选手数量对应。
pub fn batch_rate(
    target_groups: &[String],
    player_groups: &[String],
    n: usize,
    player_labels: Option<Vec<String>>,
    keep_rq: bool,
    thread: u32,
) -> CliApiResult<Vec<BatchRateResult>> {
    if target_groups.is_empty() {
        return Err(invalid_input("batch_rate requires at least one target group"));
    }
    if player_groups.is_empty() {
        return Err(invalid_input("batch_rate requires at least one player group"));
    }
    let labels = player_labels.unwrap_or_else(|| player_groups.to_vec());
    if labels.len() != player_groups.len() {
        return Err(invalid_input("player_labels must match player_groups length"));
    }
    let eval_rq = win_rate_eval_rq(keep_rq);
    player_groups
        .iter()
        .zip(labels.iter())
        .map(|(player, label)| {
            let summary = bench::batch_rate_for_group(player, target_groups, None, n.max(1), thread, eval_rq)?;
            Ok(BatchRateResult {
                label: label.clone(),
                avg_win_rate: summary.avg,
                aggregate_win_rate: summary.aggregate_rate,
                wins: summary.wins,
                total: summary.total,
                valid_matchups: summary.valid_matchups,
                skipped_matchups: summary.skipped_matchups,
                init_nanos: summary.timing.init_nanos,
                fight_nanos: summary.timing.fight_nanos,
            })
        })
        .collect()
}

/// 二人组评分：每名选手与每个队友各组合一次，按 `head` 取最高的若干个胜率求和。
///
/// 选手与队友都会先按“逐个成员单独构建”冻结成 `+ol` 再组队，同公会（`@队名` 相同）
/// 成员之间的组队加成不会计入 cqp，两侧都按各自单独构建的强度计算；已经带
/// `+diy` / `+ol` 的输入原样保留，`top_pairs` 里的名字是调用方传入的队友原文。
/// 按给定选手、队友和靶子评估组合，返回 Top 明细与最终统计。
pub fn pair_rate(
    target_groups: &[String],
    players: &[String],
    teammates: &[String],
    head: usize,
    n: usize,
    keep_rq: bool,
    thread: u32,
) -> CliApiResult<Vec<PairRateResult>> {
    if target_groups.is_empty() {
        return Err(invalid_input("pair_rate requires at least one target group"));
    }
    if players.is_empty() {
        return Err(invalid_input("pair_rate requires at least one player"));
    }
    if teammates.is_empty() {
        return Err(invalid_input("pair_rate requires at least one teammate"));
    }
    if head == 0 {
        return Err(invalid_input("head must be positive"));
    }

    let eval_rq = win_rate_eval_rq(keep_rq);
    players
        .iter()
        .map(|player| bench::pair_rate_for_player(player, target_groups, None, teammates, head, n.max(1), thread, eval_rq))
        .collect()
}

pub fn batch_rate_factored(
    target_groups: &[String],
    target_factors: &[f64],
    player_groups: &[String],
    n: usize,
    player_labels: Option<Vec<String>>,
    keep_rq: bool,
    thread: u32,
) -> CliApiResult<Vec<BatchRateResult>> {
    validate_factored_targets(target_groups, target_factors, "batch_rate_factored")?;
    if player_groups.is_empty() {
        return Err(invalid_input("batch_rate_factored requires at least one player group"));
    }
    let labels = player_labels.unwrap_or_else(|| player_groups.to_vec());
    if labels.len() != player_groups.len() {
        return Err(invalid_input("player_labels must match player_groups length"));
    }
    let eval_rq = win_rate_eval_rq(keep_rq);
    player_groups
        .iter()
        .zip(labels.iter())
        .map(|(player, label)| {
            let summary = bench::batch_rate_for_group(player, target_groups, Some(target_factors), n.max(1), thread, eval_rq)?;
            Ok(BatchRateResult {
                label: label.clone(),
                avg_win_rate: summary.avg,
                aggregate_win_rate: summary.aggregate_rate,
                wins: summary.wins,
                total: summary.total,
                valid_matchups: summary.valid_matchups,
                skipped_matchups: summary.skipped_matchups,
                init_nanos: summary.timing.init_nanos,
                fight_nanos: summary.timing.fight_nanos,
            })
        })
        .collect()
}

/// 带权靶子的二人组评分：每个队友组合先按靶子权重求平均，再按 `head` 取最高分求和。
///
/// 两侧的冻结语义与 [`pair_rate`] 相同。
pub fn pair_rate_factored(
    target_groups: &[String],
    target_factors: &[f64],
    players: &[String],
    teammates: &[String],
    head: usize,
    n: usize,
    keep_rq: bool,
    thread: u32,
) -> CliApiResult<Vec<PairRateResult>> {
    validate_factored_targets(target_groups, target_factors, "pair_rate_factored")?;
    if players.is_empty() {
        return Err(invalid_input("pair_rate_factored requires at least one player"));
    }
    if teammates.is_empty() {
        return Err(invalid_input("pair_rate_factored requires at least one teammate"));
    }
    if head == 0 {
        return Err(invalid_input("head must be positive"));
    }
    let eval_rq = win_rate_eval_rq(keep_rq);
    players
        .iter()
        .map(|player| {
            bench::pair_rate_for_player(
                player,
                target_groups,
                Some(target_factors),
                teammates,
                head,
                n.max(1),
                thread,
                eval_rq,
            )
        })
        .collect()
}

pub fn to_diy(name: &str, old: bool, minions: bool) -> CliApiResult<String> { parse::export_player(name, old, minions) }

/// 导出已构建的角色，保留其属性和技能，不重新解析名字或构建队伍。
///
/// 与 [`to_diy`] 一样，旧版 DIY 格式（`old`）不能同时导出召唤物（`minions`）。
pub fn to_diy_prepared(player: &crate::namerena::PreparedPlayer, old: bool, minions: bool) -> CliApiResult<String> {
    if old && minions {
        return Err(invalid_input("old and minions are mutually exclusive"));
    }
    Ok(parse::export_built_player(player, old, minions))
}

pub fn to_diy_batch(names: &[String], old: bool, minions: bool) -> CliApiResult<Vec<String>> {
    names.iter().map(|name| parse::export_player(name, old, minions)).collect()
}

pub fn icon_info(name: &str) -> IconInfo {
    let icon = icon_from_raw_name(name);
    IconInfo {
        border_style: icon.border_style,
        shapes: icon.shapes,
        bg_color_idx: icon.bg_color_idx,
        bg_color: icon.bg_color,
        fg_color_indices: icon.fg_color_indices,
        fg_colors: icon.fg_colors,
        colors_consumed: icon.colors_consumed,
    }
}

pub fn parse_group_lines(content: &str, double_plus: bool) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let sep = if double_plus { "++" } else { "+" };
            line.split(sep)
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

pub fn custom_runtime_mixed_runner(raw: &str, config: CustomRuntimeImportConfig<'_>) -> CliApiResult<RuntimeRunner> {
    RuntimeRunner::from_custom_mixed_namerena_raw(raw.to_owned(), config).map_err(custom_runtime_import_error)
}

pub fn default_custom_runtime_mixed_runner(raw: &str) -> CliApiResult<RuntimeRunner> {
    let config = default_custom_runtime_import_config().map_err(default_custom_runtime_profile_error)?;
    custom_runtime_mixed_runner(raw, config)
}

pub fn custom_runtime_normalized_run(
    raw: &str,
    max_rounds: usize,
    config: CustomRuntimeImportConfig<'_>,
) -> CliApiResult<RuntimeNormalizedRun> {
    ensure_runtime_max_rounds(max_rounds)?;
    let mut runner = custom_runtime_mixed_runner(raw, config)?;
    Ok(runner.run_until_winner_normalized_rounds(max_rounds))
}

pub fn default_custom_runtime_normalized_run(raw: &str, max_rounds: usize) -> CliApiResult<RuntimeNormalizedRun> {
    ensure_runtime_max_rounds(max_rounds)?;
    let mut runner = default_custom_runtime_mixed_runner(raw)?;
    Ok(runner.run_until_winner_normalized_rounds(max_rounds))
}

/// 运行一场战斗，并返回供每种语言绑定共用、可直接供 UI 使用的完整回放。
pub fn battle_replay(raw: &str, options: BattleOptions) -> CliApiResult<BattleReplay> { battle::battle_replay(raw, options) }

pub(super) fn invalid_input(message: impl Into<String>) -> CliApiError { CliApiError::InvalidInput(message.into()) }

fn validate_factored_targets(target_groups: &[String], target_factors: &[f64], api: &str) -> CliApiResult<()> {
    if target_groups.is_empty() {
        return Err(invalid_input(format!("{api} requires at least one target group")));
    }
    if target_factors.len() != target_groups.len() {
        return Err(invalid_input("target_factors must match target_groups length"));
    }
    if target_factors.iter().any(|factor| !factor.is_finite() || *factor <= 0.0) {
        return Err(invalid_input("target_factors must contain only finite positive values"));
    }
    Ok(())
}

fn runtime_batch_error(error: crate::runtime::RuntimeBatchError) -> CliApiError { CliApiError::Runtime(error.to_string()) }

fn custom_runtime_import_error(error: crate::runtime::CustomRuntimeImportError) -> CliApiError {
    match error {
        crate::runtime::CustomRuntimeImportError::NotReady(error) => invalid_input(error.to_string()),
        error => invalid_input(format!("custom runtime import failed: {error:?}")),
    }
}

fn default_custom_runtime_profile_error(error: crate::runtime::DefaultCustomRuntimeProfileError) -> CliApiError {
    invalid_input(format!("default custom runtime profile failed: {error:?}"))
}

fn ensure_runtime_max_rounds(max_rounds: usize) -> CliApiResult<()> {
    if max_rounds == 0 {
        Err(invalid_input("runtime max_rounds must be positive"))
    } else {
        Ok(())
    }
}

fn ensure_win_rate_group_count(groups: &[Vec<String>]) -> CliApiResult<()> {
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    if group_count < 2 {
        Err(invalid_input("win_rate requires at least two non-empty groups"))
    } else {
        Ok(())
    }
}

fn win_rate_eval_rq(keep_rq: bool) -> f64 {
    if keep_rq {
        eval_name::DEFAULT_EVAL_RQ
    } else {
        eval_name::WIN_RATE_EVAL_RQ
    }
}

fn parse_score_mode(mode: &str) -> CliApiResult<ScoreMode> {
    match mode {
        "normal" | "pp" | "pd" => Ok(ScoreMode::Normal),
        "bang" | "!" | "qp" | "qd" => Ok(ScoreMode::Bang),
        _ => Err(invalid_input("mode must be 'normal' or 'bang'")),
    }
}

fn normalize_namer_pf_modes(modes: Option<Vec<String>>) -> CliApiResult<Vec<NamerPfMode>> {
    let Some(raw_modes) = modes else {
        return Ok(NamerPfMode::ALL.to_vec());
    };
    if raw_modes.is_empty() {
        return Ok(NamerPfMode::ALL.to_vec());
    }

    let mut out = Vec::new();
    for expected in NamerPfMode::ALL {
        if raw_modes.iter().any(|mode| mode == expected.label()) {
            out.push(expected);
        }
    }
    for mode in raw_modes {
        if !NamerPfMode::ALL.iter().any(|expected| mode == expected.label()) {
            return Err(invalid_input("namer_pf modes must be pp, pd, qp, or qd"));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;

//! High-level helper APIs aligned with `tswn-cli`.

mod bench;
mod parse;

use crate::Runner;
use crate::engine::update::UpdateType;
use crate::error::runner::RunnerError;
use crate::player::eval_name;
use crate::player::icon::icon_from_raw_name;
use crate::runtime_v2::{
    CustomRuntimeV2ImportConfig, NormalizedOutcome, NormalizedUpdateFrame, RuntimeV2BatchSummary, RuntimeV2NormalizedRun,
    RuntimeV2Runner, StrictRunDiff, default_custom_runtime_v2_import_config, normalize_legacy_run, runtime_v2_groups_win_rate,
    runtime_v2_score, strict_diff_runs,
};
use crate::win_rate::{WinRateSummary, WinRateTiming};

pub type CliApiResult<T> = Result<T, CliApiError>;

#[derive(Debug)]
pub enum CliApiError {
    InvalidInput(String),
    Runner(RunnerError),
    RuntimeV2(String),
}

impl std::fmt::Display for CliApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::Runner(err) => err.fmt(f),
            Self::RuntimeV2(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for CliApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidInput(_) => None,
            Self::Runner(err) => Some(err),
            Self::RuntimeV2(_) => None,
        }
    }
}

impl From<RunnerError> for CliApiError {
    fn from(value: RunnerError) -> Self { Self::Runner(value) }
}

#[derive(Debug, Clone, PartialEq)]
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

impl From<RuntimeV2BatchSummary> for WinRateResult {
    fn from(value: RuntimeV2BatchSummary) -> Self {
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
pub struct GroupWinRateResult {
    pub opponent: String,
    pub result: WinRateResult,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScoreResult {
    pub score: f64,
    pub wins: usize,
    pub total: usize,
    pub errors: usize,
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeV2ParityReport {
    pub legacy: RuntimeV2NormalizedRun,
    pub v2: RuntimeV2NormalizedRun,
    pub first_diff: Option<StrictRunDiff>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeV2ParityReport {
    pub matched: bool,
    pub first_diff: Option<String>,
    pub legacy: JsonRuntimeV2NormalizedRun,
    pub v2: JsonRuntimeV2NormalizedRun,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeV2NormalizedRun {
    pub rounds: Vec<JsonRuntimeV2NormalizedOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
    pub total_score: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeV2NormalizedOutcome {
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
    pub actions: Vec<JsonRuntimeV2ActionBoundary>,
    pub frames: Vec<JsonRuntimeV2UpdateFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeV2ActionBoundary {
    pub round: u64,
    pub actor: usize,
    pub target: usize,
    pub amount: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct JsonRuntimeV2UpdateFrame {
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

impl From<RuntimeV2ParityReport> for JsonRuntimeV2ParityReport {
    fn from(value: RuntimeV2ParityReport) -> Self {
        let RuntimeV2ParityReport { legacy, v2, first_diff } = value;
        Self {
            matched: first_diff.is_none(),
            first_diff: first_diff.map(|diff| format!("{diff:?}")),
            legacy: legacy.into(),
            v2: v2.into(),
        }
    }
}

impl From<RuntimeV2NormalizedRun> for JsonRuntimeV2NormalizedRun {
    fn from(value: RuntimeV2NormalizedRun) -> Self {
        Self {
            rounds: value.rounds.into_iter().map(Into::into).collect(),
            winner_team: value.winner_team,
            guard_exhausted: value.guard_exhausted,
            total_score: value.total_score,
        }
    }
}

impl From<NormalizedOutcome> for JsonRuntimeV2NormalizedOutcome {
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
                .map(|action| JsonRuntimeV2ActionBoundary {
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

impl From<NormalizedUpdateFrame> for JsonRuntimeV2UpdateFrame {
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
            update_type: runtime_v2_update_type_name(value.update_type),
        }
    }
}

pub fn runtime_v2_update_type_name(value: UpdateType) -> &'static str {
    match value {
        UpdateType::Win => "win",
        UpdateType::None => "none",
        UpdateType::NextLine => "next_line",
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NamerPfResult {
    pub group: Vec<String>,
    pub modes: Vec<String>,
    pub scores: Vec<f64>,
    pub total_score: f64,
}

#[derive(Debug, Clone, PartialEq)]
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

pub fn win_rate_summary(raw: &str, n: usize, eval_rq: Option<f64>, thread: u32) -> CliApiResult<WinRateResult> {
    let eval_rq = eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ);
    let groups = Runner::split_namerena_into_groups(raw.to_owned()).0;
    ensure_win_rate_group_count(&groups)?;
    runtime_v2_groups_win_rate(&groups, n.max(1), eval_rq, thread)
        .map(Into::into)
        .map_err(runtime_v2_batch_error)
}

pub fn team_win_rate_summary(
    team1: &str,
    team2: &str,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> CliApiResult<WinRateResult> {
    win_rate_summary(&format!("{team1}\n\n{team2}"), n, eval_rq, thread)
}

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

pub fn score(raw: &str, n: usize, mode: &str, eval_rq: Option<f64>, thread: u32) -> CliApiResult<ScoreResult> {
    let score_mode = parse_score_mode(mode)?;
    let (groups, _) = Runner::split_namerena_into_groups(raw.to_owned());
    let target_group = groups.into_iter().next().unwrap_or_default();
    if target_group.is_empty() {
        return Err(invalid_input("score requires at least one player"));
    }
    let summary = runtime_v2_score(
        &target_group,
        score_mode.modifier(),
        n.max(1),
        eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ),
        thread,
    )
    .map_err(runtime_v2_batch_error)?;
    Ok(ScoreResult {
        score: summary.score_10000(),
        wins: summary.wins,
        total: summary.total,
        errors: summary.errors + summary.guard_exhausted,
        init_nanos: summary.timing.init_nanos,
        fight_nanos: summary.timing.fight_nanos,
    })
}

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
            let summary = bench::batch_rate_for_group(player, target_groups, n.max(1), thread, eval_rq)?;
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
        .map(|player| bench::pair_rate_for_player(player, target_groups, teammates, head, n.max(1), thread, eval_rq))
        .collect()
}

pub fn to_diy(name: &str, old: bool, minions: bool) -> CliApiResult<String> { parse::export_player(name, old, minions) }

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

pub fn custom_runtime_v2_mixed_runner(raw: &str, config: CustomRuntimeV2ImportConfig<'_>) -> CliApiResult<RuntimeV2Runner> {
    RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config).map_err(custom_runtime_v2_import_error)
}

pub fn default_custom_runtime_v2_mixed_runner(raw: &str) -> CliApiResult<RuntimeV2Runner> {
    let config = default_custom_runtime_v2_import_config().map_err(default_custom_runtime_v2_profile_error)?;
    custom_runtime_v2_mixed_runner(raw, config)
}

pub fn custom_runtime_v2_normalized_run(
    raw: &str,
    max_rounds: usize,
    config: CustomRuntimeV2ImportConfig<'_>,
) -> CliApiResult<RuntimeV2NormalizedRun> {
    ensure_runtime_v2_max_rounds(max_rounds)?;
    let mut runner = custom_runtime_v2_mixed_runner(raw, config)?;
    Ok(runner.run_until_winner_normalized_rounds(max_rounds))
}

pub fn default_custom_runtime_v2_normalized_run(raw: &str, max_rounds: usize) -> CliApiResult<RuntimeV2NormalizedRun> {
    ensure_runtime_v2_max_rounds(max_rounds)?;
    let mut runner = default_custom_runtime_v2_mixed_runner(raw)?;
    Ok(runner.run_until_winner_normalized_rounds(max_rounds))
}

pub fn default_custom_runtime_v2_parity_report(raw: &str, max_rounds: usize) -> CliApiResult<RuntimeV2ParityReport> {
    ensure_runtime_v2_max_rounds(max_rounds)?;
    let mut legacy_runner = Runner::new_from_namerena_raw(raw.to_owned())?;
    let legacy = normalize_legacy_run(&mut legacy_runner, max_rounds);
    let v2 = default_custom_runtime_v2_normalized_run(raw, max_rounds)?;
    let first_diff = strict_diff_runs(&legacy, &v2).err();
    Ok(RuntimeV2ParityReport { legacy, v2, first_diff })
}

pub(super) fn invalid_input(message: impl Into<String>) -> CliApiError { CliApiError::InvalidInput(message.into()) }

fn runtime_v2_batch_error(error: crate::runtime_v2::RuntimeV2BatchError) -> CliApiError {
    CliApiError::RuntimeV2(error.to_string())
}

fn custom_runtime_v2_import_error(error: crate::runtime_v2::CustomRuntimeV2ImportError) -> CliApiError {
    match error {
        crate::runtime_v2::CustomRuntimeV2ImportError::NotReady(error) => invalid_input(error.to_string()),
        error => invalid_input(format!("custom runtime v2 import failed: {error:?}")),
    }
}

fn default_custom_runtime_v2_profile_error(error: crate::runtime_v2::DefaultCustomRuntimeV2ProfileError) -> CliApiError {
    invalid_input(format!("default custom runtime v2 profile failed: {error:?}"))
}

fn ensure_runtime_v2_max_rounds(max_rounds: usize) -> CliApiResult<()> {
    if max_rounds == 0 {
        Err(invalid_input("runtime v2 max_rounds must be positive"))
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
mod tests {
    use super::*;
    use crate::runtime_v2::{
        DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT, DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT,
        DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT, DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT,
        DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT, DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT,
        DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT, DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT,
        DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT, DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT, EntityIdx, SlotValue,
        TemplateSlotId, default_custom_runtime_v2_import_config,
    };

    #[test]
    fn runtime_v2_update_type_names_are_stable_json_tokens() {
        assert_eq!(runtime_v2_update_type_name(UpdateType::Win), "win");
        assert_eq!(runtime_v2_update_type_name(UpdateType::None), "none");
        assert_eq!(runtime_v2_update_type_name(UpdateType::NextLine), "next_line");
    }

    #[test]
    fn cli_api_custom_runtime_v2_mixed_runner_imports_custom_profile_raw() {
        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let bed2 = config.bed2_kind;
        let raw = "plain@red\nalpha@red@bed2\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

        let runner = custom_runtime_v2_mixed_runner(raw, config).expect("custom runtime v2 mixed runner should build");
        let legacy = Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy runner should build");
        let expected_round_order = legacy
            .world
            .players
            .iter()
            .map(|plr_id| u32::try_from(*plr_id).expect("legacy player id should fit runtime v2 entity index"))
            .collect::<Vec<_>>();

        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
        assert_eq!(runner.runtime().entities.get(EntityIdx(2)).unwrap().template.kind, bed2);
        assert_eq!(
            runner.runtime().world.round_order().iter().map(|idx| idx.0).collect::<Vec<_>>(),
            expected_round_order
        );
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_mixed_runner_imports_ol_minion_overlays() {
        let raw = "plain@red\n\
alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";

        let runner = default_custom_runtime_v2_mixed_runner(raw).expect("default custom runtime v2 mixed runner should build");
        let runtime = runner.runtime();
        let summon_skill = runtime
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT)
            .expect("default profile should register summon skill");
        let fire_skill = runtime
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT)
            .expect("default profile should register summon fire skill");
        let explode_skill = runtime
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT)
            .expect("default profile should register summon explode skill");
        let possess_skill = runtime
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT)
            .expect("default profile should register possess skill");
        assert!(runtime.skill_handlers.get(summon_skill).is_some());
        assert!(runtime.skill_handlers.get(fire_skill).is_some());
        assert!(runtime.skill_handlers.get(explode_skill).is_some());
        assert!(runtime.skill_handlers.get(possess_skill).is_some());
        let summon_kind = runtime
            .registry
            .player_kinds()
            .iter()
            .find(|kind| kind.export_name == DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT)
            .expect("default profile should register summon kind")
            .id;
        let shadow_kind = runtime
            .registry
            .player_kinds()
            .iter()
            .find(|kind| kind.export_name == DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT)
            .expect("default profile should register shadow kind")
            .id;
        let zombie_kind = runtime
            .registry
            .player_kinds()
            .iter()
            .find(|kind| kind.export_name == DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT)
            .expect("default profile should register zombie kind")
            .id;
        let summon_template_slot = runtime
            .registry
            .template_slots()
            .iter()
            .find(|slot| slot.export_name == DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT)
            .expect("default profile should reserve summon template slot")
            .id;
        let shadow_template_slot = runtime
            .registry
            .template_slots()
            .iter()
            .find(|slot| slot.export_name == DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT)
            .expect("default profile should reserve shadow template slot")
            .id;
        let zombie_template_slot = runtime
            .registry
            .template_slots()
            .iter()
            .find(|slot| slot.export_name == DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT)
            .expect("default profile should reserve zombie template slot")
            .id;
        assert_eq!(summon_template_slot, TemplateSlotId(0));

        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().template.skills.skills(),
            &[summon_skill]
        );
        let SlotValue::PlayerTemplate(summon_template) = runtime
            .template_slots
            .get(summon_template_slot)
            .expect("default cli api should populate summon template slot")
        else {
            panic!("default cli api summon overlay slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.max_hp, 123);
        assert_eq!(summon_template.skills.skills(), &[fire_skill, fire_skill, explode_skill]);
        assert_eq!(summon_template.skills.active_order(), &[1, 0]);

        let SlotValue::PlayerTemplate(shadow_template) = runtime
            .template_slots
            .get(shadow_template_slot)
            .expect("default cli api should populate shadow template slot")
        else {
            panic!("default cli api shadow overlay slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.kind, shadow_kind);
        assert_eq!(shadow_template.max_hp, 88);
        assert_eq!(shadow_template.skills.skills(), &[possess_skill]);

        let SlotValue::PlayerTemplate(zombie_template) = runtime
            .template_slots
            .get(zombie_template_slot)
            .expect("default cli api should populate zombie template slot")
        else {
            panic!("default cli api zombie overlay slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.kind, zombie_kind);
        assert_eq!(zombie_template.max_hp, 77);
        assert!(zombie_template.skills.skills().is_empty());
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_mixed_runner_rejects_unimplemented_minion_heal() {
        let raw = "alpha@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{\"sklheal\":3}}}\n\n\
beta@blue\n";

        let err = default_custom_runtime_v2_mixed_runner(raw)
            .expect_err("default custom runtime v2 profile must reject imported skills without handlers");

        assert_eq!(
            err.to_string(),
            "runtime v2 missing skill handlers: custom.minion.heal (id 4) used by template slot 2"
        );
    }

    #[test]
    fn cli_api_custom_runtime_v2_normalized_run_executes_plain_raw() {
        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let raw = "left@red\n\nright@blue\n";

        let run = custom_runtime_v2_normalized_run(raw, 1, config).expect("custom runtime v2 normalized run should execute");

        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.guard_exhausted, run.winner_team.is_none());
        assert!(!run.rounds[0].frames.is_empty());
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_normalized_run_executes_plain_raw() {
        let raw = "left@red\n\nright@blue\n";

        let run =
            default_custom_runtime_v2_normalized_run(raw, 1).expect("default custom runtime v2 normalized run should execute");

        assert_eq!(run.rounds.len(), 1);
        assert_eq!(run.guard_exhausted, run.winner_team.is_none());
        assert!(!run.rounds[0].frames.is_empty());
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_parity_report_matches_converged_first_round() {
        let report = default_custom_runtime_v2_parity_report("left@red\n\nright@blue\n", 1)
            .expect("default custom runtime v2 parity report should execute");

        assert_eq!(report.legacy.rounds.len(), 1);
        assert_eq!(report.v2.rounds.len(), 1);
        assert_eq!(report.first_diff, None);

        #[cfg(not(feature = "no_debug"))]
        assert_eq!(report.legacy, report.v2);

        #[cfg(feature = "no_debug")]
        {
            assert_eq!(report.legacy.total_score, report.v2.total_score);
            assert_eq!(report.legacy.rounds[0].frames, report.v2.rounds[0].frames);
            assert_eq!(report.legacy.rounds[0].rng, report.v2.rounds[0].rng);
        }
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_normalized_run_executes_bed2_summon_overlay() {
        let raw = "alpha@red+bed2[3000]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire1\":5}}}\n\n\
beta@blue\n";

        let run = default_custom_runtime_v2_normalized_run(raw, 1).expect("default custom runtime v2 summon run should execute");

        assert_eq!(run.rounds.len(), 1);
        assert_eq!(
            run.rounds[0]
                .frames
                .iter()
                .take(2)
                .map(|frame| frame.message.as_str())
                .collect::<Vec<_>>(),
            vec!["[0]使用[血祭]", "召唤出[1]"]
        );
        assert_eq!(run.rounds[0].frames[0].score, 60);
        assert_eq!(run.rounds[0].frames[1].target, 2);
        assert_eq!(run.rounds[0].hp, vec![3000, 295, 123]);
        assert_eq!(run.rounds[0].alive, vec![true, true, true]);
        assert_eq!(run.rounds[0].team_alive, vec![vec![0, 2], vec![1]]);
        assert_eq!(run.rounds[0].flat_alive, vec![0, 2, 1]);
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_normalized_run_executes_spawned_summon_fire() {
        let raw = "alpha@red+bed2[3000]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire1\":5}}}\n\n\
beta@blue\n";

        let run =
            default_custom_runtime_v2_normalized_run(raw, 3).expect("default custom runtime v2 summon fire run should execute");

        assert_eq!(run.rounds.len(), 3);
        assert_eq!(run.rounds[2].actions[0].actor, 2);
        let fire = &run.rounds[2].frames[0];
        assert_eq!(fire.message, "[0]使用[火球术]");
        assert_eq!(fire.caster, 2);
        assert_eq!(fire.target, 1);
        assert_eq!(fire.score, 1);

        let resolution = &run.rounds[2].frames[1];
        match resolution.message.as_str() {
            "[0]攻击[1]" => {
                assert_eq!(resolution.caster, 2);
                assert_eq!(resolution.target, 1);
                assert!(resolution.score > 0);
            }
            "[0][回避]了攻击" => {
                assert_eq!(resolution.caster, 1);
                assert_eq!(resolution.target, 2);
                assert_eq!(resolution.score, 20);
            }
            message => panic!("unexpected summon fire resolution: {message}"),
        }
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_normalized_run_executes_spawned_summon_explode() {
        let raw = "alpha@red+bed2[3000]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklexplode\":5}}}\n\n\
beta@blue\n";

        let run = default_custom_runtime_v2_normalized_run(raw, 3)
            .expect("default custom runtime v2 summon explode run should execute");

        assert_eq!(run.rounds.len(), 3);
        assert_eq!(run.rounds[2].actions[0].actor, 2);
        assert_eq!(
            run.rounds[2]
                .frames
                .iter()
                .take(2)
                .map(|frame| frame.message.as_str())
                .collect::<Vec<_>>(),
            vec!["[0]使用[自爆]", "[1]受到[2]点伤害[s_dmg160]"]
        );
        assert_eq!(run.rounds[2].frames[0].caster, 2);
        assert_eq!(run.rounds[2].frames[0].target, 1);
        assert_eq!(run.rounds[2].frames[0].score, 0);
        assert!(!run.rounds[2].alive[2]);
        assert_eq!(run.rounds[2].team_alive[0], vec![0]);
        assert!(run.rounds[2].hp[1] < run.rounds[1].hp[1]);
    }

    #[test]
    fn cli_api_custom_runtime_v2_normalized_run_rejects_zero_max_rounds() {
        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let err = custom_runtime_v2_normalized_run("left@red\n\nright@blue\n", 0, config)
            .expect_err("custom runtime v2 normalized run should reject zero max rounds");

        assert_eq!(err.to_string(), "runtime v2 max_rounds must be positive");
    }

    #[test]
    fn cli_api_default_custom_runtime_v2_normalized_run_rejects_zero_max_rounds() {
        let err = default_custom_runtime_v2_normalized_run("left@red\n\nright@blue\n", 0)
            .expect_err("default custom runtime v2 normalized run should reject zero max rounds");

        assert_eq!(err.to_string(), "runtime v2 max_rounds must be positive");
    }
}

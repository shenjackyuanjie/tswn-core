//! High-level helper APIs aligned with `tswn-cli`.

mod bench;
mod parse;

use crate::Runner;
use crate::error::runner::RunnerError;
use crate::player::eval_name;
use crate::player::icon::icon_from_raw_name;
use crate::win_rate::{WinRateSummary, WinRateTiming, groups_win_rate};

pub type CliApiResult<T> = Result<T, CliApiError>;

#[derive(Debug)]
pub enum CliApiError {
    InvalidInput(String),
    Runner(RunnerError),
}

impl std::fmt::Display for CliApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::Runner(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for CliApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidInput(_) => None,
            Self::Runner(err) => Some(err),
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

impl ScoreResult {
    fn from_summary(summary: BenchSummary) -> Self {
        Self {
            score: summary.score_10000(),
            wins: summary.wins,
            total: summary.total,
            errors: summary.errors,
            init_nanos: summary.timing.init_nanos,
            fight_nanos: summary.timing.fight_nanos,
        }
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
pub(super) struct BenchSummary {
    wins: usize,
    total: usize,
    errors: usize,
    timing: WinRateTiming,
}

impl BenchSummary {
    fn score_10000(self) -> f64 { self.wins as f64 * 10_000.0 / self.total.max(1) as f64 }
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
    groups_win_rate(&groups, n.max(1), eval_rq, thread).map(Into::into).map_err(Into::into)
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
    let summary = bench::run_score_inner(
        &target_group,
        score_mode.modifier(),
        n.max(1),
        eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ),
        thread,
    );
    Ok(ScoreResult::from_summary(summary))
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
                .collect::<Vec<_>>();
            let total_score = scores.iter().sum();
            NamerPfResult {
                group,
                modes: labels.clone(),
                scores,
                total_score,
            }
        })
        .collect())
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

pub(super) fn invalid_input(message: impl Into<String>) -> CliApiError { CliApiError::InvalidInput(message.into()) }

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

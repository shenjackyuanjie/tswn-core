//! Python-facing helpers aligned with the high-level `tswn-cli` commands.

use pyo3::{PyResult, exceptions::PyValueError, pyclass, pyfunction, pymethods};
use tswn_core::cli_api::{self as core_cli_api, CliApiError};

use crate::wrapper;

#[pyclass(skip_from_py_object)]
#[pyo3(name = "WinRateResult")]
#[derive(Clone)]
pub struct PyWinRateResult {
    wins: usize,
    total: usize,
    win_rate: f64,
    init_nanos: u128,
    fight_nanos: u128,
}

impl From<core_cli_api::WinRateResult> for PyWinRateResult {
    fn from(value: core_cli_api::WinRateResult) -> Self {
        Self {
            wins: value.wins,
            total: value.total,
            win_rate: value.win_rate,
            init_nanos: value.init_nanos,
            fight_nanos: value.fight_nanos,
        }
    }
}

#[pymethods]
impl PyWinRateResult {
    #[getter]
    fn wins(&self) -> usize { self.wins }

    #[getter]
    fn total(&self) -> usize { self.total }

    #[getter]
    fn win_rate(&self) -> f64 { self.win_rate }

    #[getter]
    fn init_nanos(&self) -> u128 { self.init_nanos }

    #[getter]
    fn fight_nanos(&self) -> u128 { self.fight_nanos }

    fn __repr__(&self) -> String {
        format!(
            "WinRateResult(wins={}, total={}, win_rate={:.6})",
            self.wins, self.total, self.win_rate
        )
    }
}

#[pyclass(skip_from_py_object)]
#[pyo3(name = "ScoreResult")]
#[derive(Clone)]
pub struct PyScoreResult {
    score: f64,
    wins: usize,
    total: usize,
    errors: usize,
    init_nanos: u128,
    fight_nanos: u128,
}

impl From<core_cli_api::ScoreResult> for PyScoreResult {
    fn from(value: core_cli_api::ScoreResult) -> Self {
        Self {
            score: value.score,
            wins: value.wins,
            total: value.total,
            errors: value.errors,
            init_nanos: value.init_nanos,
            fight_nanos: value.fight_nanos,
        }
    }
}

#[pymethods]
impl PyScoreResult {
    #[getter]
    fn score(&self) -> f64 { self.score }

    #[getter]
    fn wins(&self) -> usize { self.wins }

    #[getter]
    fn total(&self) -> usize { self.total }

    #[getter]
    fn errors(&self) -> usize { self.errors }

    #[getter]
    fn init_nanos(&self) -> u128 { self.init_nanos }

    #[getter]
    fn fight_nanos(&self) -> u128 { self.fight_nanos }

    fn __repr__(&self) -> String {
        format!(
            "ScoreResult(score={:.6}, wins={}, total={}, errors={})",
            self.score, self.wins, self.total, self.errors
        )
    }
}

#[pyclass(skip_from_py_object)]
#[pyo3(name = "NamerPfResult")]
#[derive(Clone)]
pub struct PyNamerPfResult {
    group: Vec<String>,
    modes: Vec<String>,
    scores: Vec<f64>,
    total_score: f64,
}

impl From<core_cli_api::NamerPfResult> for PyNamerPfResult {
    fn from(value: core_cli_api::NamerPfResult) -> Self {
        Self {
            group: value.group,
            modes: value.modes,
            scores: value.scores,
            total_score: value.total_score,
        }
    }
}

#[pymethods]
impl PyNamerPfResult {
    #[getter]
    fn group(&self) -> Vec<String> { self.group.clone() }

    #[getter]
    fn modes(&self) -> Vec<String> { self.modes.clone() }

    #[getter]
    fn scores(&self) -> Vec<f64> { self.scores.clone() }

    #[getter]
    fn total_score(&self) -> f64 { self.total_score }

    fn as_line(&self, precision: usize) -> String {
        self.scores
            .iter()
            .copied()
            .chain(std::iter::once(self.total_score))
            .map(|score| format_rate(score, precision))
            .collect::<Vec<_>>()
            .join("|")
    }

    fn __repr__(&self) -> String {
        format!(
            "NamerPfResult(group={:?}, modes={:?}, total_score={:.6})",
            self.group, self.modes, self.total_score
        )
    }
}

#[pyclass(skip_from_py_object)]
#[pyo3(name = "BatchRateResult")]
#[derive(Clone)]
pub struct PyBatchRateResult {
    label: String,
    avg_win_rate: f64,
    aggregate_win_rate: f64,
    wins: usize,
    total: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    init_nanos: u128,
    fight_nanos: u128,
}

impl From<core_cli_api::BatchRateResult> for PyBatchRateResult {
    fn from(value: core_cli_api::BatchRateResult) -> Self {
        Self {
            label: value.label,
            avg_win_rate: value.avg_win_rate,
            aggregate_win_rate: value.aggregate_win_rate,
            wins: value.wins,
            total: value.total,
            valid_matchups: value.valid_matchups,
            skipped_matchups: value.skipped_matchups,
            init_nanos: value.init_nanos,
            fight_nanos: value.fight_nanos,
        }
    }
}

#[pymethods]
impl PyBatchRateResult {
    #[getter]
    fn label(&self) -> String { self.label.clone() }

    #[getter]
    fn avg_win_rate(&self) -> f64 { self.avg_win_rate }

    #[getter]
    fn aggregate_win_rate(&self) -> f64 { self.aggregate_win_rate }

    #[getter]
    fn wins(&self) -> usize { self.wins }

    #[getter]
    fn total(&self) -> usize { self.total }

    #[getter]
    fn valid_matchups(&self) -> usize { self.valid_matchups }

    #[getter]
    fn skipped_matchups(&self) -> usize { self.skipped_matchups }

    #[getter]
    fn init_nanos(&self) -> u128 { self.init_nanos }

    #[getter]
    fn fight_nanos(&self) -> u128 { self.fight_nanos }

    fn __repr__(&self) -> String {
        format!(
            "BatchRateResult(label={:?}, avg_win_rate={:.6}, valid_matchups={}, skipped_matchups={})",
            self.label, self.avg_win_rate, self.valid_matchups, self.skipped_matchups
        )
    }
}

#[pyclass(skip_from_py_object)]
#[pyo3(name = "PairRateResult")]
#[derive(Clone)]
pub struct PyPairRateResult {
    label: String,
    final_score: f64,
    head: usize,
    selected: usize,
    top_pairs: Vec<(String, f64)>,
    aggregate_win_rate: f64,
    wins: usize,
    total: usize,
    valid_matchups: usize,
    skipped_matchups: usize,
    init_nanos: u128,
    fight_nanos: u128,
}

impl From<core_cli_api::PairRateResult> for PyPairRateResult {
    fn from(value: core_cli_api::PairRateResult) -> Self {
        Self {
            label: value.label,
            final_score: value.final_score,
            head: value.head,
            selected: value.selected,
            top_pairs: value.top_pairs.into_iter().map(|pair| (pair.name, pair.rate)).collect(),
            aggregate_win_rate: value.aggregate_win_rate,
            wins: value.wins,
            total: value.total,
            valid_matchups: value.valid_matchups,
            skipped_matchups: value.skipped_matchups,
            init_nanos: value.init_nanos,
            fight_nanos: value.fight_nanos,
        }
    }
}

#[pymethods]
impl PyPairRateResult {
    #[getter]
    fn label(&self) -> String { self.label.clone() }

    #[getter]
    fn final_score(&self) -> f64 { self.final_score }

    #[getter]
    fn head(&self) -> usize { self.head }

    #[getter]
    fn selected(&self) -> usize { self.selected }

    #[getter]
    fn top_pairs(&self) -> Vec<(String, f64)> { self.top_pairs.clone() }

    #[getter]
    fn aggregate_win_rate(&self) -> f64 { self.aggregate_win_rate }

    #[getter]
    fn wins(&self) -> usize { self.wins }

    #[getter]
    fn total(&self) -> usize { self.total }

    #[getter]
    fn valid_matchups(&self) -> usize { self.valid_matchups }

    #[getter]
    fn skipped_matchups(&self) -> usize { self.skipped_matchups }

    #[getter]
    fn init_nanos(&self) -> u128 { self.init_nanos }

    #[getter]
    fn fight_nanos(&self) -> u128 { self.fight_nanos }

    fn __repr__(&self) -> String {
        format!(
            "PairRateResult(label={:?}, final_score={:.6}, selected={}, head={})",
            self.label, self.final_score, self.selected, self.head
        )
    }
}

#[pyclass(skip_from_py_object)]
#[pyo3(name = "IconInfo")]
#[derive(Clone)]
pub struct PyIconInfo {
    border_style: usize,
    shapes: Vec<usize>,
    bg_color_idx: usize,
    bg_color: (u8, u8, u8),
    fg_color_indices: Vec<usize>,
    fg_colors: Vec<(u8, u8, u8)>,
    colors_consumed: usize,
}

impl From<core_cli_api::IconInfo> for PyIconInfo {
    fn from(value: core_cli_api::IconInfo) -> Self {
        Self {
            border_style: value.border_style,
            shapes: value.shapes,
            bg_color_idx: value.bg_color_idx,
            bg_color: (value.bg_color[0], value.bg_color[1], value.bg_color[2]),
            fg_color_indices: value.fg_color_indices,
            fg_colors: value.fg_colors.into_iter().map(|color| (color[0], color[1], color[2])).collect(),
            colors_consumed: value.colors_consumed,
        }
    }
}

#[pymethods]
impl PyIconInfo {
    #[getter]
    fn border_style(&self) -> usize { self.border_style }

    #[getter]
    fn shapes(&self) -> Vec<usize> { self.shapes.clone() }

    #[getter]
    fn bg_color_idx(&self) -> usize { self.bg_color_idx }

    #[getter]
    fn bg_color(&self) -> (u8, u8, u8) { self.bg_color }

    #[getter]
    fn fg_color_indices(&self) -> Vec<usize> { self.fg_color_indices.clone() }

    #[getter]
    fn fg_colors(&self) -> Vec<(u8, u8, u8)> { self.fg_colors.clone() }

    #[getter]
    fn colors_consumed(&self) -> usize { self.colors_consumed }

    fn __repr__(&self) -> String {
        format!(
            "IconInfo(border_style={}, shapes={:?}, bg_color={:?}, fg_colors={:?})",
            self.border_style, self.shapes, self.bg_color, self.fg_colors
        )
    }
}

fn map_cli_error(err: CliApiError) -> pyo3::PyErr {
    match err {
        CliApiError::InvalidInput(message) => PyValueError::new_err(message),
        CliApiError::Runner(err) => wrapper::error::PyRunnerError::new(err).into(),
    }
}

#[pyfunction(signature = (raw, n, eval_rq=None, thread=0))]
pub fn win_rate_summary(raw: String, n: usize, eval_rq: Option<f64>, thread: u32) -> PyResult<PyWinRateResult> {
    core_cli_api::win_rate_summary(&raw, n, eval_rq, thread)
        .map(Into::into)
        .map_err(map_cli_error)
}

#[pyfunction(signature = (team1, team2, n, eval_rq=None, thread=0))]
pub fn team_win_rate_summary(
    team1: String,
    team2: String,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<PyWinRateResult> {
    core_cli_api::team_win_rate_summary(&team1, &team2, n, eval_rq, thread)
        .map(Into::into)
        .map_err(map_cli_error)
}

#[pyfunction(signature = (target, against, n, eval_rq=None, thread=0))]
pub fn group_win_rate_summary(
    target: String,
    against: Vec<String>,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<Vec<(String, PyWinRateResult)>> {
    core_cli_api::group_win_rate_summary(&target, &against, n, eval_rq, thread)
        .map(|results| results.into_iter().map(|entry| (entry.opponent, entry.result.into())).collect())
        .map_err(map_cli_error)
}

#[pyfunction(signature = (raw, n, mode="normal", eval_rq=None, thread=0))]
pub fn score(raw: String, n: usize, mode: &str, eval_rq: Option<f64>, thread: u32) -> PyResult<PyScoreResult> {
    core_cli_api::score(&raw, n, mode, eval_rq, thread).map(Into::into).map_err(map_cli_error)
}

#[pyfunction(signature = (raw, n, modes=None, keep_rq=false, thread=0))]
pub fn namer_pf(raw: String, n: usize, modes: Option<Vec<String>>, keep_rq: bool, thread: u32) -> PyResult<Vec<PyNamerPfResult>> {
    core_cli_api::namer_pf(&raw, n, modes, keep_rq, thread)
        .map(|results| results.into_iter().map(Into::into).collect())
        .map_err(map_cli_error)
}

#[pyfunction(signature = (target_groups, player_groups, n, player_labels=None, keep_rq=false, thread=0))]
pub fn batch_rate(
    target_groups: Vec<String>,
    player_groups: Vec<String>,
    n: usize,
    player_labels: Option<Vec<String>>,
    keep_rq: bool,
    thread: u32,
) -> PyResult<Vec<PyBatchRateResult>> {
    core_cli_api::batch_rate(&target_groups, &player_groups, n, player_labels, keep_rq, thread)
        .map(|results| results.into_iter().map(Into::into).collect())
        .map_err(map_cli_error)
}

#[pyfunction(signature = (target_groups, players, teammates, head, n, keep_rq=false, thread=0))]
pub fn pair_rate(
    target_groups: Vec<String>,
    players: Vec<String>,
    teammates: Vec<String>,
    head: usize,
    n: usize,
    keep_rq: bool,
    thread: u32,
) -> PyResult<Vec<PyPairRateResult>> {
    core_cli_api::pair_rate(&target_groups, &players, &teammates, head, n, keep_rq, thread)
        .map(|results| results.into_iter().map(Into::into).collect())
        .map_err(map_cli_error)
}

#[pyfunction(signature = (name, old=false, minions=false))]
pub fn to_diy(name: String, old: bool, minions: bool) -> PyResult<String> {
    core_cli_api::to_diy(&name, old, minions).map_err(map_cli_error)
}

#[pyfunction(signature = (names, old=false, minions=false))]
pub fn to_diy_batch(names: Vec<String>, old: bool, minions: bool) -> PyResult<Vec<String>> {
    core_cli_api::to_diy_batch(&names, old, minions).map_err(map_cli_error)
}

#[pyfunction]
pub fn icon_info(name: String) -> PyIconInfo { core_cli_api::icon_info(&name).into() }

#[pyfunction(signature = (content, double_plus=false))]
pub fn parse_group_lines(content: String, double_plus: bool) -> Vec<String> {
    core_cli_api::parse_group_lines(&content, double_plus)
}

fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

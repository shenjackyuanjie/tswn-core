//! Python-facing helpers aligned with the high-level `tswn-cli` commands.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use pyo3::{PyResult, exceptions::PyValueError, pyclass, pyfunction, pymethods};
use tswn_core::engine::storage::Storage;
use tswn_core::player::icon::icon_from_raw_name;
use tswn_core::player::{Player, eval_name};
use tswn_core::win_rate::{WinRateTiming, groups_win_rate, resolve_win_rate_workers};
use tswn_core::{Runner, win_rate::WinRateSummary};

use crate::wrapper;

const BENCH_PARALLEL_THRESHOLD: usize = 64;

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

impl From<WinRateSummary> for PyWinRateResult {
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
    init_nanos: u128,
    fight_nanos: u128,
}

impl PyScoreResult {
    fn from_summary(summary: BenchSummary) -> Self {
        Self {
            score: summary.score_10000(),
            wins: summary.wins,
            total: summary.total,
            init_nanos: summary.timing.init_nanos,
            fight_nanos: summary.timing.fight_nanos,
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
    fn init_nanos(&self) -> u128 { self.init_nanos }

    #[getter]
    fn fight_nanos(&self) -> u128 { self.fight_nanos }

    fn __repr__(&self) -> String { format!("ScoreResult(score={:.6}, wins={}, total={})", self.score, self.wins, self.total) }
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

#[derive(Debug, Clone, Copy)]
enum ScoreMode {
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
enum NamerPfMode {
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
struct BenchSummary {
    wins: usize,
    total: usize,
    timing: WinRateTiming,
}

impl BenchSummary {
    fn score_10000(self) -> f64 { self.wins as f64 * 10_000.0 / self.total.max(1) as f64 }
}

#[derive(Debug, Clone, Copy, Default)]
struct BatchSummary {
    avg: f64,
    aggregate_rate: f64,
    wins: usize,
    total: usize,
    timing: WinRateTiming,
    valid_matchups: usize,
    skipped_matchups: usize,
}

#[pyfunction(signature = (raw, n, eval_rq=None, thread=0))]
pub fn win_rate_summary(raw: String, n: usize, eval_rq: Option<f64>, thread: u32) -> PyResult<PyWinRateResult> {
    let eval_rq = eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ);
    let groups = Runner::split_namerena_into_groups(raw).0;
    ensure_win_rate_group_count(&groups)?;
    groups_win_rate(&groups, n.max(1), eval_rq, thread)
        .map(Into::into)
        .map_err(wrapper::error::PyRunnerError::new)
        .map_err(Into::into)
}

#[pyfunction(signature = (team1, team2, n, eval_rq=None, thread=0))]
pub fn team_win_rate_summary(
    team1: String,
    team2: String,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<PyWinRateResult> {
    win_rate_summary(format!("{team1}\n\n{team2}"), n, eval_rq, thread)
}

#[pyfunction(signature = (target, against, n, eval_rq=None, thread=0))]
pub fn group_win_rate_summary(
    target: String,
    against: Vec<String>,
    n: usize,
    eval_rq: Option<f64>,
    thread: u32,
) -> PyResult<Vec<(String, PyWinRateResult)>> {
    let eval_rq = eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ);
    against
        .into_iter()
        .map(|opponent| {
            let result = team_win_rate_summary(target.clone(), opponent.clone(), n, Some(eval_rq), thread)?;
            Ok((opponent, result))
        })
        .collect()
}

#[pyfunction(signature = (raw, n, mode="normal", eval_rq=None, thread=0))]
pub fn score(raw: String, n: usize, mode: &str, eval_rq: Option<f64>, thread: u32) -> PyResult<PyScoreResult> {
    let score_mode = parse_score_mode(mode)?;
    let (groups, _) = Runner::split_namerena_into_groups(raw);
    let target_group = groups.into_iter().next().unwrap_or_default();
    if target_group.is_empty() {
        return Err(PyValueError::new_err("score requires at least one player"));
    }
    let summary = run_score_inner(
        &target_group,
        score_mode.modifier(),
        n.max(1),
        eval_rq.unwrap_or(eval_name::WIN_RATE_EVAL_RQ),
        thread,
    );
    Ok(PyScoreResult::from_summary(summary))
}

#[pyfunction(signature = (raw, n, modes=None, keep_rq=false, thread=0))]
pub fn namer_pf(raw: String, n: usize, modes: Option<Vec<String>>, keep_rq: bool, thread: u32) -> PyResult<Vec<PyNamerPfResult>> {
    let groups = parse_plus_separated_groups(&raw);
    if groups.is_empty() {
        return Err(PyValueError::new_err("namer_pf requires at least one non-empty group"));
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
                    namer_pf_score(&group, modifier, duplicate, n.max(1), thread, eval_rq)
                })
                .collect::<Vec<_>>();
            let total_score = scores.iter().sum();
            PyNamerPfResult {
                group,
                modes: labels.clone(),
                scores,
                total_score,
            }
        })
        .collect())
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
    if target_groups.is_empty() {
        return Err(PyValueError::new_err("batch_rate requires at least one target group"));
    }
    if player_groups.is_empty() {
        return Err(PyValueError::new_err("batch_rate requires at least one player group"));
    }
    let labels = player_labels.unwrap_or_else(|| player_groups.clone());
    if labels.len() != player_groups.len() {
        return Err(PyValueError::new_err("player_labels must match player_groups length"));
    }
    let eval_rq = win_rate_eval_rq(keep_rq);
    player_groups
        .iter()
        .zip(labels.iter())
        .map(|(player, label)| {
            let summary = batch_rate_for_group(player, &target_groups, n.max(1), thread, eval_rq)?;
            Ok(PyBatchRateResult {
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
    if target_groups.is_empty() {
        return Err(PyValueError::new_err("pair_rate requires at least one target group"));
    }
    if players.is_empty() {
        return Err(PyValueError::new_err("pair_rate requires at least one player"));
    }
    if teammates.is_empty() {
        return Err(PyValueError::new_err("pair_rate requires at least one teammate"));
    }
    if head == 0 {
        return Err(PyValueError::new_err("head must be positive"));
    }

    let eval_rq = win_rate_eval_rq(keep_rq);
    players
        .iter()
        .map(|player| pair_rate_for_player(player, &target_groups, &teammates, head, n.max(1), thread, eval_rq))
        .collect()
}

#[pyfunction(signature = (name, old=false, minions=false))]
pub fn to_diy(name: String, old: bool, minions: bool) -> PyResult<String> { export_player(&name, old, minions) }

#[pyfunction(signature = (names, old=false, minions=false))]
pub fn to_diy_batch(names: Vec<String>, old: bool, minions: bool) -> PyResult<Vec<String>> {
    names.into_iter().map(|name| export_player(&name, old, minions)).collect()
}

#[pyfunction]
pub fn icon_info(name: String) -> PyIconInfo {
    let icon = icon_from_raw_name(&name);
    PyIconInfo {
        border_style: icon.border_style,
        shapes: icon.shapes,
        bg_color_idx: icon.bg_color_idx,
        bg_color: (icon.bg_color[0], icon.bg_color[1], icon.bg_color[2]),
        fg_color_indices: icon.fg_color_indices,
        fg_colors: icon.fg_colors.into_iter().map(|c| (c[0], c[1], c[2])).collect(),
        colors_consumed: icon.colors_consumed,
    }
}

#[pyfunction(signature = (content, double_plus=false))]
pub fn parse_group_lines(content: String, double_plus: bool) -> Vec<String> {
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

fn ensure_win_rate_group_count(groups: &[Vec<String>]) -> PyResult<()> {
    let group_count = groups.iter().filter(|g| !g.is_empty()).count();
    if group_count < 2 {
        Err(PyValueError::new_err("win_rate requires at least two non-empty groups"))
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

fn parse_score_mode(mode: &str) -> PyResult<ScoreMode> {
    match mode {
        "normal" | "pp" | "pd" => Ok(ScoreMode::Normal),
        "bang" | "!" | "qp" | "qd" => Ok(ScoreMode::Bang),
        _ => Err(PyValueError::new_err("mode must be 'normal' or 'bang'")),
    }
}

fn normalize_namer_pf_modes(modes: Option<Vec<String>>) -> PyResult<Vec<NamerPfMode>> {
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
            return Err(PyValueError::new_err("namer_pf modes must be pp, pd, qp, or qd"));
        }
    }
    Ok(out)
}

fn run_score_inner(target_group: &[String], modifier: &str, n: usize, eval_rq: f64, thread: u32) -> BenchSummary {
    let workers = resolve_win_rate_workers(thread, n);
    if workers <= 1 || n < BENCH_PARALLEL_THRESHOLD {
        let (wins, total, timing) = run_score_range(target_group, modifier, 0, n, eval_rq);
        return BenchSummary { wins, total, timing };
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
        let (wins, total, timing) = handle.join().expect("score worker thread panicked");
        merged.wins += wins;
        merged.total += total;
        merged.timing.merge(timing);
    }
    merged
}

fn run_score_worker(
    target_group: &[String],
    modifier: &str,
    next: &AtomicUsize,
    end: usize,
    eval_rq: f64,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    loop {
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= end {
            break;
        }
        run_score_round(
            target_group,
            modifier,
            i,
            eval_rq,
            &mut bench_input,
            &mut wins,
            &mut total,
            &mut timing,
        );
    }

    (wins, total, timing)
}

fn run_score_range(
    target_group: &[String],
    modifier: &str,
    start: usize,
    end: usize,
    eval_rq: f64,
) -> (usize, usize, WinRateTiming) {
    let mut wins = 0usize;
    let mut total = 0usize;
    let mut timing = WinRateTiming::default();
    let mut bench_input = String::with_capacity(target_group.iter().map(|name| name.len() + 1).sum::<usize>() + 96);

    for i in start..end {
        run_score_round(
            target_group,
            modifier,
            i,
            eval_rq,
            &mut bench_input,
            &mut wins,
            &mut total,
            &mut timing,
        );
    }

    (wins, total, timing)
}

fn run_score_round(
    target_group: &[String],
    modifier: &str,
    round: usize,
    eval_rq: f64,
    bench_input: &mut String,
    wins: &mut usize,
    total: &mut usize,
    timing: &mut WinRateTiming,
) {
    build_js_score_match_input(target_group, modifier, round, bench_input);

    let t_init = Instant::now();
    let (groups, seed) = Runner::split_namerena_into_groups(bench_input.clone());
    let Ok(mut runner) = Runner::new_from_groups_with_seed_and_eval_rq_uncached(&groups, &seed, eval_rq) else {
        return;
    };
    let target_team: Vec<usize> = runner.input_groups.first().map(|group| group.to_vec()).unwrap_or_default();
    timing.init_nanos += t_init.elapsed().as_nanos();

    let t_fight = Instant::now();
    runner.run_to_completion();
    timing.fight_nanos += t_fight.elapsed().as_nanos();
    *total += 1;
    if let Some(ref winners) = runner.world.winner
        && winners.first().is_some_and(|winner| target_team.contains(winner))
    {
        *wins += 1;
    }
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

fn build_js_score_match_input(target_group: &[String], modifier: &str, round: usize, bench_input: &mut String) {
    bench_input.clear();

    let tracked_targets = js_score_targets_per_round(target_group);
    let profile_count = js_score_profiles_per_round(target_group);
    let profile_base = tswn_core::engine::PROFILE_START as usize + round * profile_count;

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

fn namer_pf_score(base_group: &[String], modifier: &str, duplicate: bool, n: usize, thread: u32, eval_rq: f64) -> f64 {
    let mut target_group = base_group.to_vec();
    if duplicate {
        target_group.extend(base_group.iter().cloned());
    }

    run_score_inner(&target_group, modifier, n, eval_rq, thread).score_10000()
}

fn batch_rate_for_group(player: &str, target_groups: &[String], n: usize, thread: u32, eval_rq: f64) -> PyResult<BatchSummary> {
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
        let summary = win_rate_summary(raw, n, Some(eval_rq), thread)?;
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

fn pair_rate_for_player(
    player: &str,
    target_groups: &[String],
    teammates: &[String],
    head: usize,
    n: usize,
    thread: u32,
    eval_rq: f64,
) -> PyResult<PyPairRateResult> {
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
            pair_rates.push((teammate.clone(), summary.avg));
        }
        total_wins += summary.wins;
        total_battles += summary.total;
        total_valid_matchups += summary.valid_matchups;
        total_skipped_matchups += summary.skipped_matchups;
        total_timing.merge(summary.timing);
    }

    pair_rates.sort_by(|a, b| b.1.total_cmp(&a.1));
    let selected = head.min(pair_rates.len());
    let final_score = pair_rates.iter().take(selected).map(|(_, rate)| *rate).sum::<f64>();
    let aggregate_win_rate = total_wins as f64 * 100.0 / total_battles.max(1) as f64;

    Ok(PyPairRateResult {
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

fn player_to_ol(raw: &str) -> PyResult<String> {
    if raw.contains("+diy[") || raw.contains("+ol:") {
        return Ok(raw.to_string());
    }
    export_player(raw, false, false)
}

fn export_player(raw: &str, old: bool, minions: bool) -> PyResult<String> {
    if old && minions {
        return Err(PyValueError::new_err("old and minions are mutually exclusive"));
    }
    let storage = Storage::new_arc();
    let mut player = Player::new_from_namerena_raw(raw.to_string(), storage)
        .map_err(|err| PyValueError::new_err(format!("failed to build player from {raw}: {err}")))?;
    player.build();
    Ok(if old {
        player.to_diy_compact()
    } else if minions {
        player.to_ol_json_with_minions()
    } else {
        player.to_ol_json()
    })
}

fn parse_plus_separated_groups(raw: &str) -> Vec<Vec<String>> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(parse_plus_group_line)
        .filter(|group| !group.is_empty())
        .collect()
}

fn parse_plus_group_line(line: &str) -> Vec<String> {
    let mut group = Vec::new();
    let mut current = String::new();
    let mut idx = 0usize;

    while idx < line.len() {
        let rest = &line[idx..];
        if rest.starts_with('+') {
            let after_plus = &line[idx + 1..];
            if let Some(overlay_end) = overlay_suffix_end(after_plus) {
                current.push('+');
                current.push_str(&after_plus[..overlay_end]);
                idx += 1 + overlay_end;
                continue;
            }

            push_group_segment(&mut group, &mut current);
            idx += 1;
            continue;
        }

        let ch = rest.chars().next().expect("rest should contain a char");
        current.push(ch);
        idx += ch.len_utf8();
    }
    push_group_segment(&mut group, &mut current);

    group
}

fn push_group_segment(group: &mut Vec<String>, current: &mut String) {
    let segment = current.trim();
    if !segment.is_empty() {
        group.push(segment.to_string());
    }
    current.clear();
}

fn overlay_suffix_end(raw: &str) -> Option<usize> {
    if raw.starts_with("ol:") {
        let mut idx = 3usize;
        skip_ascii_ws(raw, &mut idx);
        return consume_balanced_ascii(raw, idx, b'{', b'}');
    }

    if raw.starts_with("diy[") {
        let mut idx = consume_balanced_ascii(raw, 3, b'[', b']')?;
        skip_ascii_ws(raw, &mut idx);
        if raw.as_bytes().get(idx).copied() == Some(b'{') {
            idx = consume_balanced_ascii(raw, idx, b'{', b'}')?;
        }
        return Some(idx);
    }

    None
}

fn skip_ascii_ws(raw: &str, idx: &mut usize) {
    let bytes = raw.as_bytes();
    while *idx < bytes.len() && bytes[*idx].is_ascii_whitespace() {
        *idx += 1;
    }
}

fn consume_balanced_ascii(raw: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = raw.as_bytes();
    if bytes.get(start).copied() != Some(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut idx = start;

    while idx < bytes.len() {
        let byte = bytes[idx];
        if in_string {
            if escaped {
                escaped = false;
            } else {
                match byte {
                    b'\\' => escaped = true,
                    b'"' => in_string = false,
                    _ => {}
                }
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b if b == open => depth += 1,
                b if b == close => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(idx + 1);
                    }
                }
                _ => {}
            }
        }

        idx += 1;
    }

    None
}

fn format_rate(value: f64, precision: usize) -> String {
    let value = if value.abs() < 0.5_f64 * 10_f64.powi(-(precision as i32)) {
        0.0
    } else {
        value
    };
    format!("{value:.precision$}")
}

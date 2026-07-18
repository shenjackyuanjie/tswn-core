use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use std::time::Instant;

use anyhow::Context;
use rusqlite::Connection;
use serde_json::{json, Value};

use crate::db::Db;
use crate::model::{GroupId, LaneResultRow, RankNode, StoredGroup};
use crate::ranker::RankerConfig;
use crate::team::TeamDsu;
use crate::winrate::compute_rate_without_db;

/// Default threshold retained for backward-compatible callers. New UI/service
/// calls `default_selection_cqd_threshold(lane_size)` so single and pair lanes
/// can use different defaults without hard-coding in the front end.
pub const DEFAULT_SELECTION_CQD_THRESHOLD: f64 = 48.5;

pub fn default_selection_cqd_threshold(lane_size: usize) -> f64 {
    if lane_size == 1 { 47.5 } else { 48.5 }
}

#[derive(Debug, Clone)]
pub struct PairwiseCalibrationReport {
    pub candidate_count: usize,
    pub edge_count: usize,
    pub selected_count: usize,
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct CalibGroup {
    row_idx: usize,
    group_id: GroupId,
    raw_score: f64,
    is_blocked: bool,
    train_eligible: bool,
}

#[derive(Debug, Clone)]
struct Edge {
    ia: usize,
    ib: usize,
}

#[derive(Debug, Clone)]
struct StrictPythonScoreRow {
    group_id: GroupId,
    correct_score: f64,
    raw_score: Option<f64>,
    rsw_type: Option<String>,
    uncertainty_cqd: Option<f64>,
    resolver_selected: bool,
    candidate_model_missing: bool,
    diagnostic_row_type: Option<String>,
    scout_candidate: bool,
}

#[derive(Debug, Clone)]
struct StrictPythonRun {
    out_dir: PathBuf,
    scores: Vec<StrictPythonScoreRow>,
    stdout: String,
}

#[derive(Debug, Clone)]
struct RequiredRatePair {
    a: StoredGroup,
    b: StoredGroup,
}

#[derive(Debug, Clone)]
struct StrictPythonMissingRateRequest {
    out_dir: PathBuf,
    pairs: Vec<(GroupId, GroupId)>,
    context: String,
}

const STRICT_PYTHON_CALIBRATOR: &str = include_str!("../tools/strict_python_calibrator.py");

fn reset_calibration_fields(row: &mut LaneResultRow) {
    row.pair_score = None;
    row.pair_rank = None;
    row.uncertainty = None;
    row.raw_delta = None;
    row.marginal_value = None;
    row.constrained_rank = None;

    // Clear stale native-Rust calibration/profile diagnostics. Strict Python
    // calibration owns the corrected score and RSW-Type now; keeping old values
    // here would make the UI/export look partially calibrated even when those
    // fields no longer correspond to the active score model.
    row.winrate_type_label = None;
    row.winrate_profile_distance = None;
    row.winrate_profile_second_distance = None;
    row.winrate_profile_margin = None;
    row.winrate_profile_soft_confidence = None;
    row.winrate_profile_soft_entropy = None;
    row.winrate_profile_soft_confidence_calibrated = None;
    row.winrate_profile_soft_entropy_calibrated = None;
    row.winrate_profile_bootstrap_stability = None;
    row.winrate_profile_fixed_center_stability = None;
    row.winrate_profile_recluster_stability = None;
    row.winrate_profile_recluster_jaccard = None;
    row.winrate_profile_assignment_entropy = None;
    row.winrate_profile_recluster_ari = None;
    row.winrate_profile_embedding_x = None;
    row.winrate_profile_embedding_y = None;

    row.residual_type_label = None;
    row.residual_profile_distance = None;
    row.residual_profile_second_distance = None;
    row.residual_profile_margin = None;
    row.residual_profile_soft_confidence_calibrated = None;
    row.residual_profile_soft_entropy_calibrated = None;
    row.residual_profile_recluster_stability = None;
    row.residual_profile_assignment_entropy = None;
    row.residual_profile_embedding_x = None;
    row.residual_profile_embedding_y = None;
    row.residual_profile_shape_rms = None;
    row.residual_profile_variance_feature = None;
}


fn run_strict_python_calibrator(
    db_path: &str,
    lane_size: usize,
    cqd_threshold: f64,
    folds: usize,
    seed: u64,
) -> anyhow::Result<StrictPythonRun> {
    if !Path::new(db_path).exists() {
        anyhow::bail!("strict Python calibration cannot start: sqlite path does not exist: {db_path}");
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out_dir = std::env::temp_dir().join(format!(
        "tswn_strict_python_calibration_lane{}_{}_{}",
        lane_size,
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("create strict Python calibration output dir: {}", out_dir.display()))?;
    let script_path = out_dir.join("strict_python_calibrator.py");
    fs::write(&script_path, STRICT_PYTHON_CALIBRATOR)
        .with_context(|| format!("write embedded strict Python calibrator: {}", script_path.display()))?;

    let mut last_error = None;
    for exe in ["python3", "python"] {
        let output = Command::new(exe)
            .arg(&script_path)
            .arg("--sqlite")
            .arg(db_path)
            .arg("--out")
            .arg(&out_dir)
            .arg("--lane-size")
            .arg(lane_size.to_string())
            .arg("--folds")
            .arg(folds.to_string())
            .arg("--seed")
            .arg(seed.to_string())
            .arg("--raw-min")
            .arg(format!("{:.12}", cqd_threshold))
            .env("TSWN_STRICT_NO_ARTIFACTS", "1")
            .output();
        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let scores = read_strict_python_scores(&out_dir)?;
                return Ok(StrictPythonRun { out_dir, scores, stdout });
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let missing_path = out_dir.join("strict_python_missing_rate_pairs.json");
                if missing_path.exists() {
                    anyhow::bail!(
                        "STRICT_PYTHON_MISSING_RATE_PAIRS_JSON={}",
                        missing_path.display()
                    );
                }
                last_error = Some(format!(
                    "{exe} exited with status {:?}\nstdout:\n{}\nstderr:\n{}",
                    out.status.code(),
                    stdout.trim(),
                    stderr.trim()
                ));
            }
            Err(err) => {
                last_error = Some(format!("failed to execute {exe}: {err}"));
            }
        }
    }
    anyhow::bail!(
        "strict Python calibration failed. The Rust native approximation was intentionally not used because it is not equivalent to the Python exact beta-binomial/L-BFGS-B implementation. Last error: {}",
        last_error.unwrap_or_else(|| "unknown python launcher failure".to_string())
    );
}

fn read_strict_python_scores(out_dir: &Path) -> anyhow::Result<Vec<StrictPythonScoreRow>> {
    let path = out_dir.join("final_total_table_ALL_GROUPS.csv");
    if !path.exists() {
        anyhow::bail!(
            "strict Python calibration finished but did not produce {}; cannot safely apply corrected scores",
            path.display()
        );
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("read strict Python total score table: {}", path.display()))?;
    let records = parse_csv_records(&text);
    if records.len() < 2 {
        anyhow::bail!("strict Python total score table is empty: {}", path.display());
    }
    let header = &records[0];
    let idx_group = csv_col(header, "group_id")?;
    // Frontend/export C-Score must be the explicit selection/output weight.
    // Do not silently fall back to model Correct Cqd here: pair_score is the
    // serialized UI/export field and must stay aligned with selection_weight_cqd.
    let idx_correct = csv_col(header, "Selection Weight Cqd Display")
        .or_else(|_| csv_col(header, "selection_weight_cqd"))
        .with_context(|| {
            format!(
                "strict Python final_total_table_ALL_GROUPS.csv is missing selection_weight_cqd / Selection Weight Cqd Display; refusing to display/export model Correct Cqd as C-Score"
            )
        })?;
    let idx_raw = csv_col(header, "Raw Cqd Display")
        .or_else(|_| csv_col(header, "raw_cqd"))
        .ok();
    let idx_rsw = csv_col(header, "RSW-Type").ok();
    let idx_uncertainty = csv_col(header, "stability_strength_sd_cqd").ok();
    let idx_selected = csv_col(header, "resolver_selected").ok();
    let idx_model_missing = csv_col(header, "candidate_model_missing").ok();
    let idx_diag = csv_col(header, "diagnostic_row_type").ok();
    let idx_scout = csv_col(header, "scout_candidate").ok();

    let mut out = Vec::new();
    for (line_idx, rec) in records.iter().enumerate().skip(1) {
        if rec.iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        let group_id = rec.get(idx_group)
            .and_then(|s| s.trim().parse::<GroupId>().ok())
            .with_context(|| format!("parse group_id at strict Python CSV record {}", line_idx + 1))?;
        let correct_score = rec.get(idx_correct)
            .and_then(|s| parse_optional_f64(s))
            .with_context(|| format!("parse Selection Weight Cqd Display / selection_weight_cqd for group_id={group_id}"))?;
        if !correct_score.is_finite() {
            anyhow::bail!("strict Python produced non-finite corrected score for group_id={group_id}");
        }
        let raw_score = idx_raw.and_then(|i| rec.get(i)).and_then(|s| parse_optional_f64(s));
        let rsw_type = idx_rsw
            .and_then(|i| rec.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "nan");
        let uncertainty_cqd = idx_uncertainty
            .and_then(|i| rec.get(i))
            .and_then(|s| parse_optional_f64(s));
        let resolver_selected = idx_selected
            .and_then(|i| rec.get(i))
            .map(|s| matches!(s.trim(), "1" | "1.0" | "true" | "True"))
            .unwrap_or(false);
        let candidate_model_missing = idx_model_missing
            .and_then(|i| rec.get(i))
            .map(|s| matches!(s.trim(), "1" | "1.0" | "true" | "True"))
            .unwrap_or(false);
        let diagnostic_row_type = idx_diag
            .and_then(|i| rec.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "nan");
        let scout_candidate = idx_scout
            .and_then(|i| rec.get(i))
            .map(|s| matches!(s.trim(), "1" | "1.0" | "true" | "True"))
            .unwrap_or(false);
        out.push(StrictPythonScoreRow {
            group_id,
            correct_score,
            raw_score,
            rsw_type,
            uncertainty_cqd,
            resolver_selected,
            candidate_model_missing,
            diagnostic_row_type,
            scout_candidate,
        });
    }
    if out.is_empty() {
        anyhow::bail!("strict Python total score table contains no usable score rows: {}", path.display());
    }
    Ok(out)
}

fn csv_col(header: &[String], name: &str) -> anyhow::Result<usize> {
    header
        .iter()
        .position(|h| h.trim() == name)
        .with_context(|| format!("missing required CSV column `{name}` in strict Python output"))
}

fn parse_optional_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("nan") || t.eq_ignore_ascii_case("none") {
        None
    } else {
        t.parse::<f64>().ok()
    }
}

fn parse_csv_records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut in_quotes = false;
    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                record.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            _ => field.push(ch),
        }
    }
    if in_quotes || !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

/// Compatibility entry point used by older recompute paths. The new calibration
/// is intentionally driven from saved Raw lane results; `nodes` and `dsu` are not
/// part of the score model, which keeps Raw generation separate from correction.
pub fn calibrate_lane_rows(
    db: &Db,
    lane_size: usize,
    _nodes: &[RankNode],
    rows: &mut Vec<LaneResultRow>,
    _dsu: &TeamDsu,
    config: &RankerConfig,
    _final_round: usize,
    cqd_threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    run_crossfit_betabinomial_lowrank_calibration(db, lane_size, rows, config, cqd_threshold)
}

pub fn calibrate_saved_lane_results(
    db: &Db,
    lane_size: usize,
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    if !cqd_threshold.is_finite() || !(0.0..=100.0).contains(&cqd_threshold) {
        anyhow::bail!("校准池 CQD 阈值必须是 0 到 100 之间的数字");
    }

    let mut rows = db.lane_results(lane_size)?;
    if rows.is_empty() {
        anyhow::bail!("该赛道还没有结果；请先完成一次默认重算");
    }

    db.set_lane_progress(
        lane_size,
        "calibration_loading",
        0,
        config.total_rounds,
        0,
        rows.len(),
        0,
        &format!(
            "strict Python crossfit beta-binomial interaction-rich calibration loading; pool: Raw Score >= {cqd_threshold:.3}, blocked excluded from fitting but scored for review/export"
        ),
    )?;

    let report = run_crossfit_betabinomial_lowrank_calibration(db, lane_size, &mut rows, config, cqd_threshold)?;
    db.save_lane_results(lane_size, &rows)?;
    db.set_lane_status(lane_size, "ready", rows.len())?;
    db.set_lane_progress(
        lane_size,
        "calibration_ready",
        0,
        config.total_rounds,
        report.edge_count,
        report.candidate_count,
        0,
        &format!(
            "calibration done: raw_score_threshold={:.3}, fit_candidates={}, cached_edges={}, score_mode=strict_python_crossfit_betabinomial_interactionrich_anysize_scoreblocked, blocked_scored_not_fit=true, skipped_reason={}",
            cqd_threshold,
            report.candidate_count,
            report.edge_count,
            report.skipped_reason.clone().unwrap_or_else(|| "none".to_string())
        ),
    )?;
    Ok(report)
}

pub fn validate_saved_pair_strength_results(
    db: &Db,
    lane_size: usize,
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<Value> {
    let rows = db.lane_results(lane_size)?;
    let groups = build_groups(&rows, cqd_threshold);
    let computed_rate_count = ensure_required_winrates_for_strict_python(db, lane_size, &groups, config, cqd_threshold)?;
    let edges = load_edges(db, &groups)?;
    let train: HashSet<usize> = groups
        .iter()
        .enumerate()
        .filter_map(|(idx, g)| g.train_eligible.then_some(idx))
        .collect();
    let train_edges: Vec<Edge> = edges
        .iter()
        .filter(|e| train.contains(&e.ia) && train.contains(&e.ib))
        .cloned()
        .collect();
    if train_edges.is_empty() || train.len() < 2 {
        anyhow::bail!(
            "strict Python validation aborted: not_enough_fit_edges; fit_candidates={}, fit_edges={}. Rust approximation is disabled, so no synthetic validation metrics are returned.",
            train.len(),
            train_edges.len()
        );
    }

    let (py, dynamic_rate_count) = run_strict_python_calibrator_with_dynamic_rate_fill(db, lane_size, cqd_threshold, 5, 123, config)?;
    let selected_count = py.scores.iter().filter(|r| r.resolver_selected).count();
    let mut max_abs_delta = 0.0_f64;
    let raw_by_gid: HashMap<GroupId, f64> = groups.iter().map(|g| (g.group_id, g.raw_score)).collect();
    for score in &py.scores {
        if let Some(raw) = raw_by_gid.get(&score.group_id) {
            max_abs_delta = max_abs_delta.max((score.correct_score - raw).abs());
        }
    }
    if max_abs_delta <= 1e-10 {
        anyhow::bail!(
            "strict Python validation produced Correct Score == Raw Score for all available rows (max_abs_delta={:.12}); refusing to report this as a successful calibration. Python out_dir={}",
            max_abs_delta,
            py.out_dir.display()
        );
    }

    Ok(json!({
        "version": "strict_python_crossfit_betabinomial_interactionrich_anysize_scoreblocked",
        "lane_size": lane_size,
        "raw_score_threshold": cqd_threshold,
        "threshold": cqd_threshold,
        "fit_candidates": train.len(),
        "fit_edges": train_edges.len(),
        "computed_missing_rate_pairs_before_python": computed_rate_count,
        "computed_dynamic_rate_pairs_during_python": dynamic_rate_count,
        "score_rows": py.scores.len(),
        "selected_count": selected_count,
        "max_abs_delta_cqd": max_abs_delta,
        "strict_python_out_dir": py.out_dir.display().to_string(),
        "rust_native_approximation_used": false,
        "cleanliness": {
            "raw_recomputed": false,
            "golden_used": false,
            "legacy_winrate_type_used": false,
            "manual_cap_or_topk_used": false,
            "blocked_affects_fit": false
        }
    }))
}

fn run_crossfit_betabinomial_lowrank_calibration(
    db: &Db,
    lane_size: usize,
    rows: &mut Vec<LaneResultRow>,
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    let groups = build_groups(rows, cqd_threshold);
    let train: HashSet<usize> = groups
        .iter()
        .enumerate()
        .filter_map(|(idx, g)| g.train_eligible.then_some(idx))
        .collect();
    let computed_rate_count = ensure_required_winrates_for_strict_python(db, lane_size, &groups, config, cqd_threshold)?;
    let edges = load_edges(db, &groups)?;
    let train_edges: Vec<Edge> = edges
        .iter()
        .filter(|e| train.contains(&e.ia) && train.contains(&e.ib))
        .cloned()
        .collect();

    if train.len() < 2 || train_edges.is_empty() {
        anyhow::bail!(
            "strict Python calibration aborted: not_enough_fit_edges; fit_candidates={}, fit_edges={}. Correct Score was not copied from Raw Score because strict mode forbids silent fallback.",
            train.len(),
            train_edges.len()
        );
    }

    db.set_lane_progress(
        lane_size,
        "calibration_fitting_strict_python",
        0,
        config.total_rounds,
        train_edges.len(),
        train.len(),
        0,
        &format!(
            "running strict Python exact beta-binomial + RSW counter + lowrank calibrator; Rust approximation disabled; newly_computed_rate_pairs_before_python={computed_rate_count}; dynamic Python rate requests enabled"
        ),
    )?;

    let (py, dynamic_rate_count) = run_strict_python_calibrator_with_dynamic_rate_fill(db, lane_size, cqd_threshold, 5, 123, config)?;
    let py_out_dir = py.out_dir.clone();
    let py_stdout_last = py.stdout.lines().last().unwrap_or("").to_string();
    let mut by_gid: HashMap<GroupId, StrictPythonScoreRow> = HashMap::new();
    for score in py.scores.iter().cloned() {
        by_gid.insert(score.group_id, score);
    }

    let mut scores = vec![0.0; groups.len()];
    let mut selected_count = 0usize;
    for (idx, g) in groups.iter().enumerate() {
        let Some(score) = by_gid.get(&g.group_id) else {
            anyhow::bail!(
                "strict Python output missing group_id={} from final_total_table_ALL_GROUPS.csv; refusing partial calibration",
                g.group_id
            );
        };
        scores[idx] = score.correct_score;
        if score.resolver_selected {
            selected_count += 1;
        }
    }

    let max_fit_abs_delta = groups
        .iter()
        .enumerate()
        .filter(|(idx, _)| train.contains(idx))
        .map(|(idx, g)| (scores[idx] - g.raw_score).abs())
        .fold(0.0_f64, f64::max);
    if max_fit_abs_delta <= 1e-10 {
        anyhow::bail!(
            "strict Python calibration produced Correct Score == Raw Score for every fit candidate (max_abs_delta={:.12}). This is treated as a failed calibration instead of silently accepting raw-as-correct. Python out_dir={}",
            max_fit_abs_delta,
            py_out_dir.display()
        );
    }

    let mut order: Vec<usize> = groups
        .iter()
        .enumerate()
        .filter(|(_, g)| {
            if g.raw_score >= cqd_threshold {
                return true;
            }
            by_gid
                .get(&g.group_id)
                .map(|score| !score.candidate_model_missing)
                .unwrap_or(false)
        })
        .map(|(idx, _)| idx)
        .collect();
    order.sort_by(|&a, &b| {
        scores[b]
            .total_cmp(&scores[a])
            .then_with(|| groups[b].raw_score.total_cmp(&groups[a].raw_score))
    });
    let mut pair_rank = vec![None; groups.len()];
    for (rank, idx) in order.iter().enumerate() {
        pair_rank[*idx] = Some(rank + 1);
    }

    for (idx, g) in groups.iter().enumerate() {
        let row = &mut rows[g.row_idx];
        let score = by_gid.get(&g.group_id).expect("checked above");
        reset_calibration_fields(row);
        row.raw_average_cqd = score.raw_score.unwrap_or(g.raw_score);

        if g.raw_score < cqd_threshold && score.candidate_model_missing {
            // Below-threshold rows stay hidden unless Python explicitly scored
            // them through the Raw-below-threshold scout/rescue challenger path.
            row.selection_status = "below_threshold".to_string();
            continue;
        }

        row.pair_score = Some(round_to_6(scores[idx]));
        row.pair_rank = pair_rank[idx];
        row.raw_delta = Some(round_to_6(scores[idx] - g.raw_score));
        row.uncertainty = score.uncertainty_cqd.map(round_to_6);
        row.residual_type_label = score.rsw_type.clone();
        row.selection_status = if g.is_blocked {
            "blocked".to_string()
        } else if g.raw_score < cqd_threshold && score.resolver_selected {
            "rescued_scout".to_string()
        } else if g.raw_score < cqd_threshold {
            "scout_score_only".to_string()
        } else if score.resolver_selected {
            "calibrated".to_string()
        } else {
            "not_selected_by_active_set".to_string()
        };
    }

    db.set_lane_progress(
        lane_size,
        "calibration_applying_strict_python",
        0,
        config.total_rounds,
        train_edges.len(),
        train.len(),
        0,
        &format!(
            "strict Python calibration applied; out_dir={}; max_fit_abs_delta={:.9}; newly_computed_rate_pairs_before_python={}; newly_computed_dynamic_rate_pairs={}; stdout={}",
            py_out_dir.display(),
            max_fit_abs_delta,
            computed_rate_count,
            dynamic_rate_count,
            py_stdout_last
        ),
    )?;

    Ok(PairwiseCalibrationReport {
        candidate_count: train.len(),
        edge_count: train_edges.len(),
        selected_count,
        skipped_reason: None,
    })
}

fn read_strict_python_missing_rate_request(path: &Path) -> anyhow::Result<StrictPythonMissingRateRequest> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("read strict Python missing-rate request: {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse strict Python missing-rate request JSON: {}", path.display()))?;
    let context = value
        .get("context")
        .and_then(|v| v.as_str())
        .unwrap_or("strict_python_dynamic_rate_request")
        .to_string();
    let pairs_v = value
        .get("pairs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("missing-rate request has no pairs array: {}", path.display()))?;
    let mut pairs = Vec::with_capacity(pairs_v.len());
    for p in pairs_v {
        let a = p.get("group_a").and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("missing-rate request pair missing group_a"))? as GroupId;
        let b = p.get("group_b").and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("missing-rate request pair missing group_b"))? as GroupId;
        if a != b {
            pairs.push(ordered_group_id_pair(a, b));
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    let out_dir = path.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    Ok(StrictPythonMissingRateRequest { out_dir, pairs, context })
}

fn extract_missing_rate_request_path(err: &anyhow::Error) -> Option<PathBuf> {
    let s = err.to_string();
    let marker = "STRICT_PYTHON_MISSING_RATE_PAIRS_JSON=";
    let idx = s.find(marker)?;
    let rest = &s[(idx + marker.len())..];
    let path = rest.lines().next().unwrap_or(rest).trim();
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

fn compute_rate_pairs_by_id_request(
    db: &Db,
    lane_size: usize,
    config: &RankerConfig,
    pairs: &[(GroupId, GroupId)],
    context: &str,
) -> anyhow::Result<usize> {
    let existing_rates = load_existing_rate_pairs_for_python(db)?;
    let stored_by_id: HashMap<GroupId, StoredGroup> = db
        .load_groups_by_lane(lane_size)?
        .into_iter()
        .map(|g| (g.id, g))
        .collect();
    let mut missing = Vec::<RequiredRatePair>::new();
    let mut required = HashSet::<(GroupId, GroupId)>::new();
    for &(a_id, b_id) in pairs {
        if a_id == b_id {
            continue;
        }
        let pair = ordered_group_id_pair(a_id, b_id);
        required.insert(pair);
        if existing_rates.contains(&pair) {
            continue;
        }
        let a = stored_by_id
            .get(&pair.0)
            .cloned()
            .with_context(|| format!("dynamic strict Python rate request references missing group_id={}", pair.0))?;
        let b = stored_by_id
            .get(&pair.1)
            .cloned()
            .with_context(|| format!("dynamic strict Python rate request references missing group_id={}", pair.1))?;
        missing.push(RequiredRatePair { a, b });
    }
    if missing.is_empty() {
        return Ok(0);
    }

    let total = missing.len();
    let workers = resolve_calibration_rate_pair_workers(config.outer_workers, total);
    let outer_label = format_calibration_outer_workers(config.outer_workers, workers);
    let inner_label = format_calibration_inner_workers(config.inner_workers);
    let mode = if config.outer_workers == 0 { "dynamic_queue" } else { "static_chunks" };
    let done = Arc::new(AtomicUsize::new(0));
    let rate_started = Arc::new(Instant::now());
    let missing = Arc::new(missing);

    db.set_lane_progress(
        lane_size,
        "calibration_dynamic_missing_rates",
        0,
        config.total_rounds,
        0,
        total,
        0,
        &format!(
            "computing dynamic strict Python missing rates for {context}: 0/{total}, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"
        ),
    )?;

    let mut handles = Vec::with_capacity(workers);
    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let next_pair = Arc::clone(&next_pair);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            let context = context.to_string();
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::new();
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else { break; };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers)
                        .with_context(|| format!("compute dynamic strict calibration win rate: {} vs {}", pair.a.canonical, pair.b.canonical))?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            &context,
                            rate_started.elapsed().as_secs_f64(),
                        )?;
                    }
                }
                Ok(computed)
            }));
        }
    } else {
        for worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            let context = context.to_string();
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::with_capacity(end.saturating_sub(start));
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else { break; };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers)
                        .with_context(|| format!("compute dynamic strict calibration win rate: {} vs {}", pair.a.canonical, pair.b.canonical))?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            &context,
                            rate_started.elapsed().as_secs_f64(),
                        )?;
                    }
                }
                Ok(computed)
            }));
        }
    }

    let mut computed = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
    for handle in handles {
        let mut rows = handle
            .join()
            .map_err(|_| anyhow::anyhow!("dynamic strict calibration rate worker panicked"))??;
        computed.append(&mut rows);
    }
    if computed.len() != total {
        anyhow::bail!(
            "dynamic strict calibration rate fill computed {} pairs but expected {}; refusing to continue",
            computed.len(),
            total
        );
    }
    db.save_rate_pairs_bulk(&computed, config.win_rate_samples)?;
    Ok(computed.len())
}

fn run_strict_python_calibrator_with_dynamic_rate_fill(
    db: &Db,
    lane_size: usize,
    cqd_threshold: f64,
    folds: usize,
    seed: u64,
    config: &RankerConfig,
) -> anyhow::Result<(StrictPythonRun, usize)> {
    let mut total_computed = 0usize;
    let mut seen_requests = HashSet::<String>::new();
    for attempt in 0..256 {
        match run_strict_python_calibrator(db.path(), lane_size, cqd_threshold, folds, seed) {
            Ok(run) => return Ok((run, total_computed)),
            Err(err) => {
                let Some(path) = extract_missing_rate_request_path(&err) else {
                    return Err(err);
                };
                let request = read_strict_python_missing_rate_request(&path)?;
                let request_key = format!("{}:{:?}", request.context, request.pairs);
                if !seen_requests.insert(request_key) {
                    anyhow::bail!(
                        "strict Python repeatedly requested the same missing rate pairs for context {}; request_path={}",
                        request.context,
                        path.display()
                    );
                }
                let computed = compute_rate_pairs_by_id_request(
                    db,
                    lane_size,
                    config,
                    &request.pairs,
                    &request.context,
                )?;
                total_computed += computed;
                db.set_lane_progress(
                    lane_size,
                    "calibration_dynamic_rates_ready",
                    0,
                    config.total_rounds,
                    computed,
                    request.pairs.len(),
                    0,
                    &format!(
                        "strict Python dynamic rate request satisfied: attempt={}, context={}, requested_pairs={}, newly_computed_pairs={}, request_out_dir={}",
                        attempt + 1,
                        request.context,
                        request.pairs.len(),
                        computed,
                        request.out_dir.display()
                    ),
                )?;
            }
        }
    }
    anyhow::bail!("strict Python dynamic rate-fill loop exceeded maximum attempts (256); this usually means the Python active/scout loop keeps discovering new challenger-active edge requirements faster than the Rust bridge can satisfy them, or it repeatedly emits the same unresolved request")
}

fn ensure_required_winrates_for_strict_python(
    db: &Db,
    lane_size: usize,
    groups: &[CalibGroup],
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<usize> {
    let train_ids: Vec<GroupId> = groups
        .iter()
        .filter(|g| g.train_eligible)
        .map(|g| g.group_id)
        .collect();
    if train_ids.len() < 2 {
        anyhow::bail!(
            "strict Python calibration aborted before rate fill: not_enough_fit_candidates; fit_candidates={}",
            train_ids.len()
        );
    }

    // Python fits all nonblocked rows with Raw >= --raw-min. Blocked rows are
    // excluded from fit, but the Python score-only branch needs every blocked
    // row with Raw >= --raw-min to have edges against the frozen train pool.
    let blocked_score_only_ids: Vec<GroupId> = groups
        .iter()
        .filter(|g| g.is_blocked && g.raw_score >= cqd_threshold)
        .map(|g| g.group_id)
        .collect();

    let mut required = HashSet::<(GroupId, GroupId)>::new();
    for i in 0..train_ids.len() {
        for j in (i + 1)..train_ids.len() {
            required.insert(ordered_group_id_pair(train_ids[i], train_ids[j]));
        }
    }
    for &blocked_id in &blocked_score_only_ids {
        for &train_id in &train_ids {
            if blocked_id != train_id {
                required.insert(ordered_group_id_pair(blocked_id, train_id));
            }
        }
    }

    let existing_rates = load_existing_rate_pairs_for_python(db)?;
    let stored_by_id: HashMap<GroupId, StoredGroup> = db
        .load_groups_by_lane(lane_size)?
        .into_iter()
        .map(|g| (g.id, g))
        .collect();
    let mut missing = Vec::<RequiredRatePair>::new();
    for (a_id, b_id) in required.iter().copied() {
        if existing_rates.contains(&ordered_group_id_pair(a_id, b_id)) {
            continue;
        }
        let a = stored_by_id
            .get(&a_id)
            .cloned()
            .with_context(|| format!("required calibration rate references missing group_id={a_id}"))?;
        let b = stored_by_id
            .get(&b_id)
            .cloned()
            .with_context(|| format!("required calibration rate references missing group_id={b_id}"))?;
        missing.push(RequiredRatePair { a, b });
    }

    if missing.is_empty() {
        db.set_lane_progress(
            lane_size,
            "calibration_rates_ready",
            0,
            config.total_rounds,
            required.len(),
            required.len(),
            0,
            &format!(
                "all strict Python calibration win-rate pairs already cached: required_pairs={}, train_pairs={}, blocked_score_only_pairs={}",
                required.len(),
                train_ids.len().saturating_mul(train_ids.len().saturating_sub(1)) / 2,
                blocked_score_only_ids.len().saturating_mul(train_ids.len())
            ),
        )?;
        return Ok(0);
    }

    let total = missing.len();
    let workers = resolve_calibration_rate_pair_workers(config.outer_workers, total);
    let outer_label = format_calibration_outer_workers(config.outer_workers, workers);
    let inner_label = format_calibration_inner_workers(config.inner_workers);
    let mode = if config.outer_workers == 0 { "dynamic_queue" } else { "static_chunks" };
    let done = Arc::new(AtomicUsize::new(0));
    let rate_started = Arc::new(Instant::now());
    let missing = Arc::new(missing);

    db.set_lane_progress(
        lane_size,
        "calibration_computing_missing_rates",
        0,
        config.total_rounds,
        0,
        total,
        0,
        &format!(
            "computing missing strict Python calibration rates 0/{total}, 0.00 pair/s, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"
        ),
    )?;

    let mut handles = Vec::with_capacity(workers);
    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let next_pair = Arc::clone(&next_pair);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::new();
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else { break; };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers)
                        .with_context(|| format!("compute strict calibration win rate: {} vs {}", pair.a.canonical, pair.b.canonical))?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            "dynamic_queue",
                            rate_started.elapsed().as_secs_f64(),
                        )?;
                    }
                }
                Ok(computed)
            }));
        }
    } else {
        for worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::with_capacity(end.saturating_sub(start));
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else { break; };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers)
                        .with_context(|| format!("compute strict calibration win rate: {} vs {}", pair.a.canonical, pair.b.canonical))?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            "static_chunks",
                            rate_started.elapsed().as_secs_f64(),
                        )?;
                    }
                }
                Ok(computed)
            }));
        }
    }

    let mut computed = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
    for handle in handles {
        let mut rows = handle
            .join()
            .map_err(|_| anyhow::anyhow!("strict calibration rate worker panicked"))??;
        computed.append(&mut rows);
    }
    if computed.len() != total {
        anyhow::bail!(
            "strict calibration rate fill computed {} pairs but expected {}; refusing to call Python with incomplete win-rate data",
            computed.len(),
            total
        );
    }
    db.save_rate_pairs_bulk(&computed, config.win_rate_samples)?;
    db.set_lane_progress(
        lane_size,
        "calibration_rates_ready",
        0,
        config.total_rounds,
        required.len(),
        required.len(),
        0,
        &format!(
            "strict Python calibration rates ready: newly_computed_pairs={}, total_required_pairs={}",
            computed.len(),
            required.len()
        ),
    )?;
    Ok(computed.len())
}

fn load_existing_rate_pairs_for_python(db: &Db) -> anyhow::Result<HashSet<(GroupId, GroupId)>> {
    let conn = Connection::open(db.path()).with_context(|| format!("open sqlite database: {}", db.path()))?;
    let mut stmt = conn.prepare(
        "SELECT group_a, group_b FROM group_rates WHERE samples > 0 AND win_rate_a IS NOT NULL"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, GroupId>(0)?, row.get::<_, GroupId>(1)?))
    })?;
    let mut out = HashSet::new();
    for row in rows {
        let (a, b) = row?;
        if a != b {
            out.insert(ordered_group_id_pair(a, b));
        }
    }
    Ok(out)
}

fn ordered_group_id_pair(a: GroupId, b: GroupId) -> (GroupId, GroupId) {
    if a <= b { (a, b) } else { (b, a) }
}

fn resolve_calibration_rate_pair_workers(requested_outer_workers: usize, total: usize) -> usize {
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }
    let available = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    available.max(1).min(total.max(1))
}

fn format_calibration_outer_workers(requested_outer_workers: usize, actual_workers: usize) -> String {
    if requested_outer_workers == 0 {
        format!("auto({actual_workers})")
    } else {
        format!("static({actual_workers})")
    }
}

fn format_calibration_inner_workers(inner_workers: u32) -> String {
    if inner_workers == 0 {
        "auto(0)".to_string()
    } else {
        inner_workers.to_string()
    }
}

fn should_report_calibration_rate_progress(done: usize, total: usize) -> bool {
    done == total || done % 10 == 0
}

fn report_calibration_rate_progress(
    db: &Db,
    lane_size: usize,
    total_rounds: usize,
    current: usize,
    total: usize,
    outer_label: &str,
    inner_label: &str,
    mode: &str,
    elapsed_secs: f64,
) -> anyhow::Result<()> {
    let elapsed = elapsed_secs.max(0.001);
    let pairs_per_sec = current as f64 / elapsed;
    let eta_sec = if pairs_per_sec > 0.0 {
        total.saturating_sub(current) as f64 / pairs_per_sec
    } else {
        0.0
    };
    db.set_lane_progress(
        lane_size,
        "calibration_computing_missing_rates",
        0,
        total_rounds,
        current,
        total,
        0,
        &format!(
            "computing missing strict Python calibration rates {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}",
            pairs_per_sec,
            format_calibration_duration(elapsed),
            format_calibration_duration(eta_sec)
        ),
    )
}

fn format_calibration_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

fn build_groups(rows: &[LaneResultRow], threshold: f64) -> Vec<CalibGroup> {
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let raw_score = if row.raw_average_cqd.is_finite() {
                row.raw_average_cqd
            } else {
                row.average_cqd
            };
            CalibGroup {
                row_idx,
                group_id: row.group_id,
                raw_score,
                is_blocked: row.is_blocked,
                train_eligible: !row.is_blocked && raw_score >= threshold,
            }
        })
        .collect()
}

fn load_edges(db: &Db, groups: &[CalibGroup]) -> anyhow::Result<Vec<Edge>> {
    let mut group_to_idx = HashMap::new();
    for (idx, g) in groups.iter().enumerate() {
        group_to_idx.insert(g.group_id, idx);
    }

    let conn = Connection::open(db.path()).with_context(|| format!("open sqlite database: {}", db.path()))?;
    let mut stmt = conn.prepare("SELECT group_a, group_b, samples FROM group_rates WHERE samples > 0 AND win_rate_a IS NOT NULL")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, GroupId>(0)?,
            row.get::<_, GroupId>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;

    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let (ga, gb, samples) = row?;
        if samples <= 0 {
            continue;
        }
        let Some(&ia0) = group_to_idx.get(&ga) else { continue; };
        let Some(&ib0) = group_to_idx.get(&gb) else { continue; };
        if ia0 == ib0 {
            continue;
        }
        let (ia, ib) = if ia0 < ib0 { (ia0, ib0) } else { (ib0, ia0) };
        if seen.insert((ia, ib)) {
            out.push(Edge { ia, ib });
        }
    }
    Ok(out)
}

fn round_to_6(x: f64) -> f64 {
    (x * 1_000_000.0).round() / 1_000_000.0
}

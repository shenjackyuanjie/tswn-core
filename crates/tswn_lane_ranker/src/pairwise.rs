//! 已存 Raw 结果的 Correct 校准，以及独立的严格 Python 验证入口。
//!
//! 正常校准使用 Rust 固定槽位替换闭式公式；相同阈值/score_mode 且已有有限结果时
//! 可复用追溯。严格验证另行运行 Python，不以 Rust 近似指标代替其数值过程。
//! 缺失对局会补算并保存；结果与追溯分别提交，追溯保存失败会尽力清除旧记录。

mod correct;
mod python;
mod rates;

use correct::*;
use python::*;
use rates::*;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Instant;

use anyhow::Context;
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::db::Db;
use crate::model::{CorrectTargetTrace, CorrectTargetTraceWeight, GroupId, LaneResultRow, StoredGroup};
use crate::ranker::RankerConfig;
use crate::winrate::compute_rate_without_db;

pub fn default_selection_cqd_threshold(lane_size: usize) -> f64 { if lane_size == 1 { 48.0 } else { 48.7 } }

/// 前瞻性 Correct 替换槽位因 lane 而异：单人 lane 使用十个假设替换项，所有多成员 lane 保留生产版五槽位设置。
pub fn replacement_k_for_lane(lane_size: usize) -> f64 { if lane_size == 1 { 10.0 } else { 5.0 } }

fn format_k_label(k: f64) -> String {
    if (k.fract()).abs() <= 1e-9 {
        format!("{k:.0}")
    } else {
        format!("{k:.1}")
    }
}

fn correct_score_mode(k: f64) -> String { format!("exact_k{}_replacement_correct_big_target", format_k_label(k)) }

#[derive(Debug, Clone)]
pub struct PairwiseCalibrationReport {
    pub candidate_count: usize,
    pub edge_count: usize,
    pub skipped_reason: Option<String>,
    target_trace: CorrectTargetTrace,
}

#[derive(Debug, Clone)]
struct StrictPythonScoreRow {
    group_id: GroupId,
    correct_score: f64,
    resolver_selected: bool,
}

#[derive(Debug, Clone)]
struct StrictPythonRun {
    out_dir: PathBuf,
    scores: Vec<StrictPythonScoreRow>,
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

    // 清除过期的校准/画像诊断。生产版 lane 特定前瞻性 Correct 路径拥有校正分数；在此保留旧值会让 UI/导出
    // 在这些字段已不再对应活跃评分模型时看似仍部分完成校准。
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

    if let Some(trace) = db.correct_target_trace(lane_size)? {
        let trace_threshold = serde_json::from_str::<Value>(&trace.metadata_json)
            .ok()
            .and_then(|metadata| metadata.get("calibration_raw_min").and_then(Value::as_f64));
        let expected_score_mode = correct_score_mode(replacement_k_for_lane(lane_size));
        let trace_is_current = trace.score_mode == expected_score_mode
            && trace_threshold.is_some_and(|value| (value - cqd_threshold).abs() <= 1e-9)
            && rows.iter().all(|row| row.pair_score.is_some_and(f64::is_finite));
        if trace_is_current {
            let candidate_count = rows.iter().filter(|row| !row.is_blocked && row.raw_average_cqd >= cqd_threshold).count();
            db.set_lane_status(lane_size, "ready", rows.len())?;
            db.set_lane_progress(
                lane_size,
                "calibration_ready",
                0,
                config.total_rounds,
                0,
                candidate_count,
                0,
                &format!("reused current {expected_score_mode} trace; fast Correct validation skipped"),
            )?;
            return Ok(PairwiseCalibrationReport {
                candidate_count,
                edge_count: 0,
                skipped_reason: Some("unchanged_exact_trace_reused".to_string()),
                target_trace: trace,
            });
        }
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
            "fast lane-specific Correct loading (K={:.1}); pool: Raw Score >= {cqd_threshold:.3}, blocked/score-only excluded from prospective references",
            replacement_k_for_lane(lane_size)
        ),
    )?;

    let report = run_fast_correct(db, lane_size, &mut rows, config, cqd_threshold)?;
    db.save_lane_results(lane_size, &rows)?;
    if let Err(err) = db.replace_correct_target_trace(lane_size, &report.target_trace) {
        let _ = db.clear_correct_target_trace(lane_size);
        return Err(err)
            .context("Correct scores were saved, but their direct trace could not be persisted; stale trace was cleared");
    }
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
            "calibration done: raw_score_threshold={:.3}, fit_candidates={}, cached_edges={}, score_mode={}, blocked_scored_not_fit=true, skipped_reason={}",
            cqd_threshold,
            report.candidate_count,
            report.edge_count,
            correct_score_mode(replacement_k_for_lane(lane_size)),
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
    let train: HashSet<usize> = groups.iter().enumerate().filter_map(|(idx, g)| g.train_eligible.then_some(idx)).collect();
    let train_edges: Vec<Edge> = edges.iter().filter(|e| train.contains(&e.ia) && train.contains(&e.ib)).cloned().collect();
    if train_edges.is_empty() || train.len() < 2 {
        anyhow::bail!(
            "strict Python validation aborted: not_enough_fit_edges; fit_candidates={}, fit_edges={}. Rust approximation is disabled, so no synthetic validation metrics are returned.",
            train.len(),
            train_edges.len()
        );
    }

    let (py, dynamic_rate_count) =
        run_strict_python_calibrator_with_dynamic_rate_fill(db, lane_size, cqd_threshold, 5, 123, config)?;
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
            "golden_used_for_score_calibration": false,
            "legacy_winrate_type_used": false,
            "manual_cap_or_topk_used": false,
            "blocked_affects_fit": false
        }
    }))
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
                let computed = compute_rate_pairs_by_id_request(db, lane_size, config, &request.pairs, &request.context)?;
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
    anyhow::bail!(
        "strict Python dynamic rate-fill loop exceeded maximum attempts (256); this usually means the Python active/scout loop keeps discovering new challenger-active edge requirements faster than the Rust bridge can satisfy them, or it repeatedly emits the same unresolved request"
    )
}

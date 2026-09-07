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
use crate::model::{CorrectTargetTrace, CorrectTargetTraceWeight, GroupId, LaneResultRow, RankNode, StoredGroup};
use crate::ranker::RankerConfig;
use crate::team::TeamDsu;
use crate::winrate::compute_rate_without_db;

/// 为向后兼容调用方保留的默认阈值。新的 UI/service 调用 `default_selection_cqd_threshold(lane_size)`，
/// 使单人和双人 lane 无需在前端硬编码即可使用不同默认值。
pub const DEFAULT_SELECTION_CQD_THRESHOLD: f64 = 48.7;

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
    pub selected_count: usize,
    pub skipped_reason: Option<String>,
    target_trace: CorrectTargetTrace,
}

fn run_fast_correct(
    db: &Db,
    lane_size: usize,
    rows: &mut [LaneResultRow],
    config: &RankerConfig,
    threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    let replacement_k = replacement_k_for_lane(lane_size);
    let score_mode = correct_score_mode(replacement_k);
    let k_label = format_k_label(replacement_k);

    let groups = db.load_groups_by_lane(lane_size)?;
    let group_map: HashMap<GroupId, StoredGroup> = groups.into_iter().map(|g| (g.id, g)).collect();
    let mut eligible_count = 0usize;
    let mut refs: Vec<(GroupId, f64)> = rows
        .iter()
        .filter_map(|r| {
            let raw = if r.raw_average_cqd.is_finite() {
                r.raw_average_cqd
            } else {
                r.average_cqd
            };
            let status = r.selection_status.to_ascii_lowercase();
            let score_only = status.contains("score_only") || status.contains("scout");
            if raw.is_finite() && raw >= threshold && !r.is_blocked && !score_only {
                eligible_count += 1;
                Some((r.group_id, raw))
            } else {
                None
            }
        })
        .collect();
    refs.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let mut seen_members: HashSet<String> = HashSet::new();
    refs.retain(|(gid, _)| {
        let Some(g) = group_map.get(gid) else { return false };
        let mut members: Vec<String> = g.members.iter().filter(|member| !member.is_empty()).cloned().collect();
        if members.is_empty() {
            members.push(format!("__gid__{gid}"));
        }
        if members.iter().any(|m| seen_members.contains(m)) {
            return false;
        }
        seen_members.extend(members);
        true
    });
    if refs.is_empty() {
        anyhow::bail!("fast K={k_label} Correct found no legal references")
    }
    let initial_rate_map = db.lane_rate_map(lane_size)?;
    let mut missing_pairs = HashSet::<(GroupId, GroupId)>::new();
    for row in rows.iter() {
        for (reference_id, _) in &refs {
            if row.group_id == *reference_id
                || initial_rate_map.contains_key(&(row.group_id, *reference_id))
                || initial_rate_map.contains_key(&(*reference_id, row.group_id))
            {
                continue;
            }
            missing_pairs.insert(ordered_group_id_pair(row.group_id, *reference_id));
        }
    }
    if !missing_pairs.is_empty() {
        let mut missing_pairs = missing_pairs.into_iter().collect::<Vec<_>>();
        missing_pairs.sort_unstable();
        compute_rate_pairs_by_id_request(db, lane_size, config, &missing_pairs, "fast_correct_reference_replay")?;
    }
    let rate_map = db.lane_rate_map(lane_size)?;
    let rate = |a: GroupId, b: GroupId| -> anyhow::Result<f64> {
        let value = if a == b {
            50.0
        } else if let Some(v) = rate_map.get(&(a, b)) {
            *v
        } else if let Some(v) = rate_map.get(&(b, a)) {
            100.0 - *v
        } else {
            anyhow::bail!("fast K={k_label} Correct missing rate {a} vs {b}")
        };
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            anyhow::bail!("fast K={k_label} Correct found invalid rate {a} vs {b}: {value}");
        }
        Ok(value)
    };
    let n = refs.len() as f64;
    let mut correct_by_gid = HashMap::new();
    for row in rows.iter() {
        let raw = if row.raw_average_cqd.is_finite() {
            row.raw_average_cqd
        } else {
            row.average_cqd
        };
        if !raw.is_finite() {
            anyhow::bail!(
                "fast K={k_label} Correct found non-finite Raw score for group_id={}",
                row.group_id
            );
        }
        let sum = refs.iter().try_fold(0.0_f64, |sum, (rid, _)| {
            Ok::<f64, anyhow::Error>(sum + rate(row.group_id, *rid)?)
        })?;
        let mean = sum / n;
        correct_by_gid.insert(row.group_id, round_to_6(raw + (replacement_k / 50.0) * (mean - raw)));
    }
    let mut ranked: Vec<(usize, f64)> = rows.iter().enumerate().map(|(i, r)| (i, correct_by_gid[&r.group_id])).collect();
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| rows[a.0].group_id.cmp(&rows[b.0].group_id))
    });
    for (rank, (idx, score)) in ranked.iter().enumerate() {
        reset_calibration_fields(&mut rows[*idx]);
        rows[*idx].pair_score = Some(*score);
        rows[*idx].pair_rank = Some(rank + 1);
        let raw = if rows[*idx].raw_average_cqd.is_finite() {
            rows[*idx].raw_average_cqd
        } else {
            rows[*idx].average_cqd
        };
        rows[*idx].raw_average_cqd = raw;
        rows[*idx].raw_delta = Some(round_to_6(*score - raw));
        rows[*idx].selection_status = if rows[*idx].is_blocked {
            "blocked".to_string()
        } else if raw >= threshold {
            "calibrated".to_string()
        } else {
            "below_threshold".to_string()
        };
    }
    let mut golden = HashMap::<GroupId, f64>::with_capacity(rows.len());
    let mut golden_sum = 0.0_f64;
    for row in rows.iter() {
        if !row.golden_rate.is_finite() || row.golden_rate < 0.0 {
            anyhow::bail!("invalid Raw Golden weight for group_id={}: {}", row.group_id, row.golden_rate);
        }
        golden_sum += row.golden_rate;
        golden.insert(row.group_id, row.golden_rate);
    }
    if (golden_sum - 50.0).abs() > 1e-7 {
        anyhow::bail!(
            "Raw Golden weight sum must be 50 for exact K={k_label} Correct, got {:.12}",
            golden_sum
        );
    }
    let ref_ids: HashSet<GroupId> = refs.iter().map(|(g, _)| *g).collect();
    let mut ids: HashSet<GroupId> = golden.iter().filter(|(_, w)| **w > 1e-15).map(|(g, _)| *g).collect();
    ids.extend(ref_ids.iter().copied());
    let mut weights = Vec::new();
    for gid in ids {
        let w = (1.0 - replacement_k / 50.0) * golden.get(&gid).copied().unwrap_or(0.0)
            + if ref_ids.contains(&gid) { replacement_k / n } else { 0.0 };
        if w <= 1e-15 {
            continue;
        }
        weights.push(CorrectTargetTraceWeight {
            reference_scope: "common_correct_candidate_coefficients".into(),
            group_id: gid,
            reference_weight: w / 50.0,
            nominal_weight: w,
            raw_golden_weight: golden.get(&gid).copied().unwrap_or(0.0),
            common_coefficient: w / 50.0,
            coefficient_mean: w / 50.0,
            coefficient_stddev: 0.0,
            correct_target_weight: w,
            source: format!(
                "closed_form_{}_golden_plus_{}_over_n_legal_references",
                format_k_label(50.0 - replacement_k),
                format_k_label(replacement_k)
            ),
        });
    }
    weights.sort_by_key(|w| w.group_id);
    let weight_sum: f64 = weights.iter().map(|w| w.correct_target_weight).sum();
    if !weight_sum.is_finite() || (weight_sum - 50.0).abs() > 1e-7 {
        anyhow::bail!("fast K={k_label} Correct produced invalid target mass: {weight_sum}");
    }
    let reference_group_ids: Vec<GroupId> = refs.iter().map(|(gid, _)| *gid).collect();
    let metadata_json = json!({
        "calibration_raw_min": threshold,
        "score_mode": score_mode.clone(),
        "correct_formula": format!("Correct=(1-{replacement_k}/50)*Raw+({replacement_k}/50)*mean_rate_against_threshold_eligible_member_unique_references"),
        "prospective_replacement_k": replacement_k,
        "prospective_replacement_target_slots": 50.0,
        "reference_definition": "enabled_nonblocked_nonscoreonly_raw_min_group_unique_member_unique",
        "reference_count": refs.len(),
        "reference_group_ids": reference_group_ids,
        "score_universe_count": rows.len(),
        "eligible_before_member_dedup_count": eligible_count,
        "raw_golden_weight_sum": golden_sum,
        "correct_target_weight_sum": weight_sum,
        "common_correct_component": {
            "coefficient_rule": format!("weight=(1-{replacement_k}/50)*Golden+({replacement_k}/N)*legal_reference_indicator"),
            "weight_sum": weight_sum,
            "target_count": weights.len(),
            "solver": "closed_form_fixed_slot_replacement",
        },
    })
    .to_string();
    let trace = CorrectTargetTrace {
        trace_version: "fixed_slot_replacement_exact_big_target_v1".into(),
        score_mode,
        metadata_json,
        weights,
    };
    Ok(PairwiseCalibrationReport {
        candidate_count: eligible_count,
        edge_count: refs.len() * rows.len(),
        selected_count: rows.iter().filter(|r| !r.is_blocked && r.raw_average_cqd >= threshold).count(),
        skipped_reason: Some(format!("fast_closed_form_k{}", format_k_label(replacement_k))),
        target_trace: trace,
    })
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
    target_trace: CorrectTargetTrace,
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
                let target_trace = read_strict_python_target_trace(&out_dir, lane_size)?;
                return Ok(StrictPythonRun {
                    out_dir,
                    scores,
                    stdout,
                    target_trace,
                });
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let missing_path = out_dir.join("strict_python_missing_rate_pairs.json");
                if missing_path.exists() {
                    anyhow::bail!("STRICT_PYTHON_MISSING_RATE_PAIRS_JSON={}", missing_path.display());
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
    let text = fs::read_to_string(&path).with_context(|| format!("read strict Python total score table: {}", path.display()))?;
    let records = parse_csv_records(&text);
    if records.len() < 2 {
        anyhow::bail!("strict Python total score table is empty: {}", path.display());
    }
    let header = &records[0];
    let idx_group = csv_col(header, "group_id")?;
    // 前端/导出的 C-Score 必须是明确的选择/输出权重。不要在此静默回退至模型 Correct Cqd：pair_score 是
    // 序列化的 UI/导出字段，必须与 selection_weight_cqd 保持一致。
    let idx_correct = csv_col(header, "Selection Weight Cqd Display")
        .or_else(|_| csv_col(header, "selection_weight_cqd"))
        .with_context(|| {
            format!(
                "strict Python final_total_table_ALL_GROUPS.csv is missing selection_weight_cqd / Selection Weight Cqd Display; refusing to display/export model Correct Cqd as C-Score"
            )
        })?;
    let idx_raw = csv_col(header, "Raw Cqd Display").or_else(|_| csv_col(header, "raw_cqd")).ok();
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
        let group_id = rec
            .get(idx_group)
            .and_then(|s| s.trim().parse::<GroupId>().ok())
            .with_context(|| format!("parse group_id at strict Python CSV record {}", line_idx + 1))?;
        let correct_score = rec
            .get(idx_correct)
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
        let uncertainty_cqd = idx_uncertainty.and_then(|i| rec.get(i)).and_then(|s| parse_optional_f64(s));
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
        anyhow::bail!(
            "strict Python total score table contains no usable score rows: {}",
            path.display()
        );
    }
    Ok(out)
}

fn read_strict_python_target_trace(out_dir: &Path, expected_lane_size: usize) -> anyhow::Result<CorrectTargetTrace> {
    let csv_path = out_dir.join("target_correct_trace_weights.csv");
    let json_path = out_dir.join("target_correct_trace_metadata.json");
    let text = fs::read_to_string(&csv_path).with_context(|| {
        format!(
            "strict Python calibration did not produce a readable Correct trace: {}",
            csv_path.display()
        )
    })?;
    let metadata_json = fs::read_to_string(&json_path).with_context(|| {
        format!(
            "strict Python calibration did not produce readable trace metadata: {}",
            json_path.display()
        )
    })?;
    let metadata: Value =
        serde_json::from_str(&metadata_json).with_context(|| format!("parse Correct trace metadata: {}", json_path.display()))?;
    let trace_version = metadata
        .get("trace_version")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .context("Correct trace metadata is missing trace_version")?
        .to_string();
    let score_mode = metadata
        .get("score_mode")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .context("Correct trace metadata is missing score_mode")?
        .to_string();
    let metadata_lane_size = metadata
        .get("lane_size")
        .and_then(Value::as_u64)
        .context("Correct trace metadata is missing lane_size")? as usize;
    if metadata_lane_size != expected_lane_size {
        anyhow::bail!(
            "Correct trace lane_size mismatch: expected {}, got {}",
            expected_lane_size,
            metadata_lane_size
        );
    }

    let records = parse_csv_records(&text);
    if records.len() < 2 {
        anyhow::bail!("Correct trace CSV is empty: {}", csv_path.display());
    }
    let header = &records[0];
    let idx_version = csv_col(header, "trace_version")?;
    let idx_mode = csv_col(header, "score_mode")?;
    let idx_lane = csv_col(header, "lane_size")?;
    let idx_scope = csv_col(header, "reference_scope")?;
    let idx_group = csv_col(header, "group_id")?;
    let idx_reference_weight = csv_col(header, "reference_weight")?;
    let idx_nominal_weight = csv_col(header, "nominal_weight")?;
    let idx_raw_golden_weight = csv_col(header, "raw_golden_weight")?;
    let idx_common_coefficient = csv_col(header, "common_coefficient")?;
    let idx_coefficient_mean = csv_col(header, "coefficient_mean")?;
    let idx_coefficient_stddev = csv_col(header, "coefficient_stddev")?;
    let idx_correct_target_weight = csv_col(header, "correct_target_weight")?;
    let idx_source = csv_col(header, "source")?;

    let mut seen = HashSet::new();
    let mut weights = Vec::new();
    let mut reference_weight_sum = 0.0_f64;
    for (line_idx, rec) in records.iter().enumerate().skip(1) {
        if rec.iter().all(|v| v.trim().is_empty()) {
            continue;
        }
        let row_version = csv_record_field(rec, idx_version, "trace_version", line_idx + 1)?.trim();
        let row_mode = csv_record_field(rec, idx_mode, "score_mode", line_idx + 1)?.trim();
        if row_version != trace_version || row_mode != score_mode {
            anyhow::bail!("Correct trace CSV/metadata identity mismatch at row {}", line_idx + 1);
        }
        let lane_size = csv_record_field(rec, idx_lane, "lane_size", line_idx + 1)?
            .trim()
            .parse::<usize>()
            .with_context(|| format!("parse Correct trace lane_size at row {}", line_idx + 1))?;
        if lane_size != expected_lane_size {
            anyhow::bail!(
                "Correct trace CSV lane_size mismatch at row {}: expected {}, got {}",
                line_idx + 1,
                expected_lane_size,
                lane_size
            );
        }
        let reference_scope = csv_record_field(rec, idx_scope, "reference_scope", line_idx + 1)?.trim().to_string();
        let group_id = csv_record_field(rec, idx_group, "group_id", line_idx + 1)?
            .trim()
            .parse::<GroupId>()
            .with_context(|| format!("parse Correct trace group_id at row {}", line_idx + 1))?;
        let reference_weight = csv_record_field(rec, idx_reference_weight, "reference_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Correct reference_weight for group_id={group_id}"))?;
        let nominal_weight = csv_record_field(rec, idx_nominal_weight, "nominal_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Correct nominal_weight for group_id={group_id}"))?;
        let raw_golden_weight = csv_record_field(rec, idx_raw_golden_weight, "raw_golden_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Raw Golden weight for group_id={group_id}"))?;
        let common_coefficient = csv_record_field(rec, idx_common_coefficient, "common_coefficient", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse common Correct coefficient for group_id={group_id}"))?;
        let coefficient_mean = csv_record_field(rec, idx_coefficient_mean, "coefficient_mean", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse mean row coefficient for group_id={group_id}"))?;
        let coefficient_stddev = csv_record_field(rec, idx_coefficient_stddev, "coefficient_stddev", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse row coefficient stddev for group_id={group_id}"))?;
        let correct_target_weight = csv_record_field(rec, idx_correct_target_weight, "correct_target_weight", line_idx + 1)?
            .trim()
            .parse::<f64>()
            .with_context(|| format!("parse Correct target weight for group_id={group_id}"))?;
        let source = csv_record_field(rec, idx_source, "source", line_idx + 1)?.trim().to_string();
        if reference_scope.is_empty()
            || source.is_empty()
            || !reference_weight.is_finite()
            || reference_weight <= 0.0
            || !nominal_weight.is_finite()
            || nominal_weight.abs() <= 1e-15
            || !raw_golden_weight.is_finite()
            || raw_golden_weight < 0.0
            || !common_coefficient.is_finite()
            || common_coefficient.abs() <= 1e-15
            || !coefficient_mean.is_finite()
            || !coefficient_stddev.is_finite()
            || coefficient_stddev < 0.0
            || !correct_target_weight.is_finite()
            || correct_target_weight.abs() <= 1e-15
            || (nominal_weight - correct_target_weight).abs() > 1e-10
            || (50.0 * common_coefficient - correct_target_weight).abs() > 1e-8
        {
            anyhow::bail!("invalid Correct target trace row for group_id={group_id}");
        }
        if !seen.insert((reference_scope.clone(), group_id)) {
            anyhow::bail!(
                "duplicate Correct target trace row for scope={} group_id={}",
                reference_scope,
                group_id
            );
        }
        reference_weight_sum += reference_weight;
        weights.push(CorrectTargetTraceWeight {
            reference_scope,
            group_id,
            reference_weight,
            nominal_weight,
            raw_golden_weight,
            common_coefficient,
            coefficient_mean,
            coefficient_stddev,
            correct_target_weight,
            source,
        });
    }
    if weights.is_empty() {
        anyhow::bail!("Correct target trace contains no usable reference rows");
    }
    if (reference_weight_sum - 1.0).abs() > 1e-8 {
        anyhow::bail!(
            "Correct target trace normalized weight sum must be 1, got {:.12}",
            reference_weight_sum
        );
    }
    Ok(CorrectTargetTrace {
        trace_version,
        score_mode,
        metadata_json,
        weights,
    })
}

fn csv_record_field<'a>(record: &'a [String], index: usize, name: &str, row_number: usize) -> anyhow::Result<&'a str> {
    record
        .get(index)
        .map(String::as_str)
        .with_context(|| format!("Correct trace row {row_number} is missing {name}"))
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

/// 旧重算路径使用的兼容入口。新校准有意由已保存的 Raw lane 结果驱动；`nodes` 和 `dsu` 不属于评分模型，
/// 从而将 Raw 生成与修正分开。
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
    // 兼容调用方现在使用与已保存 lane 校准相同的 lane 特定生产路径。旧版 Python 校验器仅能通过下方明确的
    // 配对校验/审计端点使用。
    run_fast_correct(db, lane_size, rows, config, cqd_threshold)
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
            let selected_count = rows.iter().filter(|row| row.selection_status == "calibrated").count();
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
                selected_count,
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

fn run_crossfit_betabinomial_lowrank_calibration(
    db: &Db,
    lane_size: usize,
    rows: &mut Vec<LaneResultRow>,
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    let groups = build_groups(rows, cqd_threshold);
    let train: HashSet<usize> = groups.iter().enumerate().filter_map(|(idx, g)| g.train_eligible.then_some(idx)).collect();
    let computed_rate_count = ensure_required_winrates_for_strict_python(db, lane_size, &groups, config, cqd_threshold)?;
    let edges = load_edges(db, &groups)?;
    let train_edges: Vec<Edge> = edges.iter().filter(|e| train.contains(&e.ia) && train.contains(&e.ib)).cloned().collect();

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

    let (py, dynamic_rate_count) =
        run_strict_python_calibrator_with_dynamic_rate_fill(db, lane_size, cqd_threshold, 5, 123, config)?;
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
            by_gid.get(&g.group_id).map(|score| !score.candidate_model_missing).unwrap_or(false)
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
            // 低于阈值的行保持隐藏，除非 Python 已通过 Raw 低于阈值侦察/救援挑战者路径明确为其评分。
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
        target_trace: py.target_trace,
    })
}

fn read_strict_python_missing_rate_request(path: &Path) -> anyhow::Result<StrictPythonMissingRateRequest> {
    let text =
        fs::read_to_string(path).with_context(|| format!("read strict Python missing-rate request: {}", path.display()))?;
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
        let a = p
            .get("group_a")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| anyhow::anyhow!("missing-rate request pair missing group_a"))? as GroupId;
        let b = p
            .get("group_b")
            .and_then(|v| v.as_i64())
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
    if path.is_empty() { None } else { Some(PathBuf::from(path)) }
}

fn compute_rate_pairs_by_id_request(
    db: &Db,
    lane_size: usize,
    config: &RankerConfig,
    pairs: &[(GroupId, GroupId)],
    context: &str,
) -> anyhow::Result<usize> {
    let existing_rates = load_existing_rate_pairs_for_python(db)?;
    let stored_by_id: HashMap<GroupId, StoredGroup> = db.load_groups_by_lane(lane_size)?.into_iter().map(|g| (g.id, g)).collect();
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
    let mode = if config.outer_workers == 0 {
        "dynamic_queue"
    } else {
        "static_chunks"
    };
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
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute dynamic strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
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
                persist_rate_checkpoint(&db, &mut pending, samples, true)?;
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
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute dynamic strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
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
                persist_rate_checkpoint(&db, &mut pending, samples, true)?;
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

fn ensure_required_winrates_for_strict_python(
    db: &Db,
    lane_size: usize,
    groups: &[CalibGroup],
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<usize> {
    let train_ids: Vec<GroupId> = groups.iter().filter(|g| g.train_eligible).map(|g| g.group_id).collect();
    if train_ids.len() < 2 {
        anyhow::bail!(
            "strict Python calibration aborted before rate fill: not_enough_fit_candidates; fit_candidates={}",
            train_ids.len()
        );
    }

    // Python 拟合 Raw >= --raw-min 的所有未阻塞行。阻塞行被排除在拟合外，但 Python 仅评分分支需要每条
    // Raw >= --raw-min 的阻塞行均拥有对冻结训练池的边。
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
    let stored_by_id: HashMap<GroupId, StoredGroup> = db.load_groups_by_lane(lane_size)?.into_iter().map(|g| (g.id, g)).collect();
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
    let mode = if config.outer_workers == 0 {
        "dynamic_queue"
    } else {
        "static_chunks"
    };
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
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
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
                persist_rate_checkpoint(&db, &mut pending, samples, true)?;
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
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
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
                persist_rate_checkpoint(&db, &mut pending, samples, true)?;
                Ok(computed)
            }));
        }
    }

    let mut computed = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
    for handle in handles {
        let mut rows = handle.join().map_err(|_| anyhow::anyhow!("strict calibration rate worker panicked"))??;
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
    let mut stmt = conn.prepare("SELECT group_a, group_b FROM group_rates WHERE samples > 0 AND win_rate_a IS NOT NULL")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, GroupId>(0)?, row.get::<_, GroupId>(1)?)))?;
    let mut out = HashSet::new();
    for row in rows {
        let (a, b) = row?;
        if a != b {
            out.insert(ordered_group_id_pair(a, b));
        }
    }
    Ok(out)
}

fn ordered_group_id_pair(a: GroupId, b: GroupId) -> (GroupId, GroupId) { if a <= b { (a, b) } else { (b, a) } }

fn resolve_calibration_rate_pair_workers(requested_outer_workers: usize, total: usize) -> usize {
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }
    let available = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
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

fn should_report_calibration_rate_progress(done: usize, total: usize) -> bool { done == total || done % 10 == 0 }

const RATE_PERSIST_CHECKPOINT_SIZE: usize = 100;

fn persist_rate_checkpoint(
    db: &Db,
    pending: &mut Vec<(GroupId, GroupId, f64)>,
    samples: usize,
    force: bool,
) -> anyhow::Result<()> {
    if pending.len() < RATE_PERSIST_CHECKPOINT_SIZE && !force {
        return Ok(());
    }
    db.save_rate_pairs_bulk(pending, samples)?;
    pending.clear();
    Ok(())
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
    let mut stmt =
        conn.prepare("SELECT group_a, group_b, samples FROM group_rates WHERE samples > 0 AND win_rate_a IS NOT NULL")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, GroupId>(0)?, row.get::<_, GroupId>(1)?, row.get::<_, i64>(2)?))
    })?;

    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let (ga, gb, samples) = row?;
        if samples <= 0 {
            continue;
        }
        let Some(&ia0) = group_to_idx.get(&ga) else {
            continue;
        };
        let Some(&ib0) = group_to_idx.get(&gb) else {
            continue;
        };
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

fn round_to_6(x: f64) -> f64 { (x * 1_000_000.0).round() / 1_000_000.0 }

#[cfg(test)]
mod tests {
    use super::{default_selection_cqd_threshold, replacement_k_for_lane};

    #[test]
    fn default_calibration_thresholds_match_browser_defaults() {
        assert_eq!(default_selection_cqd_threshold(1), 48.0);
        assert_eq!(default_selection_cqd_threshold(2), 48.7);
    }

    #[test]
    fn replacement_slots_are_lane_specific() {
        assert_eq!(replacement_k_for_lane(1), 10.0);
        assert_eq!(replacement_k_for_lane(2), 5.0);
        assert_eq!(replacement_k_for_lane(3), 5.0);
    }
}

//! 继承大靶子的压缩协议与 Python 求解器调用。
//!
//! 临时 JSON 按统一候选索引携带大靶质量、真实胜率、账户键与合并队伍键，调用内嵌 Python。
//! 返回后检查支持集、账户唯一、队伍上限、权重血缘/质量和正仿射斜率，再交给导出层。
//! 仅结构上不可入选且零质量的列允许中性占位；必需观测列缺失则失败，临时目录保留供审计。

use super::*;

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct InheritedCompressionMetrics {
    pub(super) big_mean_abs_diff: f64,
    pub(super) big_max_abs_diff: f64,
    pub(super) big_p95_abs_diff: f64,
    pub(super) big_rmse: f64,
    pub(super) correct_mean_abs_diff: f64,
    pub(super) correct_max_abs_diff: f64,
    pub(super) correct_p95_abs_diff: f64,
    pub(super) correct_rmse: f64,
    pub(super) aligned_mean_abs_diff: f64,
    pub(super) aligned_max_abs_diff: f64,
    pub(super) aligned_p95_abs_diff: f64,
    pub(super) aligned_rmse: f64,
    pub(super) flat_raw_mean_abs_diff: f64,
    pub(super) flat_raw_max_abs_diff: f64,
    pub(super) flat_raw_rmse: f64,
    pub(super) affine_slope: f64,
    pub(super) affine_intercept: f64,
    pub(super) score_spearman: Option<f64>,
    pub(super) anchor_distance_l2: f64,
    pub(super) addition_l2: f64,
    pub(super) addition_max_abs: f64,
    pub(super) similar_addition_rms: f64,
    pub(super) local_order_inversion_rms: f64,
    pub(super) local_order_inversion_rate: f64,
    pub(super) addition_cancellation_ratio: f64,
    pub(super) weight_cancellation_ratio: f64,
    pub(super) final_weight_std: f64,
    pub(super) final_weight_sum: f64,
    pub(super) final_weight_monotonic_violation_count: usize,
    pub(super) final_weight_monotonic_violation_max: f64,
    pub(super) final_weight_c_score_spearman: Option<f64>,
    pub(super) adjacent_weight_gap_rms: f64,
    pub(super) adjacent_weight_gap_max: f64,
    pub(super) weight_gap_second_difference_rms: f64,
    #[serde(default)]
    pub(super) tail_weight_monotonic_violation_count: usize,
    #[serde(default)]
    pub(super) tail_weight_monotonic_violation_rms: f64,
    #[serde(default)]
    pub(super) tail_low_effective_slot_penalty: f64,
    pub(super) compressed_corr_to_big: Option<f64>,
    pub(super) compressed_corr_to_correct: Option<f64>,
    pub(super) regularization_factor: f64,
    pub(super) pure_structural_prior: bool,
    pub(super) structure_score: f64,
    pub(super) max_diff_target: f64,
    pub(super) max_diff_target_met: bool,
    pub(super) max_diff_target_margin: f64,
    pub(super) selection_error_cost_normalized: f64,
    pub(super) selection_structure_cost_normalized: f64,
    pub(super) selection_score: f64,
    pub(super) selection_rule: String,
    #[serde(default)]
    pub(super) golden_unit_equalization_count: usize,
    #[serde(default)]
    pub(super) golden_unit_final_spread: f64,
    #[serde(default)]
    pub(super) golden_unit_projection_l2: f64,
    #[serde(default)]
    pub(super) golden_nonunit_free_count: usize,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct InheritedCompressionSolverResponse {
    pub(super) status: String,
    pub(super) browser_initial_indices: Vec<usize>,
    pub(super) initial_indices: Vec<usize>,
    pub(super) locked_indices: Vec<usize>,
    pub(super) pre_deletion_selected_indices: Vec<usize>,
    pub(super) deletion_locked_indices: Vec<usize>,
    pub(super) locked_seed_weight_scale: f64,
    pub(super) locked_seed_original_weight_sum: f64,
    pub(super) locked_seed_target_weight_sum: f64,
    pub(super) initial_anchor_weights: Vec<f64>,
    pub(super) final_seed_anchor_weights: Vec<f64>,
    pub(super) seed_selection_steps: Vec<serde_json::Value>,
    pub(super) selected_indices: Vec<usize>,
    #[serde(default)]
    pub(super) effective_big_weights: Option<Vec<f64>>,
    pub(super) base_weights: Vec<f64>,
    pub(super) fitted_additions: Vec<f64>,
    pub(super) lineage_weights: Vec<f64>,
    pub(super) selected_weights: Vec<f64>,
    pub(super) replacements: Vec<serde_json::Value>,
    pub(super) deletions: Vec<serde_json::Value>,
    pub(super) deletion_path: Vec<serde_json::Value>,
    #[serde(default)]
    pub(super) final_tail_swaps: Vec<serde_json::Value>,
    #[serde(default)]
    pub(super) final_support_edits: Vec<serde_json::Value>,
    #[serde(default)]
    pub(super) final_support_search: serde_json::Value,
    pub(super) owner_cap: usize,
    pub(super) target_total: usize,
    pub(super) deletion_lock_count: usize,
    pub(super) output_target_count: usize,
    pub(super) score_denominator: usize,
    pub(super) tail_group_count: usize,
    pub(super) tail_weight_sum: f64,
    pub(super) tail_l1_weight_sum: f64,
    pub(super) big_target_weight_sum: f64,
    pub(super) initial_metrics: InheritedCompressionMetrics,
    pub(super) initial_transport: serde_json::Value,
    pub(super) pre_deletion_metrics: InheritedCompressionMetrics,
    pub(super) pre_deletion_transport: serde_json::Value,
    pub(super) final_metrics: InheritedCompressionMetrics,
    pub(super) final_transport: serde_json::Value,
    pub(super) regularization_path: Vec<serde_json::Value>,
}

pub(super) fn inherited_target_feasibility_error(
    indices: &[usize],
    candidates: &[TargetCandidate],
    raw_min: f64,
    owner_cap: usize,
) -> Option<String> {
    let mut seen_groups = HashSet::new();
    let mut seen_accounts = HashSet::new();
    let mut owner_counts: HashMap<&str, usize> = HashMap::new();
    for &idx in indices {
        let Some(candidate) = candidates.get(idx) else {
            return Some(format!("靶子索引越界：{idx}"));
        };
        if !seen_groups.insert(idx) {
            return Some(format!("重复靶子 group_id={}", candidate.row.group_id));
        }
        if !target_candidate_is_eligible(candidate, raw_min) {
            return Some(format!(
                "非法靶子 group_id={} status={} blocked={}",
                candidate.row.group_id, candidate.row.selection_status, candidate.row.is_blocked,
            ));
        }
        for key in &candidate.player_keys {
            if !seen_accounts.insert(key.as_str()) {
                return Some(format!("重复号约束失败：账号 {key} 同时出现在多个靶子中"));
            }
        }
        let count = owner_counts.entry(candidate.row.root_team_name.as_str()).or_insert(0);
        *count += 1;
        if *count > owner_cap {
            return Some(format!(
                "合并战队 {} 的靶子数 {} 超过上限 {}",
                candidate.row.root_team_name, *count, owner_cap,
            ));
        }
    }
    None
}

pub(super) fn run_inherited_big_target_compression_solver(
    big_weights: &[f64],
    reference_indices: &[usize],
    compression_target_indices: &HashSet<usize>,
    target_raw_min: f64,
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> anyhow::Result<InheritedCompressionSolverResponse> {
    const TARGET_TOTAL: usize = 50;
    const OWNER_CAP: usize = 5;
    if big_weights.len() != candidates.len() {
        anyhow::bail!("大靶权重与候选数不一致：{} vs {}", big_weights.len(), candidates.len(),);
    }
    if candidates.len() < TARGET_TOTAL || reference_indices.is_empty() {
        anyhow::bail!("继承式压缩需要至少 {} 个候选和非空 reference", TARGET_TOTAL);
    }

    let mut rate_matrix = Vec::with_capacity(reference_indices.len());
    let mut reference_scores = Vec::with_capacity(reference_indices.len());
    for &ref_idx in reference_indices {
        let ref_gid = candidates[ref_idx].row.group_id;
        let mut row_rates = Vec::with_capacity(candidates.len());
        for (target_idx, candidate) in candidates.iter().enumerate() {
            let rate = if target_rate_column_requires_observed(target_idx, big_weights[target_idx], compression_target_indices) {
                rate_between(rate_map, ref_gid, candidate.row.group_id).with_context(|| {
                    format!(
                        "继承式可变数量压缩缺少必要胜率：ref_group_id={} target_group_id={}",
                        ref_gid, candidate.row.group_id,
                    )
                })?
            } else {
                // 此列在结构上不合资格且 Correct 质量为零，因而求解器绝不会使用它。中性的有限占位符可保持
                // 全候选元数据索引稳定，而不凭空制造 low×low 模拟需求。
                rate_between(rate_map, ref_gid, candidate.row.group_id).unwrap_or(50.0)
            };
            row_rates.push(rate);
        }
        rate_matrix.push(row_rates);
        reference_scores.push(
            candidates[ref_idx]
                .row
                .pair_score
                .filter(|score| score.is_finite())
                .unwrap_or(candidates[ref_idx].row.raw_average_cqd),
        );
    }

    let candidate_correct_scores = candidates
        .iter()
        .map(|candidate| {
            candidate
                .row
                .pair_score
                .filter(|score| score.is_finite())
                .unwrap_or(candidate.row.raw_average_cqd)
        })
        .collect::<Vec<_>>();
    let golden_weights = candidates.iter().map(|candidate| candidate.row.golden_rate).collect::<Vec<_>>();
    if golden_weights.iter().any(|weight| !weight.is_finite() || *weight < 0.0) || golden_weights.iter().sum::<f64>() <= 1e-12 {
        anyhow::bail!("继承式压缩缺少有效的非负 Golden 吸收权重");
    }
    let pair_ranks = candidates.iter().map(|candidate| candidate.row.pair_rank).collect::<Vec<_>>();
    let raw_ranks = candidates.iter().map(|candidate| candidate.row.rank).collect::<Vec<_>>();
    let group_ids = candidates.iter().map(|candidate| candidate.row.group_id).collect::<Vec<_>>();
    let selection_status = candidates
        .iter()
        .map(|candidate| candidate.row.selection_status.clone())
        .enumerate()
        .map(|(idx, status)| {
            if compression_target_indices.contains(&idx) {
                status
            } else {
                "below_threshold".to_string()
            }
        })
        .collect::<Vec<_>>();
    let blocked = candidates.iter().map(|candidate| candidate.row.is_blocked).collect::<Vec<_>>();
    let raw_members = candidates
        .iter()
        .map(|candidate| {
            candidate
                .row
                .canonical
                .split('+')
                .map(str::trim)
                .filter(|member| !member.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let account_keys = candidates.iter().map(|candidate| candidate.player_keys.clone()).collect::<Vec<_>>();
    let owner_keys = candidates
        .iter()
        .map(|candidate| candidate.row.root_team_name.clone())
        .collect::<Vec<_>>();

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let out_dir = std::env::temp_dir().join(format!("tswn_inherited_target_compression_{}_{}", std::process::id(), stamp));
    fs::create_dir_all(&out_dir).with_context(|| format!("create inherited compression temp dir: {}", out_dir.display()))?;
    let script_path = out_dir.join("target_milp_solver.py");
    let input_path = out_dir.join("inherited_compression_input.json");
    let output_path = out_dir.join("inherited_compression_output.json");
    fs::write(&script_path, TARGET_MILP_SOLVER)
        .with_context(|| format!("write inherited compression solver: {}", script_path.display()))?;
    let payload = serde_json::json!({
        "mode": "compress_inherited_big_target_to_top50",
        "big_weights": big_weights,
        "golden_weights": golden_weights,
        "rate_matrix": rate_matrix,
        "reference_scores": reference_scores,
        "candidate_correct_scores": candidate_correct_scores,
        "pair_ranks": pair_ranks,
        "raw_ranks": raw_ranks,
        "group_ids": group_ids,
        "selection_status": selection_status,
        "blocked": blocked,
        "raw_members": raw_members,
        "account_keys": account_keys,
        "owner_keys": owner_keys,
        "target_total": TARGET_TOTAL,
        "owner_cap": OWNER_CAP,
        "max_replacements": 8,
        "max_deletions": 10,
        "max_final_edits": 4,
        "deletion_lock_count": 30,
        "compression_algorithm": "big_weight_top40_dynamic10_normalized_delta_minimax_v1",
        "locked_big_weight_count": 40,
        "dynamic_big_weight_count": 10,
        "delta_limit": 0.1,
    });
    fs::write(&input_path, serde_json::to_vec(&payload)?)
        .with_context(|| format!("write inherited compression input: {}", input_path.display()))?;

    let mut last_error = None;
    for exe in ["python3", "python"] {
        let output = Command::new(exe)
            .arg(&script_path)
            .arg("--input")
            .arg(&input_path)
            .arg("--output")
            .arg(&output_path)
            .output();
        match output {
            Ok(process) if process.status.success() && output_path.exists() => {
                let response: InheritedCompressionSolverResponse = serde_json::from_slice(
                    &fs::read(&output_path)
                        .with_context(|| format!("read inherited compression output: {}", output_path.display()))?,
                )
                .with_context(|| format!("parse inherited compression output: {}", output_path.display()))?;
                if !response.status.starts_with("ok")
                    || response.target_total != TARGET_TOTAL
                    || response.owner_cap != OWNER_CAP
                    || response.deletion_lock_count != 30
                    || response.browser_initial_indices.len() != TARGET_TOTAL
                    || response.initial_indices.len() != TARGET_TOTAL
                    || response.initial_anchor_weights.len() != TARGET_TOTAL
                    || response.pre_deletion_selected_indices.len() != TARGET_TOTAL
                    || response.output_target_count != response.selected_indices.len()
                    || response.output_target_count > TARGET_TOTAL
                    || response.output_target_count < TARGET_TOTAL.saturating_sub(10)
                    || response.score_denominator != response.output_target_count
                    || response.base_weights.len() != response.output_target_count
                    || response.final_seed_anchor_weights.len() != response.output_target_count
                    || response.fitted_additions.len() != response.output_target_count
                    || response.lineage_weights.len() != response.output_target_count
                    || response.selected_weights.len() != response.output_target_count
                {
                    anyhow::bail!(
                        "继承式可变数量压缩器返回非法状态或长度：status={} selected={} denominator={}",
                        response.status,
                        response.selected_indices.len(),
                        response.score_denominator,
                    );
                }
                if let Some(error) =
                    inherited_target_feasibility_error(&response.selected_indices, candidates, target_raw_min, OWNER_CAP)
                {
                    anyhow::bail!("继承式可变数量约束校验失败：{error}");
                }
                let selected_set = response.selected_indices.iter().copied().collect::<HashSet<_>>();
                if response.deletion_locked_indices.iter().any(|idx| !selected_set.contains(idx)) {
                    anyhow::bail!("可变数量删除移除了初版靶子 C-Score 前30组合");
                }
                let pre_deletion_set = response.pre_deletion_selected_indices.iter().copied().collect::<HashSet<_>>();
                if response.locked_indices.iter().any(|idx| !pre_deletion_set.contains(idx)) {
                    anyhow::bail!("继承式 Top50 替换移除了初版锁定组合");
                }
                let initial_set = response.initial_indices.iter().copied().collect::<HashSet<_>>();
                if response.locked_indices.iter().any(|idx| !initial_set.contains(idx)) {
                    anyhow::bail!("C-Score 感知初始选号遗漏了锁定组合");
                }
                let validation_big_weights = response
                    .effective_big_weights
                    .as_deref()
                    .filter(|weights| weights.len() == big_weights.len())
                    .unwrap_or(big_weights);
                let total_weight = validation_big_weights.iter().sum::<f64>();
                let locked_original_weight_sum =
                    response.locked_indices.iter().map(|&idx| validation_big_weights[idx]).sum::<f64>();
                let initial_selected_original_weight_sum =
                    response.initial_indices.iter().map(|&idx| validation_big_weights[idx]).sum::<f64>();
                let initial_tail_weight_sum = total_weight - initial_selected_original_weight_sum;
                let locked_target_weight_sum = response
                    .initial_indices
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, &idx)| {
                        response.locked_indices.contains(&idx).then_some(response.initial_anchor_weights[slot])
                    })
                    .sum::<f64>();
                if !response.locked_seed_weight_scale.is_finite()
                    || (response.locked_seed_original_weight_sum - locked_original_weight_sum).abs() > 1e-8
                    || (response.locked_seed_target_weight_sum - locked_target_weight_sum).abs() > 1e-8
                    || (response.locked_seed_weight_scale - 1.0).abs() > 1e-12
                {
                    anyhow::bail!(
                        "初版组合大靶基础权重/共同吸收审计失败：original={} selected_original={} target={} scale={}",
                        response.locked_seed_original_weight_sum,
                        initial_selected_original_weight_sum,
                        response.locked_seed_target_weight_sum,
                        response.locked_seed_weight_scale,
                    );
                }
                let mut initial_absorption_sum = 0.0;
                for (slot, &idx) in response.initial_indices.iter().enumerate() {
                    let base = validation_big_weights
                        .get(idx)
                        .copied()
                        .ok_or_else(|| anyhow::anyhow!("压缩器返回越界初始索引 {idx}"))?;
                    let weight = response.initial_anchor_weights[slot];
                    let addition = weight - base;
                    if !weight.is_finite() || !addition.is_finite() {
                        anyhow::bail!(
                            "C-Score 感知初始权重或共同吸收增量含非有限值：group_id={}",
                            candidates[idx].row.group_id,
                        );
                    }
                    initial_absorption_sum += addition;
                }
                if (initial_absorption_sum - initial_tail_weight_sum).abs() > 1e-7 {
                    anyhow::bail!(
                        "初版支持集共同吸收尾部质量不守恒：selected_original={} tail={} absorbed={}",
                        initial_selected_original_weight_sum,
                        initial_tail_weight_sum,
                        initial_absorption_sum,
                    );
                }
                for (slot, &idx) in response.selected_indices.iter().enumerate() {
                    let expected_base = validation_big_weights
                        .get(idx)
                        .copied()
                        .ok_or_else(|| anyhow::anyhow!("压缩器返回越界索引 {idx}"))?;
                    let base = response.base_weights[slot];
                    let addition = response.fitted_additions[slot];
                    let lineage = response.lineage_weights[slot];
                    let selected = response.selected_weights[slot];
                    let seed = response.final_seed_anchor_weights[slot];
                    if !base.is_finite()
                        || !seed.is_finite()
                        || !addition.is_finite()
                        || !lineage.is_finite()
                        || !selected.is_finite()
                        || (base - expected_base).abs() > 1e-9
                        || (lineage - base - addition).abs() > 1e-8
                    {
                        anyhow::bail!(
                            "继承式可变数量权重血缘校验失败：slot={} group_id={}",
                            slot,
                            candidates[idx].row.group_id,
                        );
                    }
                    let expected_selected = lineage * response.output_target_count as f64 / response.big_target_weight_sum;
                    if (selected - expected_selected).abs() > 1e-8 {
                        anyhow::bail!(
                            "导出权重 /n 缩放校验失败：slot={} group_id={} selected={} expected={}",
                            slot,
                            candidates[idx].row.group_id,
                            selected,
                            expected_selected,
                        );
                    }
                }
                let initial_anchor_sum = response.initial_anchor_weights.iter().sum::<f64>();
                let final_seed_anchor_sum = response.final_seed_anchor_weights.iter().sum::<f64>();
                let lineage_sum = response.lineage_weights.iter().sum::<f64>();
                let selected_sum = response.selected_weights.iter().sum::<f64>();
                let addition_sum = response.fitted_additions.iter().sum::<f64>();
                if (response.big_target_weight_sum - total_weight).abs() > 1e-8
                    || (initial_anchor_sum - total_weight).abs() > 1e-7
                    || (final_seed_anchor_sum - total_weight).abs() > 1e-7
                    || (lineage_sum - total_weight).abs() > 1e-7
                    || (selected_sum - response.output_target_count as f64).abs() > 1e-7
                    || (addition_sum - response.tail_weight_sum).abs() > 1e-7
                {
                    anyhow::bail!(
                        "继承式可变数量权重不守恒：big={} initial_seed={} final_seed={} lineage={} selected={} target_count={} tail={} additions={}",
                        total_weight,
                        initial_anchor_sum,
                        final_seed_anchor_sum,
                        lineage_sum,
                        selected_sum,
                        response.output_target_count,
                        response.tail_weight_sum,
                        addition_sum,
                    );
                }
                let deletion_locked_set = response.deletion_locked_indices.iter().copied().collect::<HashSet<_>>();
                let mut priority_slots = response
                    .selected_indices
                    .iter()
                    .enumerate()
                    .filter_map(|(slot, idx)| deletion_locked_set.contains(idx).then_some(slot))
                    .collect::<Vec<_>>();
                priority_slots.sort_by(|&left, &right| {
                    compare_target_candidates(
                        &candidates[response.selected_indices[left]],
                        &candidates[response.selected_indices[right]],
                    )
                });
                let priority_count = priority_slots.len();
                if priority_count != response.deletion_lock_count {
                    anyhow::bail!(
                        "最终支持集遗漏固定删除锁定项：expected={} actual={}",
                        response.deletion_lock_count,
                        priority_count,
                    );
                }
                let metrics = [
                    response.initial_metrics.big_mean_abs_diff,
                    response.initial_metrics.big_max_abs_diff,
                    response.initial_metrics.big_rmse,
                    response.final_metrics.big_mean_abs_diff,
                    response.final_metrics.big_max_abs_diff,
                    response.final_metrics.big_rmse,
                    response.final_metrics.correct_mean_abs_diff,
                    response.final_metrics.correct_max_abs_diff,
                    response.final_metrics.correct_p95_abs_diff,
                    response.final_metrics.correct_rmse,
                    response.initial_metrics.aligned_mean_abs_diff,
                    response.initial_metrics.aligned_max_abs_diff,
                    response.initial_metrics.aligned_rmse,
                    response.pre_deletion_metrics.aligned_mean_abs_diff,
                    response.pre_deletion_metrics.aligned_max_abs_diff,
                    response.pre_deletion_metrics.aligned_rmse,
                    response.final_metrics.aligned_mean_abs_diff,
                    response.final_metrics.aligned_max_abs_diff,
                    response.final_metrics.aligned_p95_abs_diff,
                    response.final_metrics.aligned_rmse,
                    response.final_metrics.flat_raw_mean_abs_diff,
                    response.final_metrics.flat_raw_max_abs_diff,
                    response.final_metrics.flat_raw_rmse,
                    response.final_metrics.affine_slope,
                    response.final_metrics.affine_intercept,
                    response.final_metrics.anchor_distance_l2,
                    response.final_metrics.similar_addition_rms,
                    response.final_metrics.local_order_inversion_rms,
                    response.final_metrics.final_weight_monotonic_violation_max,
                    response.final_metrics.adjacent_weight_gap_rms,
                    response.final_metrics.adjacent_weight_gap_max,
                    response.final_metrics.weight_gap_second_difference_rms,
                    response.final_metrics.final_weight_sum,
                    response.final_metrics.max_diff_target,
                    response.final_metrics.max_diff_target_margin,
                    response.final_metrics.selection_error_cost_normalized,
                    response.final_metrics.selection_structure_cost_normalized,
                    response.final_metrics.selection_score,
                    response.final_metrics.golden_unit_final_spread,
                    response.final_metrics.golden_unit_projection_l2,
                ];
                if metrics.iter().any(|value| !value.is_finite()) {
                    anyhow::bail!("继承式可变数量压缩器返回非有限指标");
                }
                if response.final_metrics.affine_slope <= 0.0 {
                    anyhow::bail!(
                        "仿射压平可变数量靶校验失败：slope={}（必须为正）",
                        response.final_metrics.affine_slope,
                    );
                }
                return Ok(response);
            }
            Ok(process) => {
                last_error = Some(format!(
                    "{} exited with {:?}; stdout={}; stderr={}",
                    exe,
                    process.status.code(),
                    String::from_utf8_lossy(&process.stdout).trim(),
                    String::from_utf8_lossy(&process.stderr).trim(),
                ));
            }
            Err(error) => {
                last_error = Some(format!("failed to start {exe}: {error}"));
            }
        }
    }
    anyhow::bail!(
        "继承式可变数量压缩器不可用：{}",
        last_error.unwrap_or_else(|| "python3/python 均未返回具体错误".to_string())
    )
}

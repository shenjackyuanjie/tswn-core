//! 靶子候选、胜率补算、导出与审计。
//!
//! 从当前 Correct 追溯继承大靶质量，验证 Golden 未变化，再补齐支持列所需的真实胜率。
//! 求解器可输出 40–50 个组合；导出权重和等于最终数量 n，以 Σ(rate*weight)/n 计分。
//! 旧请求的阈值/固定前缀字段只保留兼容，实际候选阈值来自追溯；失败保留已补算缓存。

use super::*;

impl AppService {
    pub fn generate_lane_targets(&self, lane: usize, req: TargetGenerationRequest) -> anyhow::Result<TargetGenerationResponse> {
        #[cfg(any())]
        const SUPPORT_COUNT: usize = 50;

        // 仅为兼容旧的 target-bar 请求而保留。
        let _ = (req.cqd_threshold, req.fixed_main_count);
        let lane_rows = self.db.lane_results(lane)?;
        if lane_rows.is_empty() {
            anyhow::bail!("该赛道还没有结果；请先读取/重算赛道");
        }
        let lane_group_count = lane_rows.len();
        self.db.set_lane_status(lane, "generating_targets", lane_group_count)?;
        self.db.set_lane_progress(
            lane,
            "target_preparing",
            0,
            1,
            0,
            0,
            0,
            "preparing target candidates and checking required rate coverage; 0.00 pair/s",
        )?;
        let trace = self
            .db
            .correct_target_trace(lane)?
            .ok_or_else(|| anyhow::anyhow!("该赛道没有与当前结果匹配的 Correct 追溯记录；请先运行一次 Correct 校准"))?;
        let mut trace_metadata: serde_json::Value =
            serde_json::from_str(&trace.metadata_json).context("数据库中的 Correct 追溯 metadata 不是有效 JSON")?;
        let target_raw_min = trace_metadata
            .get("calibration_raw_min")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite())
            .ok_or_else(|| anyhow::anyhow!("Correct 追溯缺少有限的 calibration_raw_min；请按当前阈值重新校准"))?;
        if trace.weights.is_empty() {
            anyhow::bail!("Correct 追溯记录为空；请重新运行 Correct 校准");
        }

        let candidates: Vec<TargetCandidate> = lane_rows.into_iter().map(TargetCandidate::from_row).collect();
        let index_by_group: HashMap<crate::model::GroupId, usize> = candidates
            .iter()
            .enumerate()
            .map(|(idx, candidate)| (candidate.row.group_id, idx))
            .collect();
        let mut merged_weights = vec![0.0_f64; candidates.len()];
        let mut raw_weight_sum = 0.0_f64;
        let mut raw_trace_count = 0usize;
        for candidate in &candidates {
            let weight = candidate.row.golden_rate;
            if weight.is_finite() && weight > 0.0 {
                raw_weight_sum += weight;
                raw_trace_count += 1;
            }
        }
        if raw_trace_count == 0 {
            anyhow::bail!("当前结果没有正且有限的 Golden 权重，无法追溯 Raw");
        }

        let mut scope_sums: HashMap<String, f64> = HashMap::new();
        let mut seen_correct = HashSet::new();
        let mut correct_target_indices = Vec::new();
        let mut correct_target_weights = Vec::new();
        let mut correct_nominal_weight_sum = 0.0_f64;
        let mut correct_target_weight_sum = 0.0_f64;
        for weight in &trace.weights {
            let Some(&candidate_idx) = index_by_group.get(&weight.group_id) else {
                anyhow::bail!(
                    "Correct 追溯引用了当前赛道不存在的 group_id={}；请重新运行 Correct 校准",
                    weight.group_id
                );
            };
            if !weight.reference_weight.is_finite()
                || weight.reference_weight <= 0.0
                || !weight.nominal_weight.is_finite()
                || weight.nominal_weight.abs() <= 1e-15
                || !weight.raw_golden_weight.is_finite()
                || weight.raw_golden_weight < 0.0
                || !weight.common_coefficient.is_finite()
                || weight.common_coefficient.abs() <= 1e-15
                || !weight.coefficient_mean.is_finite()
                || !weight.coefficient_stddev.is_finite()
                || weight.coefficient_stddev < 0.0
                || !weight.correct_target_weight.is_finite()
                || weight.correct_target_weight.abs() <= 1e-15
                || (weight.nominal_weight - weight.correct_target_weight).abs() > 1e-10
                || (50.0 * weight.common_coefficient - weight.correct_target_weight).abs() > 1e-8
            {
                anyhow::bail!(
                    "Correct 追溯权重非法：scope={} group_id={}",
                    weight.reference_scope,
                    weight.group_id
                );
            }
            if !seen_correct.insert((weight.reference_scope.clone(), weight.group_id)) {
                anyhow::bail!(
                    "Correct 追溯存在重复引用：scope={} group_id={}",
                    weight.reference_scope,
                    weight.group_id
                );
            }
            *scope_sums.entry(weight.reference_scope.clone()).or_insert(0.0) += weight.reference_weight;
            correct_nominal_weight_sum += weight.nominal_weight;
            correct_target_weight_sum += weight.correct_target_weight;
            if (candidates[candidate_idx].row.golden_rate - weight.raw_golden_weight).abs() > 1e-8 {
                anyhow::bail!(
                    "Correct 靶权重对应的 Golden 已变化：group_id={}；请重新运行 Correct 校准",
                    weight.group_id
                );
            }
            merged_weights[candidate_idx] += weight.correct_target_weight;
            correct_target_indices.push(candidate_idx);
            correct_target_weights.push(weight.correct_target_weight);
        }
        for (scope, sum) in &scope_sums {
            if (*sum - 1.0).abs() > 1e-8 {
                anyhow::bail!(
                    "Correct 追溯 scope={} 的 reference_weight 总和应为 1，实际为 {:.12}",
                    scope,
                    sum
                );
            }
        }
        let mut rate_map = self.db.lane_rate_map(lane)?;
        let all_candidate_indices = (0..candidates.len()).collect::<Vec<_>>();
        let compression_target_indices = candidates
            .iter()
            .enumerate()
            .filter_map(|(idx, candidate)| target_candidate_is_eligible(candidate, target_raw_min).then_some(idx))
            .collect::<Vec<_>>();
        if compression_target_indices.len() < 50 {
            anyhow::bail!("Raw 阈值内合法靶候选不足 50 个：{}", compression_target_indices.len(),);
        }
        let mut required_rate_target_indices = compression_target_indices.clone();
        required_rate_target_indices.extend(
            merged_weights
                .iter()
                .enumerate()
                .filter_map(|(idx, weight)| (weight.abs() > 1e-15).then_some(idx)),
        );
        required_rate_target_indices.sort_unstable();
        required_rate_target_indices.dedup();
        // 每个已保存行仍是误差审计行。矩阵列由满足阈值的支持候选项和非零 Correct 谱系列的并集构成
        // （Raw Golden 可能贡献低于阈值的列）。零质量、低于阈值的侦察列无法被使用。
        let missing_correct_pairs =
            collect_missing_target_rate_pairs(&all_candidate_indices, &required_rate_target_indices, &candidates, &rate_map);
        fill_missing_target_rates_with_checkpoints(
            &self.db,
            &self.config,
            lane,
            &missing_correct_pairs,
            &candidates,
            &mut rate_map,
        )?;
        let mut correct_forward_diffs = Vec::with_capacity(candidates.len());
        for candidate in &candidates {
            let mut weighted_rate_sum = 0.0_f64;
            for (&target_idx, &target_weight) in correct_target_indices.iter().zip(correct_target_weights.iter()) {
                let rate =
                    rate_between(&rate_map, candidate.row.group_id, candidates[target_idx].row.group_id).with_context(|| {
                        format!(
                            "Correct 靶权重前向重放缺少胜率：score_group_id={} target_group_id={}",
                            candidate.row.group_id, candidates[target_idx].row.group_id
                        )
                    })?;
                weighted_rate_sum += target_weight * rate;
            }
            let replay_score = weighted_rate_sum / 50.0;
            let saved_score = candidate
                .row
                .pair_score
                .filter(|score| score.is_finite())
                .unwrap_or(candidate.row.raw_average_cqd);
            correct_forward_diffs.push(replay_score - saved_score);
        }
        let correct_forward_replay_mean_abs_diff =
            correct_forward_diffs.iter().map(|diff| diff.abs()).sum::<f64>() / correct_forward_diffs.len().max(1) as f64;
        let correct_forward_replay_max_abs_diff = correct_forward_diffs.iter().map(|diff| diff.abs()).fold(0.0_f64, f64::max);
        let correct_forward_replay_rmse = (correct_forward_diffs.iter().map(|diff| diff * diff).sum::<f64>()
            / correct_forward_diffs.len().max(1) as f64)
            .sqrt();
        let row_coefficient_replay_mean_abs_diff = trace_metadata
            .pointer("/rowwise_correct_component/row_replay_mean_abs_diff")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let row_coefficient_replay_max_abs_diff = trace_metadata
            .pointer("/rowwise_correct_component/row_replay_max_abs_diff")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);

        let reference_indices = all_candidate_indices.clone();
        let big_target_weight_sum = merged_weights.iter().sum::<f64>();
        let compression_target_set = compression_target_indices.iter().copied().collect::<HashSet<_>>();

        // 从实际浏览器 Top50 开始，并为替换一次性冻结其 C-Score Top30。较低的 20 项按低 C-Score 优先搜索；
        // 删除阶段一次性冻结自己的替换后 Top30。
        self.db.set_lane_progress(
            lane,
            "target_compression",
            0,
            1,
            required_rate_target_indices.len(),
            required_rate_target_indices.len(),
            0,
            "target rate matrix ready; optimizing initial support, weights, replacements, deletion path, and final support",
        )?;
        let compression = run_inherited_big_target_compression_solver(
            &merged_weights,
            &reference_indices,
            &compression_target_set,
            target_raw_min,
            &candidates,
            &rate_map,
        )?;
        let mut selected_records = compression
            .selected_indices
            .iter()
            .copied()
            .zip(compression.base_weights.iter().copied())
            .zip(compression.final_seed_anchor_weights.iter().copied())
            .zip(compression.fitted_additions.iter().copied())
            .zip(compression.lineage_weights.iter().copied())
            .zip(compression.selected_weights.iter().copied())
            .map(|(((((idx, base), seed), addition), lineage), weight)| (idx, base, seed, addition, lineage, weight))
            .collect::<Vec<_>>();
        selected_records.sort_by(|a, b| compare_target_candidates(&candidates[a.0], &candidates[b.0]));
        let selected_indices = selected_records.iter().map(|record| record.0).collect::<Vec<_>>();
        let selected_weights = selected_records.iter().map(|record| record.5).collect::<Vec<_>>();
        let support_base_weight_sum = selected_records.iter().map(|record| record.1).sum::<f64>();
        let target_weight_sum = selected_weights.iter().sum::<f64>();
        let target_weight_min = selected_weights.iter().copied().fold(f64::INFINITY, f64::min);
        let target_weight_max = selected_weights.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let target_weight_mean = target_weight_sum / selected_weights.len().max(1) as f64;
        let target_weight_max_deviation = selected_weights
            .iter()
            .map(|weight| (*weight - target_weight_mean).abs())
            .fold(0.0_f64, f64::max);

        let mut output_rows = Vec::with_capacity(selected_records.len());
        for (slot, &(idx, inherited_base, seed_initial_weight, transported_addition, lineage_weight, target_weight)) in
            selected_records.iter().enumerate()
        {
            let candidate = &candidates[idx];
            let (average_reference_winrate, reference_rate_count) =
                average_reference_winrate_for_target(&reference_indices, idx, &candidates, &rate_map);
            let raw_rank = candidate.row.rank;
            let correct_rank = candidate.row.pair_rank;
            output_rows.push(TargetGenerationRow {
                target_rank: slot + 1,
                target_weight,
                base_weight: Some(inherited_base),
                seed_initial_weight: Some(seed_initial_weight),
                fitted_tail_weight: Some(transported_addition),
                lineage_weight: Some(lineage_weight),
                phase: "fixed_c_score_top30_replace_and_delete_collective_absorption".to_string(),
                trace_component: "VariableCountCorrectCompressedTarget".to_string(),
                trace_scope: "browser_top50_then_collective_absorption_with_fixed_c_score_top30_delete_lock".to_string(),
                trace_source: "big_target_lineage_collectively_absorbed_then_exported_at_n_over_total_mass".to_string(),
                reference_weight: None,
                group_id: candidate.row.group_id,
                canonical: candidate.row.canonical.clone(),
                team_name: candidate.row.team_name.clone(),
                root_team_name: candidate.row.root_team_name.clone(),
                correct_rank,
                correct_score: candidate
                    .row
                    .pair_score
                    .filter(|score| score.is_finite())
                    .unwrap_or(candidate.row.raw_average_cqd),
                raw_rank,
                raw_score: candidate.row.raw_average_cqd,
                delta_rank: correct_rank.map(|rank| raw_rank as i64 - rank as i64),
                selection_status: candidate.row.selection_status.clone(),
                type_label: candidate.row.type_label.clone(),
                simple_type_label: candidate.row.simple_type_label.clone(),
                average_reference_winrate,
                reference_rate_count,
                player_keys: candidate.player_keys.clone(),
            });
        }

        let cqd_threshold = trace_metadata
            .get("calibration_raw_min")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let target_config_text = build_target_config_text(&selected_indices, &selected_weights, &candidates);
        let initial_group_ids = compression
            .initial_indices
            .iter()
            .map(|&idx| candidates[idx].row.group_id)
            .collect::<Vec<_>>();
        let browser_initial_group_ids = compression
            .browser_initial_indices
            .iter()
            .map(|&idx| candidates[idx].row.group_id)
            .collect::<Vec<_>>();
        let locked_group_ids = compression
            .locked_indices
            .iter()
            .map(|&idx| candidates[idx].row.group_id)
            .collect::<Vec<_>>();
        let pre_deletion_group_ids = compression
            .pre_deletion_selected_indices
            .iter()
            .map(|&idx| candidates[idx].row.group_id)
            .collect::<Vec<_>>();
        let deletion_locked_group_ids = compression
            .deletion_locked_indices
            .iter()
            .map(|&idx| candidates[idx].row.group_id)
            .collect::<Vec<_>>();
        let final_group_ids = selected_indices.iter().map(|&idx| candidates[idx].row.group_id).collect::<Vec<_>>();
        let replacement_audit = compression.replacements.clone();
        let deletion_audit = compression.deletions.clone();
        let deletion_path = compression.deletion_path.clone();
        let final_tail_swap_audit = compression.final_tail_swaps.clone();
        let final_support_edit_audit = compression.final_support_edits.clone();
        let final_support_search_audit = compression.final_support_search.clone();
        let seed_selection_audit = compression.seed_selection_steps.clone();
        let initial_transport = compression.initial_transport.clone();
        let final_transport = compression.final_transport.clone();
        let regularization_path = compression.regularization_path.clone();
        if let Some(metadata) = trace_metadata.as_object_mut() {
            // 保持每个 json! 片段较小。单个过大的对象会耗尽 rustc 默认的宏递归限制。
            let generation_parts = [
                serde_json::json!({
                    "version": "variable_count_reset_support_search_v21",
                    "compression_applied": true,
                    "support_count": compression.output_target_count,
                    "pre_deletion_support_count": 50,
                    "browser_seed_rule": "browser_c_score_main_board_with_account_uniqueness_and_merged_team_cap_5_is_the_reset_top50",
                    "initial_support_rule": "use_actual_browser_top50_without_supplement_reselection",
                    "account_uniqueness_rule": "one_normalized_account_may_appear_in_only_one_selected_combination",
                    "owner_cap_rule": "at_most_5_combinations_per_root_team_name",
                    "rate_coverage_rule": "all_audit_rows_cross_union_of_threshold_eligible_support_columns_and_nonzero_correct_lineage_columns;zero_mass_below_threshold_scout_columns_are_not_required",
                    "replacement_locked_rule": "fixed_c_score_top30_of_browser_initial_top50_without_dynamic_replenishment",
                    "deletion_locked_rule": "fixed_c_score_top30_of_post_replacement_top50",
                    "deletion_rank_recompute_rule": "the_pre_deletion_top30_is_frozen_once_and_is_not_recomputed_after_deletion",
                    "locked_initial_weight_rule": "every_selected_row_starts_from_its_own_complete_big_target_base_weight_without_uniform_locked_scaling",
                    "unlocked_initial_weight_rule": "golden_equals_one_anchor_is_a_soft_cohesion_prior;golden_nonunit_rows_are_free_forward_fit_variables",
                    "flattening_rule": "positive_affine_audit_with_golden_equals_one_soft_cohesion_not_hard_equality",
                    "front_priority_rule": "c_score_is_a_soft_preference_and_low_c_score_rows_are_considered_first_for_replacement",
                    "base_weight_rule": "selected_base_weight_equals_that_rows_complete_big_target_weight",
                    "tail_rule": "every_omitted_big_target_weight_is_transported_to_selected_support",
                    "transport_rule": "golden_equals_one_complete_weights_use_soft_cohesion;golden_nonunit_weights_are_allocated_by_forward_error_fit",
                    "deletion_absorption_rule": "each_stage_returns_to_the_single_predeletion_base;golden_equals_one_rows_use_soft_cohesion_and_nonunit_rows_refit_freely",
                    "deletion_path_selection_rule": "continuous_82pct_error_15pct_structure_3pct_target_count_without_hard_max_diff_threshold",
                    "final_weight_rule": "only_absorption_additions_of_the_fixed_non_deletable_c_score_top30_are_nonincreasing_by_c_score; final_weights_and_the_removable_tail_have_no_hard_order",
                    "output_weight_rule": "exported_weight=lineage_weight*final_target_count/big_target_mass_so_exported_weight_sum_equals_n",
                    "score_comparison_rule": "flattened_z=a*(sum_rate_times_exported_weight_over_n)+b_with_a_positive_is_compared_to_c_score",
                    "raw_direct_equality_rule": "raw_direct_score_vs_c_score_or_big_target_is_audit_only_not_the_primary_objective",
                    "replacement_rule": "low_c_score_first_one_swap_search_with_error_and_low_effective_slot_frontiers_full_weight_refit_and_incumbent_guards_without_text_type_features",
                    "final_support_search_rule": "reset_add_remove_swap_edits_across_40_to_50_with_fixed_deletion_top30",
                }),
                serde_json::json!({
                    "max_replacements": 8,
                    "replacement_count": replacement_audit.len(),
                    "max_deletions": 10,
                    "deletion_count": deletion_audit.len(),
                    "max_final_support_edits": 4,
                    "final_support_edit_count": final_support_edit_audit.len(),
                    "final_tail_swap_count": final_tail_swap_audit.len(),
                    "deletion_lock_count": compression.deletion_lock_count,
                    "score_denominator": compression.score_denominator,
                    "big_target_weight_sum": big_target_weight_sum,
                    "target_weight_sum": target_weight_sum,
                    "lineage_weight_sum": compression.lineage_weights.iter().sum::<f64>(),
                    "support_inherited_base_weight_sum": support_base_weight_sum,
                    "tail_group_count": compression.tail_group_count,
                    "tail_weight_sum": compression.tail_weight_sum,
                    "tail_l1_weight_sum": compression.tail_l1_weight_sum,
                    "transported_addition_sum": compression.fitted_additions.iter().sum::<f64>(),
                    "initial_anchor_weight_sum": compression.initial_anchor_weights.iter().sum::<f64>(),
                    "final_support_seed_anchor_weight_sum": compression.final_seed_anchor_weights.iter().sum::<f64>(),
                    "c_score_aware_seed_added_count": seed_selection_audit.len(),
                    "profile_seed_added_count": seed_selection_audit.len(),
                    "locked_seed_original_weight_sum": compression.locked_seed_original_weight_sum,
                    "locked_seed_target_weight_sum": compression.locked_seed_target_weight_sum,
                    "locked_seed_weight_scale": compression.locked_seed_weight_scale,
                    "supplement_seed_target_weight_sum": big_target_weight_sum - compression.locked_seed_target_weight_sum,
                }),
                serde_json::json!({
                    "initial_support_big_replay_mean_abs_diff": compression.initial_metrics.big_mean_abs_diff,
                    "initial_support_big_replay_max_abs_diff": compression.initial_metrics.big_max_abs_diff,
                    "initial_support_big_replay_rmse": compression.initial_metrics.big_rmse,
                    "final_big_replay_mean_abs_diff": compression.final_metrics.big_mean_abs_diff,
                    "final_big_replay_max_abs_diff": compression.final_metrics.big_max_abs_diff,
                    "final_big_replay_p95_abs_diff": compression.final_metrics.big_p95_abs_diff,
                    "final_big_replay_rmse": compression.final_metrics.big_rmse,
                    "final_correct_replay_mean_abs_diff": compression.final_metrics.correct_mean_abs_diff,
                    "final_correct_replay_max_abs_diff": compression.final_metrics.correct_max_abs_diff,
                    "final_correct_replay_p95_abs_diff": compression.final_metrics.correct_p95_abs_diff,
                    "final_correct_replay_rmse": compression.final_metrics.correct_rmse,
                    "initial_affine_aligned_c_score_mean_abs_diff": compression.initial_metrics.aligned_mean_abs_diff,
                    "initial_affine_aligned_c_score_max_abs_diff": compression.initial_metrics.aligned_max_abs_diff,
                    "initial_affine_aligned_c_score_rmse": compression.initial_metrics.aligned_rmse,
                    "pre_deletion_affine_aligned_c_score_mean_abs_diff": compression.pre_deletion_metrics.aligned_mean_abs_diff,
                    "pre_deletion_affine_aligned_c_score_max_abs_diff": compression.pre_deletion_metrics.aligned_max_abs_diff,
                    "pre_deletion_affine_aligned_c_score_rmse": compression.pre_deletion_metrics.aligned_rmse,
                    "final_affine_aligned_c_score_mean_abs_diff": compression.final_metrics.aligned_mean_abs_diff,
                    "final_affine_aligned_c_score_max_abs_diff": compression.final_metrics.aligned_max_abs_diff,
                    "final_affine_aligned_c_score_p95_abs_diff": compression.final_metrics.aligned_p95_abs_diff,
                    "final_affine_aligned_c_score_rmse": compression.final_metrics.aligned_rmse,
                    "flattened_c_score_raw_mean_abs_diff": compression.final_metrics.flat_raw_mean_abs_diff,
                    "flattened_c_score_raw_max_abs_diff": compression.final_metrics.flat_raw_max_abs_diff,
                    "flattened_c_score_raw_rmse": compression.final_metrics.flat_raw_rmse,
                    "positive_affine_slope": compression.final_metrics.affine_slope,
                    "positive_affine_intercept": compression.final_metrics.affine_intercept,
                    "direct_score_c_score_spearman": compression.final_metrics.score_spearman,
                    "flattened_max_diff_target": compression.final_metrics.max_diff_target,
                    "flattened_max_diff_target_met": compression.final_metrics.max_diff_target_met,
                    "flattened_max_diff_target_margin": compression.final_metrics.max_diff_target_margin,
                    "path_selection_error_cost_normalized": compression.final_metrics.selection_error_cost_normalized,
                    "path_selection_structure_cost_normalized": compression.final_metrics.selection_structure_cost_normalized,
                    "path_selection_score": compression.final_metrics.selection_score,
                    "path_selection_rule": compression.final_metrics.selection_rule.clone(),
                }),
                serde_json::json!({
                    "selected_regularization_factor": compression.final_metrics.regularization_factor,
                    "pure_structural_prior_selected": compression.final_metrics.pure_structural_prior,
                    "structure_score": compression.final_metrics.structure_score,
                    "anchor_distance_l2": compression.final_metrics.anchor_distance_l2,
                    "transported_addition_l2": compression.final_metrics.addition_l2,
                    "transported_addition_max_abs": compression.final_metrics.addition_max_abs,
                    "similar_type_addition_rms": compression.final_metrics.similar_addition_rms,
                    "local_c_score_addition_inversion_rms": compression.final_metrics.local_order_inversion_rms,
                    "local_c_score_addition_inversion_rate": compression.final_metrics.local_order_inversion_rate,
                    "addition_cancellation_ratio": compression.final_metrics.addition_cancellation_ratio,
                    "weight_cancellation_ratio": compression.final_metrics.weight_cancellation_ratio,
                    "final_weight_c_score_spearman": compression.final_metrics.final_weight_c_score_spearman,
                    "final_weight_monotonic_violation_count": compression.final_metrics.final_weight_monotonic_violation_count,
                    "final_weight_monotonic_violation_max": compression.final_metrics.final_weight_monotonic_violation_max,
                    "adjacent_weight_gap_rms": compression.final_metrics.adjacent_weight_gap_rms,
                    "adjacent_weight_gap_max": compression.final_metrics.adjacent_weight_gap_max,
                    "weight_gap_second_difference_rms": compression.final_metrics.weight_gap_second_difference_rms,
                    "tail_weight_monotonic_violation_count": compression.final_metrics.tail_weight_monotonic_violation_count,
                    "tail_weight_monotonic_violation_rms": compression.final_metrics.tail_weight_monotonic_violation_rms,
                    "tail_low_effective_slot_penalty": compression.final_metrics.tail_low_effective_slot_penalty,
                    "compressed_corr_to_big": compression.final_metrics.compressed_corr_to_big,
                    "compressed_corr_to_correct": compression.final_metrics.compressed_corr_to_correct,
                    "golden_unit_equalization_count": compression.final_metrics.golden_unit_equalization_count,
                    "golden_unit_final_spread": compression.final_metrics.golden_unit_final_spread,
                    "golden_unit_projection_l2": compression.final_metrics.golden_unit_projection_l2,
                    "golden_nonunit_free_count": compression.final_metrics.golden_nonunit_free_count,
                }),
                serde_json::json!({
                    "browser_initial_group_ids": browser_initial_group_ids,
                    "initial_group_ids": initial_group_ids,
                    "locked_group_ids": locked_group_ids,
                    "pre_deletion_group_ids": pre_deletion_group_ids,
                    "deletion_locked_group_ids": deletion_locked_group_ids,
                    "initial_anchor_weights": compression.initial_anchor_weights.clone(),
                    "final_seed_anchor_weights": compression.final_seed_anchor_weights.clone(),
                    "seed_selection_steps": seed_selection_audit,
                    "final_group_ids": final_group_ids,
                    "replacements": replacement_audit,
                    "deletions": deletion_audit,
                    "deletion_path": deletion_path,
                    "final_tail_swaps": final_tail_swap_audit,
                    "final_support_edits": final_support_edit_audit,
                    "final_support_search": final_support_search_audit,
                    "initial_transport": initial_transport,
                    "pre_deletion_transport": compression.pre_deletion_transport.clone(),
                    "final_transport": final_transport,
                    "regularization_path": regularization_path,
                }),
            ];
            let mut generation = serde_json::Map::new();
            for part in generation_parts {
                if let serde_json::Value::Object(object) = part {
                    generation.extend(object);
                }
            }
            metadata.insert("generation".to_string(), serde_json::Value::Object(generation));
        }

        self.db.set_lane_progress(
            lane,
            "target_generation_ready",
            1,
            1,
            required_rate_target_indices.len(),
            required_rate_target_indices.len(),
            0,
            &format!("target generation complete: {} exported targets", output_rows.len()),
        )?;
        self.db.set_lane_status(lane, "ready", lane_group_count)?;
        Ok(TargetGenerationResponse {
            summary: TargetGenerationSummary {
                algorithm: "variable_count_reset_support_search_v21".to_string(),
                trace_version: trace.trace_version.clone(),
                score_mode: "variable_denominator_n_positive_affine_c_score_alignment".to_string(),
                lane_size: lane,
                target_count: output_rows.len(),
                score_denominator: compression.score_denominator,
                unique_group_count: output_rows.len(),
                raw_trace_count,
                correct_trace_count: trace.weights.len(),
                correct_reference_scope_count: scope_sums.len(),
                raw_weight_sum,
                correct_nominal_weight_sum,
                correct_target_weight_sum,
                correct_target_candidate_count: correct_target_indices.len(),
                correct_target_nonzero_count: correct_target_indices.len(),
                correct_forward_replay_mean_abs_diff,
                correct_forward_replay_max_abs_diff,
                correct_forward_replay_rmse,
                row_coefficient_replay_mean_abs_diff,
                row_coefficient_replay_max_abs_diff,
                exact_replay_requires_trace_semantics: true,
                merged_weight_sum_before_normalization: big_target_weight_sum,
                merged_normalization_scale: compression.output_target_count as f64 / big_target_weight_sum,
                support_base_weight_sum,
                tail_group_count: compression.tail_group_count,
                tail_weight_sum: compression.tail_weight_sum,
                fitted_addition_sum: compression.fitted_additions.iter().sum::<f64>(),
                fit_baseline_mean_abs_diff: compression.initial_metrics.aligned_mean_abs_diff,
                fit_baseline_max_abs_diff: compression.initial_metrics.aligned_max_abs_diff,
                fit_baseline_rmse: compression.initial_metrics.aligned_rmse,
                fit_optimized_mean_abs_diff: compression.final_metrics.aligned_mean_abs_diff,
                fit_optimized_max_abs_diff: compression.final_metrics.aligned_max_abs_diff,
                fit_optimized_rmse: compression.final_metrics.aligned_rmse,
                fit_weight_regularization_applied: compression.final_metrics.regularization_factor > 0.0,
                fit_baseline_addition_l2_norm: compression.initial_metrics.addition_l2,
                fit_optimized_addition_l2_norm: compression.final_metrics.addition_l2,
                fit_optimized_addition_max_abs: compression.final_metrics.addition_max_abs,
                fit_optimized_distance_from_proportional_l2: compression.final_metrics.anchor_distance_l2,
                fit_final_weight_std_limit: 0.0,
                fit_final_weight_max_deviation_limit: 0.0,
                fit_optimized_final_weight_std: compression.final_metrics.final_weight_std,
                fit_optimized_final_weight_max_deviation: target_weight_max_deviation,
                compressed_replay_mean_abs_diff: compression.final_metrics.aligned_mean_abs_diff,
                compressed_replay_max_abs_diff: compression.final_metrics.aligned_max_abs_diff,
                compressed_replay_rmse: compression.final_metrics.aligned_rmse,
                fixed_main_count: compression.deletion_locked_indices.len(),
                optimized_count: output_rows.len().saturating_sub(compression.deletion_locked_indices.len()),
                player_cap: compression.owner_cap,
                target_weight_sum,
                target_weight_min,
                target_weight_max,
                player_weight_cap: 0.0,
                cqd_threshold,
                reference_limit: reference_indices.len(),
                reference_count: reference_indices.len(),
                candidate_count: candidates.len(),
                objective_mse: compression.final_metrics.aligned_rmse * compression.final_metrics.aligned_rmse,
                objective_corr: compression.final_metrics.score_spearman,
                reference_avg_winrate_mean: 0.0,
                reference_avg_winrate_std: 0.0,
                reference_c_score_mean: 0.0,
                reference_c_score_std: 0.0,
                audit_reference_rows: reference_indices.len(),
                audit_mean_diff: None,
                audit_mean_abs_diff: Some(compression.final_metrics.aligned_mean_abs_diff),
                audit_max_abs_diff: Some(compression.final_metrics.aligned_max_abs_diff),
                audit_rmse: Some(compression.final_metrics.aligned_rmse),
                audit_p95_abs_diff: Some(compression.final_metrics.aligned_p95_abs_diff),
            },
            target_config_text,
            trace_metadata,
            rows: output_rows,
            reference_audit_rows: Vec::<TargetReferenceAuditRow>::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct TargetCandidate {
    pub(super) row: crate::model::LaneResultRow,
    /// 归一化账户键（`name@root_team`），仅用于全局账户不重复规则。每位玩家/所有者上限由
    /// `row.root_team_name` 单独跟踪。
    pub(super) player_keys: Vec<String>,
}

impl TargetCandidate {
    pub(super) fn from_row(row: crate::model::LaneResultRow) -> Self {
        let player_keys = merged_player_keys(&row);
        Self { row, player_keys }
    }

    pub(super) fn correct_score(&self) -> f64 { self.row.pair_score.unwrap_or(f64::NEG_INFINITY) }
}

pub(super) fn target_candidate_is_eligible(candidate: &TargetCandidate, raw_min: f64) -> bool {
    !candidate.row.is_blocked
        && candidate.row.raw_average_cqd >= raw_min
        && candidate.row.selection_status != "below_threshold"
        && candidate.row.selection_status != "blocked"
        && candidate.row.pair_rank.is_some()
        && candidate.correct_score().is_finite()
}

pub(super) fn target_rate_column_requires_observed(
    target_idx: usize,
    big_weight: f64,
    compression_target_indices: &HashSet<usize>,
) -> bool {
    compression_target_indices.contains(&target_idx) || big_weight.abs() > 1e-15
}

pub(super) fn merged_player_keys(row: &crate::model::LaneResultRow) -> Vec<String> {
    let root_team = row.root_team_name.as_str();
    let mut keys = Vec::new();

    for member in row.canonical.split('+') {
        let member = member.trim();
        if member.is_empty() {
            continue;
        }
        if let Some((name, _team)) = parse_member_team(member) {
            keys.push(format!("{name}@{root_team}"));
        } else {
            keys.push(format!("{member}@{root_team}"));
        }
    }

    keys.sort();
    keys.dedup();
    keys
}

pub(super) fn compare_target_candidates(a: &TargetCandidate, b: &TargetCandidate) -> std::cmp::Ordering {
    let ar = a.row.pair_rank.unwrap_or(usize::MAX);
    let br = b.row.pair_rank.unwrap_or(usize::MAX);
    ar.cmp(&br)
        .then_with(|| b.correct_score().total_cmp(&a.correct_score()))
        .then_with(|| a.row.rank.cmp(&b.row.rank))
        .then_with(|| a.row.group_id.cmp(&b.row.group_id))
}

pub(super) fn rate_between(
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    a: crate::model::GroupId,
    b: crate::model::GroupId,
) -> Option<f64> {
    if a == b {
        return Some(50.0);
    }
    if let Some(rate) = rate_map.get(&(a, b)) {
        Some(*rate)
    } else {
        rate_map.get(&(b, a)).map(|rate_b| 100.0 - *rate_b)
    }
}

pub(super) fn build_target_config_text(indices: &[usize], weights: &[f64], candidates: &[TargetCandidate]) -> String {
    let mut lines = Vec::with_capacity(indices.len());
    for (&idx, &weight) in indices.iter().zip(weights.iter()) {
        lines.push(format!("{:.12}\t{}", weight, candidates[idx].row.canonical));
    }
    lines.join("\n")
}

pub(super) fn collect_missing_target_rate_pairs(
    reference_indices: &[usize],
    fill_pool: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for &ref_idx in reference_indices {
        let a = candidates[ref_idx].row.group_id;
        for &target_idx in fill_pool {
            let b = candidates[target_idx].row.group_id;
            if a == b || rate_between(rate_map, a, b).is_some() {
                continue;
            }
            let key = if a < b { (a, b) } else { (b, a) };
            if seen.insert(key) {
                out.push((ref_idx, target_idx));
            }
        }
    }

    out
}

pub(super) fn format_target_rate_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

pub(super) fn fill_missing_target_rates_with_checkpoints(
    db: &Db,
    config: &RankerConfig,
    lane_size: usize,
    missing_pairs: &[(usize, usize)],
    candidates: &[TargetCandidate],
    rate_map: &mut HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> anyhow::Result<()> {
    let total = missing_pairs.len();
    if total == 0 {
        return Ok(());
    }

    let mut group_cache: HashMap<crate::model::GroupId, StoredGroup> = HashMap::new();
    let mut pairs = Vec::with_capacity(total);
    for (pair_index, &(a_idx, b_idx)) in missing_pairs.iter().enumerate() {
        let a_id = candidates[a_idx].row.group_id;
        let b_id = candidates[b_idx].row.group_id;
        let group_a = if let Some(group) = group_cache.get(&a_id) {
            group.clone()
        } else {
            let group = db
                .get_group(a_id)?
                .with_context(|| format!("missing group_id {a_id} while computing target rate coverage"))?;
            group_cache.insert(a_id, group.clone());
            group
        };
        let group_b = if let Some(group) = group_cache.get(&b_id) {
            group.clone()
        } else {
            let group = db
                .get_group(b_id)?
                .with_context(|| format!("missing group_id {b_id} while computing target rate coverage"))?;
            group_cache.insert(b_id, group.clone());
            group
        };
        pairs.push(ManualWinratePair {
            pair_index,
            group_a,
            group_b,
        });
    }
    let workers = resolve_manual_winrate_workers(config.outer_workers, total);
    let mode = if config.outer_workers == 0 {
        "dynamic_queue"
    } else {
        "static_chunks"
    };
    let checkpoint_size = RATE_PERSIST_CHECKPOINT_SIZE;
    let started = Instant::now();
    db.set_lane_progress(
        lane_size,
        "target_rate_coverage",
        0,
        1,
        0,
        total,
        0,
        &format!("computing missing target-matrix rates 0/{total}, 0.00 pair/s, workers={workers}, mode={mode}"),
    )?;

    let pairs = Arc::new(pairs);
    let (sender, receiver) = mpsc::channel::<anyhow::Result<(crate::model::GroupId, crate::model::GroupId, f64)>>();
    let mut handles = Vec::with_capacity(workers);
    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _ in 0..workers {
            let pairs = Arc::clone(&pairs);
            let next_pair = Arc::clone(&next_pair);
            let sender = sender.clone();
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            handles.push(thread::spawn(move || {
                loop {
                    let idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = pairs.get(idx) else {
                        break;
                    };
                    let result = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)
                        .map(|rate| (pair.group_a.id, pair.group_b.id, rate));
                    if sender.send(result).is_err() {
                        break;
                    }
                }
            }));
        }
    } else {
        for worker_id in 0..workers {
            let pairs = Arc::clone(&pairs);
            let sender = sender.clone();
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;
            handles.push(thread::spawn(move || {
                for idx in start..end {
                    let Some(pair) = pairs.get(idx) else {
                        break;
                    };
                    let result = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)
                        .map(|rate| (pair.group_a.id, pair.group_b.id, rate));
                    if sender.send(result).is_err() {
                        break;
                    }
                }
            }));
        }
    }
    drop(sender);

    let mut completed = 0usize;
    let mut persisted = 0usize;
    let mut pending = Vec::with_capacity(checkpoint_size);
    for result in receiver {
        pending.push(result?);
        completed += 1;
        if pending.len() >= checkpoint_size || completed == total {
            pending.sort_by_key(|(a, b, _)| (*a, *b));
            db.save_rate_pairs_bulk(&pending, config.win_rate_samples)?;
            for &(a, b, rate) in &pending {
                rate_map.insert((a, b), rate);
                rate_map.insert((b, a), 100.0 - rate);
            }
            persisted += pending.len();
            pending.clear();
        }
        if completed != total && completed % 10 != 0 {
            continue;
        }
        let elapsed = started.elapsed().as_secs_f64().max(0.001);
        let pairs_per_sec = completed as f64 / elapsed;
        let remaining_seconds = (total - completed) as f64 / pairs_per_sec.max(f64::EPSILON);
        db.set_lane_progress(
            lane_size,
            "target_rate_coverage",
            0,
            1,
            completed,
            total,
            0,
            &format!(
                "computing missing target-matrix rates {completed}/{total}, {pairs_per_sec:.2} pair/s, elapsed {}, eta {}, persisted={persisted}, workers={workers}, mode={mode}",
                format_target_rate_duration(elapsed),
                format_target_rate_duration(remaining_seconds),
            ),
        )?;
    }
    for handle in handles {
        handle.join().expect("target rate worker thread panicked");
    }

    Ok(())
}

pub(super) fn average_reference_winrate_for_target(
    reference_indices: &[usize],
    target_idx: usize,
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> (Option<f64>, usize) {
    let target_gid = candidates[target_idx].row.group_id;
    let mut sum = 0.0;
    let mut count = 0usize;

    for &ref_idx in reference_indices {
        let ref_gid = candidates[ref_idx].row.group_id;
        if let Some(rate) = rate_between(rate_map, ref_gid, target_gid) {
            sum += rate;
            count += 1;
        }
    }

    if count == 0 {
        (None, 0)
    } else {
        (Some(sum / count as f64), count)
    }
}
#[cfg(test)]
mod target_rate_scope_tests {
    use super::target_rate_column_requires_observed;
    use std::collections::HashSet;

    #[test]
    pub(super) fn below_threshold_zero_mass_columns_do_not_require_low_by_low_rates() {
        let eligible = HashSet::from([0usize, 1usize]);

        // 所有评分行仍要求对合资格目标列的观测（high×high 和 low×high 均使用这一列规则）。
        assert!(target_rate_column_requires_observed(0, 0.0, &eligible));
        assert!(target_rate_column_requires_observed(1, 0.0, &eligible));

        // 没有 Correct 质量的低于阈值列无法被选中，因而不得凭空制造 low×low 模拟需求。
        assert!(!target_rate_column_requires_observed(2, 0.0, &eligible));

        // 防御性不变式：绝不以占位率静默替换意外的非零大目标列。
        assert!(target_rate_column_requires_observed(2, 0.25, &eligible));
    }
}

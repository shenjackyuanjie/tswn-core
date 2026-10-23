//! lane 进度、排名结果和 Correct 靶子轨迹存储。
//!
//! 结果集合与 Correct 追溯各自以事务整体替换；清空结果会同时清空追溯，
//! 单独保存结果则由调用方决定何时使旧追溯失效。追溯写入前校验身份、有限权重与总质量。
//! 查询结果会重新计算文字技能摘要和合并队伍；状态和进度查询不承诺同一计算快照。

use super::*;

impl Db {
    pub fn set_lane_status(&self, lane_size: usize, status: &str, group_count: usize) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO lane_status (lane_size, status, group_count, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
            params![lane_size as i64, status, group_count as i64],
        )?;
        Ok(())
    }

    pub fn set_lane_progress(
        &self,
        lane_size: usize,
        phase: &str,
        round: usize,
        total_rounds: usize,
        rate_done: usize,
        rate_total: usize,
        kicked_count: usize,
        message: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO lane_progress
             (lane_size, phase, round, total_rounds, rate_done, rate_total, kicked_count, message, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)",
            params![
                lane_size as i64,
                phase,
                round as i64,
                total_rounds as i64,
                rate_done as i64,
                rate_total as i64,
                kicked_count as i64,
                message,
            ],
        )?;
        Ok(())
    }

    pub fn lane_progress(&self, lane_size: usize) -> anyhow::Result<Option<LaneProgress>> {
        let conn = self.conn.lock().unwrap();
        let progress = conn
            .query_row(
                "SELECT lane_size, phase, round, total_rounds, rate_done, rate_total, kicked_count, message
                 FROM lane_progress WHERE lane_size = ?1",
                params![lane_size as i64],
                |row| {
                    Ok(LaneProgress {
                        lane_size: row.get::<_, i64>(0)? as usize,
                        phase: row.get(1)?,
                        round: row.get::<_, i64>(2)? as usize,
                        total_rounds: row.get::<_, i64>(3)? as usize,
                        rate_done: row.get::<_, i64>(4)? as usize,
                        rate_total: row.get::<_, i64>(5)? as usize,
                        kicked_count: row.get::<_, i64>(6)? as usize,
                        message: row.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(progress)
    }

    pub fn clear_lane_results(&self, lane_size: usize) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM correct_target_trace_weights WHERE lane_size = ?1",
            params![lane_size as i64],
        )?;
        tx.execute(
            "DELETE FROM correct_target_trace_meta WHERE lane_size = ?1",
            params![lane_size as i64],
        )?;
        tx.execute("DELETE FROM lane_results WHERE lane_size = ?1", params![lane_size as i64])?;
        tx.commit()?;
        Ok(())
    }

    pub fn clear_correct_target_trace(&self, lane_size: usize) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM correct_target_trace_weights WHERE lane_size = ?1",
            params![lane_size as i64],
        )?;
        tx.execute(
            "DELETE FROM correct_target_trace_meta WHERE lane_size = ?1",
            params![lane_size as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn replace_correct_target_trace(&self, lane_size: usize, trace: &CorrectTargetTrace) -> anyhow::Result<()> {
        if trace.trace_version.trim().is_empty() || trace.score_mode.trim().is_empty() {
            anyhow::bail!("correct target trace version and score mode must be non-empty");
        }
        let _: serde_json::Value =
            serde_json::from_str(&trace.metadata_json).context("correct target trace metadata_json must be valid JSON")?;
        if trace.weights.is_empty() {
            anyhow::bail!("correct target trace must contain at least one reference weight");
        }
        let mut normalized_scope_sums: HashMap<&str, f64> = HashMap::new();
        for row in &trace.weights {
            if row.reference_scope.trim().is_empty() || row.source.trim().is_empty() {
                anyhow::bail!("correct target trace scope and source must be non-empty");
            }
            if !row.reference_weight.is_finite()
                || row.reference_weight <= 0.0
                || !row.nominal_weight.is_finite()
                || row.nominal_weight.abs() <= 1e-15
                || !row.raw_golden_weight.is_finite()
                || row.raw_golden_weight < 0.0
                || !row.common_coefficient.is_finite()
                || row.common_coefficient.abs() <= 1e-15
                || !row.coefficient_mean.is_finite()
                || !row.coefficient_stddev.is_finite()
                || row.coefficient_stddev < 0.0
                || !row.correct_target_weight.is_finite()
                || row.correct_target_weight.abs() <= 1e-15
                || (row.nominal_weight - row.correct_target_weight).abs() > 1e-10
                || (50.0 * row.common_coefficient - row.correct_target_weight).abs() > 1e-8
            {
                anyhow::bail!(
                    "correct target trace weights must be finite and non-zero for group_id={}",
                    row.group_id
                );
            }
            *normalized_scope_sums.entry(row.reference_scope.as_str()).or_insert(0.0) += row.reference_weight;
        }
        for (scope, sum) in normalized_scope_sums {
            if (sum - 1.0).abs() > 1e-8 {
                anyhow::bail!(
                    "correct target trace normalized weights for scope={} must sum to 1, got {:.12}",
                    scope,
                    sum
                );
            }
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM correct_target_trace_weights WHERE lane_size = ?1",
            params![lane_size as i64],
        )?;
        tx.execute(
            "DELETE FROM correct_target_trace_meta WHERE lane_size = ?1",
            params![lane_size as i64],
        )?;
        tx.execute(
            "INSERT INTO correct_target_trace_meta
             (lane_size, trace_version, score_mode, metadata_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
            params![lane_size as i64, trace.trace_version, trace.score_mode, trace.metadata_json,],
        )?;
        for row in &trace.weights {
            tx.execute(
                "INSERT INTO correct_target_trace_weights
                 (lane_size, reference_scope, group_id, reference_weight, nominal_weight,
                  raw_golden_weight, calibration_log_multiplier, calibration_multiplier,
                  common_coefficient, coefficient_mean, coefficient_stddev,
                  correct_target_weight, source, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 1, ?7, ?8, ?9, ?10, ?11, CURRENT_TIMESTAMP)",
                params![
                    lane_size as i64,
                    row.reference_scope,
                    row.group_id,
                    row.reference_weight,
                    row.nominal_weight,
                    row.raw_golden_weight,
                    row.common_coefficient,
                    row.coefficient_mean,
                    row.coefficient_stddev,
                    row.correct_target_weight,
                    row.source,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn correct_target_trace(&self, lane_size: usize) -> anyhow::Result<Option<CorrectTargetTrace>> {
        let conn = self.conn.lock().unwrap();
        let meta = conn
            .query_row(
                "SELECT trace_version, score_mode, metadata_json
                 FROM correct_target_trace_meta WHERE lane_size = ?1",
                params![lane_size as i64],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
            )
            .optional()?;
        let Some((trace_version, score_mode, metadata_json)) = meta else {
            return Ok(None);
        };

        let mut stmt = conn.prepare(
            "SELECT reference_scope, group_id, reference_weight, nominal_weight,
                    raw_golden_weight, common_coefficient, coefficient_mean,
                    coefficient_stddev, correct_target_weight, source
             FROM correct_target_trace_weights
             WHERE lane_size = ?1
             ORDER BY reference_scope ASC, group_id ASC",
        )?;
        let rows = stmt.query_map(params![lane_size as i64], |row| {
            Ok(CorrectTargetTraceWeight {
                reference_scope: row.get(0)?,
                group_id: row.get(1)?,
                reference_weight: row.get(2)?,
                nominal_weight: row.get(3)?,
                raw_golden_weight: row.get(4)?,
                common_coefficient: row.get(5)?,
                coefficient_mean: row.get(6)?,
                coefficient_stddev: row.get(7)?,
                correct_target_weight: row.get(8)?,
                source: row.get(9)?,
            })
        })?;
        let mut weights = Vec::new();
        for row in rows {
            weights.push(row?);
        }
        Ok(Some(CorrectTargetTrace {
            trace_version,
            score_mode,
            metadata_json,
            weights,
        }))
    }

    pub fn save_lane_results(&self, lane_size: usize, rows: &[LaneResultRow]) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM lane_results WHERE lane_size = ?1", params![lane_size as i64])?;
        for row in rows {
            tx.execute(
                "INSERT INTO lane_results
                 (lane_size, group_id, rank, average_cqd, raw_average_cqd, pair_score, pair_rank, uncertainty, raw_delta, marginal_value, constrained_rank, selection_status, winrate_type_label, winrate_profile_distance, winrate_profile_second_distance, winrate_profile_margin, winrate_profile_soft_confidence, winrate_profile_soft_entropy, winrate_profile_soft_confidence_calibrated, winrate_profile_soft_entropy_calibrated, winrate_profile_bootstrap_stability, winrate_profile_fixed_center_stability, winrate_profile_recluster_stability, winrate_profile_recluster_jaccard, winrate_profile_assignment_entropy, winrate_profile_recluster_ari, winrate_profile_embedding_x, winrate_profile_embedding_y, residual_type_label, residual_profile_distance, residual_profile_second_distance, residual_profile_margin, residual_profile_soft_confidence_calibrated, residual_profile_soft_entropy_calibrated, residual_profile_recluster_stability, residual_profile_assignment_entropy, residual_profile_embedding_x, residual_profile_embedding_y, residual_profile_shape_rms, residual_profile_variance_feature, min_cqd, max_cqd, variance_cqd, golden_rate, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39, ?40, ?41, ?42, ?43, ?44, CURRENT_TIMESTAMP)",
                params![
                    lane_size as i64,
                    row.group_id,
                    row.rank as i64,
                    row.average_cqd,
                    row.raw_average_cqd,
                    row.pair_score,
                    row.pair_rank.map(|x| x as i64),
                    row.uncertainty,
                    row.raw_delta,
                    row.marginal_value,
                    row.constrained_rank.map(|x| x as i64),
                    row.selection_status,
                    row.winrate_type_label.clone(),
                    row.winrate_profile_distance,
                    row.winrate_profile_second_distance,
                    row.winrate_profile_margin,
                    row.winrate_profile_soft_confidence,
                    row.winrate_profile_soft_entropy,
                    row.winrate_profile_soft_confidence_calibrated,
                    row.winrate_profile_soft_entropy_calibrated,
                    row.winrate_profile_bootstrap_stability,
                    row.winrate_profile_fixed_center_stability,
                    row.winrate_profile_recluster_stability,
                    row.winrate_profile_recluster_jaccard,
                    row.winrate_profile_assignment_entropy,
                    row.winrate_profile_recluster_ari,
                    row.winrate_profile_embedding_x,
                    row.winrate_profile_embedding_y,
                    row.residual_type_label.clone(),
                    row.residual_profile_distance,
                    row.residual_profile_second_distance,
                    row.residual_profile_margin,
                    row.residual_profile_soft_confidence_calibrated,
                    row.residual_profile_soft_entropy_calibrated,
                    row.residual_profile_recluster_stability,
                    row.residual_profile_assignment_entropy,
                    row.residual_profile_embedding_x,
                    row.residual_profile_embedding_y,
                    row.residual_profile_shape_rms,
                    row.residual_profile_variance_feature,
                    row.min_cqd,
                    row.max_cqd,
                    row.variance_cqd,
                    row.golden_rate,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn lane_statuses(&self) -> anyhow::Result<Vec<LaneStatus>> {
        let tuples = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT lane_size, status, group_count FROM lane_status ORDER BY lane_size ASC")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as usize,
                ))
            })?;

            let mut tuples = Vec::new();
            for row in rows {
                tuples.push(row?);
            }
            tuples
        };

        let mut out = Vec::with_capacity(tuples.len());
        for (lane_size, status, group_count) in tuples {
            let progress = self.lane_progress(lane_size)?;
            out.push(LaneStatus {
                lane_size,
                status,
                group_count,
                progress,
            });
        }
        Ok(out)
    }

    pub fn lane_top_score_group_ids(
        &self,
        lane_size: usize,
        limit: usize,
        min_average_cqd: Option<f64>,
    ) -> anyhow::Result<Vec<GroupId>> {
        let conn = self.conn.lock().unwrap();
        let mut ids = Vec::new();

        if let Some(min_average_cqd) = min_average_cqd {
            let mut stmt = conn.prepare(
                "SELECT r.group_id
             FROM lane_results r
             JOIN groups g ON g.id = r.group_id
             WHERE r.lane_size = ?1
               AND r.average_cqd >= ?2
             ORDER BY r.average_cqd DESC, r.rank ASC
             LIMIT ?3",
            )?;

            let rows = stmt.query_map(params![lane_size as i64, min_average_cqd, limit as i64], |row| row.get(0))?;

            for row in rows {
                ids.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT r.group_id
             FROM lane_results r
             JOIN groups g ON g.id = r.group_id
             WHERE r.lane_size = ?1
             ORDER BY r.average_cqd DESC, r.rank ASC
             LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![lane_size as i64, limit as i64], |row| row.get(0))?;

            for row in rows {
                ids.push(row?);
            }
        }

        Ok(ids)
    }

    pub fn lane_rate_map(&self, lane_size: usize) -> anyhow::Result<HashMap<(GroupId, GroupId), f64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT gr.group_a, gr.group_b, gr.win_rate_a
             FROM group_rates gr
             JOIN groups ga ON ga.id = gr.group_a
             JOIN groups gb ON gb.id = gr.group_b
             WHERE ga.lane_size = ?1 AND gb.lane_size = ?1",
        )?;
        let rows = stmt.query_map(params![lane_size as i64], |row| {
            Ok((row.get::<_, GroupId>(0)?, row.get::<_, GroupId>(1)?, row.get::<_, f64>(2)?))
        })?;

        let mut out = HashMap::new();
        for row in rows {
            let (a, b, rate_a) = row?;
            out.insert((a, b), rate_a);
        }
        Ok(out)
    }

    pub fn lane_results(&self, lane_size: usize) -> anyhow::Result<Vec<LaneResultRow>> {
        let conn = self.conn.lock().unwrap();

        let mut team_stmt = conn.prepare("SELECT name, parent FROM teams")?;
        let team_rows = team_stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        let mut team_pairs = Vec::new();
        for row in team_rows {
            team_pairs.push(row?);
        }
        drop(team_stmt);
        let dsu = TeamDsu::from_pairs(team_pairs);

        let mut stmt = conn.prepare(
            "SELECT r.lane_size, r.group_id, r.rank, g.canonical, g.team_name,
                    r.average_cqd, COALESCE(r.raw_average_cqd, r.average_cqd) AS raw_average_cqd,
                    r.pair_score, r.pair_rank, r.uncertainty, r.raw_delta, r.marginal_value,
                    r.constrained_rank, COALESCE(r.selection_status, 'calibration_skipped') AS selection_status,
                    r.winrate_type_label, r.winrate_profile_distance, r.winrate_profile_second_distance, r.winrate_profile_margin,
                    r.winrate_profile_soft_confidence, r.winrate_profile_soft_entropy,
                    r.winrate_profile_soft_confidence_calibrated, r.winrate_profile_soft_entropy_calibrated,
                    r.winrate_profile_bootstrap_stability, r.winrate_profile_fixed_center_stability,
                    r.winrate_profile_recluster_stability, r.winrate_profile_recluster_jaccard,
                    r.winrate_profile_assignment_entropy, r.winrate_profile_recluster_ari,
                    r.winrate_profile_embedding_x, r.winrate_profile_embedding_y,
                    r.residual_type_label, r.residual_profile_distance, r.residual_profile_second_distance,
                    r.residual_profile_margin, r.residual_profile_soft_confidence_calibrated,
                    r.residual_profile_soft_entropy_calibrated, r.residual_profile_recluster_stability,
                    r.residual_profile_assignment_entropy, r.residual_profile_embedding_x, r.residual_profile_embedding_y,
                    r.residual_profile_shape_rms, r.residual_profile_variance_feature,
                    r.min_cqd, r.max_cqd, r.variance_cqd, r.golden_rate,
                    CASE WHEN b.group_id IS NULL THEN 0 ELSE 1 END AS is_blocked
             FROM lane_results r
             JOIN groups g ON g.id = r.group_id
             LEFT JOIN blocked_groups b ON b.group_id = r.group_id
             WHERE r.lane_size = ?1
             ORDER BY r.rank ASC",
        )?;

        let rows = stmt.query_map(params![lane_size as i64], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, GroupId>(1)?,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<i64>>(8)?.map(|x| x as usize),
                row.get::<_, Option<f64>>(9)?,
                row.get::<_, Option<f64>>(10)?,
                row.get::<_, Option<f64>>(11)?,
                row.get::<_, Option<i64>>(12)?.map(|x| x as usize),
                row.get::<_, String>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<f64>>(15)?,
                row.get::<_, Option<f64>>(16)?,
                row.get::<_, Option<f64>>(17)?,
                row.get::<_, Option<f64>>(18)?,
                row.get::<_, Option<f64>>(19)?,
                row.get::<_, Option<f64>>(20)?,
                row.get::<_, Option<f64>>(21)?,
                row.get::<_, Option<f64>>(22)?,
                row.get::<_, Option<f64>>(23)?,
                row.get::<_, Option<f64>>(24)?,
                row.get::<_, Option<f64>>(25)?,
                row.get::<_, Option<f64>>(26)?,
                row.get::<_, Option<f64>>(27)?,
                row.get::<_, Option<f64>>(28)?,
                row.get::<_, Option<f64>>(29)?,
                row.get::<_, Option<String>>(30)?,
                row.get::<_, Option<f64>>(31)?,
                row.get::<_, Option<f64>>(32)?,
                row.get::<_, Option<f64>>(33)?,
                row.get::<_, Option<f64>>(34)?,
                row.get::<_, Option<f64>>(35)?,
                row.get::<_, Option<f64>>(36)?,
                row.get::<_, Option<f64>>(37)?,
                row.get::<_, Option<f64>>(38)?,
                row.get::<_, Option<f64>>(39)?,
                row.get::<_, Option<f64>>(40)?,
                row.get::<_, Option<f64>>(41)?,
                row.get::<_, f64>(42)?,
                row.get::<_, f64>(43)?,
                row.get::<_, f64>(44)?,
                row.get::<_, f64>(45)?,
                row.get::<_, i64>(46)? != 0,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (
                lane_size,
                group_id,
                rank,
                canonical,
                team_name,
                average_cqd,
                raw_average_cqd,
                pair_score,
                pair_rank,
                uncertainty,
                raw_delta,
                marginal_value,
                constrained_rank,
                selection_status,
                winrate_type_label,
                winrate_profile_distance,
                winrate_profile_second_distance,
                winrate_profile_margin,
                winrate_profile_soft_confidence,
                winrate_profile_soft_entropy,
                winrate_profile_soft_confidence_calibrated,
                winrate_profile_soft_entropy_calibrated,
                winrate_profile_bootstrap_stability,
                winrate_profile_fixed_center_stability,
                winrate_profile_recluster_stability,
                winrate_profile_recluster_jaccard,
                winrate_profile_assignment_entropy,
                winrate_profile_recluster_ari,
                winrate_profile_embedding_x,
                winrate_profile_embedding_y,
                residual_type_label,
                residual_profile_distance,
                residual_profile_second_distance,
                residual_profile_margin,
                residual_profile_soft_confidence_calibrated,
                residual_profile_soft_entropy_calibrated,
                residual_profile_recluster_stability,
                residual_profile_assignment_entropy,
                residual_profile_embedding_x,
                residual_profile_embedding_y,
                residual_profile_shape_rms,
                residual_profile_variance_feature,
                min_cqd,
                max_cqd,
                variance_cqd,
                golden_rate,
                is_blocked,
            ) = row?;
            let root_team_name = dsu.find_readonly(&team_name);
            let skill_summary = crate::skill_eq::compute_group_skill_summary_from_canonical(&canonical);

            out.push(LaneResultRow {
                lane_size,
                group_id,
                rank,
                canonical: skill_summary.display_canonical,
                team_name,
                root_team_name,
                average_cqd,
                raw_average_cqd,
                pair_score,
                pair_rank,
                uncertainty,
                raw_delta,
                marginal_value,
                constrained_rank,
                selection_status,
                type_label: skill_summary.type_label,
                simple_type_label: skill_summary.simple_type_label,
                winrate_type_label,
                winrate_profile_distance,
                winrate_profile_second_distance,
                winrate_profile_margin,
                winrate_profile_soft_confidence,
                winrate_profile_soft_entropy,
                winrate_profile_soft_confidence_calibrated,
                winrate_profile_soft_entropy_calibrated,
                winrate_profile_bootstrap_stability,
                winrate_profile_fixed_center_stability,
                winrate_profile_recluster_stability,
                winrate_profile_recluster_jaccard,
                winrate_profile_assignment_entropy,
                winrate_profile_recluster_ari,
                winrate_profile_embedding_x,
                winrate_profile_embedding_y,
                residual_type_label,
                residual_profile_distance,
                residual_profile_second_distance,
                residual_profile_margin,
                residual_profile_soft_confidence_calibrated,
                residual_profile_soft_entropy_calibrated,
                residual_profile_recluster_stability,
                residual_profile_assignment_entropy,
                residual_profile_embedding_x,
                residual_profile_embedding_y,
                residual_profile_shape_rms,
                residual_profile_variance_feature,
                skill_totals: skill_summary.skill_totals,
                min_cqd,
                max_cqd,
                variance_cqd,
                golden_rate,
                is_blocked,
            });
        }
        Ok(out)
    }
}

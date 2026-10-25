//! Rust 快速 Correct 校准、候选分组与边加载。
//!
//! 参考池按 Raw 阈值过滤屏蔽/score-only 行，再按分数选择成员不重叠的组合。
//! 每行 Correct=(1-K/50)*Raw+(K/50)*参考池平均胜率；单人 K=10，多人 K=5。
//! 参考池为空、必需胜率缺失或 Golden 质量不为 50 时返回错误，不生成伪造的闭式追溯。

use super::*;

pub(super) fn run_fast_correct(
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
        skipped_reason: Some(format!("fast_closed_form_k{}", format_k_label(replacement_k))),
        target_trace: trace,
    })
}

#[derive(Debug, Clone)]
pub(super) struct CalibGroup {
    pub(super) group_id: GroupId,
    pub(super) raw_score: f64,
    pub(super) is_blocked: bool,
    pub(super) train_eligible: bool,
}

#[derive(Debug, Clone)]
pub(super) struct Edge {
    pub(super) ia: usize,
    pub(super) ib: usize,
}

pub(super) fn build_groups(rows: &[LaneResultRow], threshold: f64) -> Vec<CalibGroup> {
    rows.iter()
        .map(|row| {
            let raw_score = if row.raw_average_cqd.is_finite() {
                row.raw_average_cqd
            } else {
                row.average_cqd
            };
            CalibGroup {
                group_id: row.group_id,
                raw_score,
                is_blocked: row.is_blocked,
                train_eligible: !row.is_blocked && raw_score >= threshold,
            }
        })
        .collect()
}

pub(super) fn load_edges(db: &Db, groups: &[CalibGroup]) -> anyhow::Result<Vec<Edge>> {
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

pub(super) fn round_to_6(x: f64) -> f64 { (x * 1_000_000.0).round() / 1_000_000.0 }

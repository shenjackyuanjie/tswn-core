//! Raw/Golden 核心迭代、标准集合选择和分数重建。
//!
//! 标准集合限制成员不重复、同合并战队最多 5 组，并排除手动屏蔽项。
//! Raw 与 Golden 使用相同热身后窗口，最终按 Golden 权重重放全部分数并核对恒等式。
//! 稳定判断只观察三位小数；进入 core-only 后冻结非零 Golden 支持集，不承诺全局最优。

use super::*;

#[derive(Debug)]
pub(super) struct CoreAlgorithmResult {
    pub(super) rows: Vec<LaneResultRow>,
    pub(super) final_round: usize,
    pub(super) early_stopped: bool,
    pub(super) archive_candidates: Vec<ArchiveCandidate>,
}
impl CoreAlgorithmResult {
    pub(super) fn empty(final_round: usize) -> Self {
        Self {
            rows: Vec::new(),
            final_round,
            early_stopped: false,
            archive_candidates: Vec::new(),
        }
    }
}

pub(super) fn run_core_algorithm(
    db: &Db,
    lane_size: usize,
    groups: Vec<StoredGroup>,
    mut dsu: TeamDsu,
    config: &RankerConfig,
) -> anyhow::Result<CoreAlgorithmResult> {
    let mut nodes: Vec<RankNode> = groups.into_iter().map(RankNode::new).collect();

    // 初始靶子优先使用数据库中上一轮结果的 Score 前 50。
    let (mut standard, smooth_from_start) = make_initial_standard_indices(db, lane_size, &nodes, &mut dsu)?;
    if standard.len() < STANDARD_SIZE {
        return Ok(CoreAlgorithmResult::empty(0));
    }

    let initial_target_message = if smooth_from_start {
        format!("initial target selected from database Score order with standard constraints; smoothing from round 1")
    } else {
        format!("initial target selected by current make logic; warmup remains unsmoothed")
    };

    db.set_lane_progress(
        lane_size,
        "initial_target",
        0,
        config.total_rounds,
        standard.len(),
        STANDARD_SIZE,
        0,
        &initial_target_message,
    )?;

    // v13 关键优化：
    // 这里不再全量预计算整个赛道的两两胜率。
    // RateMatrix 只懒加载当前 standard 需要的列；核心热循环仍然只读内存。
    let mut rate_matrix = RateMatrix::new(nodes.len());

    let core_started = Instant::now();
    let stickiness = config.effective_stickiness(lane_size);
    let mut last_rounded_avg_cqd = vec![i64::MIN; nodes.len()];
    let mut stable_avg_rounds = 0usize;
    let mut final_round = config.total_rounds;
    let mut early_stopped = false;
    let mut core_indices: Option<Vec<usize>> = None;
    let mut selected_flags = vec![false; nodes.len()];

    for round in 1..=config.total_rounds {
        rate_matrix.ensure_rates_for_standard(db, lane_size, round, &nodes, &standard, config)?;

        if let Some(core) = core_indices.as_deref() {
            fight_in_memory_for_indices(&mut nodes, core, &standard, &rate_matrix);
        } else {
            fight_in_memory(&mut nodes, &standard, &rate_matrix);
        }

        if let Some(core) = core_indices.as_deref() {
            data_for_indices(&mut nodes, core, round, config.warmup_rounds, stickiness, smooth_from_start);
        } else {
            data(&mut nodes, round, config.warmup_rounds, stickiness, smooth_from_start);
        }

        // Golden 必须记录本轮实际用于 fight 的同一组 standard，并与 Raw
        // 平均使用完全相同的统计窗口。这样对未四舍五入的 Raw 有严格恒等式：
        // Raw(i) = sum_g rate(i,g) * Golden(g) / STANDARD_SIZE。
        if round >= config.warmup_rounds {
            record_golden_round(&mut nodes, core_indices.as_deref(), &standard, &mut selected_flags);
        }

        if core_indices.is_none() && round >= RAW_CORE_ONLY_START_ROUND && round >= config.warmup_rounds {
            let frozen_core = nodes
                .iter()
                .enumerate()
                .filter_map(|(idx, node)| (node.bz > 0).then_some(idx))
                .collect::<Vec<_>>();
            if frozen_core.len() < STANDARD_SIZE {
                anyhow::bail!(
                    "cannot enter Raw core-only mode: only {} groups have non-zero Golden after round {}",
                    frozen_core.len(),
                    round
                );
            }
            core_indices = Some(frozen_core);
            stable_avg_rounds = 0;
            last_rounded_avg_cqd.fill(i64::MIN);
        }

        standard = if let Some(core) = core_indices.as_deref() {
            make_standard_indices_from_candidates(&nodes, core, &mut dsu)
        } else {
            make_standard_indices(&nodes, &mut dsu)
        };
        if standard.len() < STANDARD_SIZE {
            return Ok(CoreAlgorithmResult::empty(round));
        }

        if round >= EARLY_STOP_START_ROUND {
            let unchanged = if let Some(core) = core_indices.as_deref() {
                avg_cqd_rounded_unchanged_for_indices(&nodes, core, &mut last_rounded_avg_cqd)
            } else {
                avg_cqd_rounded_unchanged(&nodes, &mut last_rounded_avg_cqd)
            };
            if unchanged {
                stable_avg_rounds += 1;
            } else {
                stable_avg_rounds = 0;
            }

            if stable_avg_rounds >= EARLY_STOP_STABLE_ROUNDS {
                final_round = round;
                early_stopped = true;

                db.set_lane_progress(
                    lane_size,
                    "early_stop",
                    round,
                    config.total_rounds,
                    rate_matrix.loaded_pair_count(),
                    0,
                    0,
                    &format!(
                        "early stop at round {round}: Raw core avg cqd unchanged for {EARLY_STOP_STABLE_ROUNDS} consecutive rounds at 3 decimals, core_groups={}, loaded_pairs={}, stickiness={stickiness}, mode=golden_core_then_exact_replay",
                        core_indices.as_ref().map_or(nodes.len(), Vec::len),
                        rate_matrix.loaded_pair_count(),
                    ),
                )?;
                break;
            }
        }

        if should_report_round(round, config.total_rounds) {
            let elapsed = core_started.elapsed().as_secs_f64().max(0.001);
            let rounds_per_sec = round as f64 / elapsed;
            let eta_sec = if rounds_per_sec > 0.0 {
                (config.total_rounds.saturating_sub(round)) as f64 / rounds_per_sec
            } else {
                0.0
            };

            db.set_lane_progress(
                lane_size,
                "iterating",
                round,
                config.total_rounds,
                rate_matrix.loaded_pair_count(),
                0,
                0,
                &format!(
                    "core algorithm round {}/{}, {:.2} rounds/s, elapsed {}, eta {}, active_groups={}, loaded_pairs={}, stickiness={}, mode={}",
                    round,
                    config.total_rounds,
                    rounds_per_sec,
                    format_duration(elapsed),
                    format_duration(eta_sec),
                    core_indices.as_ref().map_or(nodes.len(), Vec::len),
                    rate_matrix.loaded_pair_count(),
                    stickiness,
                    if core_indices.is_some() { "golden_core_only" } else { "full_iteration" },
                ),
            )?;
        }
    }

    let raw_scores = replay_raw_scores_from_golden(&nodes, &rate_matrix)?;
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| raw_scores[b].total_cmp(&raw_scores[a]));

    validate_raw_golden_identity(&nodes, &raw_scores, &rate_matrix)?;

    let rows: Vec<LaneResultRow> = order
        .into_iter()
        .enumerate()
        .map(|(rank_idx, node_idx)| {
            let node = &nodes[node_idx];
            let skill_summary = crate::skill_eq::compute_group_skill_summary(&node.group.members);
            let raw_average_cqd = round_to_3(raw_scores[node_idx]);
            LaneResultRow {
                lane_size,
                group_id: node.group.id,
                rank: rank_idx + 1,
                canonical: skill_summary.display_canonical,
                team_name: node.group.team_name.clone(),
                root_team_name: dsu.find_readonly(&node.group.team_name),
                average_cqd: raw_average_cqd,
                raw_average_cqd,
                pair_score: None,
                pair_rank: None,
                uncertainty: None,
                raw_delta: None,
                marginal_value: None,
                constrained_rank: None,
                selection_status: if node.group.is_blocked {
                    "blocked".to_string()
                } else {
                    "calibration_skipped".to_string()
                },
                type_label: skill_summary.type_label,
                simple_type_label: skill_summary.simple_type_label,
                winrate_type_label: None,
                winrate_profile_distance: None,
                winrate_profile_second_distance: None,
                winrate_profile_margin: None,
                winrate_profile_soft_confidence: None,
                winrate_profile_soft_entropy: None,
                winrate_profile_soft_confidence_calibrated: None,
                winrate_profile_soft_entropy_calibrated: None,
                winrate_profile_bootstrap_stability: None,
                winrate_profile_fixed_center_stability: None,
                winrate_profile_recluster_stability: None,
                winrate_profile_recluster_jaccard: None,
                winrate_profile_assignment_entropy: None,
                winrate_profile_recluster_ari: None,
                winrate_profile_embedding_x: None,
                winrate_profile_embedding_y: None,
                residual_type_label: None,
                residual_profile_distance: None,
                residual_profile_second_distance: None,
                residual_profile_margin: None,
                residual_profile_soft_confidence_calibrated: None,
                residual_profile_soft_entropy_calibrated: None,
                residual_profile_recluster_stability: None,
                residual_profile_assignment_entropy: None,
                residual_profile_embedding_x: None,
                residual_profile_embedding_y: None,
                residual_profile_shape_rms: None,
                residual_profile_variance_feature: None,
                skill_totals: skill_summary.skill_totals,
                min_cqd: if node.cqdmin.is_finite() { node.cqdmin } else { 0.0 },
                max_cqd: if node.cqdmax.is_finite() { node.cqdmax } else { 0.0 },
                variance_cqd: node.variance_cqd(),
                golden_rate: node.golden_rate(),
                is_blocked: node.group.is_blocked,
            }
        })
        .collect::<Vec<LaneResultRow>>();

    let archive_candidates = find_archive_candidates(&rows, &nodes);

    Ok(CoreAlgorithmResult {
        rows,
        final_round,
        early_stopped,
        archive_candidates,
    })
}

pub(super) fn replay_raw_scores_from_golden(nodes: &[RankNode], rate_matrix: &RateMatrix) -> anyhow::Result<Vec<f64>> {
    let golden_support = nodes
        .iter()
        .enumerate()
        .filter_map(|(idx, node)| {
            let weight = node.golden_rate();
            (weight > 0.0).then_some((idx, weight))
        })
        .collect::<Vec<_>>();

    if golden_support.is_empty() {
        return Ok(nodes.iter().map(RankNode::avg_cqd).collect());
    }

    let mut scores = Vec::with_capacity(nodes.len());
    for (idx, node) in nodes.iter().enumerate() {
        let mut weighted_rate = 0.0;
        for &(target_idx, weight) in &golden_support {
            let rate = rate_matrix.rates[idx][target_idx].ok_or_else(|| {
                anyhow::anyhow!(
                    "missing rate needed for Raw Golden replay: group_id={} target_group_id={}",
                    node.group.id,
                    nodes[target_idx].group.id
                )
            })?;
            weighted_rate += rate * weight;
        }
        scores.push(weighted_rate / STANDARD_SIZE as f64);
    }

    Ok(scores)
}

pub(super) fn validate_raw_golden_identity(
    nodes: &[RankNode],
    raw_scores: &[f64],
    rate_matrix: &RateMatrix,
) -> anyhow::Result<()> {
    let Some(sample_count) = nodes.iter().find(|node| node.bz > 0).map(|node| node.odds_n) else {
        return Ok(());
    };
    if sample_count == 0 {
        return Ok(());
    }
    if nodes
        .iter()
        .any(|node| node.bz > 0 && (node.odds_n != sample_count || node.n != sample_count))
    {
        anyhow::bail!("Raw/Golden 核心统计窗口不一致，无法建立精确靶权重");
    }
    let golden_sum = nodes.iter().map(RankNode::golden_rate).sum::<f64>();
    if (golden_sum - STANDARD_SIZE as f64).abs() > 1e-9 {
        anyhow::bail!("Golden 权重总和应为 {}，实际为 {:.12}", STANDARD_SIZE, golden_sum);
    }

    if raw_scores.len() != nodes.len() {
        anyhow::bail!("Raw Golden replay row count mismatch");
    }
    for (idx, node) in nodes.iter().enumerate().filter(|(_, node)| node.bz > 0) {
        let replayed = raw_scores[idx];
        let iterated = node.avg_cqd();
        if (replayed - iterated).abs() > 1e-9 {
            anyhow::bail!(
                "Raw Golden replay mismatch for group_id={}: iterated={:.12}, replayed={:.12}",
                node.group.id,
                iterated,
                replayed
            );
        }
    }

    // 将此参数保留在验证边界中，以免未来存储变更意外地依据不同的率来源验证分数。
    let _ = rate_matrix.loaded_pair_count();
    Ok(())
}

pub(super) fn fight_in_memory(nodes: &mut [RankNode], standard: &[usize], rate_matrix: &RateMatrix) {
    let standard_len = standard.len() as f64;

    for (idx, node) in nodes.iter_mut().enumerate() {
        let mut score = 0.0;
        for &bz_idx in standard {
            score += rate_matrix.get(idx, bz_idx);
        }
        node.cqd = score / standard_len;
    }
}

pub(super) fn fight_in_memory_for_indices(
    nodes: &mut [RankNode],
    active_indices: &[usize],
    standard: &[usize],
    rate_matrix: &RateMatrix,
) {
    let standard_len = standard.len() as f64;

    for &idx in active_indices {
        let mut score = 0.0;
        for &bz_idx in standard {
            score += rate_matrix.get(idx, bz_idx);
        }
        nodes[idx].cqd = score / standard_len;
    }
}

pub(super) fn make_initial_standard_indices(
    db: &Db,
    lane_size: usize,
    nodes: &[RankNode],
    dsu: &mut TeamDsu,
) -> anyhow::Result<(Vec<usize>, bool)> {
    // 返回值第二项 used_history_target：
    // - true：确实从数据库历史 Score 结果中选入了至少一个初始靶子；
    //         此时神秘系数从第 1 轮开始直接平滑学习。
    // - false：没有可用历史靶子，只能走当前 make 逻辑；
    //          此时保持原算法，热身期前 shenmixishu = cqd，不平滑。
    //
    // 注意：不能直接把数据库 Score 前 50 搬过来。
    // 初始靶子也必须满足 make 的基本条件：
    // 1. 同一个合并后战队最多 TEAM_LIMIT 个
    // 2. 同一个 member 在靶子中只能出现一次
    let top_ids = db.lane_top_score_group_ids(lane_size, nodes.len(), None)?;

    let id_to_idx: HashMap<GroupId, usize> = nodes.iter().enumerate().map(|(idx, node)| (node.group.id, idx)).collect();

    let mut selected = Vec::with_capacity(STANDARD_SIZE);
    let mut selected_set = HashSet::<usize>::new();
    let mut team_count: HashMap<String, usize> = HashMap::new();
    let mut used_members: HashSet<String> = HashSet::new();
    let mut used_history_target = false;

    for group_id in top_ids {
        let Some(&idx) = id_to_idx.get(&group_id) else {
            continue;
        };

        if try_select_standard_index(
            idx,
            nodes,
            dsu,
            &mut selected,
            &mut selected_set,
            &mut team_count,
            &mut used_members,
            None,
        ) {
            used_history_target = true;
        }

        if selected.len() >= STANDARD_SIZE {
            return Ok((selected, used_history_target));
        }
    }

    // 如果历史结果不足 50 个，或者历史高分里有大量冲突项，
    // 则按当前 make 逻辑继续补足，但仍然沿用已选靶子的 team/member 占用状态。
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| nodes[b].shenmixishu.total_cmp(&nodes[a].shenmixishu));

    for idx in order {
        let _ = try_select_standard_index(
            idx,
            nodes,
            dsu,
            &mut selected,
            &mut selected_set,
            &mut team_count,
            &mut used_members,
            None,
        );

        if selected.len() >= STANDARD_SIZE {
            break;
        }
    }

    Ok((selected, used_history_target))
}
pub(super) fn make_standard_indices(nodes: &[RankNode], dsu: &mut TeamDsu) -> Vec<usize> {
    let candidates = (0..nodes.len()).collect::<Vec<_>>();
    make_standard_indices_from_candidates(nodes, &candidates, dsu)
}

pub(super) fn make_standard_indices_from_candidates(nodes: &[RankNode], candidates: &[usize], dsu: &mut TeamDsu) -> Vec<usize> {
    let mut order = candidates.to_vec();
    order.sort_by(|&a, &b| nodes[b].shenmixishu.total_cmp(&nodes[a].shenmixishu));

    let mut selected = Vec::with_capacity(STANDARD_SIZE);
    let mut selected_set = HashSet::<usize>::new();
    let mut team_count: HashMap<String, usize> = HashMap::new();
    let mut used_members: HashSet<String> = HashSet::new();

    for idx in order {
        try_select_standard_index(
            idx,
            nodes,
            dsu,
            &mut selected,
            &mut selected_set,
            &mut team_count,
            &mut used_members,
            None,
        );

        if selected.len() >= STANDARD_SIZE {
            break;
        }
    }

    selected
}

pub(super) fn try_select_standard_index(
    idx: usize,
    nodes: &[RankNode],
    dsu: &mut TeamDsu,
    selected: &mut Vec<usize>,
    selected_set: &mut HashSet<usize>,
    team_count: &mut HashMap<String, usize>,
    used_members: &mut HashSet<String>,
    min_current_cqd: Option<f64>,
) -> bool {
    if selected.len() >= STANDARD_SIZE {
        return false;
    }

    // 手动屏蔽的组合正常参与 CQD/Score 计算，但不能进入靶子。
    if nodes[idx].group.is_blocked {
        return false;
    }

    // CQD 48.7 只属于手动约束选号训练池；原靶子选择不使用这个门槛。
    if let Some(min_cqd) = min_current_cqd {
        let current_cqd = nodes[idx].cqd;
        if current_cqd != 0.0 && current_cqd < min_cqd {
            return false;
        }
    }

    if !selected_set.insert(idx) {
        return false;
    }

    let group = &nodes[idx].group;
    let root_team = dsu.find(&group.team_name);

    if team_count.get(&root_team).copied().unwrap_or(0) >= TEAM_LIMIT {
        selected_set.remove(&idx);
        return false;
    }

    if group.members.iter().any(|member| used_members.contains(member)) {
        selected_set.remove(&idx);
        return false;
    }

    selected.push(idx);
    *team_count.entry(root_team).or_insert(0) += 1;

    for member in &group.members {
        used_members.insert(member.clone());
    }

    true
}
pub(super) fn data(nodes: &mut [RankNode], round: usize, warmup_rounds: usize, stickiness: usize, smooth_from_start: bool) {
    let alpha = 1.0 / stickiness.max(1) as f64;
    let should_smooth = smooth_from_start || round >= warmup_rounds;
    let starts_smoothing_this_round = (smooth_from_start && round == 1) || (!smooth_from_start && round == warmup_rounds);

    for node in nodes {
        update_node_data(node, round, warmup_rounds, alpha, should_smooth, starts_smoothing_this_round);
    }
}

pub(super) fn data_for_indices(
    nodes: &mut [RankNode],
    active_indices: &[usize],
    round: usize,
    warmup_rounds: usize,
    stickiness: usize,
    smooth_from_start: bool,
) {
    // 粘性就是神秘系数平滑分母：alpha = 1 / stickiness。
    // 默认情况下：单人组 stickiness=10，双人组 stickiness=20，x 人组 stickiness=10*x。
    //
    // 关键尺度约定：
    // 一旦进入“平滑学习”阶段，shenmixishu 必须始终保持 cqd * 100 的尺度。
    // 因此进入平滑的第一轮，不从 0 开始平滑，而是先初始化为当前 cqd * 100。
    //
    // 最终 Score 的统计窗口不变，仍然只在 warmup_rounds 后累计。
    let alpha = 1.0 / stickiness.max(1) as f64;
    let should_smooth = smooth_from_start || round >= warmup_rounds;
    let starts_smoothing_this_round = (smooth_from_start && round == 1) || (!smooth_from_start && round == warmup_rounds);

    for &idx in active_indices {
        update_node_data(
            &mut nodes[idx],
            round,
            warmup_rounds,
            alpha,
            should_smooth,
            starts_smoothing_this_round,
        );
    }
}

pub(super) fn update_node_data(
    node: &mut RankNode,
    round: usize,
    warmup_rounds: usize,
    alpha: f64,
    should_smooth: bool,
    starts_smoothing_this_round: bool,
) {
    if round >= warmup_rounds {
        node.n += 1;
        node.cqds += node.cqd;
        node.cqdss += node.cqd * node.cqd;
        node.cqdmin = node.cqdmin.min(node.cqd);
        node.cqdmax = node.cqdmax.max(node.cqd);
    }

    if should_smooth {
        if starts_smoothing_this_round {
            node.shenmixishu = node.cqd * 100.0;
        } else {
            node.shenmixishu = (1.0 - alpha) * node.shenmixishu + (100.0 * alpha) * node.cqd;
        }
    } else {
        node.shenmixishu = node.cqd;
    }
}

pub(super) fn record_golden_round(
    nodes: &mut [RankNode],
    active_indices: Option<&[usize]>,
    standard: &[usize],
    selected_flags: &mut [bool],
) {
    for &idx in standard {
        selected_flags[idx] = true;
    }

    if let Some(active) = active_indices {
        for &idx in active {
            nodes[idx].odds_n += 1;
            if selected_flags[idx] {
                nodes[idx].bz += 1;
            }
        }
    } else {
        for (idx, node) in nodes.iter_mut().enumerate() {
            node.odds_n += 1;
            if selected_flags[idx] {
                node.bz += 1;
            }
        }
    }

    for &idx in standard {
        selected_flags[idx] = false;
    }
}

pub(super) fn avg_cqd_rounded_unchanged(nodes: &[RankNode], last: &mut [i64]) -> bool {
    let mut changed = false;

    for (idx, node) in nodes.iter().enumerate() {
        let value = round_to_3_int(node.avg_cqd());
        if last[idx] != value {
            last[idx] = value;
            changed = true;
        }
    }

    !changed
}

pub(super) fn avg_cqd_rounded_unchanged_for_indices(nodes: &[RankNode], indices: &[usize], last: &mut [i64]) -> bool {
    let mut changed = false;

    for &idx in indices {
        let value = round_to_3_int(nodes[idx].avg_cqd());
        if last[idx] != value {
            last[idx] = value;
            changed = true;
        }
    }

    !changed
}

pub(super) fn round_to_3(value: f64) -> f64 { round_to_3_int(value) as f64 / 1000.0 }

pub(super) fn round_to_3_int(value: f64) -> i64 { (value * 1000.0).round() as i64 }
#[cfg(test)]
mod raw_core_tests {
    use super::*;

    pub(super) fn test_node(id: GroupId) -> RankNode {
        RankNode::new(StoredGroup {
            id,
            canonical: format!("group-{id}"),
            display_raw: format!("group-{id}"),
            lane_size: 1,
            team_name: format!("team-{id}"),
            members: vec![format!("member-{id}")],
            is_blocked: false,
        })
    }

    #[test]
    pub(super) fn core_only_golden_round_does_not_update_inactive_nodes() {
        let mut nodes = vec![test_node(1), test_node(2), test_node(3)];
        let mut selected_flags = vec![false; nodes.len()];

        record_golden_round(&mut nodes, Some(&[0, 1]), &[1], &mut selected_flags);

        assert_eq!((nodes[0].odds_n, nodes[0].bz), (1, 0));
        assert_eq!((nodes[1].odds_n, nodes[1].bz), (1, 1));
        assert_eq!((nodes[2].odds_n, nodes[2].bz), (0, 0));
        assert!(selected_flags.iter().all(|selected| !selected));
    }

    #[test]
    pub(super) fn final_raw_is_exactly_replayed_for_inactive_nodes() {
        let mut nodes = (0..=STANDARD_SIZE as i64).map(test_node).collect::<Vec<_>>();
        let mut rate_matrix = RateMatrix::new(nodes.len());
        for i in 0..nodes.len() {
            for j in (i + 1)..nodes.len() {
                rate_matrix.set_pair(i, j, 50.0 + (i as f64 - j as f64) * 0.1);
            }
        }

        for node in nodes.iter_mut().take(STANDARD_SIZE) {
            node.bz = 1;
            node.odds_n = 1;
            node.n = 1;
        }

        let raw_scores = replay_raw_scores_from_golden(&nodes, &rate_matrix).unwrap();
        for (idx, node) in nodes.iter_mut().take(STANDARD_SIZE).enumerate() {
            node.cqd = raw_scores[idx];
            node.cqds = raw_scores[idx];
            node.cqdss = raw_scores[idx] * raw_scores[idx];
        }

        let inactive_idx = STANDARD_SIZE;
        let expected = (0..STANDARD_SIZE)
            .map(|target_idx| rate_matrix.get(inactive_idx, target_idx))
            .sum::<f64>()
            / STANDARD_SIZE as f64;
        assert!((raw_scores[inactive_idx] - expected).abs() < 1e-12);
        validate_raw_golden_identity(&nodes, &raw_scores, &rate_matrix).unwrap();
    }
}

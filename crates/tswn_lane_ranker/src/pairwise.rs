use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Instant;

use crate::db::Db;
use crate::model::{GroupId, LaneResultRow, RankNode, StoredGroup};
use crate::ranker::{RankerConfig, STANDARD_SIZE, TEAM_LIMIT};
use crate::team::TeamDsu;
use crate::winrate::compute_rate_without_db;

/// 手动修正默认环境阈值；只在用户点击“执行修正”后使用，不参与原靶子/默认排名流程。
pub const DEFAULT_SELECTION_CQD_THRESHOLD: f64 = 48.5;

const PAIRWISE_CANDIDATE_LIMIT: usize = 1_500;
const EDGE_BAG_SEED_COUNT: usize = 8;
const RANDOM_EXTRA_EDGE_BUDGET_PER_SEED: usize = 2_500;
const LOCAL_OFFSETS: &[usize] = &[1, 2, 3, 5, 8, 13, 20, 35, 60, 100, 160];
const BACKBONE_RANKS: &[usize] = &[1, 3, 5, 10, 20, 35, 50, 75, 100, 125, 150, 200, 250, 300, 400, 500, 750, 1_000, 1_250, 1_500];
const BRIDGE_WINDOW: usize = 24;
const MAX_TYPE_LABELS: usize = 32;
const MAX_TYPE_REPRESENTATIVES: usize = 12;
const MAX_MEMBER_BUCKET: usize = 160;
const MAX_TEAM_BUCKET: usize = 220;
const BT_ITERATIONS: usize = 5_000;
const BT_BASE_LR: f64 = 0.020;
const BT_L2: f64 = 0.0003;

// v43: orientation-checked no-cap residual calibration.
// The v42 failure mode was a sign/orientation error: the learned residual
// was strongly anti-correlated with raw CQD. Removing the old ±1.20 cap then
// let that anti-correlation overwhelm the raw axis and reverse the ranking.
//
// v43 fixes that in two ways:
// 1) auto-detect whether the pairwise rate target agrees with raw CQD;
//    if not, use logit(1-rate) == -logit(rate).
// 2) after fitting, enforce a minimum positive global slope vs raw CQD.
//    This is NOT a hard delta cap. Local reorder and large residuals remain allowed.
const V43_DIM: usize = 8;
const V43_RAW_LOGIT_SCALE: f64 = 0.68;
const V43_RAW_ANCHOR_CLIP: f64 = 10.0;
const V43_BIAS_L2: f64 = 0.006;
const V43_VECTOR_L2: f64 = 0.018;
const V43_VECTOR_INIT_SCALE: f64 = 0.030;
const V43_MATCHUP_TO_SCALAR: f64 = 0.85;
const V43_RESIDUAL_GAIN: f64 = 1.75;
const V43_RAW_SPREAD_GAIN: f64 = 1.03;
const V43_MIN_OUTPUT_STD_GAIN: f64 = 1.00;
const V43_MIN_RAW_SLOPE: f64 = 0.35;
const V43_EVIDENCE_HALF_LIFE: f64 = 5.0;
const V43_GRAD_CLIP: f64 = 5.0;
const V43_BIAS_NUMERIC_CLIP: f64 = 10.0;
const V43_VECTOR_NUMERIC_CLIP: f64 = 1.8;

// Constrained top-N search: multi-restart local repair, not a single greedy pass.
const CONSTRAINED_SEARCH_RESTARTS: usize = 160;
const CONSTRAINED_LOCAL_STEPS_PER_RESTART: usize = 60_000;
const CONSTRAINED_NO_IMPROVE_LIMIT: usize = 15_000;
const CONSTRAINED_SCORE_EPS: f64 = 1e-9;


#[derive(Debug, Clone)]
pub struct PairwiseCalibrationReport {
    pub candidate_count: usize,
    pub edge_count: usize,
    pub selected_count: usize,
    pub skipped_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct Candidate {
    node_idx: usize,
    row_idx: usize,
    raw_score: f64,
    root_team: String,
    type_label: String,
}

#[derive(Debug, Clone)]
struct PairEdge {
    a: usize,
    b: usize,
    rate_a_to_b: f64,
}

#[derive(Debug, Clone)]
struct MissingPair {
    a: usize,
    b: usize,
    group_a: StoredGroup,
    group_b: StoredGroup,
}

#[derive(Debug, Default, Clone)]
struct SelectionResources {
    used_members: HashSet<String>,
    team_count: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
struct SeedFit {
    pair_scores: Vec<f64>,
    ranks: Vec<usize>,
    uncertainties: Vec<f64>,
    degrees: Vec<usize>,
    edge_count: usize,
}

pub fn calibrate_lane_rows(
    db: &Db,
    lane_size: usize,
    nodes: &[RankNode],
    rows: &mut Vec<LaneResultRow>,
    dsu: &TeamDsu,
    config: &RankerConfig,
    final_round: usize,
    threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    recompute_raw_ranks(rows);
    let mut candidates = build_candidates(nodes, rows, dsu, threshold);
    if candidates.len() > PAIRWISE_CANDIDATE_LIMIT {
        candidates.truncate(PAIRWISE_CANDIDATE_LIMIT);
    }

    mark_rows_outside_candidate_pool(rows, threshold);
    recompute_raw_ranks(rows);

    if candidates.len() < 4 {
        return Ok(PairwiseCalibrationReport {
            candidate_count: candidates.len(),
            edge_count: 0,
            selected_count: 0,
            skipped_reason: Some(format!(
                "candidate_count_too_small_for_v49_edge_bagging: {} < 4",
                candidates.len()
            )),
        });
    }

    db.set_lane_progress(
        lane_size,
        "pairwise_building_base_graph",
        final_round,
        config.total_rounds,
        0,
        candidates.len(),
        0,
        &format!(
            "building v49 edge-bagging base graph: candidates={}, threshold={:.3}, seeds={}, random_extra_per_seed={}, fixed_context_size={}, constraints=no_duplicate_member+team_limit_{}, no_final_select",
            candidates.len(),
            threshold,
            EDGE_BAG_SEED_COUNT,
            RANDOM_EXTRA_EDGE_BUDGET_PER_SEED,
            STANDARD_SIZE,
            TEAM_LIMIT
        ),
    )?;

    let base_pair_keys = build_pair_keys(&candidates, nodes, STANDARD_SIZE);
    if base_pair_keys.is_empty() {
        return Ok(PairwiseCalibrationReport {
            candidate_count: candidates.len(),
            edge_count: 0,
            selected_count: 0,
            skipped_reason: Some("legal_base_pair_graph_empty".to_string()),
        });
    }

    let raw_scores: Vec<f64> = candidates.iter().map(|c| c.raw_score).collect();
    let mut seed_fits = Vec::<SeedFit>::with_capacity(EDGE_BAG_SEED_COUNT);
    let mut total_edge_count = 0usize;

    for seed_idx in 0..EDGE_BAG_SEED_COUNT {
        let seed = make_bag_seed(lane_size, final_round, seed_idx, candidates.len(), base_pair_keys.len());
        let pair_keys = build_seed_pair_keys(
            &base_pair_keys,
            &candidates,
            nodes,
            STANDARD_SIZE,
            seed,
            RANDOM_EXTRA_EDGE_BUDGET_PER_SEED,
        );
        if pair_keys.is_empty() {
            continue;
        }

        db.set_lane_progress(
            lane_size,
            "pairwise_edge_bagging",
            final_round,
            config.total_rounds,
            seed_idx + 1,
            EDGE_BAG_SEED_COUNT,
            0,
            &format!(
                "v49 fitting seed {}/{}: base_edges={}, seed_edges={}, random_extra_budget={}, dim={}, iterations={}",
                seed_idx + 1,
                EDGE_BAG_SEED_COUNT,
                base_pair_keys.len(),
                pair_keys.len(),
                RANDOM_EXTRA_EDGE_BUDGET_PER_SEED,
                V43_DIM,
                BT_ITERATIONS
            ),
        )?;

        let edges = load_or_compute_pair_edges(db, lane_size, final_round, &candidates, nodes, &pair_keys, config)?;
        if edges.is_empty() {
            continue;
        }

        let degrees = edge_degrees(candidates.len(), &edges);
        let (pair_scores, uncertainties) = fit_pair_scores(candidates.len(), &edges, &raw_scores);
        let ranks = ranks_from_scores(&pair_scores);
        total_edge_count += edges.len();

        seed_fits.push(SeedFit {
            pair_scores,
            ranks,
            uncertainties,
            degrees,
            edge_count: edges.len(),
        });
    }

    if seed_fits.is_empty() {
        return Ok(PairwiseCalibrationReport {
            candidate_count: candidates.len(),
            edge_count: 0,
            selected_count: 0,
            skipped_reason: Some("no_seed_pair_rates_available".to_string()),
        });
    }

    db.set_lane_progress(
        lane_size,
        "pairwise_stability_aggregating",
        final_round,
        config.total_rounds,
        seed_fits.len(),
        EDGE_BAG_SEED_COUNT,
        0,
        &format!(
            "aggregating v49 edge-bagging stability: effective_seeds={}, total_seed_edges={}",
            seed_fits.len(),
            total_edge_count
        ),
    )?;

    let aggregate = aggregate_seed_fits(&candidates, &seed_fits);
    write_pair_stability(rows, &candidates, &aggregate);

    let pair_rank_order = sorted_candidate_order(&aggregate.mean_scores);
    for (rank_idx, &candidate_pos) in pair_rank_order.iter().enumerate() {
        let row = &mut rows[candidates[candidate_pos].row_idx];
        row.pair_rank = Some(rank_idx + 1);
        row.constrained_rank = None;
        row.marginal_value = None;
    }

    // v49: do not reorder saved rows here. row.rank stays R-Rank; row.pair_rank stores P-Rank.

    Ok(PairwiseCalibrationReport {
        candidate_count: candidates.len(),
        edge_count: total_edge_count / seed_fits.len().max(1),
        selected_count: 0,
        skipped_reason: None,
    })
}



pub fn calibrate_saved_lane_results(
    db: &Db,
    lane_size: usize,
    config: &RankerConfig,
    threshold: f64,
) -> anyhow::Result<PairwiseCalibrationReport> {
    if !threshold.is_finite() || !(0.0..=100.0).contains(&threshold) {
        anyhow::bail!("环境阈值必须是 0 到 100 之间的数字");
    }

    let mut rows = db.lane_results(lane_size)?;
    if rows.is_empty() {
        anyhow::bail!("该赛道还没有结果；请先完成一次默认重算");
    }

    db.set_lane_progress(
        lane_size,
        "pairwise_stability_loading",
        0,
        config.total_rounds,
        0,
        rows.len(),
        0,
        &format!("manual v49 edge-bagging stability queued from saved raw results; threshold={threshold:.3}, no_final_select"),
    )?;

    let groups = db.load_groups_by_lane(lane_size)?;
    let group_ids_in_rows: HashSet<GroupId> = rows.iter().map(|row| row.group_id).collect();
    let nodes: Vec<RankNode> = groups
        .into_iter()
        .filter(|group| group_ids_in_rows.contains(&group.id))
        .map(RankNode::new)
        .collect();

    let dsu = db.load_team_dsu()?;
    let report = calibrate_lane_rows(
        db,
        lane_size,
        &nodes,
        &mut rows,
        &dsu,
        config,
        0,
        threshold,
    )?;

    db.save_lane_results(lane_size, &rows)?;
    db.set_lane_status(lane_size, "ready", rows.len())?;
    db.set_lane_progress(
        lane_size,
        "pairwise_stability_ready",
        0,
        config.total_rounds,
        report.edge_count,
        report.candidate_count,
        0,
        &format!(
            "v49 edge-bagging stability done: threshold={:.3}, candidates={}, mean_edges_per_seed={}, seeds={}, skipped_reason={}",
            threshold,
            report.candidate_count,
            report.edge_count,
            EDGE_BAG_SEED_COUNT,
            report.skipped_reason.clone().unwrap_or_else(|| "none".to_string())
        ),
    )?;

    Ok(report)
}



#[derive(Debug, Clone)]
struct AggregateScores {
    mean_scores: Vec<f64>,
    score_std: Vec<f64>,
    mean_uncertainty: Vec<f64>,
    rank_std: Vec<f64>,
    delta_std: Vec<f64>,
    edge_count_mean: Vec<f64>,
    stability_flags: Vec<String>,
}

fn aggregate_seed_fits(candidates: &[Candidate], seed_fits: &[SeedFit]) -> AggregateScores {
    let n = candidates.len();
    let seed_count = seed_fits.len().max(1);
    let mut mean_scores = vec![0.0; n];
    let mut score_std = vec![0.0; n];
    let mut mean_uncertainty = vec![0.0; n];
    let mut rank_std = vec![0.0; n];
    let mut delta_std = vec![0.0; n];
    let mut edge_count_mean = vec![0.0; n];
    let mut stability_flags = vec![String::new(); n];

    for pos in 0..n {
        let scores: Vec<f64> = seed_fits.iter().map(|fit| fit.pair_scores[pos]).collect();
        let ranks: Vec<f64> = seed_fits.iter().map(|fit| fit.ranks[pos] as f64).collect();
        let deltas: Vec<f64> = scores.iter().map(|score| *score - candidates[pos].raw_score).collect();
        let uncs: Vec<f64> = seed_fits.iter().map(|fit| fit.uncertainties[pos]).collect();
        let degrees: Vec<f64> = seed_fits.iter().map(|fit| fit.degrees[pos] as f64).collect();

        let score_mean = mean(&scores);
        let rank_mean = mean(&ranks);
        let delta_mean = mean(&deltas);
        let unc_mean = mean(&uncs);
        let degree_mean = mean(&degrees);
        let s_std = stddev(&scores, score_mean);
        let r_std = stddev(&ranks, rank_mean);
        let d_std = stddev(&deltas, delta_mean);

        mean_scores[pos] = round_to_3(score_mean);
        score_std[pos] = round_to_3(s_std);
        mean_uncertainty[pos] = round_to_3(unc_mean.max(s_std));
        rank_std[pos] = round_to_3(r_std);
        delta_std[pos] = round_to_3(d_std);
        edge_count_mean[pos] = round_to_3(degree_mean);
        stability_flags[pos] = classify_stability(r_std, d_std, degree_mean, seed_count);
    }

    AggregateScores {
        mean_scores,
        score_std,
        mean_uncertainty,
        rank_std,
        delta_std,
        edge_count_mean,
        stability_flags,
    }
}

fn classify_stability(rank_std: f64, delta_std: f64, edge_count_mean: f64, seed_count: usize) -> String {
    if seed_count < 3 || edge_count_mean < 6.0 {
        return "unstable".to_string();
    }
    if rank_std <= 10.0 && delta_std <= 0.25 {
        "stable".to_string()
    } else if rank_std <= 25.0 && delta_std <= 0.60 {
        "watch".to_string()
    } else {
        "unstable".to_string()
    }
}

fn build_candidates(
    nodes: &[RankNode],
    rows: &[LaneResultRow],
    dsu: &TeamDsu,
    threshold: f64,
) -> Vec<Candidate> {
    let node_idx_by_group_id: HashMap<GroupId, usize> = nodes
        .iter()
        .enumerate()
        .map(|(idx, node)| (node.group.id, idx))
        .collect();

    let mut candidates = Vec::new();
    for (row_idx, row) in rows.iter().enumerate() {
        let raw = if row.raw_average_cqd.is_finite() && row.raw_average_cqd != 0.0 {
            row.raw_average_cqd
        } else {
            row.average_cqd
        };
        if row.is_blocked || raw < threshold {
            continue;
        }

        let Some(&node_idx) = node_idx_by_group_id.get(&row.group_id) else {
            continue;
        };

        candidates.push(Candidate {
            node_idx,
            row_idx,
            raw_score: raw,
            root_team: dsu.find_readonly(&nodes[node_idx].group.team_name),
            type_label: row.type_label.clone(),
        });
    }

    candidates.sort_by(|a, b| b.raw_score.total_cmp(&a.raw_score));
    candidates
}

fn mark_rows_outside_candidate_pool(rows: &mut [LaneResultRow], threshold: f64) {
    for row in rows {
        row.raw_average_cqd = if row.raw_average_cqd.is_finite() && row.raw_average_cqd != 0.0 {
            row.raw_average_cqd
        } else {
            row.average_cqd
        };
        row.pair_score = None;
        row.pair_rank = None;
        row.uncertainty = None;
        row.raw_delta = None;
        row.pair_score_std = None;
        row.pair_rank_std = None;
        row.delta_std = None;
        row.edge_count_mean = None;
        row.marginal_value = None;
        row.constrained_rank = None;
        row.stability_flag = if row.is_blocked {
            "blocked".to_string()
        } else if row.raw_average_cqd < threshold {
            "below_threshold".to_string()
        } else {
            "calibration_skipped".to_string()
        };
        row.selection_status = row.stability_flag.clone();
    }
}

fn recompute_raw_ranks(rows: &mut [LaneResultRow]) {
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| {
        raw_score_for_rank(&rows[b])
            .total_cmp(&raw_score_for_rank(&rows[a]))
            .then_with(|| rows[a].group_id.cmp(&rows[b].group_id))
    });

    for (idx, row_idx) in order.into_iter().enumerate() {
        let raw = raw_score_for_rank(&rows[row_idx]);
        rows[row_idx].rank = idx + 1;
        rows[row_idx].raw_average_cqd = raw;
    }
}

fn raw_score_for_rank(row: &LaneResultRow) -> f64 {
    if row.raw_average_cqd.is_finite() && row.raw_average_cqd != 0.0 {
        row.raw_average_cqd
    } else {
        row.average_cqd
    }
}

fn build_pair_keys(candidates: &[Candidate], nodes: &[RankNode], selection_size: usize) -> Vec<(usize, usize)> {
    let mut keys = HashSet::<(usize, usize)>::new();

    // 1) backbone edges：每个候选都连到固定分位/名次标尺点。
    let anchors: Vec<usize> = BACKBONE_RANKS
        .iter()
        .filter_map(|rank| rank.checked_sub(1))
        .filter(|&idx| idx < candidates.len())
        .collect();
    for pos in 0..candidates.len() {
        for &anchor in &anchors {
            add_pair_key(&mut keys, candidates, nodes, pos, anchor, selection_size);
        }
    }

    // 2) local edges：当前排序附近的精排边。
    for pos in 0..candidates.len() {
        for &offset in LOCAL_OFFSETS {
            if let Some(other) = pos.checked_add(offset).filter(|&idx| idx < candidates.len()) {
                add_pair_key(&mut keys, candidates, nodes, pos, other, selection_size);
            }
        }
    }

    // 3) rank-band bridge edges：跨层边界的校准桥。
    let boundaries = [25usize, 50, 75, 100, 125, 150, 200, 250, 325, 400, 500, 650, 800, 1_000, 1_250];
    for &boundary in &boundaries {
        if boundary == 0 || boundary >= candidates.len() {
            continue;
        }
        let left_start = boundary.saturating_sub(BRIDGE_WINDOW);
        let left_end = boundary;
        let right_end = (boundary + BRIDGE_WINDOW).min(candidates.len());
        for left in left_start..left_end {
            for right in boundary..right_end {
                add_pair_key(&mut keys, candidates, nodes, left, right, selection_size);
            }
        }
    }

    // 4) type bridge edges：跨技能类型/行为区桥接，降低类型分层造成的坐标扭曲。
    let mut by_type: HashMap<String, Vec<usize>> = HashMap::new();
    for (pos, candidate) in candidates.iter().enumerate() {
        by_type.entry(candidate.type_label.clone()).or_default().push(pos);
    }
    let mut type_groups: Vec<_> = by_type.into_iter().collect();
    type_groups.sort_by(|a, b| a.1[0].cmp(&b.1[0]));
    type_groups.truncate(MAX_TYPE_LABELS);
    let type_reps: Vec<Vec<usize>> = type_groups
        .iter()
        .map(|(_, positions)| representative_positions(positions, MAX_TYPE_REPRESENTATIVES))
        .collect();
    for i in 0..type_groups.len() {
        for j in (i + 1)..type_groups.len() {
            for &a in &type_reps[i] {
                for &b in &type_reps[j] {
                    add_pair_key(&mut keys, candidates, nodes, a, b, selection_size);
                }
            }
        }
    }

    // 5) 共享号互斥竞争：这些 pair 不能同时上，反而最该比较。
    let mut by_member: HashMap<String, Vec<usize>> = HashMap::new();
    for (pos, candidate) in candidates.iter().enumerate() {
        for member in &nodes[candidate.node_idx].group.members {
            by_member.entry(member.clone()).or_default().push(pos);
        }
    }
    for (_member, mut bucket) in by_member {
        bucket.sort_unstable();
        bucket.truncate(MAX_MEMBER_BUCKET);
        for w in bucket.windows(2) {
            add_pair_key(&mut keys, candidates, nodes, w[0], w[1], selection_size);
        }
        for i in 0..bucket.len().min(12) {
            for j in (i + 1)..bucket.len().min(i + 12) {
                add_pair_key(&mut keys, candidates, nodes, bucket[i], bucket[j], selection_size);
            }
        }
    }

    // 6) 同战队内部竞争与战队名额边界：尤其关注第 TEAM_LIMIT 个坑附近。
    let mut by_team: HashMap<String, Vec<usize>> = HashMap::new();
    for (pos, candidate) in candidates.iter().enumerate() {
        by_team.entry(candidate.root_team.clone()).or_default().push(pos);
    }
    for (_team, mut bucket) in by_team {
        bucket.sort_unstable();
        bucket.truncate(MAX_TEAM_BUCKET);
        for w in bucket.windows(2) {
            add_pair_key(&mut keys, candidates, nodes, w[0], w[1], selection_size);
        }

        let boundary_start = TEAM_LIMIT.saturating_sub(2);
        let boundary_end = (TEAM_LIMIT + 6).min(bucket.len());
        for i in 0..boundary_end {
            for j in boundary_start..boundary_end {
                if i != j {
                    add_pair_key(&mut keys, candidates, nodes, bucket[i], bucket[j], selection_size);
                }
            }
        }
    }

    let mut out: Vec<_> = keys.into_iter().collect();
    out.sort_unstable();
    out
}


fn representative_positions(bucket: &[usize], limit: usize) -> Vec<usize> {
    if bucket.is_empty() || limit == 0 {
        return Vec::new();
    }
    if bucket.len() <= limit {
        return bucket.to_vec();
    }

    let mut out = Vec::with_capacity(limit);
    for slot in 0..limit {
        let idx = if limit == 1 {
            0
        } else {
            slot * (bucket.len() - 1) / (limit - 1)
        };
        out.push(bucket[idx]);
    }
    out.sort_unstable();
    out.dedup();
    out
}


fn member_buckets(candidates: &[Candidate], nodes: &[RankNode]) -> Vec<Vec<usize>> {
    let mut by_member: HashMap<String, Vec<usize>> = HashMap::new();
    for (pos, candidate) in candidates.iter().enumerate() {
        for member in &nodes[candidate.node_idx].group.members {
            by_member.entry(member.clone()).or_default().push(pos);
        }
    }
    by_member.into_values().filter(|bucket| bucket.len() > 1).collect()
}

fn team_buckets(candidates: &[Candidate]) -> Vec<Vec<usize>> {
    let mut by_team: HashMap<String, Vec<usize>> = HashMap::new();
    for (pos, candidate) in candidates.iter().enumerate() {
        by_team.entry(candidate.root_team.clone()).or_default().push(pos);
    }
    by_team.into_values().filter(|bucket| bucket.len() > 1).collect()
}

fn type_buckets(candidates: &[Candidate]) -> Vec<Vec<usize>> {
    let mut by_type: HashMap<String, Vec<usize>> = HashMap::new();
    for (pos, candidate) in candidates.iter().enumerate() {
        by_type.entry(candidate.type_label.clone()).or_default().push(pos);
    }
    by_type.into_values().filter(|bucket| bucket.len() > 1).collect()
}

fn build_seed_pair_keys(
    base_pair_keys: &[(usize, usize)],
    candidates: &[Candidate],
    nodes: &[RankNode],
    selection_size: usize,
    seed: u64,
    random_budget: usize,
) -> Vec<(usize, usize)> {
    let mut keys: HashSet<(usize, usize)> = base_pair_keys.iter().copied().collect();
    if candidates.len() < 2 || random_budget == 0 {
        let mut out: Vec<_> = keys.into_iter().collect();
        out.sort_unstable();
        return out;
    }

    let mut rng = SplitMix64::new(seed);
    let mut added = 0usize;
    let max_trials = random_budget.saturating_mul(50).max(1_000);
    let member_buckets = member_buckets(candidates, nodes);
    let team_buckets = team_buckets(candidates);
    let type_buckets = type_buckets(candidates);
    let local_windows = [4usize, 8, 16, 32, 64, 128, 256];

    for _ in 0..max_trials {
        if added >= random_budget {
            break;
        }
        let before = keys.len();
        let roll = rng.usize(100);
        let mut pair: Option<(usize, usize)> = None;

        if roll < 42 {
            let a = rng.usize(candidates.len());
            let w = local_windows[rng.usize(local_windows.len())];
            let lo = a.saturating_sub(w);
            let hi = (a + w + 1).min(candidates.len());
            if hi > lo + 1 {
                let mut b = lo + rng.usize(hi - lo);
                if b == a {
                    b = (b + 1).min(hi - 1);
                }
                pair = Some((a, b));
            }
        } else if roll < 65 {
            let a = rng.usize(candidates.len());
            let mut b = rng.usize(candidates.len());
            if b == a {
                b = (b + 1) % candidates.len();
            }
            pair = Some((a, b));
        } else if roll < 80 {
            if let Some(bucket) = random_bucket(&type_buckets, &mut rng) {
                let a = bucket[rng.usize(bucket.len())];
                for _ in 0..8 {
                    let b = rng.usize(candidates.len());
                    if b != a && candidates[b].type_label != candidates[a].type_label {
                        pair = Some((a, b));
                        break;
                    }
                }
            }
        } else if roll < 91 {
            if let Some(bucket) = random_bucket(&member_buckets, &mut rng) {
                let a = bucket[rng.usize(bucket.len())];
                let mut b = bucket[rng.usize(bucket.len())];
                if b == a && bucket.len() > 1 {
                    b = bucket[(rng.usize(bucket.len() - 1) + 1) % bucket.len()];
                }
                pair = Some((a, b));
            }
        } else if let Some(bucket) = random_bucket(&team_buckets, &mut rng) {
            let a = bucket[rng.usize(bucket.len())];
            let mut b = bucket[rng.usize(bucket.len())];
            if b == a && bucket.len() > 1 {
                b = bucket[(rng.usize(bucket.len() - 1) + 1) % bucket.len()];
            }
            pair = Some((a, b));
        }

        if let Some((a, b)) = pair {
            add_pair_key(&mut keys, candidates, nodes, a, b, selection_size);
            if keys.len() > before {
                added += 1;
            }
        }
    }

    let mut out: Vec<_> = keys.into_iter().collect();
    out.sort_unstable();
    out
}

fn random_bucket<'a>(buckets: &'a [Vec<usize>], rng: &mut SplitMix64) -> Option<&'a Vec<usize>> {
    if buckets.is_empty() {
        None
    } else {
        Some(&buckets[rng.usize(buckets.len())])
    }
}

fn add_pair_key(
    keys: &mut HashSet<(usize, usize)>,
    candidates: &[Candidate],
    nodes: &[RankNode],
    a: usize,
    b: usize,
    selection_size: usize,
) {
    if a == b || a >= candidates.len() || b >= candidates.len() {
        return;
    }

    if !has_shared_legal_replacement_context(candidates, nodes, a, b, selection_size) {
        return;
    }

    let key = ordered_pair(a, b);
    keys.insert(key);
}

fn has_shared_legal_replacement_context(
    candidates: &[Candidate],
    nodes: &[RankNode],
    a: usize,
    b: usize,
    selection_size: usize,
) -> bool {
    // A 与 B 可以共享号、同战队，因为它们是互斥替换项；背景 C 不允许占用 A/B 的号，
    // 并且 C + A 与 C + B 都必须满足战队上限。
    let mut forbidden_members = HashSet::<String>::new();
    for member in &nodes[candidates[a].node_idx].group.members {
        forbidden_members.insert(member.clone());
    }
    for member in &nodes[candidates[b].node_idx].group.members {
        forbidden_members.insert(member.clone());
    }

    let mut reserved_team_slots = HashMap::<String, usize>::new();
    reserved_team_slots.insert(candidates[a].root_team.clone(), 1);
    reserved_team_slots.insert(candidates[b].root_team.clone(), 1);

    let desired_context = selection_size
        .saturating_sub(1)
        .min(candidates.len().saturating_sub(2));
    if desired_context == 0 {
        return true;
    }

    let mut resources = SelectionResources::default();
    let mut context_count = 0usize;
    for (pos, candidate) in candidates.iter().enumerate() {
        if pos == a || pos == b {
            continue;
        }

        let group = &nodes[candidate.node_idx].group;
        if group.members.iter().any(|member| forbidden_members.contains(member)) {
            continue;
        }

        if try_add_to_context(&mut resources, candidate, group, &reserved_team_slots) {
            context_count += 1;
            if context_count >= desired_context {
                return true;
            }
        }
    }

    false
}

fn try_add_to_context(
    resources: &mut SelectionResources,
    candidate: &Candidate,
    group: &StoredGroup,
    reserved_team_slots: &HashMap<String, usize>,
) -> bool {
    let reserved = reserved_team_slots.get(&candidate.root_team).copied().unwrap_or(0);
    let current_team_count = resources.team_count.get(&candidate.root_team).copied().unwrap_or(0);
    if current_team_count + reserved >= TEAM_LIMIT {
        return false;
    }

    if group.members.iter().any(|member| resources.used_members.contains(member)) {
        return false;
    }

    *resources.team_count.entry(candidate.root_team.clone()).or_insert(0) += 1;
    for member in &group.members {
        resources.used_members.insert(member.clone());
    }
    true
}

fn load_or_compute_pair_edges(
    db: &Db,
    lane_size: usize,
    final_round: usize,
    candidates: &[Candidate],
    nodes: &[RankNode],
    pair_keys: &[(usize, usize)],
    config: &RankerConfig,
) -> anyhow::Result<Vec<PairEdge>> {
    let mut edges = Vec::new();
    let mut missing = Vec::new();

    for &(a, b) in pair_keys {
        let group_a = &nodes[candidates[a].node_idx].group;
        let group_b = &nodes[candidates[b].node_idx].group;

        if let Some(rate) = db.get_rate(group_a.id, group_b.id)? {
            edges.push(PairEdge { a, b, rate_a_to_b: rate });
            continue;
        }

        if let Some(reverse_rate) = db.get_rate(group_b.id, group_a.id)? {
            edges.push(PairEdge { a, b, rate_a_to_b: 100.0 - reverse_rate });
            continue;
        }

        missing.push(MissingPair {
            a,
            b,
            group_a: group_a.clone(),
            group_b: group_b.clone(),
        });
    }

    if missing.is_empty() {
        db.set_lane_progress(
            lane_size,
            "pairwise_rates_ready",
            final_round,
            config.total_rounds,
            edges.len(),
            pair_keys.len(),
            0,
            &format!("pairwise calibration rates already cached: edges={}", edges.len()),
        )?;
        return Ok(edges);
    }

    let total = missing.len();
    let workers = resolve_pair_workers(config.outer_workers, total);
    let mode = if config.outer_workers == 0 { "dynamic_queue" } else { "static_chunks" };
    let outer_label = if config.outer_workers == 0 {
        format!("auto({workers})")
    } else {
        format!("static({workers})")
    };
    let done = Arc::new(AtomicUsize::new(0));
    let started = Arc::new(Instant::now());
    let missing = Arc::new(missing);

    db.set_lane_progress(
        lane_size,
        "pairwise_rates",
        final_round,
        config.total_rounds,
        0,
        total,
        0,
        &format!(
            "computing pairwise calibration rates 0/{total}, outer_workers={outer_label}, inner_threads={}, mode={mode}",
            config.inner_workers
        ),
    )?;

    let mut handles = Vec::with_capacity(workers);

    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _ in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let next_pair = Arc::clone(&next_pair);
            let done = Arc::clone(&done);
            let started = Arc::clone(&started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();

            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(usize, usize, GroupId, GroupId, f64)>> {
                let mut computed = Vec::new();
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };

                    let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                    computed.push((pair.a, pair.b, pair.group_a.id, pair.group_b.id, rate));

                    report_pair_progress(
                        &db,
                        lane_size,
                        total_rounds,
                        &done,
                        total,
                        &started,
                        &outer_label,
                        inner_workers,
                        "dynamic_queue",
                    )?;
                }
                Ok(computed)
            }));
        }
    } else {
        for worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let done = Arc::clone(&done);
            let started = Arc::clone(&started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;

            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(usize, usize, GroupId, GroupId, f64)>> {
                let mut computed = Vec::with_capacity(end.saturating_sub(start));
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };

                    let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                    computed.push((pair.a, pair.b, pair.group_a.id, pair.group_b.id, rate));

                    report_pair_progress(
                        &db,
                        lane_size,
                        total_rounds,
                        &done,
                        total,
                        &started,
                        &outer_label,
                        inner_workers,
                        "static_chunks",
                    )?;
                }
                Ok(computed)
            }));
        }
    }

    let mut db_rates = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
    let mut computed_edges = Vec::<PairEdge>::with_capacity(total);
    for handle in handles {
        let part = handle.join().expect("pairwise calibration worker thread panicked")?;
        for (a, b, group_a_id, group_b_id, rate) in part {
            db_rates.push((group_a_id, group_b_id, rate));
            computed_edges.push(PairEdge { a, b, rate_a_to_b: rate });
        }
    }

    db.set_lane_progress(
        lane_size,
        "pairwise_saving_rates",
        final_round,
        config.total_rounds,
        total,
        total,
        0,
        &format!("saving {total} pairwise calibration rates, outer_workers={outer_label}, mode={mode}"),
    )?;
    db.save_rate_pairs_bulk(&db_rates, config.win_rate_samples)?;

    edges.extend(computed_edges);
    edges.sort_by_key(|edge| (edge.a, edge.b));
    Ok(edges)
}

fn report_pair_progress(
    db: &Db,
    lane_size: usize,
    total_rounds: usize,
    done: &AtomicUsize,
    total: usize,
    started: &Instant,
    outer_label: &str,
    inner_workers: u32,
    mode: &str,
) -> anyhow::Result<()> {
    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
    if current != total && current % 10 != 0 {
        return Ok(());
    }

    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let pairs_per_sec = current as f64 / elapsed;
    let eta_sec = if pairs_per_sec > 0.0 {
        total.saturating_sub(current) as f64 / pairs_per_sec
    } else {
        0.0
    };

    db.set_lane_progress(
        lane_size,
        "pairwise_rates",
        0,
        total_rounds,
        current,
        total,
        0,
        &format!(
            "computing pairwise calibration rates {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_workers}, mode={mode}",
            pairs_per_sec,
            format_duration(elapsed),
            format_duration(eta_sec),
        ),
    )?;
    Ok(())
}

fn fit_pair_scores(candidate_count: usize, edges: &[PairEdge], raw_scores: &[f64]) -> (Vec<f64>, Vec<f64>) {
    if candidate_count == 0 {
        return (Vec::new(), Vec::new());
    }

    // v43:
    // raw CQD is the main axis, but no longer a ±delta cage.
    // The critical fix is target orientation detection. If edge.rate_a_to_b
    // is anti-correlated with raw(a)-raw(b), then treating it as P(a>b)
    // trains the model backwards. In that case we use -logit(rate).
    let target_sign = v43_target_orientation_sign(edges, raw_scores);

    let raw_mean = mean(raw_scores);
    let raw_std = stddev(raw_scores, raw_mean).max(1e-6);
    let mut bias = vec![0.0f64; candidate_count];
    let mut attack = vec![vec![0.0f64; V43_DIM]; candidate_count];
    let mut defense = vec![vec![0.0f64; V43_DIM]; candidate_count];

    for idx in 0..candidate_count {
        for dim in 0..V43_DIM {
            attack[idx][dim] = v43_deterministic_init(idx, dim, 0xA11A_CCED) * V43_VECTOR_INIT_SCALE;
            defense[idx][dim] = v43_deterministic_init(idx, dim, 0xD3F0_0D5E) * V43_VECTOR_INIT_SCALE;
        }
    }

    for iter in 0..BT_ITERATIONS {
        let lr = BT_BASE_LR / (1.0 + iter as f64 / 900.0);

        for edge in edges {
            let a = edge.a;
            let b = edge.b;
            let target = target_sign * logit((edge.rate_a_to_b / 100.0).clamp(0.005, 0.995));
            let raw_anchor = ((raw_scores[a] - raw_scores[b]) / V43_RAW_LOGIT_SCALE)
                .clamp(-V43_RAW_ANCHOR_CLIP, V43_RAW_ANCHOR_CLIP);
            let matchup = v43_interaction(&attack[a], &defense[b]) - v43_interaction(&attack[b], &defense[a]);
            let pred = raw_anchor + bias[a] - bias[b] + matchup;
            let err = (pred - target).clamp(-V43_GRAD_CLIP, V43_GRAD_CLIP);

            bias[a] -= lr * (err + V43_BIAS_L2 * bias[a]);
            bias[b] += lr * (err - V43_BIAS_L2 * bias[b]);

            for dim in 0..V43_DIM {
                let aa = attack[a][dim];
                let ab = attack[b][dim];
                let da = defense[a][dim];
                let db = defense[b][dim];

                attack[a][dim] -= lr * (err * db + V43_VECTOR_L2 * aa);
                defense[b][dim] -= lr * (err * aa + V43_VECTOR_L2 * db);
                attack[b][dim] -= lr * (-err * da + V43_VECTOR_L2 * ab);
                defense[a][dim] -= lr * (-err * ab + V43_VECTOR_L2 * da);
            }
        }

        center_scores(&mut bias);
        v43_clamp_slice(&mut bias, -V43_BIAS_NUMERIC_CLIP, V43_BIAS_NUMERIC_CLIP);
        v43_clamp_matrix(&mut attack, -V43_VECTOR_NUMERIC_CLIP, V43_VECTOR_NUMERIC_CLIP);
        v43_clamp_matrix(&mut defense, -V43_VECTOR_NUMERIC_CLIP, V43_VECTOR_NUMERIC_CLIP);
    }

    let mut degree = vec![0usize; candidate_count];
    let mut residual_sum = vec![0.0f64; candidate_count];
    let mut matchup_sum = vec![0.0f64; candidate_count];

    for edge in edges {
        let a = edge.a;
        let b = edge.b;
        let target = target_sign * logit((edge.rate_a_to_b / 100.0).clamp(0.005, 0.995));
        let raw_anchor = ((raw_scores[a] - raw_scores[b]) / V43_RAW_LOGIT_SCALE)
            .clamp(-V43_RAW_ANCHOR_CLIP, V43_RAW_ANCHOR_CLIP);
        let matchup = v43_interaction(&attack[a], &defense[b]) - v43_interaction(&attack[b], &defense[a]);
        let pred = raw_anchor + bias[a] - bias[b] + matchup;
        let residual = (pred - target).abs();

        degree[a] += 1;
        degree[b] += 1;
        residual_sum[a] += residual;
        residual_sum[b] += residual;
        matchup_sum[a] += matchup;
        matchup_sum[b] -= matchup;
    }

    let raw_pair_scores = (0..candidate_count)
        .map(|idx| {
            let evidence = degree[idx] as f64 / (degree[idx] as f64 + V43_EVIDENCE_HALF_LIFE);
            let avg_matchup = if degree[idx] == 0 {
                0.0
            } else {
                matchup_sum[idx] / degree[idx] as f64
            };
            let residual_logit = bias[idx] + V43_MATCHUP_TO_SCALAR * avg_matchup;
            let residual_cqd = residual_logit * V43_RAW_LOGIT_SCALE * V43_RESIDUAL_GAIN * evidence;
            let raw_component = raw_mean + (raw_scores[idx] - raw_mean) * V43_RAW_SPREAD_GAIN;
            raw_component + residual_cqd
        })
        .collect::<Vec<_>>();

    // Orientation guard: do not allow the calibrated axis to become globally
    // opposite to raw CQD. This is deliberately a slope correction, not a
    // pairwise delta clamp; it still allows local reorder and large deltas.
    let slope_guarded_scores = v43_enforce_min_raw_slope(&raw_pair_scores, raw_scores, V43_MIN_RAW_SLOPE);

    // Anti-compression: if the calibrated score distribution becomes flatter
    // than raw CQD, stretch it back to at least raw spread.
    let pair_mean = mean(&slope_guarded_scores);
    let pair_std = stddev(&slope_guarded_scores, pair_mean).max(1e-6);
    let min_pair_std = raw_std * V43_MIN_OUTPUT_STD_GAIN;
    let spread = if pair_std < min_pair_std { min_pair_std / pair_std } else { 1.0 };
    let pair_scores = slope_guarded_scores
        .iter()
        .map(|score| round_to_3(pair_mean + (score - pair_mean) * spread))
        .collect::<Vec<_>>();

    let uncertainties = (0..candidate_count)
        .map(|idx| {
            if degree[idx] == 0 {
                return round_to_3(raw_std);
            }
            let evidence = degree[idx] as f64 / (degree[idx] as f64 + V43_EVIDENCE_HALF_LIFE);
            let avg_residual = residual_sum[idx] / degree[idx] as f64;
            round_to_3(avg_residual * V43_RAW_LOGIT_SCALE + (1.0 - evidence) * raw_std + 1.0 / (degree[idx] as f64).sqrt())
        })
        .collect();

    (pair_scores, uncertainties)
}

fn v43_target_orientation_sign(edges: &[PairEdge], raw_scores: &[f64]) -> f64 {
    if edges.is_empty() {
        return 1.0;
    }

    let mut xs = Vec::<f64>::new();
    let mut ys = Vec::<f64>::new();

    for edge in edges {
        let x = raw_scores[edge.a] - raw_scores[edge.b];
        let y = logit((edge.rate_a_to_b / 100.0).clamp(0.005, 0.995));
        if x.is_finite() && y.is_finite() && x.abs() > 1e-9 {
            xs.push(x);
            ys.push(y);
        }
    }

    if xs.len() < 3 {
        return 1.0;
    }

    let x_mean = mean(&xs);
    let y_mean = mean(&ys);
    let mut cov = 0.0;
    for (x, y) in xs.iter().zip(ys.iter()) {
        cov += (x - x_mean) * (y - y_mean);
    }

    if cov < 0.0 { -1.0 } else { 1.0 }
}

fn v43_enforce_min_raw_slope(scores: &[f64], raw_scores: &[f64], min_slope: f64) -> Vec<f64> {
    if scores.len() != raw_scores.len() || scores.len() < 2 {
        return scores.to_vec();
    }

    let raw_mean = mean(raw_scores);
    let score_mean = mean(scores);
    let mut var_raw = 0.0;
    let mut cov = 0.0;
    for (score, raw) in scores.iter().zip(raw_scores.iter()) {
        let xr = raw - raw_mean;
        var_raw += xr * xr;
        cov += xr * (score - score_mean);
    }

    if var_raw <= 1e-12 {
        return scores.to_vec();
    }

    let slope = cov / var_raw;
    if slope >= min_slope {
        return scores.to_vec();
    }

    let correction = min_slope - slope;
    scores
        .iter()
        .zip(raw_scores.iter())
        .map(|(score, raw)| score + correction * (raw - raw_mean))
        .collect()
}

fn v43_interaction(left: &[f64], right: &[f64]) -> f64 {
    left.iter().zip(right.iter()).map(|(a, b)| a * b).sum()
}

fn v43_deterministic_init(idx: usize, dim: usize, salt: u64) -> f64 {
    let mut x = (idx as u64 + 1)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (dim as u64 + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9)
        ^ salt;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let unit = (x as f64) / (u64::MAX as f64);
    unit * 2.0 - 1.0
}

fn v43_clamp_slice(values: &mut [f64], low: f64, high: f64) {
    for value in values {
        *value = value.clamp(low, high);
    }
}

fn v43_clamp_matrix(values: &mut [Vec<f64>], low: f64, high: f64) {
    for row in values {
        v43_clamp_slice(row, low, high);
    }
}


fn edge_degrees(candidate_count: usize, edges: &[PairEdge]) -> Vec<usize> {
    let mut degree = vec![0usize; candidate_count];
    for edge in edges {
        degree[edge.a] += 1;
        degree[edge.b] += 1;
    }
    degree
}

fn write_pair_stability(
    rows: &mut [LaneResultRow],
    candidates: &[Candidate],
    aggregate: &AggregateScores,
) {
    for (pos, candidate) in candidates.iter().enumerate() {
        let row = &mut rows[candidate.row_idx];
        row.raw_average_cqd = candidate.raw_score;
        row.pair_score = Some(aggregate.mean_scores[pos]);
        row.uncertainty = Some(aggregate.mean_uncertainty[pos]);
        row.raw_delta = Some(round_to_3(aggregate.mean_scores[pos] - candidate.raw_score));
        row.pair_score_std = Some(aggregate.score_std[pos]);
        row.pair_rank_std = Some(aggregate.rank_std[pos]);
        row.delta_std = Some(aggregate.delta_std[pos]);
        row.edge_count_mean = Some(aggregate.edge_count_mean[pos]);
        row.stability_flag = aggregate.stability_flags[pos].clone();
        row.selection_status = aggregate.stability_flags[pos].clone();
        row.marginal_value = None;
        row.constrained_rank = None;
    }
}

fn write_pair_scores(
    rows: &mut [LaneResultRow],
    candidates: &[Candidate],
    pair_scores: &[f64],
    uncertainties: &[f64],
) {
    for (pos, candidate) in candidates.iter().enumerate() {
        let row = &mut rows[candidate.row_idx];
        row.raw_average_cqd = candidate.raw_score;
        row.pair_score = Some(pair_scores[pos]);
        row.uncertainty = Some(uncertainties[pos]);
        row.raw_delta = Some(round_to_3(pair_scores[pos] - candidate.raw_score));
        row.selection_status = "candidate".to_string();
    }
}

fn sorted_candidate_order(pair_scores: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..pair_scores.len()).collect();
    order.sort_by(|&a, &b| pair_scores[b].total_cmp(&pair_scores[a]).then_with(|| a.cmp(&b)));
    order
}

fn ranks_from_scores(pair_scores: &[f64]) -> Vec<usize> {
    let order = sorted_candidate_order(pair_scores);
    let mut ranks = vec![0usize; pair_scores.len()];
    for (rank_idx, &pos) in order.iter().enumerate() {
        ranks[pos] = rank_idx + 1;
    }
    ranks
}

// v49 note: the old reorder_rows_by_pair_score() was intentionally removed.
// It rewrote row.rank after calibration, which made R-Rank mirror the P-Rank.
// Keep row.rank as the raw/default rank; sort presentation views by pair_rank instead.

fn constrained_selection_search(
    candidates: &[Candidate],
    nodes: &[RankNode],
    pair_scores: &[f64],
    limit: usize,
) -> Vec<usize> {
    let base_order = sorted_candidate_order(pair_scores);
    let mut best = greedy_constrained_selection_from_order(candidates, nodes, limit, &base_order);
    let mut best_total = selection_total(&best, pair_scores);

    for restart in 0..CONSTRAINED_SEARCH_RESTARTS {
        let order = restart_order(&base_order, pair_scores, restart);
        let mut current = greedy_constrained_selection_from_order(candidates, nodes, limit, &order);
        let mut current_total = selection_total(&current, pair_scores);

        if current.len() == limit && better_selection(current_total, &current, best_total, &best, pair_scores) {
            best = current.clone();
            best_total = current_total;
        }

        let mut no_improve = 0usize;
        let mut state = (restart as u64 + 1)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(candidates.len() as u64);

        for step in 0..CONSTRAINED_LOCAL_STEPS_PER_RESTART {
            if no_improve >= CONSTRAINED_NO_IMPROVE_LIMIT {
                break;
            }
            if candidates.is_empty() || order.is_empty() {
                break;
            }

            let forced = if step % 3 == 0 {
                order[step % order.len()]
            } else {
                next_index(&mut state, candidates.len())
            };

            let trial = force_include_and_repair(candidates, nodes, pair_scores, limit, &current, forced, &order);
            let trial_total = selection_total(&trial, pair_scores);

            if trial.len() == limit && better_selection(trial_total, &trial, current_total, &current, pair_scores) {
                current = trial;
                current_total = trial_total;
                no_improve = 0;

                if better_selection(current_total, &current, best_total, &best, pair_scores) {
                    best = current.clone();
                    best_total = current_total;
                }
            } else {
                no_improve += 1;
            }
        }
    }

    best.sort_by(|&a, &b| pair_scores[b].total_cmp(&pair_scores[a]).then_with(|| a.cmp(&b)));
    best.truncate(limit);
    best
}

fn greedy_constrained_selection_from_order(
    candidates: &[Candidate],
    nodes: &[RankNode],
    limit: usize,
    order: &[usize],
) -> Vec<usize> {
    let mut resources = SelectionResources::default();
    let mut selected = Vec::with_capacity(limit);

    for &pos in order {
        if selected.len() >= limit {
            break;
        }
        if try_add_selected(&mut resources, candidates, nodes, pos) {
            selected.push(pos);
        }
    }

    selected
}

fn force_include_and_repair(
    candidates: &[Candidate],
    nodes: &[RankNode],
    pair_scores: &[f64],
    limit: usize,
    current: &[usize],
    forced: usize,
    order: &[usize],
) -> Vec<usize> {
    let mut resources = SelectionResources::default();
    let mut selected = Vec::with_capacity(limit);
    let mut selected_set = HashSet::<usize>::new();

    if !try_add_selected(&mut resources, candidates, nodes, forced) {
        return current.to_vec();
    }
    selected.push(forced);
    selected_set.insert(forced);

    let mut keep_order: Vec<usize> = current.iter().copied().filter(|&pos| pos != forced).collect();
    keep_order.sort_by(|&a, &b| pair_scores[b].total_cmp(&pair_scores[a]).then_with(|| a.cmp(&b)));

    for pos in keep_order {
        if selected.len() >= limit {
            break;
        }
        if selected_set.contains(&pos) {
            continue;
        }
        if try_add_selected(&mut resources, candidates, nodes, pos) {
            selected.push(pos);
            selected_set.insert(pos);
        }
    }

    for &pos in order {
        if selected.len() >= limit {
            break;
        }
        if selected_set.contains(&pos) {
            continue;
        }
        if try_add_selected(&mut resources, candidates, nodes, pos) {
            selected.push(pos);
            selected_set.insert(pos);
        }
    }

    selected
}

fn restart_order(base_order: &[usize], pair_scores: &[f64], restart: usize) -> Vec<usize> {
    let mut order = base_order.to_vec();
    if order.len() <= 1 {
        return order;
    }

    match restart % 4 {
        0 => order,
        1 => {
            let shift = (restart / 4 + 1) % order.len();
            order.rotate_left(shift);
            order
        }
        2 => {
            let mut state = (restart as u64 + 17)
                .wrapping_mul(0xBF58_476D_1CE4_E5B9)
                .wrapping_add(order.len() as u64);
            for i in (1..order.len()).rev() {
                let j = next_index(&mut state, i + 1);
                order.swap(i, j);
            }
            order
        }
        _ => {
            order.sort_by(|&a, &b| {
                let score_a = pair_scores[a] + deterministic_jitter(a, restart);
                let score_b = pair_scores[b] + deterministic_jitter(b, restart);
                score_b.total_cmp(&score_a).then_with(|| a.cmp(&b))
            });
            order
        }
    }
}

fn selection_total(selected: &[usize], pair_scores: &[f64]) -> f64 {
    selected.iter().map(|&pos| pair_scores[pos]).sum()
}

fn better_selection(
    candidate_total: f64,
    candidate: &[usize],
    incumbent_total: f64,
    incumbent: &[usize],
    pair_scores: &[f64],
) -> bool {
    if candidate.len() > incumbent.len() {
        return true;
    }
    if candidate.len() < incumbent.len() {
        return false;
    }
    if candidate_total > incumbent_total + CONSTRAINED_SCORE_EPS {
        return true;
    }
    if candidate_total + CONSTRAINED_SCORE_EPS < incumbent_total {
        return false;
    }

    let mut candidate_sorted = candidate.to_vec();
    let mut incumbent_sorted = incumbent.to_vec();
    candidate_sorted.sort_by(|&a, &b| pair_scores[b].total_cmp(&pair_scores[a]).then_with(|| a.cmp(&b)));
    incumbent_sorted.sort_by(|&a, &b| pair_scores[b].total_cmp(&pair_scores[a]).then_with(|| a.cmp(&b)));
    candidate_sorted < incumbent_sorted
}

fn next_index(state: &mut u64, upper: usize) -> usize {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    ((*state >> 32) as usize) % upper.max(1)
}

fn deterministic_jitter(pos: usize, restart: usize) -> f64 {
    let mut x = (pos as u64 + 1)
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (restart as u64 + 11).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let unit = (x as f64) / (u64::MAX as f64);
    (unit - 0.5) * 0.025
}

fn compute_marginal_values(
    candidates: &[Candidate],
    nodes: &[RankNode],
    pair_scores: &[f64],
    pair_rank_order: &[usize],
    selected: &[usize],
    base_total: f64,
    selection_size: usize,
) -> Vec<Option<f64>> {
    if selected.len() < selection_size {
        return vec![None; candidates.len()];
    }

    let mut out = Vec::with_capacity(candidates.len());
    for forced in 0..candidates.len() {
        let mut resources = SelectionResources::default();
        let mut total = 0.0;
        let mut count = 0usize;

        if try_add_selected(&mut resources, candidates, nodes, forced) {
            total += pair_scores[forced];
            count += 1;
        } else {
            out.push(None);
            continue;
        }

        for &pos in pair_rank_order {
            if count >= selection_size {
                break;
            }
            if pos == forced {
                continue;
            }
            if try_add_selected(&mut resources, candidates, nodes, pos) {
                total += pair_scores[pos];
                count += 1;
            }
        }

        if count >= selection_size {
            out.push(Some(total - base_total));
        } else {
            out.push(None);
        }
    }

    out
}

fn try_add_selected(
    resources: &mut SelectionResources,
    candidates: &[Candidate],
    nodes: &[RankNode],
    pos: usize,
) -> bool {
    let candidate = &candidates[pos];
    let group = &nodes[candidate.node_idx].group;

    if resources.team_count.get(&candidate.root_team).copied().unwrap_or(0) >= TEAM_LIMIT {
        return false;
    }

    if group.members.iter().any(|member| resources.used_members.contains(member)) {
        return false;
    }

    *resources.team_count.entry(candidate.root_team.clone()).or_insert(0) += 1;
    for member in &group.members {
        resources.used_members.insert(member.clone());
    }
    true
}

fn reorder_rows_by_constrained_value(rows: &mut Vec<LaneResultRow>) {
    rows.sort_by(|a, b| {
        let a_selected = a.constrained_rank.is_some();
        let b_selected = b.constrained_rank.is_some();

        b_selected.cmp(&a_selected)
            .then_with(|| match (a.constrained_rank, b.constrained_rank) {
                (Some(ar), Some(br)) => ar.cmp(&br),
                _ => std::cmp::Ordering::Equal,
            })
            .then_with(|| option_desc_cmp(a.marginal_value, b.marginal_value))
            .then_with(|| option_desc_cmp(a.pair_score, b.pair_score))
            .then_with(|| b.average_cqd.total_cmp(&a.average_cqd))
            .then_with(|| a.group_id.cmp(&b.group_id))
    });

    // v49: never rewrite row.rank here; it is the stable R-Rank.
    // Presentation order may change, but saved rank remains raw/default rank.
}

fn option_desc_cmp(a: Option<f64>, b: Option<f64>) -> std::cmp::Ordering {
    match (a, b) {
        (Some(x), Some(y)) => y.total_cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

fn resolve_pair_workers(requested_outer_workers: usize, total: usize) -> usize {
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }

    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(1)
        .min(total.max(1))
}

fn ordered_pair(a: usize, b: usize) -> (usize, usize) {
    if a <= b { (a, b) } else { (b, a) }
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let var = values.iter().map(|v| {
        let d = v - mean;
        d * d
    }).sum::<f64>() / values.len() as f64;
    var.sqrt()
}

fn center_scores(scores: &mut [f64]) {
    let mean = mean(scores);
    for score in scores {
        *score -= mean;
    }
}

fn logit(p: f64) -> f64 {
    (p / (1.0 - p)).ln()
}

fn round_to_3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}


fn make_bag_seed(lane_size: usize, final_round: usize, seed_idx: usize, candidate_count: usize, base_edges: usize) -> u64 {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    x ^= (lane_size as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= (final_round as u64).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= (seed_idx as u64).wrapping_mul(0xD6E8_FD9D_2D62_9B85);
    x ^= (candidate_count as u64).rotate_left(17);
    x ^= (base_edges as u64).rotate_right(11);
    x
}

#[derive(Debug, Clone)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn usize(&mut self, upper: usize) -> usize {
        if upper <= 1 {
            return 0;
        }
        (self.next_u64() as usize) % upper
    }
}

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

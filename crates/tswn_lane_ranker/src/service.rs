use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::process::Command;
use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::thread;

use anyhow::Context;
use tokio::task;

use crate::db::{Db, InsertGroupOutcome};
use crate::model::{
    AddGroupsRequest, AddGroupsResponse, AddWinratesRequest, AddWinratesResponse, AddedWinrateRow,
    BlockGroupRequest, BlockGroupResponse, BlockGroupsByTextRequest, BlockGroupsByTextResponse,
    ConstrainedSelectionRequest, ConstrainedSelectionResponse, IgnoredGroup, IgnoredWinratePair, JobId,
    MergeTeamsRequest, MergeTeamsResponse, RecomputeLaneResponse, StoredGroup,
    TargetGenerationRequest, TargetGenerationResponse, TargetGenerationRow, TargetGenerationSummary,
    TargetReferenceAuditRow,
};
use crate::parser::{parse_group, parse_member_team};
use crate::pairwise::{calibrate_saved_lane_results, default_selection_cqd_threshold, validate_saved_pair_strength_results};
use crate::ranker::{RankerConfig, recompute_lane_until_stable};
use crate::winrate::compute_rate_without_db;

const TARGET_MILP_SOLVER: &str = include_str!("../tools/target_milp_solver.py");

#[derive(Clone)]
pub struct AppService {
    pub db: Db,
    pub config: RankerConfig,
}

impl AppService {
    pub fn new(db: Db, config: RankerConfig) -> Self {
        Self { db, config }
    }

    pub fn add_groups(&self, req: AddGroupsRequest) -> anyhow::Result<AddGroupsResponse> {
        let AddGroupsRequest {
            groups,
            outer_workers,
            inner_workers,
            skip_archived,
        } = req;
        let config = self.config_with_run_options(
            outer_workers,
            inner_workers,
            skip_archived,
        )?;

        let mut added = Vec::new();
        let mut duplicated = Vec::new();
        let mut ignored = Vec::new();
        let mut dirty_lanes = BTreeSet::new();

        for raw in groups {
            let raw = raw.trim().to_string();
            if raw.is_empty() {
                continue;
            }

            let parsed = match parse_group(&raw) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored.push(IgnoredGroup {
                        raw,
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            if config.skip_archived && self.db.is_group_archived_by_canonical(&parsed.canonical)? {
                ignored.push(IgnoredGroup {
                    raw,
                    reason: "archived combination skipped because 不跑封存组合 is enabled".to_string(),
                });
                continue;
            }

            match self.db.insert_group(&parsed)? {
                InsertGroupOutcome::Added(_) => {
                    dirty_lanes.insert(parsed.lane_size);
                    added.push(parsed.canonical);
                }
                InsertGroupOutcome::Duplicated(_) => {
                    // 重新导入已有组号时也触发该赛道重算，避免已有数据但结果/状态缺失时无法手动刷新。
                    dirty_lanes.insert(parsed.lane_size);
                    duplicated.push(parsed.canonical);
                }
            }
        }

        let queued_lanes = self.queue_recompute_lanes_with_config(
            dirty_lanes.into_iter().collect(),
            config,
        )?;

        Ok(AddGroupsResponse {
            added,
            duplicated,
            ignored,
            queued_lanes,
        })
    }


    pub fn add_manual_winrates(&self, req: AddWinratesRequest) -> anyhow::Result<AddWinratesResponse> {
        let AddWinratesRequest {
            groups,
            outer_workers,
            inner_workers,
        } = req;

        let config = self.config_with_run_options(outer_workers, inner_workers, None)?;
        let samples = config.win_rate_samples;
        if samples == 0 {
            anyhow::bail!("服务端 win_rate_samples 必须是正整数");
        }

        let lines: Vec<String> = groups
            .into_iter()
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect();

        let requested_pairs = (lines.len() + 1) / 2;
        let mut ignored_pairs = Vec::new();
        let mut pairs = Vec::new();

        for (chunk_idx, chunk) in lines.chunks(2).enumerate() {
            let pair_index = chunk_idx + 1;
            if chunk.len() < 2 {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: chunk.get(0).cloned(),
                    group_b: None,
                    reason: "输入行数为奇数，最后一行没有对手；该 pair 未测也未写入数据库".to_string(),
                });
                continue;
            }

            let raw_a = chunk[0].trim().to_string();
            let raw_b = chunk[1].trim().to_string();

            let parsed_a = match parse_group(&raw_a) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored_pairs.push(IgnoredWinratePair {
                        pair_index,
                        group_a: Some(raw_a),
                        group_b: Some(raw_b),
                        reason: format!("第一行组合无法解析：{err}"),
                    });
                    continue;
                }
            };
            let parsed_b = match parse_group(&raw_b) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored_pairs.push(IgnoredWinratePair {
                        pair_index,
                        group_a: Some(raw_a),
                        group_b: Some(raw_b),
                        reason: format!("第二行组合无法解析：{err}"),
                    });
                    continue;
                }
            };

            if parsed_a.lane_size != parsed_b.lane_size {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(parsed_a.canonical),
                    group_b: Some(parsed_b.canonical),
                    reason: format!(
                        "两行组合人数不同：{} vs {}；该 pair 未写入数据库",
                        parsed_a.lane_size, parsed_b.lane_size
                    ),
                });
                continue;
            }

            let Some(group_a) = self.db.find_stored_group_by_canonical(&parsed_a.canonical)? else {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(parsed_a.canonical),
                    group_b: Some(parsed_b.canonical),
                    reason: "第一行组合不在数据库中；该 pair 未测也未写入数据库".to_string(),
                });
                continue;
            };
            let Some(group_b) = self.db.find_stored_group_by_canonical(&parsed_b.canonical)? else {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(group_a.canonical.clone()),
                    group_b: Some(parsed_b.canonical),
                    reason: "第二行组合不在数据库中；该 pair 未测也未写入数据库".to_string(),
                });
                continue;
            };

            if group_a.id == group_b.id {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(group_a.canonical.clone()),
                    group_b: Some(group_b.canonical.clone()),
                    reason: "同一个组合不能手动测自身胜率；该 pair 未写入数据库".to_string(),
                });
                continue;
            }

            pairs.push(ManualWinratePair {
                pair_index,
                group_a,
                group_b,
            });
        }

        let total = pairs.len();
        if total == 0 {
            return Ok(AddWinratesResponse {
                requested_pairs,
                computed_pairs: 0,
                stored_pairs: 0,
                ignored_pairs,
                results: Vec::new(),
                samples,
                outer_workers: 0,
                mode: "no_valid_pairs".to_string(),
            });
        }

        let workers = resolve_manual_winrate_workers(config.outer_workers, total);
        let mode = if config.outer_workers == 0 { "dynamic_queue" } else { "static_chunks" }.to_string();
        let pairs = Arc::new(pairs);
        let done = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);

        if config.outer_workers == 0 {
            let next_pair = Arc::new(AtomicUsize::new(0));
            for _ in 0..workers {
                let pairs = Arc::clone(&pairs);
                let next_pair = Arc::clone(&next_pair);
                let done = Arc::clone(&done);
                let inner_workers = config.inner_workers;
                handles.push(thread::spawn(move || -> anyhow::Result<Vec<ComputedManualWinrate>> {
                    let mut computed = Vec::new();
                    loop {
                        let idx = next_pair.fetch_add(1, Ordering::Relaxed);
                        let Some(pair) = pairs.get(idx) else {
                            break;
                        };
                        let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                        done.fetch_add(1, Ordering::Relaxed);
                        computed.push(ComputedManualWinrate::from_pair(pair, rate, samples));
                    }
                    Ok(computed)
                }));
            }
        } else {
            for worker_id in 0..workers {
                let pairs = Arc::clone(&pairs);
                let done = Arc::clone(&done);
                let inner_workers = config.inner_workers;
                let start = total * worker_id / workers;
                let end = total * (worker_id + 1) / workers;
                handles.push(thread::spawn(move || -> anyhow::Result<Vec<ComputedManualWinrate>> {
                    let mut computed = Vec::with_capacity(end.saturating_sub(start));
                    for idx in start..end {
                        let Some(pair) = pairs.get(idx) else {
                            break;
                        };
                        let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                        done.fetch_add(1, Ordering::Relaxed);
                        computed.push(ComputedManualWinrate::from_pair(pair, rate, samples));
                    }
                    Ok(computed)
                }));
            }
        }

        let mut computed = Vec::<ComputedManualWinrate>::with_capacity(total);
        for handle in handles {
            let part = handle.join().expect("manual winrate worker thread panicked")?;
            computed.extend(part);
        }
        computed.sort_by_key(|row| row.pair_index);

        let db_rates: Vec<_> = computed
            .iter()
            .map(|row| (row.group_a_id, row.group_b_id, row.win_rate_a))
            .collect();
        self.db.save_rate_pairs_bulk(&db_rates, samples)?;

        let results = computed
            .into_iter()
            .map(|row| AddedWinrateRow {
                pair_index: row.pair_index,
                group_a: row.group_a,
                group_b: row.group_b,
                group_a_id: row.group_a_id,
                group_b_id: row.group_b_id,
                lane_size: row.lane_size,
                win_rate_a: row.win_rate_a,
                win_rate_b: 100.0 - row.win_rate_a,
                samples: row.samples,
                stored: true,
            })
            .collect::<Vec<_>>();

        Ok(AddWinratesResponse {
            requested_pairs,
            computed_pairs: results.len(),
            stored_pairs: results.len(),
            ignored_pairs,
            results,
            samples,
            outer_workers: workers,
            mode,
        })
    }

    pub fn set_group_blocked(&self, group_id: i64, blocked: bool, req: BlockGroupRequest) -> anyhow::Result<BlockGroupResponse> {
        let config = self.config_with_run_options(
            req.outer_workers,
            req.inner_workers,
            req.skip_archived,
        )?;
        let Some((lane_size, canonical)) = self.db.set_group_blocked(group_id, blocked)? else {
            anyhow::bail!("group id {group_id} not found");
        };

        let queued_lanes = self.queue_recompute_lanes_with_config(vec![lane_size], config)?;
        Ok(BlockGroupResponse {
            group_id,
            lane_size,
            canonical,
            blocked,
            queued_lanes,
        })
    }

    pub fn set_groups_blocked_by_text(&self, blocked: bool, req: BlockGroupsByTextRequest) -> anyhow::Result<BlockGroupsByTextResponse> {
        let BlockGroupsByTextRequest {
            groups,
            outer_workers,
            inner_workers,
            skip_archived,
        } = req;
        let config = self.config_with_run_options(
            outer_workers,
            inner_workers,
            skip_archived,
        )?;

        let mut blocked_groups = BTreeSet::new();
        let mut unblocked_groups = BTreeSet::new();
        let mut ignored = Vec::new();
        let mut dirty_lanes = BTreeSet::new();

        for raw in groups {
            let raw = raw.trim().to_string();
            if raw.is_empty() {
                continue;
            }

            let parsed = match parse_group(&raw) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored.push(IgnoredGroup {
                        raw,
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            let Some((group_id, _, _)) = self.db.find_group_by_canonical(&parsed.canonical)? else {
                ignored.push(IgnoredGroup {
                    raw,
                    reason: "未找到这个组合，请先加入组合".to_string(),
                });
                continue;
            };

            let Some((changed_lane_size, changed_canonical)) = self.db.set_group_blocked(group_id, blocked)? else {
                ignored.push(IgnoredGroup {
                    raw,
                    reason: "组合更新前已经不存在".to_string(),
                });
                continue;
            };

            dirty_lanes.insert(changed_lane_size);
            if blocked {
                blocked_groups.insert(changed_canonical);
            } else {
                unblocked_groups.insert(changed_canonical);
            }
        }

        let queued_lanes = self.queue_recompute_lanes_with_config(
            dirty_lanes.into_iter().collect(),
            config,
        )?;

        Ok(BlockGroupsByTextResponse {
            blocked: blocked_groups.into_iter().collect(),
            unblocked: unblocked_groups.into_iter().collect(),
            ignored,
            queued_lanes,
        })
    }

    pub fn merge_teams(&self, req: MergeTeamsRequest) -> anyhow::Result<MergeTeamsResponse> {
        let config = self.config_with_run_options(
            req.outer_workers,
            req.inner_workers,
            req.skip_archived,
        )?;

        let mut dsu = self.db.load_team_dsu()?;
        let root = dsu.union(req.x.trim(), req.y.trim());
        self.db.save_team_dsu(&dsu)?;

        let lanes = self.db.all_nonempty_lanes()?;
        let queued_lanes = self.queue_recompute_lanes_with_config(lanes, config)?;

        Ok(MergeTeamsResponse {
            merged_root: root,
            queued_lanes,
        })
    }

    pub fn queue_recompute_lanes(&self, lanes: Vec<usize>) -> anyhow::Result<Vec<usize>> {
        self.queue_recompute_lanes_with_config(lanes, self.config.clone())
    }

    pub fn queue_recompute_lanes_with_config(
        &self,
        lanes: Vec<usize>,
        config: RankerConfig,
    ) -> anyhow::Result<Vec<usize>> {
        let mut queued = Vec::new();

        for lane in lanes {
            let job_id = self.db.create_job(lane, "recompute")?;
            self.db.set_lane_status(
                lane,
                "queued",
                self.db.load_groups_by_lane_for_run(lane, config.skip_archived)?.len(),
            )?;
            self.db.set_lane_progress(
                lane,
                "queued",
                0,
                config.total_rounds,
                0,
                0,
                0,
                &format!(
                    "queued job #{job_id}, stickiness={}, outer_workers={}, inner_threads={}, skip_archived={}",
                    config.effective_stickiness(lane),
                    if config.outer_workers == 0 { "dynamic_auto".to_string() } else { format!("static({})", config.outer_workers) },
                    if config.inner_workers == 0 { "auto(0)".to_string() } else { config.inner_workers.to_string() },
                    config.skip_archived
                ),
            )?;

            spawn_recompute_job(self.db.clone(), config.clone(), lane, job_id);
            queued.push(lane);
        }

        Ok(queued)
    }

    pub fn queue_recompute_lane(
        &self,
        lane: usize,
        stickiness: Option<usize>,
        outer_workers: Option<usize>,
        inner_workers: Option<u32>,
        skip_archived: Option<bool>,
    ) -> anyhow::Result<RecomputeLaneResponse> {
        if matches!(stickiness, Some(0)) {
            anyhow::bail!("stickiness must be a positive integer");
        }

        let mut config = self.config_with_run_options(
            outer_workers,
            inner_workers,
            skip_archived,
        )?;
        config.stickiness = stickiness;
        let queued_lanes = self.queue_recompute_lanes_with_config(vec![lane], config)?;
        Ok(RecomputeLaneResponse { queued_lanes })
    }

    pub fn queue_constrained_selection_lane(
        &self,
        lane: usize,
        req: ConstrainedSelectionRequest,
    ) -> anyhow::Result<ConstrainedSelectionResponse> {
        let mut config = self.config_with_run_options(
            req.outer_workers,
            req.inner_workers,
            None,
        )?;
        config.inner_workers = 1;

        let threshold = req.raw_score_threshold
            .or(req.cqd_threshold)
            .unwrap_or_else(|| default_selection_cqd_threshold(lane));
        if !threshold.is_finite() || !(0.0..=100.0).contains(&threshold) {
            anyhow::bail!("校准池 Raw Score 阈值必须是 0 到 100 之间的数字");
        }

        let group_count = self.db.lane_results(lane)?.len();
        if group_count == 0 {
            anyhow::bail!("该赛道还没有结果；请先完成一次默认重算");
        }
        let job_id = self.db.create_job(lane, "calibration")?;

        self.db.set_lane_status(lane, "calibrating", group_count)?;
        self.db.set_lane_progress(
            lane,
            "calibration_queued",
            0,
            config.total_rounds,
            0,
            group_count,
            0,
            &format!(
                "queued calibration job #{job_id}, raw_score_threshold={threshold:.3}, outer_workers={}, inner_threads=1",
                if config.outer_workers == 0 { "dynamic_auto".to_string() } else { format!("static({})", config.outer_workers) },
            ),
        )?;

        spawn_constrained_selection_job(self.db.clone(), config, lane, job_id, threshold);
        Ok(ConstrainedSelectionResponse {
            queued_lanes: vec![lane],
            raw_score_threshold: threshold,
            cqd_threshold: threshold,
        })
    }


    pub fn validate_pair_strength_lane(
        &self,
        lane: usize,
        req: ConstrainedSelectionRequest,
    ) -> anyhow::Result<serde_json::Value> {
        let mut config = self.config_with_run_options(
            req.outer_workers,
            req.inner_workers,
            None,
        )?;
        config.inner_workers = 1;

        let threshold = req.raw_score_threshold
            .or(req.cqd_threshold)
            .unwrap_or_else(|| default_selection_cqd_threshold(lane));
        if !threshold.is_finite() || !(0.0..=100.0).contains(&threshold) {
            anyhow::bail!("Pair 验证 Raw Score 阈值必须是 0 到 100 之间的数字");
        }

        validate_saved_pair_strength_results(&self.db, lane, &config, threshold)
    }


    pub fn generate_lane_targets(
        &self,
        lane: usize,
        req: TargetGenerationRequest,
    ) -> anyhow::Result<TargetGenerationResponse> {
        const TARGET_TOTAL: usize = 50;
        const DEFAULT_FIXED_MAIN_COUNT: usize = 40;
        const PLAYER_CAP: usize = 5;
        const TARGET_PLAYER_REPEAT_CAP: usize = 1;
        const SEED_BEAM_WIDTH: usize = 18;
        const LNS_BEAM_WIDTH: usize = 14;
        const SWAP_PASSES: usize = 10;
        const LNS_ROUNDS: usize = 42;
        const WORST_GUIDED_REFS: usize = 16;
        const TWO_SWAP_REMOVE_SHORTLIST: usize = 10;
        const TWO_SWAP_ADD_SHORTLIST: usize = 32;
        const TWO_SWAP_PASSES: usize = 2;
        const FEASIBILITY_BEAM_WIDTH: usize = 512;
        const TARGET_WEIGHT_MIN: f64 = 0.01;
        const TARGET_WEIGHT_MAX: f64 = 10.0;
        const PLAYER_WEIGHT_CAP: f64 = 5.0;

        let cqd_threshold = req.cqd_threshold.unwrap_or(49.0);
        if !cqd_threshold.is_finite() || !(0.0..=100.0).contains(&cqd_threshold) {
            anyhow::bail!("靶子 C-Score 阈值必须是 0 到 100 之间的数字");
        }

        let fixed_main_count = req.fixed_main_count.unwrap_or(DEFAULT_FIXED_MAIN_COUNT);
        if fixed_main_count > TARGET_TOTAL {
            anyhow::bail!(
                "固定主榜数量不能超过总靶子数 {TARGET_TOTAL}：当前 fixed_main_count={}",
                fixed_main_count
            );
        }

        let rows = self.db.lane_results(lane)?;
        if rows.is_empty() {
            anyhow::bail!("该赛道还没有结果；请先读取/重算赛道");
        }

        let mut rate_map = self.db.lane_rate_map(lane)?;
        if rate_map.is_empty() {
            anyhow::bail!("该赛道没有 group_rates；无法用胜率对生成靶子");
        }

        let mut candidates: Vec<TargetCandidate> = rows
            .into_iter()
            .filter(|row| !row.is_blocked)
            .filter(|row| row.pair_score.map(|x| x.is_finite()).unwrap_or(false))
            .map(TargetCandidate::from_row)
            .collect();

        if candidates.len() < TARGET_TOTAL {
            anyhow::bail!(
                "可用校准候选不足 {TARGET_TOTAL} 个：当前只有 {} 个非 blocked 且有 C-Score 的组合",
                candidates.len()
            );
        }

        candidates.sort_by(compare_target_candidates);

        // 主榜口径：Correct 排序后按玩家身份 greedy。fixed_main_count 只定义最终锁定前缀，
        // 不再决定优化器从空集还是从强完整解启动。
        let main_order = greedy_main_order(&candidates, None);
        let reference_limit = if lane == 1 { 100 } else { 200 };
        let reference_indices: Vec<usize> = main_order.iter().copied().take(reference_limit).collect();
        if reference_indices.len() < 2 {
            anyhow::bail!("主榜 reference 组合不足 2 个；无法生成 profile 靶子");
        }

        let threshold_main_order: Vec<usize> = main_order
            .iter()
            .copied()
            .filter(|&idx| candidates[idx].correct_score() >= cqd_threshold)
            .collect();

        if threshold_main_order.len() < fixed_main_count {
            anyhow::bail!(
                "C-Score ≥ {:.3} 的主榜 greedy 组合不足 {} 个：当前只有 {} 个",
                cqd_threshold,
                fixed_main_count,
                threshold_main_order.len()
            );
        }

        let locked_prefix: Vec<usize> = threshold_main_order
            .iter()
            .copied()
            .take(fixed_main_count)
            .collect();

        let fill_pool: Vec<usize> = (0..candidates.len())
            .filter(|&idx| candidates[idx].correct_score() >= cqd_threshold)
            .collect();

        if fill_pool.len() < TARGET_TOTAL {
            anyhow::bail!(
                "C-Score ≥ {:.3} 的可用候选不足 {TARGET_TOTAL} 个：当前只有 {} 个",
                cqd_threshold,
                fill_pool.len()
            );
        }

        // 缺失边是可测数据，不做 50% 填补、不在 objective 里当成常规误差。
        // 这里先补齐 reference × candidate pool 的胜率矩阵，再进入优化。
        let missing_rate_pairs = collect_missing_target_rate_pairs(
            &reference_indices,
            &fill_pool,
            &candidates,
            &rate_map,
        );
        if !missing_rate_pairs.is_empty() {
            let computed = compute_missing_target_rates(
                &self.db,
                &self.config,
                &missing_rate_pairs,
                &candidates,
            )?;
            self.db.save_rate_pairs_bulk(&computed, self.config.win_rate_samples)?;
            for (a, b, rate) in computed {
                rate_map.insert((a, b), rate);
                rate_map.insert((b, a), 100.0 - rate);
            }
        }

        let milp_solution = run_target_milp_solver(
            &locked_prefix,
            &fill_pool,
            TARGET_TOTAL,
            TARGET_PLAYER_REPEAT_CAP,
            TARGET_WEIGHT_MIN,
            TARGET_WEIGHT_MAX,
            PLAYER_WEIGHT_CAP,
            &reference_indices,
            &candidates,
            &rate_map,
        )
        .with_context(|| {
            format!(
                "weighted MILP 靶子生成失败：重复号上限={}、旧玩家计数上限={}、玩家权重上限={:.3}、C-Score≥{:.3}、fixed_main_count={}、target_total={TARGET_TOTAL}",
                TARGET_PLAYER_REPEAT_CAP,
                PLAYER_CAP,
                PLAYER_WEIGHT_CAP,
                cqd_threshold,
                fixed_main_count
            )
        })?;

        let selected = milp_solution.indices;
        let target_weights = milp_solution.weights;
        if selected.len() != TARGET_TOTAL || target_weights.len() != TARGET_TOTAL {
            anyhow::bail!(
                "weighted MILP 靶子生成返回数量错误：期望 {TARGET_TOTAL} 个，实际 selected={} weights={}",
                selected.len(),
                target_weights.len()
            );
        }
        if let Some(error) = target_solution_feasibility_error(&selected, &candidates, TARGET_PLAYER_REPEAT_CAP) {
            anyhow::bail!("weighted MILP 靶子生成返回非法解：{error}");
        }
        if let Some(error) = target_weight_feasibility_error(
            &selected,
            &target_weights,
            &candidates,
            TARGET_WEIGHT_MIN,
            TARGET_WEIGHT_MAX,
            TARGET_TOTAL as f64,
            PLAYER_WEIGHT_CAP,
        ) {
            anyhow::bail!("weighted MILP 靶子生成返回非法权重：{error}");
        }
        for &locked in &locked_prefix {
            if !selected.contains(&locked) {
                anyhow::bail!(
                    "weighted MILP 靶子生成返回解缺少锁定靶子 group_id={}",
                    candidates[locked].row.group_id
                );
            }
        }

        let final_obj = target_objective_weighted(
            &reference_indices,
            &selected,
            &target_weights,
            &candidates,
            &rate_map,
        );
        let reference_audit_rows =
            target_reference_audit_rows_weighted(&reference_indices, &selected, &target_weights, &candidates, &rate_map, &final_obj);
        let audit_stats = audit_diff_stats(&reference_audit_rows);
        let mut rows = Vec::with_capacity(selected.len());
        let target_config_text = build_target_config_text(&selected, &target_weights, &candidates);
        let target_weight_sum = target_weights.iter().copied().sum::<f64>();
        let target_weight_min = target_weights.iter().copied().fold(f64::INFINITY, f64::min);
        let target_weight_max = target_weights.iter().copied().fold(f64::NEG_INFINITY, f64::max);

        for (rank, &idx) in selected.iter().enumerate() {
            let target_weight = target_weights[rank];
            let candidate = &candidates[idx];
            let (avg_reference_winrate, reference_rate_count) =
                average_reference_winrate_for_target(&reference_indices, idx, &candidates, &rate_map);
            let raw_rank = candidate.row.rank;
            let correct_rank = candidate.row.pair_rank;
            let delta_rank = correct_rank.map(|c| raw_rank as i64 - c as i64);
            rows.push(TargetGenerationRow {
                target_rank: rank + 1,
                target_weight,
                phase: if rank < fixed_main_count {
                    "fixed_main_prefix".to_string()
                } else {
                    "weighted_milp_fill".to_string()
                },
                group_id: candidate.row.group_id,
                canonical: candidate.row.canonical.clone(),
                team_name: candidate.row.team_name.clone(),
                root_team_name: candidate.row.root_team_name.clone(),
                correct_rank,
                correct_score: candidate.correct_score(),
                raw_rank,
                raw_score: candidate.row.raw_average_cqd,
                delta_rank,
                selection_status: candidate.row.selection_status.clone(),
                type_label: candidate.row.type_label.clone(),
                simple_type_label: candidate.row.simple_type_label.clone(),
                average_reference_winrate: avg_reference_winrate,
                reference_rate_count,
                player_keys: candidate.player_keys.clone(),
            });
        }

        Ok(TargetGenerationResponse {
            summary: TargetGenerationSummary {
                lane_size: lane,
                target_count: rows.len(),
                fixed_main_count,
                optimized_count: rows.len().saturating_sub(fixed_main_count),
                player_cap: TARGET_PLAYER_REPEAT_CAP,
                target_weight_sum,
                target_weight_min,
                target_weight_max,
                player_weight_cap: PLAYER_WEIGHT_CAP,
                cqd_threshold,
                reference_limit,
                reference_count: reference_indices.len(),
                candidate_count: fill_pool.len(),
                objective_mse: final_obj.mse,
                objective_corr: final_obj.corr,
                reference_avg_winrate_mean: final_obj.avg_mean,
                reference_avg_winrate_std: final_obj.avg_std,
                reference_c_score_mean: final_obj.score_mean,
                reference_c_score_std: final_obj.score_std,
                audit_reference_rows: audit_stats.count,
                audit_mean_diff: audit_stats.mean_diff,
                audit_mean_abs_diff: Some(final_obj.mean_abs_diff),
                audit_max_abs_diff: Some(final_obj.max_abs_diff),
                audit_rmse: Some(final_obj.rmse),
                audit_p95_abs_diff: Some(final_obj.p95_abs_diff),
            },
            target_config_text,
            rows,
            reference_audit_rows,
        })
    }

    fn config_with_run_options(
        &self,
        outer_workers: Option<usize>,
        _inner_workers: Option<u32>,
        skip_archived: Option<bool>,
    ) -> anyhow::Result<RankerConfig> {
        let mut config = self.config.clone();
        if let Some(outer_workers) = outer_workers {
            config.outer_workers = outer_workers;
        }
        // Inner worker 固定写死为 1；旧版请求里的 inner_workers 字段会被兼容但忽略。
        config.inner_workers = 1;
        if let Some(skip_archived) = skip_archived {
            config.skip_archived = skip_archived;
        }
        Ok(config)
    }
}


#[derive(Debug, Clone)]
struct TargetCandidate {
    row: crate::model::LaneResultRow,
    player_keys: Vec<String>,
}

impl TargetCandidate {
    fn from_row(row: crate::model::LaneResultRow) -> Self {
        let player_keys = merged_player_keys(&row);
        Self { row, player_keys }
    }

    fn correct_score(&self) -> f64 {
        self.row.pair_score.unwrap_or(f64::NEG_INFINITY)
    }
}

#[derive(Debug, Clone)]
struct TargetObjective {
    /// Mean squared error after the affine Chebyshev alignment; kept as a late tie-breaker.
    mse: f64,
    corr: Option<f64>,
    /// Primary minimax objective on the C-Score scale after positive-slope affine alignment.
    max_abs_diff: f64,
    p95_abs_diff: f64,
    mean_abs_diff: f64,
    rmse: f64,
    avg_mean: f64,
    avg_std: f64,
    score_mean: f64,
    score_std: f64,
    align_slope: f64,
    align_intercept: f64,
}

#[derive(Debug, Clone)]
struct TargetSearchResult {
    indices: Vec<usize>,
    objective: TargetObjective,
}

#[derive(Debug, Clone)]
struct TargetAuditStats {
    count: usize,
    mean_diff: Option<f64>,
    mean_abs_diff: Option<f64>,
    max_abs_diff: Option<f64>,
    rmse: Option<f64>,
    p95_abs_diff: Option<f64>,
}

#[derive(Debug, Clone)]
struct TargetResidual {
    ref_idx: usize,
    avg_winrate: f64,
    diff: f64,
    abs_diff: f64,
}

#[derive(Debug, serde::Deserialize)]
struct TargetMilpSolverResponse {
    selected_indices: Vec<usize>,
    selected_weights: Vec<f64>,
    status: Option<String>,
    max_abs_diff: Option<f64>,
    p95_abs_diff: Option<f64>,
    slope: Option<f64>,
    intercept: Option<f64>,
}

#[derive(Debug, Clone)]
struct TargetMilpSolution {
    indices: Vec<usize>,
    weights: Vec<f64>,
}

fn run_target_milp_solver(
    locked_prefix: &[usize],
    fill_pool: &[usize],
    target_total: usize,
    player_cap: usize,
    weight_min: f64,
    weight_max: f64,
    player_weight_cap: f64,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> anyhow::Result<TargetMilpSolution> {
    if locked_prefix.len() > target_total {
        anyhow::bail!(
            "MILP 靶子生成输入非法：locked_prefix={} > target_total={}",
            locked_prefix.len(),
            target_total
        );
    }
    if fill_pool.len() < target_total {
        anyhow::bail!(
            "weighted MILP 靶子生成输入非法：候选池只有 {} 个，小于 target_total={}",
            fill_pool.len(),
            target_total
        );
    }
    if !weight_min.is_finite() || !weight_max.is_finite() || weight_min < 0.0 || weight_max < weight_min {
        anyhow::bail!(
            "weighted MILP 靶子生成输入非法：weight_min={:.6}, weight_max={:.6}",
            weight_min,
            weight_max
        );
    }
    if !player_weight_cap.is_finite() || player_weight_cap <= 0.0 {
        anyhow::bail!("weighted MILP 靶子生成输入非法：player_weight_cap={:.6}", player_weight_cap);
    }
    if let Some(error) = target_solution_feasibility_error(locked_prefix, candidates, player_cap) {
        anyhow::bail!("MILP 靶子生成输入的 locked_prefix 已违反约束：{error}");
    }

    let pool_set: HashSet<usize> = fill_pool.iter().copied().collect();
    if locked_prefix.iter().any(|idx| !pool_set.contains(idx)) {
        anyhow::bail!("MILP 靶子生成输入非法：locked_prefix 中存在不在 fill_pool 内的候选");
    }

    let mut rate_matrix: Vec<Vec<f64>> = Vec::with_capacity(reference_indices.len());
    for &ref_idx in reference_indices {
        let ref_gid = candidates[ref_idx].row.group_id;
        let mut row = Vec::with_capacity(fill_pool.len());
        for &target_idx in fill_pool {
            let target_gid = candidates[target_idx].row.group_id;
            let Some(rate) = rate_between(rate_map, ref_gid, target_gid) else {
                anyhow::bail!(
                    "MILP 靶子生成缺少 reference×candidate 胜率边：ref_group_id={} target_group_id={}",
                    ref_gid,
                    target_gid
                );
            };
            row.push(rate);
        }
        rate_matrix.push(row);
    }

    let ref_scores: Vec<f64> = reference_indices
        .iter()
        .map(|&idx| candidates[idx].correct_score())
        .collect();
    let player_keys: Vec<Vec<String>> = fill_pool
        .iter()
        .map(|&idx| candidates[idx].player_keys.clone())
        .collect();

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let out_dir = std::env::temp_dir().join(format!(
        "tswn_target_milp_{}_{}",
        std::process::id(),
        stamp
    ));
    fs::create_dir_all(&out_dir)
        .with_context(|| format!("create target MILP temp dir: {}", out_dir.display()))?;

    let script_path = out_dir.join("target_milp_solver.py");
    let input_path = out_dir.join("target_milp_input.json");
    let output_path = out_dir.join("target_milp_output.json");
    fs::write(&script_path, TARGET_MILP_SOLVER)
        .with_context(|| format!("write target MILP solver: {}", script_path.display()))?;

    let payload = serde_json::json!({
        "target_total": target_total,
        "player_cap": player_cap,
        "weight_min": weight_min,
        "weight_max": weight_max,
        "player_weight_cap": player_weight_cap,
        "pool_indices": fill_pool,
        "locked_indices": locked_prefix,
        "reference_indices": reference_indices,
        "ref_scores": ref_scores,
        "rate_matrix": rate_matrix,
        "player_keys": player_keys,
    });
    fs::write(&input_path, serde_json::to_vec(&payload)?)
        .with_context(|| format!("write target MILP input: {}", input_path.display()))?;

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
            Ok(out) if out.status.success() && output_path.exists() => {
                let bytes = fs::read(&output_path)
                    .with_context(|| format!("read target MILP output: {}", output_path.display()))?;
                let response: TargetMilpSolverResponse = serde_json::from_slice(&bytes)
                    .with_context(|| format!("parse target MILP output: {}", output_path.display()))?;
                let selected = response.selected_indices;
                let weights = response.selected_weights;
                if selected.len() != target_total || weights.len() != target_total {
                    anyhow::bail!(
                        "weighted MILP solver 返回数量错误：期望 {} 个，实际 selected={} weights={}；status={:?}",
                        target_total,
                        selected.len(),
                        weights.len(),
                        response.status
                    );
                }
                if selected.iter().any(|idx| !pool_set.contains(idx)) {
                    anyhow::bail!("weighted MILP solver 返回了不在 fill_pool 中的候选；status={:?}", response.status);
                }
                for &locked in locked_prefix {
                    if !selected.contains(&locked) {
                        anyhow::bail!(
                            "weighted MILP solver 返回解缺少锁定候选 group_id={}；status={:?}",
                            candidates[locked].row.group_id,
                            response.status
                        );
                    }
                }
                if let Some(error) = target_solution_feasibility_error(&selected, candidates, player_cap) {
                    anyhow::bail!("weighted MILP solver 返回解违反计数约束：{error}; status={:?}", response.status);
                }
                if let Some(error) = target_weight_feasibility_error(
                    &selected,
                    &weights,
                    candidates,
                    weight_min,
                    weight_max,
                    target_total as f64,
                    player_weight_cap,
                ) {
                    anyhow::bail!("weighted MILP solver 返回权重违反约束：{error}; status={:?}", response.status);
                }
                return Ok(TargetMilpSolution { indices: selected, weights });
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                last_error = Some(format!(
                    "{exe} failed with status {:?}; stdout={}; stderr={}",
                    out.status.code(),
                    stdout,
                    stderr
                ));
            }
            Err(err) => {
                last_error = Some(format!("{exe} failed to start: {err}"));
            }
        }
    }

    anyhow::bail!(
        "weighted MILP solver 不可用或未产生合法解：{}",
        last_error.unwrap_or_else(|| "python3/python 均不可用或没有返回错误信息".to_string())
    )
}

fn merged_player_keys(row: &crate::model::LaneResultRow) -> Vec<String> {
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

fn compare_target_candidates(a: &TargetCandidate, b: &TargetCandidate) -> std::cmp::Ordering {
    let ar = a.row.pair_rank.unwrap_or(usize::MAX);
    let br = b.row.pair_rank.unwrap_or(usize::MAX);
    ar.cmp(&br)
        .then_with(|| b.correct_score().total_cmp(&a.correct_score()))
        .then_with(|| a.row.rank.cmp(&b.row.rank))
        .then_with(|| a.row.group_id.cmp(&b.row.group_id))
}

fn greedy_main_order(candidates: &[TargetCandidate], min_score: Option<f64>) -> Vec<usize> {
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| compare_target_candidates(&candidates[a], &candidates[b]));

    let mut used_players = HashSet::new();
    let mut out = Vec::new();

    'candidate: for idx in order {
        if let Some(min_score) = min_score {
            if candidates[idx].correct_score() < min_score {
                continue;
            }
        }

        for player in &candidates[idx].player_keys {
            if used_players.contains(player) {
                continue 'candidate;
            }
        }

        for player in &candidates[idx].player_keys {
            used_players.insert(player.clone());
        }
        out.push(idx);
    }

    out
}

fn rate_between(
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

fn mean_std(values: &[f64]) -> (f64, f64) {
    if values.is_empty() {
        return (f64::NAN, f64::NAN);
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let var = values
        .iter()
        .map(|x| {
            let d = *x - mean;
            d * d
        })
        .sum::<f64>() / values.len().max(1) as f64;
    (mean, var.max(0.0).sqrt())
}

fn correlation(a: &[f64], b: &[f64]) -> Option<f64> {
    if a.len() != b.len() || a.len() < 2 {
        return None;
    }
    let (am, asd) = mean_std(a);
    let (bm, bsd) = mean_std(b);
    if !asd.is_finite() || !bsd.is_finite() || asd <= 1e-12 || bsd <= 1e-12 {
        return None;
    }
    let cov = a.iter()
        .zip(b.iter())
        .map(|(x, y)| (*x - am) * (*y - bm))
        .sum::<f64>() / a.len() as f64;
    Some(cov / (asd * bsd))
}

fn fixed_slope_chebyshev_fit(avg_values: &[f64], score_values: &[f64], slope: f64) -> (f64, f64) {
    let mut min_resid = f64::INFINITY;
    let mut max_resid = f64::NEG_INFINITY;
    for (avg, score) in avg_values.iter().zip(score_values.iter()) {
        let resid = *score - slope * *avg;
        min_resid = min_resid.min(resid);
        max_resid = max_resid.max(resid);
    }
    let intercept = (min_resid + max_resid) / 2.0;
    let max_abs = (max_resid - min_resid).abs() / 2.0;
    (intercept, max_abs)
}

fn affine_chebyshev_fit(avg_values: &[f64], score_values: &[f64]) -> (f64, f64, Vec<f64>) {
    if avg_values.len() != score_values.len() || avg_values.len() < 2 {
        return (1.0, 0.0, Vec::new());
    }

    let (avg_mean, avg_std) = mean_std(avg_values);
    let (score_mean, score_std) = mean_std(score_values);
    let avg_min = avg_values.iter().copied().fold(f64::INFINITY, f64::min);
    let avg_max = avg_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let score_min = score_values.iter().copied().fold(f64::INFINITY, f64::min);
    let score_max = score_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    if !avg_std.is_finite()
        || !score_std.is_finite()
        || avg_std <= 1e-12
        || score_std <= 1e-12
        || (avg_max - avg_min).abs() <= 1e-12
    {
        let intercept = score_mean - avg_mean;
        let diffs = avg_values
            .iter()
            .zip(score_values.iter())
            .map(|(avg, score)| *avg + intercept - *score)
            .collect();
        return (1.0, intercept, diffs);
    }

    let range_slope = ((score_max - score_min).abs() / (avg_max - avg_min).abs()).max(1e-6);
    let std_slope = (score_std / avg_std).max(1e-6);
    let corr_slope = correlation(avg_values, score_values)
        .map(|corr| (corr * score_std / avg_std).abs())
        .unwrap_or(std_slope)
        .max(1e-6);

    let mut lo = 0.0_f64;
    let mut hi = range_slope.max(std_slope).max(corr_slope).max(1.0) * 4.0;
    let mut last_hi_score = fixed_slope_chebyshev_fit(avg_values, score_values, hi).1;

    for _ in 0..8 {
        let mid = hi / 2.0;
        let mid_score = fixed_slope_chebyshev_fit(avg_values, score_values, mid).1;
        if last_hi_score + 1e-12 >= mid_score {
            break;
        }
        hi *= 2.0;
        last_hi_score = fixed_slope_chebyshev_fit(avg_values, score_values, hi).1;
    }

    // z(a) is convex piecewise-linear; ternary search is deterministic and accurate enough here.
    for _ in 0..80 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        let z1 = fixed_slope_chebyshev_fit(avg_values, score_values, m1).1;
        let z2 = fixed_slope_chebyshev_fit(avg_values, score_values, m2).1;
        if z1 <= z2 {
            hi = m2;
        } else {
            lo = m1;
        }
    }

    let slope = ((lo + hi) / 2.0).max(0.0);
    let (intercept, _) = fixed_slope_chebyshev_fit(avg_values, score_values, slope);
    let diffs = avg_values
        .iter()
        .zip(score_values.iter())
        .map(|(avg, score)| slope * *avg + intercept - *score)
        .collect();

    (slope, intercept, diffs)
}

fn target_objective(
    reference_indices: &[usize],
    target_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> TargetObjective {
    let mut avg_values = Vec::new();
    let mut score_values = Vec::new();
    let mut missing_edges = 0usize;
    let mut missing_refs = 0usize;

    for &ref_idx in reference_indices {
        let ref_gid = candidates[ref_idx].row.group_id;
        let mut sum = 0.0;
        let mut count = 0usize;
        let mut missing_for_ref = 0usize;

        for &target_idx in target_indices {
            let target_gid = candidates[target_idx].row.group_id;
            if let Some(rate) = rate_between(rate_map, ref_gid, target_gid) {
                sum += rate;
                count += 1;
            } else {
                missing_edges += 1;
                missing_for_ref += 1;
            }
        }

        if count == 0 {
            missing_refs += 1;
            continue;
        }

        // Production target generation should have complete reference × target coverage.
        // If an unexpected missing edge remains, keep the row but penalize the objective.
        let denominator = count + missing_for_ref;
        avg_values.push(sum / denominator.max(1) as f64);
        score_values.push(candidates[ref_idx].correct_score());
    }

    let (avg_mean, avg_std) = mean_std(&avg_values);
    let (score_mean, score_std) = mean_std(&score_values);
    let corr = correlation(&avg_values, &score_values);
    let (align_slope, align_intercept, diffs) = affine_chebyshev_fit(&avg_values, &score_values);
    let penalty = (missing_edges + missing_refs) as f64 * 1.0e6;
    let (mut max_abs_diff, mut p95_abs_diff, mut mean_abs_diff, mut rmse) = diff_summary_metrics(&diffs);
    max_abs_diff += penalty;
    p95_abs_diff += penalty;
    mean_abs_diff += penalty;
    rmse += penalty;
    let mse = if diffs.is_empty() {
        1.0e9 + penalty
    } else {
        diffs.iter().map(|x| x * x).sum::<f64>() / diffs.len() as f64 + penalty
    };

    TargetObjective {
        mse,
        corr,
        max_abs_diff,
        p95_abs_diff,
        mean_abs_diff,
        rmse,
        avg_mean,
        avg_std,
        score_mean,
        score_std,
        align_slope,
        align_intercept,
    }
}


fn target_objective_weighted(
    reference_indices: &[usize],
    target_indices: &[usize],
    target_weights: &[f64],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> TargetObjective {
    if target_indices.len() != target_weights.len() {
        let mut obj = target_objective(reference_indices, target_indices, candidates, rate_map);
        obj.max_abs_diff += 1.0e9;
        obj.p95_abs_diff += 1.0e9;
        obj.mean_abs_diff += 1.0e9;
        obj.rmse += 1.0e9;
        obj.mse += 1.0e9;
        return obj;
    }

    let weight_sum = target_weights.iter().copied().sum::<f64>();
    if !weight_sum.is_finite() || weight_sum <= 0.0 {
        let mut obj = target_objective(reference_indices, target_indices, candidates, rate_map);
        obj.max_abs_diff += 1.0e9;
        obj.p95_abs_diff += 1.0e9;
        obj.mean_abs_diff += 1.0e9;
        obj.rmse += 1.0e9;
        obj.mse += 1.0e9;
        return obj;
    }

    let mut avg_values = Vec::new();
    let mut score_values = Vec::new();
    let mut missing_edges = 0usize;
    let mut missing_refs = 0usize;

    for &ref_idx in reference_indices {
        let ref_gid = candidates[ref_idx].row.group_id;
        let mut weighted_sum = 0.0;
        let mut present_weight_sum = 0.0;

        for (&target_idx, &weight) in target_indices.iter().zip(target_weights.iter()) {
            let target_gid = candidates[target_idx].row.group_id;
            if let Some(rate) = rate_between(rate_map, ref_gid, target_gid) {
                weighted_sum += rate * weight;
                present_weight_sum += weight;
            } else {
                missing_edges += 1;
            }
        }

        if present_weight_sum <= 0.0 {
            missing_refs += 1;
            continue;
        }

        avg_values.push(weighted_sum / weight_sum);
        score_values.push(candidates[ref_idx].correct_score());
    }

    let (avg_mean, avg_std) = mean_std(&avg_values);
    let (score_mean, score_std) = mean_std(&score_values);
    let corr = correlation(&avg_values, &score_values);
    let (align_slope, align_intercept, diffs) = affine_chebyshev_fit(&avg_values, &score_values);
    let penalty = (missing_edges + missing_refs) as f64 * 1.0e6;
    let (mut max_abs_diff, mut p95_abs_diff, mut mean_abs_diff, mut rmse) = diff_summary_metrics(&diffs);
    max_abs_diff += penalty;
    p95_abs_diff += penalty;
    mean_abs_diff += penalty;
    rmse += penalty;
    let mse = if diffs.is_empty() {
        1.0e9 + penalty
    } else {
        diffs.iter().map(|x| x * x).sum::<f64>() / diffs.len() as f64 + penalty
    };

    TargetObjective {
        mse,
        corr,
        max_abs_diff,
        p95_abs_diff,
        mean_abs_diff,
        rmse,
        avg_mean,
        avg_std,
        score_mean,
        score_std,
        align_slope,
        align_intercept,
    }
}

fn diff_summary_metrics(diffs: &[f64]) -> (f64, f64, f64, f64) {
    if diffs.is_empty() {
        let penalty = 1.0e6;
        return (penalty, penalty, penalty, penalty);
    }

    let mut abs: Vec<f64> = diffs
        .iter()
        .copied()
        .filter(|x| x.is_finite())
        .map(|x| x.abs())
        .collect();

    if abs.is_empty() {
        let penalty = 1.0e6;
        return (penalty, penalty, penalty, penalty);
    }

    abs.sort_by(|a, b| a.total_cmp(b));
    let n = abs.len();
    let max_abs_diff = abs[n - 1];
    let p95_idx = ((n.saturating_sub(1)) as f64 * 0.95).round() as usize;
    let p95_abs_diff = abs[p95_idx.min(n - 1)];
    let mean_abs_diff = abs.iter().sum::<f64>() / n as f64;
    let rmse = (diffs.iter().map(|x| x * x).sum::<f64>() / diffs.len() as f64).sqrt();

    (max_abs_diff, p95_abs_diff, mean_abs_diff, rmse)
}

fn objective_loss_key(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        f64::INFINITY
    }
}

fn objective_corr_key(value: Option<f64>) -> f64 {
    match value {
        Some(x) if x.is_finite() => x,
        _ => f64::NEG_INFINITY,
    }
}

fn compare_target_objectives(a: &TargetObjective, b: &TargetObjective) -> std::cmp::Ordering {
    // IMPORTANT: this comparator is used by Rust's slice::sort_by.  It must be a
    // strict total order.  Do not use EPS/tolerance comparisons here: approximate
    // equality is not transitive, so sort_by may panic with:
    // "user-provided comparison function does not correctly implement a total order".
    //
    // If two objective values should be treated as practically equal, keep the
    // exact total order here and express the tolerance in the objective itself
    // before sorting, e.g. by rounded/quantized metrics.
    objective_loss_key(a.max_abs_diff)
        .total_cmp(&objective_loss_key(b.max_abs_diff))
        .then_with(|| objective_loss_key(a.p95_abs_diff).total_cmp(&objective_loss_key(b.p95_abs_diff)))
        .then_with(|| objective_loss_key(a.mean_abs_diff).total_cmp(&objective_loss_key(b.mean_abs_diff)))
        .then_with(|| objective_loss_key(a.rmse).total_cmp(&objective_loss_key(b.rmse)))
        .then_with(|| objective_loss_key(a.mse).total_cmp(&objective_loss_key(b.mse)))
        // Higher correlation is better, so reverse the order.
        .then_with(|| objective_corr_key(b.corr).total_cmp(&objective_corr_key(a.corr)))
}

fn objective_is_better(
    idx: usize,
    obj: &TargetObjective,
    best_idx: usize,
    best_obj: &TargetObjective,
    candidates: &[TargetCandidate],
) -> bool {
    match compare_target_objectives(obj, best_obj) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => compare_target_candidates(&candidates[idx], &candidates[best_idx]).is_lt(),
    }
}

fn target_solution_is_better(
    lhs: &TargetSearchResult,
    rhs: &TargetSearchResult,
    candidates: &[TargetCandidate],
) -> bool {
    match compare_target_objectives(&lhs.objective, &rhs.objective) {
        std::cmp::Ordering::Less => true,
        std::cmp::Ordering::Greater => false,
        std::cmp::Ordering::Equal => compare_solution_order(&lhs.indices, &rhs.indices, candidates).is_lt(),
    }
}

fn compare_solution_order(
    lhs: &[usize],
    rhs: &[usize],
    candidates: &[TargetCandidate],
) -> std::cmp::Ordering {
    for (&a, &b) in lhs.iter().zip(rhs.iter()) {
        let ord = compare_target_candidates(&candidates[a], &candidates[b]);
        if !ord.is_eq() {
            return ord;
        }
    }
    lhs.len().cmp(&rhs.len())
}

fn has_duplicate_indices(indices: &[usize]) -> bool {
    let mut seen = HashSet::new();
    for idx in indices {
        if !seen.insert(*idx) {
            return true;
        }
    }
    false
}

fn violates_player_cap(indices: &[usize], candidates: &[TargetCandidate], cap: usize) -> bool {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for &idx in indices {
        for key in &candidates[idx].player_keys {
            let count = counts.entry(key.as_str()).or_insert(0);
            *count += 1;
            if *count > cap {
                return true;
            }
        }
    }
    false
}

fn target_solution_feasibility_error(indices: &[usize], candidates: &[TargetCandidate], cap: usize) -> Option<String> {
    let mut seen = HashSet::new();
    for &idx in indices {
        if !seen.insert(idx) {
            return Some(format!("重复靶子 group_id={}", candidates[idx].row.group_id));
        }
    }

    let mut player_counts: HashMap<&str, usize> = HashMap::new();
    for &idx in indices {
        for key in &candidates[idx].player_keys {
            let count = player_counts.entry(key.as_str()).or_insert(0);
            *count += 1;
            if *count > cap {
                return Some(format!("重复号约束失败：玩家/成员 {key} 出现 {count} 次，超过上限 {cap}"));
            }
        }
    }

    None
}

fn target_solution_is_feasible(indices: &[usize], candidates: &[TargetCandidate], cap: usize) -> bool {
    target_solution_feasibility_error(indices, candidates, cap).is_none()
}

fn target_solution_can_add(indices: &[usize], idx: usize, candidates: &[TargetCandidate], cap: usize) -> bool {
    if indices.contains(&idx) {
        return false;
    }
    let mut proposed = indices.to_vec();
    proposed.push(idx);
    target_solution_is_feasible(&proposed, candidates, cap)
}

fn target_weight_feasibility_error(
    indices: &[usize],
    weights: &[f64],
    candidates: &[TargetCandidate],
    weight_min: f64,
    weight_max: f64,
    expected_weight_sum: f64,
    player_weight_cap: f64,
) -> Option<String> {
    if indices.len() != weights.len() {
        return Some(format!("indices/weights 长度不一致：{} vs {}", indices.len(), weights.len()));
    }

    let mut weight_sum = 0.0;
    let mut player_weight: HashMap<&str, f64> = HashMap::new();
    for (&idx, &weight) in indices.iter().zip(weights.iter()) {
        if !weight.is_finite() {
            return Some(format!("group_id={} 的权重不是有限数：{}", candidates[idx].row.group_id, weight));
        }
        if weight + 1e-7 < weight_min || weight > weight_max + 1e-7 {
            return Some(format!(
                "group_id={} 的权重 {:.9} 超出范围 [{:.3}, {:.3}]",
                candidates[idx].row.group_id,
                weight,
                weight_min,
                weight_max
            ));
        }
        weight_sum += weight;
        for key in &candidates[idx].player_keys {
            let entry = player_weight.entry(key.as_str()).or_insert(0.0);
            *entry += weight;
            if *entry > player_weight_cap + 1e-6 {
                return Some(format!(
                    "玩家 {key} 的靶子权重和 {:.9} 超过上限 {:.3}",
                    *entry,
                    player_weight_cap
                ));
            }
        }
    }

    if (weight_sum - expected_weight_sum).abs() > 1e-5 {
        return Some(format!(
            "靶子权重总和 {:.9} 不等于期望 {:.9}",
            weight_sum,
            expected_weight_sum
        ));
    }

    None
}

fn build_target_config_text(indices: &[usize], weights: &[f64], candidates: &[TargetCandidate]) -> String {
    let mut lines = Vec::with_capacity(indices.len());
    for (&idx, &weight) in indices.iter().zip(weights.iter()) {
        lines.push(format!("{:.12}\t{}", weight, candidates[idx].row.canonical));
    }
    lines.join("\n")
}

fn feasibility_beam_refill_solution(
    start: Vec<usize>,
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    candidates: &[TargetCandidate],
    beam_width: usize,
) -> Option<Vec<usize>> {
    if !target_solution_is_feasible(&start, candidates, player_cap) || start.len() > target_total {
        return None;
    }

    let mut beam = vec![start];
    while beam.first().map(|x| x.len()).unwrap_or(0) < target_total {
        let mut next: Vec<Vec<usize>> = Vec::new();

        for partial in &beam {
            let selected_set: HashSet<usize> = partial.iter().copied().collect();
            for &idx in pool {
                if selected_set.contains(&idx) {
                    continue;
                }
                let mut proposed = partial.clone();
                proposed.push(idx);
                if !target_solution_is_feasible(&proposed, candidates, player_cap) {
                    continue;
                }
                next.push(proposed);
            }
        }

        if next.is_empty() {
            return None;
        }

        // This fallback is only about proving/finding feasibility, so rank by
        // stable main-list order and by remaining player-cap slack instead of
        // residual objective.  That avoids falsely reporting "cannot fill" just
        // because the minimax beam pruned into a dead basin.
        next.sort_by(|a, b| {
            compare_solution_order(a, b, candidates)
                .then_with(|| b.len().cmp(&a.len()))
        });
        next.dedup_by(|a, b| solution_signature(a) == solution_signature(b));
        next.truncate(beam_width.max(1));
        beam = next;
    }

    beam.into_iter().next()
}

fn greedy_refill_solution(
    start: Vec<usize>,
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    candidates: &[TargetCandidate],
) -> Option<Vec<usize>> {
    if !target_solution_is_feasible(&start, candidates, player_cap) {
        return None;
    }

    let mut selected = start;
    let mut selected_set: HashSet<usize> = selected.iter().copied().collect();
    for &idx in pool {
        if selected.len() >= target_total {
            break;
        }
        if selected_set.contains(&idx) {
            continue;
        }
        selected.push(idx);
        if !target_solution_is_feasible(&selected, candidates, player_cap) {
            selected.pop();
            continue;
        }
        selected_set.insert(idx);
    }

    if selected.len() == target_total {
        Some(selected)
    } else {
        None
    }
}

fn target_reference_residuals(
    reference_indices: &[usize],
    target_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> Vec<TargetResidual> {
    if target_indices.is_empty() {
        return Vec::new();
    }

    let objective = target_objective(reference_indices, target_indices, candidates, rate_map);
    if !objective.align_slope.is_finite() || !objective.align_intercept.is_finite() {
        return Vec::new();
    }

    let mut out = Vec::new();
    for &ref_idx in reference_indices {
        let ref_gid = candidates[ref_idx].row.group_id;
        let mut sum = 0.0;
        let mut count = 0usize;
        let mut missing_for_ref = 0usize;

        for &target_idx in target_indices {
            let target_gid = candidates[target_idx].row.group_id;
            if let Some(rate) = rate_between(rate_map, ref_gid, target_gid) {
                sum += rate;
                count += 1;
            } else {
                missing_for_ref += 1;
            }
        }

        if count == 0 {
            continue;
        }

        let denominator = count + missing_for_ref;
        let avg = sum / denominator.max(1) as f64;
        let aligned = objective.align_slope * avg + objective.align_intercept;
        let diff = aligned - candidates[ref_idx].correct_score();
        if diff.is_finite() {
            out.push(TargetResidual {
                ref_idx,
                avg_winrate: avg,
                diff,
                abs_diff: diff.abs(),
            });
        }
    }

    out.sort_by(|a, b| {
        b.abs_diff
            .total_cmp(&a.abs_diff)
            .then_with(|| a.ref_idx.cmp(&b.ref_idx))
    });
    out
}

fn repair_gain_for_candidate(
    residuals: &[TargetResidual],
    target_idx: usize,
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    worst_ref_limit: usize,
) -> f64 {
    let target_gid = candidates[target_idx].row.group_id;
    let mut gain = 0.0;

    for residual in residuals.iter().take(worst_ref_limit.max(1)) {
        let ref_gid = candidates[residual.ref_idx].row.group_id;
        let Some(rate) = rate_between(rate_map, ref_gid, target_gid) else {
            continue;
        };
        let centered = rate - residual.avg_winrate;
        let weight = residual.abs_diff * residual.abs_diff;
        gain += -residual.diff.signum() * centered * weight;
    }

    gain
}

fn removal_harm_for_target(
    residuals: &[TargetResidual],
    target_idx: usize,
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    worst_ref_limit: usize,
) -> f64 {
    let target_gid = candidates[target_idx].row.group_id;
    let mut harm = 0.0;

    for residual in residuals.iter().take(worst_ref_limit.max(1)) {
        let ref_gid = candidates[residual.ref_idx].row.group_id;
        let Some(rate) = rate_between(rate_map, ref_gid, target_gid) else {
            continue;
        };
        let centered = rate - residual.avg_winrate;
        let weight = residual.abs_diff * residual.abs_diff;
        harm += residual.diff.signum() * centered * weight;
    }

    harm
}

fn projected_refill_objective(
    partial: &[usize],
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
) -> TargetObjective {
    if partial.len() >= target_total {
        return target_objective(reference_indices, partial, candidates, rate_map);
    }

    if !target_solution_is_feasible(partial, candidates, player_cap) {
        let mut obj = target_objective(reference_indices, partial, candidates, rate_map);
        obj.max_abs_diff += 1.0e9;
        obj.p95_abs_diff += 1.0e9;
        obj.mean_abs_diff += 1.0e9;
        obj.rmse += 1.0e9;
        obj.mse += 1.0e9;
        return obj;
    }

    // Dynamic lookahead: estimate the remaining fill from the current worst
    // residuals, not from a static pool mean.  This keeps partial beam states
    // from being scored as if their current incomplete profile were final.
    let residuals = target_reference_residuals(reference_indices, partial, candidates, rate_map);
    let mut ranked_pool: Vec<(usize, f64)> = pool
        .iter()
        .copied()
        .filter(|idx| !partial.contains(idx))
        .map(|idx| {
            let gain = repair_gain_for_candidate(&residuals, idx, candidates, rate_map, residuals.len().min(16));
            (idx, gain)
        })
        .collect();

    ranked_pool.sort_by(|(idx_a, gain_a), (idx_b, gain_b)| {
        gain_b
            .total_cmp(gain_a)
            .then_with(|| compare_target_candidates(&candidates[*idx_a], &candidates[*idx_b]))
    });

    let mut projected = partial.to_vec();
    for (idx, _) in ranked_pool {
        if projected.len() >= target_total {
            break;
        }
        if target_solution_can_add(&projected, idx, candidates, player_cap) {
            projected.push(idx);
        }
    }

    // If the residual-guided projection cannot complete the target set, use a
    // feasibility-only completion as a second lookahead.  If even that cannot
    // complete, penalize the partial heavily so the beam does not keep a dead
    // branch just because its incomplete profile looks good.
    if projected.len() < target_total {
        if let Some(completed) = feasibility_beam_refill_solution(
            partial.to_vec(),
            pool,
            target_total,
            player_cap,
            candidates,
            64,
        ) {
            projected = completed;
        } else {
            let mut obj = target_objective(reference_indices, &projected, candidates, rate_map);
            obj.max_abs_diff += 1.0e9;
            obj.p95_abs_diff += 1.0e9;
            obj.mean_abs_diff += 1.0e9;
            obj.rmse += 1.0e9;
            obj.mse += 1.0e9;
            return obj;
        }
    }

    target_objective(reference_indices, &projected, candidates, rate_map)
}

fn beam_refill_solution(
    start: Vec<usize>,
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    beam_width: usize,
) -> Option<Vec<usize>> {
    if !target_solution_is_feasible(&start, candidates, player_cap) {
        return None;
    }
    if start.len() > target_total {
        return None;
    }

    let mut beam = vec![start];
    while beam.first().map(|x| x.len()).unwrap_or(0) < target_total {
        let mut next: Vec<TargetSearchResult> = Vec::new();
        for partial in &beam {
            let selected_set: HashSet<usize> = partial.iter().copied().collect();
            for &idx in pool {
                if selected_set.contains(&idx) {
                    continue;
                }
                let mut proposed = partial.clone();
                proposed.push(idx);
                if !target_solution_is_feasible(&proposed, candidates, player_cap) {
                    continue;
                }
                let objective = projected_refill_objective(&proposed, pool, target_total, player_cap, reference_indices, candidates, rate_map);
                next.push(TargetSearchResult { indices: proposed, objective });
            }
        }

        if next.is_empty() {
            return feasibility_beam_refill_solution(
                beam.into_iter().next().unwrap_or_default(),
                pool,
                target_total,
                player_cap,
                candidates,
                beam_width.max(64),
            );
        }

        next.sort_by(|a, b| match compare_target_objectives(&a.objective, &b.objective) {
            std::cmp::Ordering::Equal => compare_solution_order(&a.indices, &b.indices, candidates),
            ord => ord,
        });
        next.truncate(beam_width.max(1));
        beam = next.into_iter().map(|x| x.indices).collect();
    }

    beam.into_iter().next()
}

fn repair_solution_for_locked_prefix(
    seed: Vec<usize>,
    locked_prefix: &[usize],
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    beam_width: usize,
) -> Option<Vec<usize>> {
    let mut repaired = locked_prefix.to_vec();
    let mut seen: HashSet<usize> = repaired.iter().copied().collect();

    for idx in seed {
        if repaired.len() >= target_total {
            break;
        }
        if seen.contains(&idx) {
            continue;
        }
        repaired.push(idx);
        if !target_solution_is_feasible(&repaired, candidates, player_cap) {
            repaired.pop();
            continue;
        }
        seen.insert(idx);
    }

    beam_refill_solution(
        repaired,
        pool,
        target_total,
        player_cap,
        reference_indices,
        candidates,
        rate_map,
        beam_width,
    )
}

fn polish_target_solution(
    selected: Vec<usize>,
    locked_count: usize,
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    max_passes: usize,
) -> TargetSearchResult {
    let mut selected = selected;
    if selected.len() != target_total {
        let objective = target_objective(reference_indices, &selected, candidates, rate_map);
        return TargetSearchResult { indices: selected, objective };
    }

    let mut current_obj = target_objective(reference_indices, &selected, candidates, rate_map);
    for _ in 0..max_passes {
        let mut best_swap: Option<(usize, usize, TargetObjective)> = None;
        for slot in locked_count..selected.len() {
            for &idx in pool {
                if selected[slot] == idx {
                    continue;
                }
                if selected.contains(&idx) {
                    continue;
                }
                let mut proposed = selected.clone();
                proposed[slot] = idx;
                if !target_solution_is_feasible(&proposed, candidates, player_cap) {
                    continue;
                }
                let obj = target_objective(reference_indices, &proposed, candidates, rate_map);
                if !matches!(compare_target_objectives(&obj, &current_obj), std::cmp::Ordering::Less) {
                    continue;
                }
                if best_swap
                    .as_ref()
                    .map(|(_, best_idx, best_obj)| objective_is_better(idx, &obj, *best_idx, best_obj, candidates))
                    .unwrap_or(true)
                {
                    best_swap = Some((slot, idx, obj));
                }
            }
        }

        let Some((slot, idx, obj)) = best_swap else {
            break;
        };
        selected[slot] = idx;
        current_obj = obj;
    }

    TargetSearchResult { indices: selected, objective: current_obj }
}

fn two_swap_polish_target_solution(
    initial: TargetSearchResult,
    locked_count: usize,
    pool: &[usize],
    player_cap: usize,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    remove_shortlist: usize,
    add_shortlist: usize,
    max_passes: usize,
) -> TargetSearchResult {
    let mut selected = initial.indices;
    let mut current_obj = target_objective(reference_indices, &selected, candidates, rate_map);

    if selected.len() <= locked_count + 1 || !target_solution_is_feasible(&selected, candidates, player_cap) {
        return TargetSearchResult { indices: selected, objective: current_obj };
    }

    for _ in 0..max_passes {
        let residuals = target_reference_residuals(reference_indices, &selected, candidates, rate_map);
        let mut remove_candidates: Vec<(usize, usize, f64)> = selected
            .iter()
            .copied()
            .enumerate()
            .skip(locked_count)
            .map(|(slot, idx)| {
                let harm = removal_harm_for_target(&residuals, idx, candidates, rate_map, residuals.len().min(16));
                (slot, idx, harm)
            })
            .collect();
        remove_candidates.sort_by(|(slot_a, idx_a, harm_a), (slot_b, idx_b, harm_b)| {
            harm_b
                .total_cmp(harm_a)
                .then_with(|| compare_target_candidates(&candidates[*idx_b], &candidates[*idx_a]))
                .then_with(|| slot_a.cmp(slot_b))
        });
        remove_candidates.truncate(remove_shortlist.max(2));

        let selected_set: HashSet<usize> = selected.iter().copied().collect();
        let mut add_candidates: Vec<(usize, f64)> = pool
            .iter()
            .copied()
            .filter(|idx| !selected_set.contains(idx))
            .map(|idx| {
                let gain = repair_gain_for_candidate(&residuals, idx, candidates, rate_map, residuals.len().min(16));
                (idx, gain)
            })
            .collect();
        add_candidates.sort_by(|(idx_a, gain_a), (idx_b, gain_b)| {
            gain_b
                .total_cmp(gain_a)
                .then_with(|| compare_target_candidates(&candidates[*idx_a], &candidates[*idx_b]))
        });
        add_candidates.truncate(add_shortlist.max(2));

        let mut best_swap: Option<(usize, usize, usize, usize, TargetObjective)> = None;

        for i in 0..remove_candidates.len() {
            for j in (i + 1)..remove_candidates.len() {
                let slot_a = remove_candidates[i].0;
                let slot_b = remove_candidates[j].0;
                if slot_a == slot_b {
                    continue;
                }

                for add_i in 0..add_candidates.len() {
                    for add_j in (add_i + 1)..add_candidates.len() {
                        let idx_a = add_candidates[add_i].0;
                        let idx_b = add_candidates[add_j].0;
                        if idx_a == idx_b {
                            continue;
                        }

                        let mut proposed = selected.clone();
                        proposed[slot_a] = idx_a;
                        proposed[slot_b] = idx_b;
                        if !target_solution_is_feasible(&proposed, candidates, player_cap) {
                            continue;
                        }

                        let obj = target_objective(reference_indices, &proposed, candidates, rate_map);
                        if !matches!(compare_target_objectives(&obj, &current_obj), std::cmp::Ordering::Less) {
                            continue;
                        }

                        let should_replace = best_swap
                            .as_ref()
                            .map(|(_, _, _, _, best_obj)| matches!(compare_target_objectives(&obj, best_obj), std::cmp::Ordering::Less))
                            .unwrap_or(true);

                        if should_replace {
                            best_swap = Some((slot_a, slot_b, idx_a, idx_b, obj));
                        }
                    }
                }
            }
        }

        let Some((slot_a, slot_b, idx_a, idx_b, obj)) = best_swap else {
            break;
        };

        selected[slot_a] = idx_a;
        selected[slot_b] = idx_b;
        current_obj = obj;
    }

    TargetSearchResult { indices: selected, objective: current_obj }
}

fn lns_target_search(
    initial: TargetSearchResult,
    locked_count: usize,
    pool: &[usize],
    target_total: usize,
    player_cap: usize,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    beam_width: usize,
    rounds: usize,
    worst_ref_limit: usize,
    two_swap_remove_shortlist: usize,
    two_swap_add_shortlist: usize,
    two_swap_passes: usize,
    seen_solutions: &mut HashSet<String>,
) -> TargetSearchResult {
    let mut best = initial.clone();
    seen_solutions.insert(solution_signature(&best.indices));
    let remove_sizes = [3usize, 5, 8, 12, 16];

    for round in 0..rounds {
        let remove_count = remove_sizes[round % remove_sizes.len()];
        let base = if round % 3 == 0 { &best.indices } else { &initial.indices };
        let partial = if round % 4 == 3 {
            remove_unlocked_targets(
                base,
                locked_count,
                remove_count,
                round as u64 + 17,
                candidates,
            )
        } else {
            remove_worst_guided_unlocked_targets(
                base,
                locked_count,
                remove_count,
                round as u64 + 17,
                reference_indices,
                candidates,
                rate_map,
                worst_ref_limit,
            )
        };

        let Some(refilled) = beam_refill_solution(
            partial,
            pool,
            target_total,
            player_cap,
            reference_indices,
            candidates,
            rate_map,
            beam_width,
        ) else {
            continue;
        };

        let sig = solution_signature(&refilled);
        if seen_solutions.contains(&sig) {
            continue;
        }
        seen_solutions.insert(sig);

        let candidate = polish_target_solution(
            refilled,
            locked_count,
            pool,
            target_total,
            player_cap,
            reference_indices,
            candidates,
            rate_map,
            4,
        );
        let candidate = two_swap_polish_target_solution(
            candidate,
            locked_count,
            pool,
            player_cap,
            reference_indices,
            candidates,
            rate_map,
            two_swap_remove_shortlist,
            two_swap_add_shortlist,
            two_swap_passes,
        );

        if target_solution_is_better(&candidate, &best, candidates) {
            best = candidate;
        }
    }

    best
}

fn remove_worst_guided_unlocked_targets(
    selected: &[usize],
    locked_count: usize,
    remove_count: usize,
    salt: u64,
    reference_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    worst_ref_limit: usize,
) -> Vec<usize> {
    if selected.len() <= locked_count {
        return selected.to_vec();
    }

    let unlocked_len = selected.len() - locked_count;
    let actual_remove = remove_count.min(unlocked_len);
    let residuals = target_reference_residuals(reference_indices, selected, candidates, rate_map);
    if residuals.is_empty() {
        return remove_unlocked_targets(selected, locked_count, remove_count, salt, candidates);
    }

    let mut unlocked: Vec<(usize, usize, f64)> = selected
        .iter()
        .copied()
        .enumerate()
        .skip(locked_count)
        .map(|(slot, idx)| {
            let harm = removal_harm_for_target(&residuals, idx, candidates, rate_map, worst_ref_limit);
            (slot, idx, harm)
        })
        .collect();

    unlocked.sort_by(|(slot_a, idx_a, harm_a), (slot_b, idx_b, harm_b)| {
        harm_b
            .total_cmp(harm_a)
            .then_with(|| {
                let ha = splitmix64((*idx_a as u64).wrapping_add(salt).wrapping_add(*slot_a as u64));
                let hb = splitmix64((*idx_b as u64).wrapping_add(salt).wrapping_add(*slot_b as u64));
                ha.cmp(&hb)
            })
            .then_with(|| slot_a.cmp(slot_b))
    });

    let remove_slots: HashSet<usize> = unlocked
        .into_iter()
        .take(actual_remove)
        .map(|(slot, _, _)| slot)
        .collect();

    selected
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(slot, idx)| if remove_slots.contains(&slot) { None } else { Some(idx) })
        .collect()
}

fn remove_unlocked_targets(
    selected: &[usize],
    locked_count: usize,
    remove_count: usize,
    salt: u64,
    candidates: &[TargetCandidate],
) -> Vec<usize> {
    if selected.len() <= locked_count {
        return selected.to_vec();
    }

    let unlocked_len = selected.len() - locked_count;
    let actual_remove = remove_count.min(unlocked_len);
    let mut unlocked: Vec<(usize, usize)> = selected
        .iter()
        .copied()
        .enumerate()
        .skip(locked_count)
        .collect();

    unlocked.sort_by(|(slot_a, idx_a), (slot_b, idx_b)| {
        let ha = splitmix64((*idx_a as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(salt).wrapping_add(*slot_a as u64));
        let hb = splitmix64((*idx_b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(salt).wrapping_add(*slot_b as u64));
        ha.cmp(&hb)
            .then_with(|| compare_target_candidates(&candidates[*idx_b], &candidates[*idx_a]))
    });

    let remove_slots: HashSet<usize> = unlocked
        .into_iter()
        .take(actual_remove)
        .map(|(slot, _)| slot)
        .collect();

    selected
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(slot, idx)| if remove_slots.contains(&slot) { None } else { Some(idx) })
        .collect()
}

fn solution_signature(indices: &[usize]) -> String {
    let mut sorted = indices.to_vec();
    sorted.sort_unstable();
    sorted
        .iter()
        .map(|idx| idx.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn deterministic_shuffled_pool(pool: &[usize], salt: u64) -> Vec<usize> {
    let mut out = pool.to_vec();
    out.sort_by_key(|idx| splitmix64((*idx as u64).wrapping_add(salt.wrapping_mul(0xD1B5_4A32_D192_ED03))));
    out
}

fn collect_missing_target_rate_pairs(
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

fn compute_missing_target_rates(
    db: &Db,
    config: &RankerConfig,
    missing_pairs: &[(usize, usize)],
    candidates: &[TargetCandidate],
) -> anyhow::Result<Vec<(crate::model::GroupId, crate::model::GroupId, f64)>> {
    if missing_pairs.is_empty() {
        return Ok(Vec::new());
    }

    let mut group_cache: HashMap<crate::model::GroupId, StoredGroup> = HashMap::new();
    let mut pairs = Vec::with_capacity(missing_pairs.len());

    for (pair_index, &(a_idx, b_idx)) in missing_pairs.iter().enumerate() {
        let a_id = candidates[a_idx].row.group_id;
        let b_id = candidates[b_idx].row.group_id;

        let group_a = if let Some(group) = group_cache.get(&a_id) {
            group.clone()
        } else {
            let group = db
                .get_group(a_id)?
                .with_context(|| format!("missing group_id {} while computing target rate coverage", a_id))?;
            group_cache.insert(a_id, group.clone());
            group
        };

        let group_b = if let Some(group) = group_cache.get(&b_id) {
            group.clone()
        } else {
            let group = db
                .get_group(b_id)?
                .with_context(|| format!("missing group_id {} while computing target rate coverage", b_id))?;
            group_cache.insert(b_id, group.clone());
            group
        };

        pairs.push(ManualWinratePair {
            pair_index,
            group_a,
            group_b,
        });
    }

    let total = pairs.len();
    let workers = resolve_manual_winrate_workers(config.outer_workers, total);
    let pairs = Arc::new(pairs);
    let mut handles = Vec::with_capacity(workers);

    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _ in 0..workers {
            let pairs = Arc::clone(&pairs);
            let next_pair = Arc::clone(&next_pair);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(crate::model::GroupId, crate::model::GroupId, f64)>> {
                let mut computed = Vec::new();
                loop {
                    let idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = pairs.get(idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                    computed.push((pair.group_a.id, pair.group_b.id, rate));
                }
                Ok(computed)
            }));
        }
    } else {
        for worker_id in 0..workers {
            let pairs = Arc::clone(&pairs);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(crate::model::GroupId, crate::model::GroupId, f64)>> {
                let mut computed = Vec::with_capacity(end.saturating_sub(start));
                for idx in start..end {
                    let Some(pair) = pairs.get(idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                    computed.push((pair.group_a.id, pair.group_b.id, rate));
                }
                Ok(computed)
            }));
        }
    }

    let mut computed = Vec::with_capacity(total);
    for handle in handles {
        computed.extend(handle.join().expect("target rate worker thread panicked")?);
    }
    computed.sort_by_key(|(a, b, _)| (*a, *b));
    Ok(computed)
}

fn target_reference_audit_rows(
    reference_indices: &[usize],
    target_indices: &[usize],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    objective: &TargetObjective,
) -> Vec<TargetReferenceAuditRow> {
    let can_align = objective.align_slope.is_finite() && objective.align_intercept.is_finite();
    let mut out = Vec::with_capacity(reference_indices.len());

    for (rank, &ref_idx) in reference_indices.iter().enumerate() {
        let candidate = &candidates[ref_idx];
        let ref_gid = candidate.row.group_id;
        let mut sum = 0.0;
        let mut count = 0usize;

        for &target_idx in target_indices {
            let target_gid = candidates[target_idx].row.group_id;
            if let Some(rate) = rate_between(rate_map, ref_gid, target_gid) {
                sum += rate;
                count += 1;
            }
        }

        let average_winrate_vs_targets = if count == 0 { None } else { Some(sum / count as f64) };
        let aligned_c_score_from_targets = average_winrate_vs_targets.and_then(|avg| {
            if can_align {
                Some(objective.align_slope * avg + objective.align_intercept)
            } else {
                None
            }
        });
        let aligned_minus_c_score = aligned_c_score_from_targets.map(|aligned| aligned - candidate.correct_score());
        let abs_aligned_minus_c_score = aligned_minus_c_score.map(|x| x.abs());

        out.push(TargetReferenceAuditRow {
            reference_rank: rank + 1,
            group_id: candidate.row.group_id,
            canonical: candidate.row.canonical.clone(),
            team_name: candidate.row.team_name.clone(),
            root_team_name: candidate.row.root_team_name.clone(),
            correct_rank: candidate.row.pair_rank,
            correct_score: candidate.correct_score(),
            raw_rank: candidate.row.rank,
            raw_score: candidate.row.raw_average_cqd,
            average_winrate_vs_targets,
            target_rate_count: count,
            aligned_c_score_from_targets,
            aligned_minus_c_score,
            abs_aligned_minus_c_score,
            type_label: candidate.row.type_label.clone(),
            simple_type_label: candidate.row.simple_type_label.clone(),
        });
    }

    out
}

fn target_reference_audit_rows_weighted(
    reference_indices: &[usize],
    target_indices: &[usize],
    target_weights: &[f64],
    candidates: &[TargetCandidate],
    rate_map: &HashMap<(crate::model::GroupId, crate::model::GroupId), f64>,
    objective: &TargetObjective,
) -> Vec<TargetReferenceAuditRow> {
    let can_align = objective.align_slope.is_finite() && objective.align_intercept.is_finite();
    let weight_sum = target_weights.iter().copied().sum::<f64>();
    let mut out = Vec::with_capacity(reference_indices.len());

    for (rank, &ref_idx) in reference_indices.iter().enumerate() {
        let candidate = &candidates[ref_idx];
        let ref_gid = candidate.row.group_id;
        let mut weighted_sum = 0.0;
        let mut count = 0usize;

        for (&target_idx, &weight) in target_indices.iter().zip(target_weights.iter()) {
            let target_gid = candidates[target_idx].row.group_id;
            if let Some(rate) = rate_between(rate_map, ref_gid, target_gid) {
                weighted_sum += rate * weight;
                count += 1;
            }
        }

        let average_winrate_vs_targets = if count == 0 || weight_sum <= 0.0 {
            None
        } else {
            Some(weighted_sum / weight_sum)
        };
        let aligned_c_score_from_targets = average_winrate_vs_targets.and_then(|avg| {
            if can_align {
                Some(objective.align_slope * avg + objective.align_intercept)
            } else {
                None
            }
        });
        let aligned_minus_c_score = aligned_c_score_from_targets.map(|aligned| aligned - candidate.correct_score());
        let abs_aligned_minus_c_score = aligned_minus_c_score.map(|x| x.abs());

        out.push(TargetReferenceAuditRow {
            reference_rank: rank + 1,
            group_id: candidate.row.group_id,
            canonical: candidate.row.canonical.clone(),
            team_name: candidate.row.team_name.clone(),
            root_team_name: candidate.row.root_team_name.clone(),
            correct_rank: candidate.row.pair_rank,
            correct_score: candidate.correct_score(),
            raw_rank: candidate.row.rank,
            raw_score: candidate.row.raw_average_cqd,
            average_winrate_vs_targets,
            target_rate_count: count,
            aligned_c_score_from_targets,
            aligned_minus_c_score,
            abs_aligned_minus_c_score,
            type_label: candidate.row.type_label.clone(),
            simple_type_label: candidate.row.simple_type_label.clone(),
        });
    }

    out
}


fn audit_diff_stats(rows: &[TargetReferenceAuditRow]) -> TargetAuditStats {
    let mut diffs: Vec<f64> = rows
        .iter()
        .filter_map(|row| row.aligned_minus_c_score)
        .filter(|x| x.is_finite())
        .collect();

    if diffs.is_empty() {
        return TargetAuditStats {
            count: 0,
            mean_diff: None,
            mean_abs_diff: None,
            max_abs_diff: None,
            rmse: None,
            p95_abs_diff: None,
        };
    }

    let count = diffs.len();
    let mean_diff = diffs.iter().sum::<f64>() / count as f64;
    let abs: Vec<f64> = diffs.iter().map(|x| x.abs()).collect();
    let mean_abs_diff = abs.iter().sum::<f64>() / count as f64;
    let max_abs_diff = abs.iter().copied().fold(0.0_f64, f64::max);
    let rmse = (diffs.iter().map(|x| x * x).sum::<f64>() / count as f64).sqrt();

    diffs.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
    let p95_idx = ((count.saturating_sub(1)) as f64 * 0.95).round() as usize;
    let p95_abs_diff = diffs[p95_idx.min(count - 1)].abs();

    TargetAuditStats {
        count,
        mean_diff: Some(mean_diff),
        mean_abs_diff: Some(mean_abs_diff),
        max_abs_diff: Some(max_abs_diff),
        rmse: Some(rmse),
        p95_abs_diff: Some(p95_abs_diff),
    }
}

fn average_reference_winrate_for_target(
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

#[derive(Debug, Clone)]
struct ManualWinratePair {
    pair_index: usize,
    group_a: StoredGroup,
    group_b: StoredGroup,
}

#[derive(Debug, Clone)]
struct ComputedManualWinrate {
    pair_index: usize,
    group_a: String,
    group_b: String,
    group_a_id: i64,
    group_b_id: i64,
    lane_size: usize,
    win_rate_a: f64,
    samples: usize,
}

impl ComputedManualWinrate {
    fn from_pair(pair: &ManualWinratePair, win_rate_a: f64, samples: usize) -> Self {
        Self {
            pair_index: pair.pair_index,
            group_a: pair.group_a.canonical.clone(),
            group_b: pair.group_b.canonical.clone(),
            group_a_id: pair.group_a.id,
            group_b_id: pair.group_b.id,
            lane_size: pair.group_a.lane_size,
            win_rate_a,
            samples,
        }
    }
}

fn resolve_manual_winrate_workers(requested_outer_workers: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }

    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(1)
        .min(total.max(1))
}

fn spawn_recompute_job(db: Db, config: RankerConfig, lane: usize, job_id: JobId) {
    task::spawn_blocking(move || {
        if let Err(err) = run_recompute_job(&db, &config, lane, job_id) {
            let error = format!("{err:#}");
            let _ = db.set_job_status(job_id, "failed", Some(&error));
            let group_count = db.load_groups_by_lane_for_run(lane, config.skip_archived).map(|x| x.len()).unwrap_or(0);
            let _ = db.set_lane_status(lane, "error", group_count);
            let _ = db.set_lane_progress(
                lane,
                "error",
                0,
                config.total_rounds,
                0,
                0,
                0,
                &error,
            );
        }
    });
}

fn run_recompute_job(db: &Db, config: &RankerConfig, lane: usize, job_id: JobId) -> anyhow::Result<()> {
    db.set_job_status(job_id, "running", None)?;
    recompute_lane_until_stable(db, lane, config)
        .with_context(|| format!("recompute lane {lane}, job #{job_id}"))?;
    db.set_job_status(job_id, "done", None)?;
    Ok(())
}

fn spawn_constrained_selection_job(db: Db, config: RankerConfig, lane: usize, job_id: JobId, threshold: f64) {
    task::spawn_blocking(move || {
        if let Err(err) = run_constrained_selection_job(&db, &config, lane, job_id, threshold) {
            let error = format!("{err:#}");
            let _ = db.set_job_status(job_id, "failed", Some(&error));
            let group_count = db.lane_results(lane).map(|x| x.len()).unwrap_or(0);
            let _ = db.set_lane_status(lane, "error", group_count);
            let _ = db.set_lane_progress(
                lane,
                "error",
                0,
                config.total_rounds,
                0,
                0,
                0,
                &error,
            );
        }
    });
}

fn run_constrained_selection_job(
    db: &Db,
    config: &RankerConfig,
    lane: usize,
    job_id: JobId,
    threshold: f64,
) -> anyhow::Result<()> {
    db.set_job_status(job_id, "running", None)?;
    calibrate_saved_lane_results(db, lane, config, threshold)
        .with_context(|| format!("calibration lane {lane}, job #{job_id}"))?;
    db.set_job_status(job_id, "done", None)?;
    Ok(())
}

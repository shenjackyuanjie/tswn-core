use std::collections::BTreeSet;

use anyhow::Context;
use tokio::task;

use crate::db::{Db, InsertGroupOutcome};
use crate::model::{
    AddGroupsRequest, AddGroupsResponse, BlockGroupRequest, BlockGroupResponse, BlockGroupsByTextRequest,
    BlockGroupsByTextResponse, ConstrainedSelectionRequest, ConstrainedSelectionResponse, IgnoredGroup, JobId, MergeTeamsRequest,
    MergeTeamsResponse, PurgeLowScoreGroupsRequest, PurgeLowScoreGroupsResponse, RecomputeLaneResponse,
};
use crate::pairwise::{DEFAULT_SELECTION_CQD_THRESHOLD, calibrate_saved_lane_results};
use crate::parser::parse_group;
use crate::ranker::{KICK_AVG_CQD_THRESHOLD, RankerConfig, recompute_lane_until_stable};

#[derive(Clone)]
pub struct AppService {
    pub db: Db,
    pub config: RankerConfig,
}

impl AppService {
    pub fn new(db: Db, config: RankerConfig) -> Self { Self { db, config } }

    pub fn add_groups(&self, req: AddGroupsRequest) -> anyhow::Result<AddGroupsResponse> {
        let AddGroupsRequest {
            groups,
            outer_workers,
            inner_workers,
            skip_archived,
        } = req;
        let config = self.config_with_run_options(outer_workers, inner_workers, skip_archived)?;

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

        let queued_lanes = self.queue_recompute_lanes_with_config(dirty_lanes.into_iter().collect(), config)?;

        Ok(AddGroupsResponse {
            added,
            duplicated,
            ignored,
            queued_lanes,
        })
    }

    pub fn set_group_blocked(&self, group_id: i64, blocked: bool, req: BlockGroupRequest) -> anyhow::Result<BlockGroupResponse> {
        let config = self.config_with_run_options(req.outer_workers, req.inner_workers, req.skip_archived)?;
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

    pub fn set_groups_blocked_by_text(
        &self,
        blocked: bool,
        req: BlockGroupsByTextRequest,
    ) -> anyhow::Result<BlockGroupsByTextResponse> {
        let BlockGroupsByTextRequest {
            groups,
            outer_workers,
            inner_workers,
            skip_archived,
        } = req;
        let config = self.config_with_run_options(outer_workers, inner_workers, skip_archived)?;

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

        let queued_lanes = self.queue_recompute_lanes_with_config(dirty_lanes.into_iter().collect(), config)?;

        Ok(BlockGroupsByTextResponse {
            blocked: blocked_groups.into_iter().collect(),
            unblocked: unblocked_groups.into_iter().collect(),
            ignored,
            queued_lanes,
        })
    }

    pub fn merge_teams(&self, req: MergeTeamsRequest) -> anyhow::Result<MergeTeamsResponse> {
        let config = self.config_with_run_options(req.outer_workers, req.inner_workers, req.skip_archived)?;

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

    pub fn purge_low_score_groups(
        &self,
        lane: usize,
        req: PurgeLowScoreGroupsRequest,
    ) -> anyhow::Result<PurgeLowScoreGroupsResponse> {
        let config = self.config_with_run_options(req.outer_workers, req.inner_workers, req.skip_archived)?;

        let deleted_count = self.db.hard_delete_groups_below_raw_score(lane, KICK_AVG_CQD_THRESHOLD)?;

        let queued_lanes = if deleted_count > 0 {
            self.queue_recompute_lanes_with_config(vec![lane], config)?
        } else {
            Vec::new()
        };

        Ok(PurgeLowScoreGroupsResponse {
            lane_size: lane,
            threshold: KICK_AVG_CQD_THRESHOLD,
            deleted_count,
            queued_lanes,
        })
    }

    pub fn queue_recompute_lanes(&self, lanes: Vec<usize>) -> anyhow::Result<Vec<usize>> {
        self.queue_recompute_lanes_with_config(lanes, self.config.clone())
    }

    pub fn queue_recompute_lanes_with_config(&self, lanes: Vec<usize>, config: RankerConfig) -> anyhow::Result<Vec<usize>> {
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
                    if config.outer_workers == 0 {
                        "dynamic_auto".to_string()
                    } else {
                        format!("static({})", config.outer_workers)
                    },
                    if config.inner_workers == 0 {
                        "auto(0)".to_string()
                    } else {
                        config.inner_workers.to_string()
                    },
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

        let mut config = self.config_with_run_options(outer_workers, inner_workers, skip_archived)?;
        config.stickiness = stickiness;
        let queued_lanes = self.queue_recompute_lanes_with_config(vec![lane], config)?;
        Ok(RecomputeLaneResponse { queued_lanes })
    }

    pub fn queue_constrained_selection_lane(
        &self,
        lane: usize,
        req: ConstrainedSelectionRequest,
    ) -> anyhow::Result<ConstrainedSelectionResponse> {
        let mut config = self.config_with_run_options(req.outer_workers, req.inner_workers, None)?;
        config.inner_workers = 1;

        let threshold = req.cqd_threshold.unwrap_or(DEFAULT_SELECTION_CQD_THRESHOLD);
        if !threshold.is_finite() || !(0.0..=100.0).contains(&threshold) {
            anyhow::bail!("环境阈值必须是 0 到 100 之间的数字");
        }

        let group_count = self.db.lane_results(lane)?.len();
        if group_count == 0 {
            anyhow::bail!("该赛道还没有结果；请先完成一次默认重算");
        }
        let job_id = self.db.create_job(lane, "v49_edge_bagging_stability")?;

        self.db.set_lane_status(lane, "calibrating", group_count)?;
        self.db.set_lane_progress(
            lane,
            "pairwise_stability_queued",
            0,
            config.total_rounds,
            0,
            group_count,
            0,
            &format!(
                "queued v49 edge-bagging stability job #{job_id}, threshold={threshold:.3}, outer_workers={}, inner_threads=1",
                if config.outer_workers == 0 {
                    "dynamic_auto".to_string()
                } else {
                    format!("static({})", config.outer_workers)
                },
            ),
        )?;

        spawn_constrained_selection_job(self.db.clone(), config, lane, job_id, threshold);
        Ok(ConstrainedSelectionResponse {
            queued_lanes: vec![lane],
            cqd_threshold: threshold,
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

fn spawn_recompute_job(db: Db, config: RankerConfig, lane: usize, job_id: JobId) {
    task::spawn_blocking(move || {
        if let Err(err) = run_recompute_job(&db, &config, lane, job_id) {
            let error = format!("{err:#}");
            let _ = db.set_job_status(job_id, "failed", Some(&error));
            let group_count = db.load_groups_by_lane_for_run(lane, config.skip_archived).map(|x| x.len()).unwrap_or(0);
            let _ = db.set_lane_status(lane, "error", group_count);
            let _ = db.set_lane_progress(lane, "error", 0, config.total_rounds, 0, 0, 0, &error);
        }
    });
}

fn run_recompute_job(db: &Db, config: &RankerConfig, lane: usize, job_id: JobId) -> anyhow::Result<()> {
    db.set_job_status(job_id, "running", None)?;

    // v49: 清理旧版本留下的懒删除数据。只物理删除 R-Score < 45；R-Score == 45 不删。
    let pre_deleted = db.hard_delete_groups_below_raw_score(lane, KICK_AVG_CQD_THRESHOLD)?;
    if pre_deleted > 0 {
        let group_count = db.load_groups_by_lane_for_run(lane, config.skip_archived)?.len();
        db.set_lane_progress(
            lane,
            "purging",
            0,
            config.total_rounds,
            0,
            0,
            pre_deleted,
            &format!("physically deleted {pre_deleted} groups with R-Score < {KICK_AVG_CQD_THRESHOLD:.3}; exact 45.000 is kept"),
        )?;
        db.set_lane_status(lane, "purging", group_count)?;
    }

    recompute_lane_until_stable(db, lane, config).with_context(|| format!("recompute lane {lane}, job #{job_id}"))?;
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
            let _ = db.set_lane_progress(lane, "error", 0, config.total_rounds, 0, 0, 0, &error);
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
        .with_context(|| format!("manual pair-score calibration lane {lane}, job #{job_id}"))?;
    db.set_job_status(job_id, "done", None)?;
    Ok(())
}

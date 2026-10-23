//! 组合排名应用门面：导入、屏蔽、重算、校准和靶子生成。
//!
//! 排队接口先持久化任务/赛道状态，再把同步计算移交 Tokio blocking 任务；返回成功
//! 表示已提交，不表示计算完成。同一赛道多次请求当前不会自动合并或串行化。
//! 每次运行克隆配置并固定内部 worker 为 1，外部并行度由请求覆盖；跨步骤没有全局事务。

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::process::Command;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::thread;
use std::time::Instant;

use anyhow::Context;
use tokio::task;

use crate::db::{Db, InsertGroupOutcome};
use crate::model::{
    AddGroupsRequest, AddGroupsResponse, AddWinratesRequest, AddWinratesResponse, AddedWinrateRow, BlockGroupRequest,
    BlockGroupResponse, BlockGroupsByTextRequest, BlockGroupsByTextResponse, ConstrainedSelectionRequest,
    ConstrainedSelectionResponse, IgnoredGroup, IgnoredWinratePair, JobId, MergeTeamsRequest, MergeTeamsResponse,
    RecomputeLaneResponse, StoredGroup, TargetGenerationRequest, TargetGenerationResponse, TargetGenerationRow,
    TargetGenerationSummary, TargetReferenceAuditRow,
};
use crate::pairwise::{calibrate_saved_lane_results, default_selection_cqd_threshold, validate_saved_pair_strength_results};
use crate::parser::{parse_group, parse_member_team};
use crate::ranker::{RankerConfig, recompute_lane_until_stable};
use crate::winrate::compute_rate_without_db;

const TARGET_MILP_SOLVER: &str = include_str!("../tools/target_milp_solver.py");
const RATE_PERSIST_CHECKPOINT_SIZE: usize = 100;
mod compression;
mod groups;
mod jobs;
mod targets;

use compression::*;
use groups::*;
use jobs::*;
use targets::*;

#[derive(Clone)]
pub struct AppService {
    pub db: Db,
    pub config: RankerConfig,
}

impl AppService {
    pub fn new(db: Db, config: RankerConfig) -> Self { Self { db, config } }

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

        let threshold = req
            .raw_score_threshold
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
            raw_score_threshold: threshold,
            cqd_threshold: threshold,
        })
    }

    pub fn validate_pair_strength_lane(
        &self,
        lane: usize,
        req: ConstrainedSelectionRequest,
    ) -> anyhow::Result<serde_json::Value> {
        let mut config = self.config_with_run_options(req.outer_workers, req.inner_workers, None)?;
        config.inner_workers = 1;

        let threshold = req
            .raw_score_threshold
            .or(req.cqd_threshold)
            .unwrap_or_else(|| default_selection_cqd_threshold(lane));
        if !threshold.is_finite() || !(0.0..=100.0).contains(&threshold) {
            anyhow::bail!("Pair 验证 Raw Score 阈值必须是 0 到 100 之间的数字");
        }

        validate_saved_pair_strength_results(&self.db, lane, &config, threshold)
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

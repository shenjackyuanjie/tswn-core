use std::collections::BTreeSet;

use anyhow::Context;
use tokio::task;

use crate::db::{Db, InsertGroupOutcome};
use crate::model::{
    AddGroupsRequest, AddGroupsResponse, IgnoredGroup, JobId, MergeTeamsRequest, MergeTeamsResponse, RecomputeLaneResponse,
};
use crate::parser::parse_group;
use crate::ranker::{RankerConfig, recompute_lane_until_stable};

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
        let config = self.config_with_run_options(outer_workers, inner_workers, skip_archived);

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
                    reason: "archived combination skipped because 跑代码时不跑封存组合 is enabled".to_string(),
                });
                continue;
            }

            match self.db.insert_group(&parsed)? {
                InsertGroupOutcome::Added => {
                    dirty_lanes.insert(parsed.lane_size);
                    added.push(parsed.canonical);
                }
                InsertGroupOutcome::Duplicated => {
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

    pub fn merge_teams(&self, req: MergeTeamsRequest) -> anyhow::Result<MergeTeamsResponse> {
        let config = self.config_with_run_options(req.outer_workers, req.inner_workers, req.skip_archived);

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

    #[allow(dead_code)]
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

        let mut config = self.config_with_run_options(outer_workers, inner_workers, skip_archived);
        config.stickiness = stickiness;
        let queued_lanes = self.queue_recompute_lanes_with_config(vec![lane], config)?;
        Ok(RecomputeLaneResponse { queued_lanes })
    }

    fn config_with_run_options(
        &self,
        outer_workers: Option<usize>,
        inner_workers: Option<u32>,
        skip_archived: Option<bool>,
    ) -> RankerConfig {
        let mut config = self.config.clone();
        if let Some(outer_workers) = outer_workers {
            config.outer_workers = outer_workers;
        }
        if let Some(inner_workers) = inner_workers {
            config.inner_workers = inner_workers;
        }
        if let Some(skip_archived) = skip_archived {
            config.skip_archived = skip_archived;
        }
        config
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
    recompute_lane_until_stable(db, lane, config).with_context(|| format!("recompute lane {lane}, job #{job_id}"))?;
    db.set_job_status(job_id, "done", None)?;
    Ok(())
}

//! Raw/Golden 排名与自动归档的同步编排。
//!
//! 每轮按成员不重复和合并队伍上限选择 50 个标准组合，胜率只按实际用到的列补算。
//! 组数或合法标准集合不足时清空已存结果并进入 waiting；默认归档后继续重算直到稳定。
//! skip_archived=false 才全量复核并替换封存集合。函数会持续更新数据库进度与缓存。

mod algorithm;
mod archive;
mod rates;

use algorithm::*;
use archive::*;
use rates::*;

use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::Instant;

use crate::db::Db;
use crate::model::{GroupId, LaneResultRow, RankNode, StoredGroup};
use crate::team::TeamDsu;
use crate::winrate::compute_rate_without_db;

pub const STANDARD_SIZE: usize = 50;
pub const TEAM_LIMIT: usize = 5;
pub const DEFAULT_WARMUP_ROUNDS: usize = 10_000;
pub const DEFAULT_TOTAL_ROUNDS: usize = 10_000_000;

/// 每条缺失配对边默认采样 100,000 场，不表示统计误差保证或胜率精度。
///
/// 外部可覆盖采样数，但已有缓存读取不据此失效；复测需由宿主管理历史观测。
pub const DEFAULT_WIN_RATE_SAMPLES: usize = 100_000;

/// 默认粘性：单人组 10，双人组 20，x 人组 10 * x。
pub const DEFAULT_STICKINESS_PER_MEMBER: usize = 10;

/// 第 100,000 轮起检查平均 CQD 的三位小数，连续稳定轮数由 EARLY_STOP_STABLE_ROUNDS 指定。
///
/// 这是有限精度的提前终止条件，不等价于所有底层胜率、支持集合或实数分数已经收敛。
pub const EARLY_STOP_START_ROUND: usize = 100_000;
pub const EARLY_STOP_STABLE_ROUNDS: usize = 100;

/// 此轮后，只有已经获得非零 Golden 质量的组继续参与迭代更新。剩余的 Raw 分数会依据最终 Golden 目标重建一次。
pub const RAW_CORE_ONLY_START_ROUND: usize = 100_000;

pub const KICK_AVG_CQD_THRESHOLD: f64 = 45.0;

/// 跑完后自动封存阈值：单人组 47.0；双人/多人组 48.0。
pub const ARCHIVE_AVG_CQD_THRESHOLD_SINGLE: f64 = 47.0;
pub const ARCHIVE_AVG_CQD_THRESHOLD_MULTI: f64 = 48.0;

pub fn archive_avg_cqd_threshold(lane_size: usize) -> f64 {
    if lane_size == 1 {
        ARCHIVE_AVG_CQD_THRESHOLD_SINGLE
    } else {
        ARCHIVE_AVG_CQD_THRESHOLD_MULTI
    }
}

/// 和浏览器折叠 UI 保持一致：每个主行的折叠内部只保留前 20 个子项；超出的子项会被封存。
pub const FOLDED_CHILD_KEEP_LIMIT: usize = 20;

/// 胜率补算支持浏览器传入外层 worker：
///
/// - outer_workers = 0：外层按系统可用并行度自动开 worker，并用动态队列分发 pair。
/// - outer_workers > 0：外层使用指定 worker 数，并按静态区间分块。
/// - inner_workers 固定写死为 1，不再暴露给浏览器端。
#[derive(Debug, Clone)]
pub struct RankerConfig {
    pub warmup_rounds: usize,
    pub total_rounds: usize,
    pub win_rate_samples: usize,
    pub stickiness: Option<usize>,
    pub outer_workers: usize,
    pub inner_workers: u32,
    pub skip_archived: bool,
}

impl RankerConfig {
    pub fn from_env() -> Self {
        Self {
            warmup_rounds: read_env_usize("LANE_WARMUP_ROUNDS", DEFAULT_WARMUP_ROUNDS),
            total_rounds: read_env_usize("LANE_TOTAL_ROUNDS", DEFAULT_TOTAL_ROUNDS),
            win_rate_samples: read_env_usize("LANE_WIN_RATE_SAMPLES", DEFAULT_WIN_RATE_SAMPLES),
            stickiness: read_env_optional_usize("LANE_STICKINESS"),
            outer_workers: read_env_usize("LANE_OUTER_WORKERS", 0),
            inner_workers: 1,
            skip_archived: read_env_bool("LANE_SKIP_ARCHIVED", true),
        }
    }

    pub fn effective_stickiness(&self, lane_size: usize) -> usize {
        self.stickiness.unwrap_or(DEFAULT_STICKINESS_PER_MEMBER * lane_size.max(1)).max(1)
    }
}

fn read_env_usize(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|x| x.parse().ok()).unwrap_or(default)
}

fn read_env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name).ok().map(|x| x.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on") => true,
        Some(value) if matches!(value.as_str(), "0" | "false" | "no" | "n" | "off") => false,
        _ => default,
    }
}

fn read_env_optional_usize(name: &str) -> Option<usize> {
    std::env::var(name).ok().and_then(|x| x.parse::<usize>().ok()).filter(|&x| x > 0)
}

fn resolve_rate_pair_workers(requested_outer_workers: usize, total: usize) -> usize {
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }

    let available = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);

    available.max(1).min(total.max(1))
}

fn format_outer_workers(requested_outer_workers: usize, actual_workers: usize) -> String {
    if requested_outer_workers == 0 {
        format!("auto({actual_workers})")
    } else {
        format!("static({actual_workers})")
    }
}

fn format_inner_workers(inner_workers: u32) -> String {
    if inner_workers == 0 {
        "auto(0)".to_string()
    } else {
        inner_workers.to_string()
    }
}

pub fn recompute_lane_until_stable(db: &Db, lane_size: usize, config: &RankerConfig) -> anyhow::Result<()> {
    let mut recompute_pass = 0usize;

    loop {
        recompute_pass += 1;
        // 日常运行在 skip_archived=true 时只计算未封存项，以保留封存带来的算力收益。
        // 只有用户显式选择跑全量（skip_archived=false）时，才重新计算并替换整个封存集合。
        let reconcile_archives = !config.skip_archived;
        let groups = db.load_groups_by_lane_for_run(lane_size, config.skip_archived)?;

        if groups.len() < STANDARD_SIZE {
            db.clear_lane_results(lane_size)?;
            db.set_lane_status(lane_size, "waiting", groups.len())?;
            db.set_lane_progress(
                lane_size,
                "waiting",
                0,
                config.total_rounds,
                0,
                0,
                0,
                &format!("waiting: {}/{} groups", groups.len(), STANDARD_SIZE),
            )?;
            return Ok(());
        }

        db.set_lane_status(lane_size, "running", groups.len())?;
        db.set_lane_progress(
            lane_size,
            "loading",
            0,
            config.total_rounds,
            0,
            0,
            0,
            &format!(
                "loading lane {lane_size}, pass {recompute_pass}, stickiness={}, outer_workers={}, inner_threads={}, skip_archived={}",
                config.effective_stickiness(lane_size),
                if config.outer_workers == 0 { "dynamic_auto".to_string() } else { format!("static({})", config.outer_workers) },
                format_inner_workers(config.inner_workers),
                config.skip_archived
            ),
        )?;

        let dsu = db.load_team_dsu()?;
        let core_result = run_core_algorithm(db, lane_size, groups, dsu, config)?;
        let final_round = core_result.final_round;
        let early_stopped = core_result.early_stopped;
        let result = core_result.rows;

        if result.is_empty() {
            db.clear_lane_results(lane_size)?;
            db.set_lane_status(lane_size, "waiting", 0)?;
            db.set_lane_progress(
                lane_size,
                "waiting",
                0,
                config.total_rounds,
                0,
                0,
                0,
                "cannot build a 50-group standard list under current constraints",
            )?;
            return Ok(());
        }

        let archive_candidates = core_result.archive_candidates;
        let archive_rows: Vec<(GroupId, String, f64)> = archive_candidates
            .iter()
            .map(|candidate| (candidate.group_id, candidate.reason.clone(), candidate.average_cqd))
            .collect();
        let archived_count = if reconcile_archives {
            db.replace_lane_archived_groups(lane_size, &archive_rows)?
        } else if archive_candidates.is_empty() {
            0
        } else {
            db.archive_group_combinations(&archive_rows)?
        };

        if archive_candidates.is_empty() {
            // Raw 已改变，因此上一次校准的任何 Correct 参考方案都不再描述已保存的 lane 结果。
            db.clear_correct_target_trace(lane_size)?;
            db.save_lane_results(lane_size, &result)?;
            db.set_lane_status(lane_size, "ready", result.len())?;
            let ready_message = if early_stopped {
                format!(
                    "done: early stopped at round {} after avg cqd stayed unchanged for {} rounds at 3 decimals; skip_archived={}",
                    final_round, EARLY_STOP_STABLE_ROUNDS, config.skip_archived
                )
            } else {
                format!("done; skip_archived={}", config.skip_archived)
            };
            db.set_lane_progress(lane_size, "ready", final_round, config.total_rounds, 0, 0, 0, &ready_message)?;
            return Ok(());
        }

        if !config.skip_archived {
            db.clear_correct_target_trace(lane_size)?;
            db.save_lane_results(lane_size, &result)?;
            db.set_lane_status(lane_size, "ready", result.len())?;
            db.set_lane_progress(
                lane_size,
                "ready",
                final_round,
                config.total_rounds,
                0,
                0,
                archived_count,
                &format!(
                    "done; archived/updated {} combinations but kept them in this run because skip_archived=false",
                    archived_count
                ),
            )?;
            return Ok(());
        }

        db.set_lane_progress(
            lane_size,
            "archiving",
            final_round,
            config.total_rounds,
            0,
            0,
            archived_count,
            &format!(
                "archived {} combinations: avg cqd < {} or folded child outside top {}; rerunning with archived combinations skipped",
                archived_count,
                archive_avg_cqd_threshold(lane_size),
                FOLDED_CHILD_KEEP_LIMIT
            ),
        )?;

        // 封存后当前有效组、标准榜、胜率需求和平均 CQD 都可能变化；默认继续重算，直到不再产生新封存组合。
    }
}

fn should_report_round(round: usize, total_rounds: usize) -> bool { round == 1 || round == total_rounds || round % 1000 == 0 }

fn format_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

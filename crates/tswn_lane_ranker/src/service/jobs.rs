//! 后台排名与约束选择任务的启动和状态更新。

use super::*;

pub(super) fn spawn_recompute_job(db: Db, config: RankerConfig, lane: usize, job_id: JobId) {
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

pub(super) fn run_recompute_job(db: &Db, config: &RankerConfig, lane: usize, job_id: JobId) -> anyhow::Result<()> {
    db.set_job_status(job_id, "running", None)?;
    recompute_lane_until_stable(db, lane, config).with_context(|| format!("recompute lane {lane}, job #{job_id}"))?;
    db.set_job_status(job_id, "done", None)?;
    Ok(())
}

pub(super) fn spawn_constrained_selection_job(db: Db, config: RankerConfig, lane: usize, job_id: JobId, threshold: f64) {
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

pub(super) fn run_constrained_selection_job(
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

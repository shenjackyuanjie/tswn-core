//! SQLite 同步存储门面，共享连接与应用级任务状态。
//!
//! 所有 Db 克隆共用一个 Mutex 保护的连接，初始化时开启外键并补齐兼容列。
//! 单方法中显式创建的事务只覆盖该批写入，不覆盖后续计算、状态或其他方法。
//! path 供外部 Python 与独立连接读取，因此实际校准需要文件数据库而非 :memory:。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{
    CorrectTargetTrace, CorrectTargetTraceWeight, GroupId, JobId, LaneJob, LaneProgress, LaneResultRow, LaneStatus, StoredGroup,
};
use crate::parser::ParsedGroup;
use crate::team::TeamDsu;
mod groups;
mod results;
mod schema;

pub use groups::InsertGroupOutcome;

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    path: Arc<String>,
}

impl Db {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("open sqlite database: {path}"))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            path: Arc::new(path.to_string()),
        };
        db.init()?;
        Ok(db)
    }

    pub fn path(&self) -> &str { self.path.as_str() }

    pub fn create_job(&self, lane_size: usize, kind: &str) -> anyhow::Result<JobId> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO lane_jobs (lane_size, kind, status, updated_at)
             VALUES (?1, ?2, 'queued', CURRENT_TIMESTAMP)",
            params![lane_size as i64, kind],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn set_job_status(&self, job_id: JobId, status: &str, error: Option<&str>) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE lane_jobs SET status = ?1, error = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?3",
            params![status, error, job_id],
        )?;
        Ok(())
    }

    pub fn job(&self, job_id: JobId) -> anyhow::Result<Option<LaneJob>> {
        let conn = self.conn.lock().unwrap();
        let job = conn
            .query_row(
                "SELECT id, lane_size, kind, status, error FROM lane_jobs WHERE id = ?1",
                params![job_id],
                |row| {
                    Ok(LaneJob {
                        id: row.get(0)?,
                        lane_size: row.get::<_, i64>(1)? as usize,
                        kind: row.get(2)?,
                        status: row.get(3)?,
                        error: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(job)
    }

    pub fn load_team_dsu(&self) -> anyhow::Result<TeamDsu> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT name, parent FROM teams")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut pairs = Vec::new();
        for row in rows {
            pairs.push(row?);
        }
        Ok(TeamDsu::from_pairs(pairs))
    }

    pub fn save_team_dsu(&self, dsu: &TeamDsu) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (name, parent) in dsu.pairs() {
            tx.execute(
                "INSERT OR REPLACE INTO teams (name, parent) VALUES (?1, ?2)",
                params![name, parent],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_group;

    #[test]
    fn correct_target_trace_roundtrips_and_is_invalidated_with_lane_results() {
        let db = Db::open(":memory:").unwrap();
        let parsed = parse_group("alice@A").unwrap();
        let group_id = match db.insert_group(&parsed).unwrap() {
            InsertGroupOutcome::Added(id) => id,
            InsertGroupOutcome::Duplicated(id) => id,
        };
        let trace = CorrectTargetTrace {
            trace_version: "raw_correct_stable_backprop_v8".to_string(),
            score_mode: "exact_k5_replacement_correct_big_target".to_string(),
            metadata_json: r#"{"lane_size":1}"#.to_string(),
            weights: vec![CorrectTargetTraceWeight {
                reference_scope: "common_correct_candidate_coefficients".to_string(),
                group_id,
                reference_weight: 1.0,
                nominal_weight: 0.75,
                raw_golden_weight: 0.5,
                common_coefficient: 0.015,
                coefficient_mean: 0.014,
                coefficient_stddev: 0.002,
                correct_target_weight: 0.75,
                source: "test_row_coefficient_projection".to_string(),
            }],
        };

        db.replace_correct_target_trace(1, &trace).unwrap();
        let loaded = db.correct_target_trace(1).unwrap().unwrap();
        assert_eq!(loaded.trace_version, trace.trace_version);
        assert_eq!(loaded.score_mode, trace.score_mode);
        assert_eq!(loaded.weights.len(), 1);
        assert_eq!(loaded.weights[0].group_id, group_id);
        assert_eq!(loaded.weights[0].reference_weight, 1.0);

        db.clear_lane_results(1).unwrap();
        assert!(db.correct_target_trace(1).unwrap().is_none());
    }

    #[test]
    fn replacing_lane_archives_restores_groups_omitted_from_the_new_set() {
        let db = Db::open(":memory:").unwrap();
        let first = parse_group("alice@A").unwrap();
        let second = parse_group("bob@B").unwrap();
        let first_id = match db.insert_group(&first).unwrap() {
            InsertGroupOutcome::Added(id) | InsertGroupOutcome::Duplicated(id) => id,
        };
        let second_id = match db.insert_group(&second).unwrap() {
            InsertGroupOutcome::Added(id) | InsertGroupOutcome::Duplicated(id) => id,
        };

        db.archive_group_combination(first_id, "old_threshold", 47.2).unwrap();
        db.archive_group_combination(second_id, "still_below_threshold", 46.8).unwrap();
        assert!(db.load_groups_by_lane_for_run(1, true).unwrap().is_empty());

        db.replace_lane_archived_groups(1, &[(second_id, "still_below_threshold".to_string(), 46.8)])
            .unwrap();

        let active = db.load_groups_by_lane_for_run(1, true).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, first_id);
    }
}

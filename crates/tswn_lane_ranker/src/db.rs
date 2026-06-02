use std::sync::{Arc, Mutex};

use anyhow::Context;
use rusqlite::{Connection, OptionalExtension, params};

use crate::model::{GroupId, JobId, LaneJob, LaneProgress, LaneResultRow, LaneStatus, StoredGroup};
use crate::parser::ParsedGroup;
use crate::team::TeamDsu;

fn ensure_lane_result_column(conn: &Connection, name: &str, definition: &str) -> anyhow::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(lane_results)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;

    for row in rows {
        if row? == name {
            return Ok(());
        }
    }

    conn.execute(&format!("ALTER TABLE lane_results ADD COLUMN {name} {definition}"), [])?;
    Ok(())
}

#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path).with_context(|| format!("open sqlite database: {path}"))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self { conn: Arc::new(Mutex::new(conn)) };
        db.init()?;
        Ok(db)
    }

    fn init(&self) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical TEXT NOT NULL UNIQUE,
                display_raw TEXT NOT NULL,
                lane_size INTEGER NOT NULL,
                team_name TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS group_members (
                group_id INTEGER NOT NULL,
                member TEXT NOT NULL,
                position INTEGER NOT NULL,
                PRIMARY KEY (group_id, member),
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS teams (
                name TEXT PRIMARY KEY,
                parent TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS group_rates (
                group_a INTEGER NOT NULL,
                group_b INTEGER NOT NULL,
                win_rate_a REAL NOT NULL,
                samples INTEGER NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (group_a, group_b),
                FOREIGN KEY (group_a) REFERENCES groups(id) ON DELETE CASCADE,
                FOREIGN KEY (group_b) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS lane_results (
                lane_size INTEGER NOT NULL,
                group_id INTEGER NOT NULL,
                rank INTEGER NOT NULL,
                average_cqd REAL NOT NULL,
                raw_average_cqd REAL,
                pair_score REAL,
                pair_rank INTEGER,
                uncertainty REAL,
                raw_delta REAL,
                pair_score_std REAL,
                pair_rank_std REAL,
                delta_std REAL,
                edge_count_mean REAL,
                stability_flag TEXT NOT NULL DEFAULT 'calibration_skipped',
                marginal_value REAL,
                constrained_rank INTEGER,
                selection_status TEXT NOT NULL DEFAULT 'calibration_skipped',
                min_cqd REAL NOT NULL,
                max_cqd REAL NOT NULL,
                variance_cqd REAL NOT NULL,
                golden_rate REAL NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (lane_size, group_id),
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS lane_status (
                lane_size INTEGER PRIMARY KEY,
                status TEXT NOT NULL,
                group_count INTEGER NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS lane_progress (
                lane_size INTEGER PRIMARY KEY,
                phase TEXT NOT NULL,
                round INTEGER NOT NULL DEFAULT 0,
                total_rounds INTEGER NOT NULL DEFAULT 0,
                rate_done INTEGER NOT NULL DEFAULT 0,
                rate_total INTEGER NOT NULL DEFAULT 0,
                kicked_count INTEGER NOT NULL DEFAULT 0,
                message TEXT NOT NULL DEFAULT '',
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS lane_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lane_size INTEGER NOT NULL,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                error TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS archived_groups (
                group_id INTEGER PRIMARY KEY,
                canonical TEXT NOT NULL UNIQUE,
                lane_size INTEGER NOT NULL,
                reason TEXT NOT NULL,
                average_cqd REAL,
                archived_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_archived_groups_lane_size
                ON archived_groups(lane_size);

            CREATE TABLE IF NOT EXISTS blocked_groups (
                group_id INTEGER PRIMARY KEY,
                canonical TEXT NOT NULL UNIQUE,
                lane_size INTEGER NOT NULL,
                reason TEXT NOT NULL DEFAULT 'manual',
                blocked_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_blocked_groups_lane_size
                ON blocked_groups(lane_size);
            "#,
        )?;

        ensure_lane_result_column(&conn, "raw_average_cqd", "REAL")?;
        ensure_lane_result_column(&conn, "pair_score", "REAL")?;
        ensure_lane_result_column(&conn, "pair_rank", "INTEGER")?;
        ensure_lane_result_column(&conn, "uncertainty", "REAL")?;
        ensure_lane_result_column(&conn, "raw_delta", "REAL")?;
        ensure_lane_result_column(&conn, "pair_score_std", "REAL")?;
        ensure_lane_result_column(&conn, "pair_rank_std", "REAL")?;
        ensure_lane_result_column(&conn, "delta_std", "REAL")?;
        ensure_lane_result_column(&conn, "edge_count_mean", "REAL")?;
        ensure_lane_result_column(&conn, "stability_flag", "TEXT NOT NULL DEFAULT 'calibration_skipped'")?;
        ensure_lane_result_column(&conn, "marginal_value", "REAL")?;
        ensure_lane_result_column(&conn, "constrained_rank", "INTEGER")?;
        ensure_lane_result_column(&conn, "selection_status", "TEXT NOT NULL DEFAULT 'calibration_skipped'")?;

        Ok(())
    }

    pub fn insert_group(&self, parsed: &ParsedGroup) -> anyhow::Result<InsertGroupOutcome> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let existing: Option<GroupId> = tx
            .query_row(
                "SELECT id FROM groups WHERE canonical = ?1",
                params![parsed.canonical],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(id) = existing {
            return Ok(InsertGroupOutcome::Duplicated(id));
        }

        tx.execute(
            "INSERT INTO groups (canonical, display_raw, lane_size, team_name) VALUES (?1, ?2, ?3, ?4)",
            params![parsed.canonical, parsed.display_raw, parsed.lane_size as i64, parsed.team_name],
        )?;
        let group_id = tx.last_insert_rowid();

        for (position, member) in parsed.members.iter().enumerate() {
            tx.execute(
                "INSERT INTO group_members (group_id, member, position) VALUES (?1, ?2, ?3)",
                params![group_id, member, position as i64],
            )?;
        }

        tx.execute(
            "INSERT OR IGNORE INTO teams (name, parent) VALUES (?1, ?1)",
            params![parsed.team_name],
        )?;

        tx.commit()?;
        Ok(InsertGroupOutcome::Added(group_id))
    }

    pub fn load_groups_by_lane(&self, lane_size: usize) -> anyhow::Result<Vec<StoredGroup>> {
        self.load_groups_by_lane_for_run(lane_size, false)
    }

    pub fn load_groups_by_lane_for_run(
        &self,
        lane_size: usize,
        skip_archived: bool,
    ) -> anyhow::Result<Vec<StoredGroup>> {
        let conn = self.conn.lock().unwrap();
        let sql = if skip_archived {
            "SELECT g.id, g.canonical, g.display_raw, g.lane_size, g.team_name,
                    CASE WHEN b.group_id IS NULL THEN 0 ELSE 1 END AS is_blocked
             FROM groups g
             LEFT JOIN archived_groups a ON a.group_id = g.id
             LEFT JOIN blocked_groups b ON b.group_id = g.id
             WHERE g.lane_size = ?1 AND a.group_id IS NULL
             ORDER BY g.id ASC"
        } else {
            "SELECT g.id, g.canonical, g.display_raw, g.lane_size, g.team_name,
                    CASE WHEN b.group_id IS NULL THEN 0 ELSE 1 END AS is_blocked
             FROM groups g
             LEFT JOIN blocked_groups b ON b.group_id = g.id
             WHERE g.lane_size = ?1
             ORDER BY g.id ASC"
        };

        let mut stmt = conn.prepare(sql)?;

        let rows = stmt.query_map(params![lane_size as i64], |row| {
            Ok(StoredGroup {
                id: row.get(0)?,
                canonical: row.get(1)?,
                display_raw: row.get(2)?,
                lane_size: row.get::<_, i64>(3)? as usize,
                team_name: row.get(4)?,
                members: Vec::new(),
                is_blocked: row.get::<_, i64>(5)? != 0,
            })
        })?;

        let mut groups = Vec::new();
        for row in rows {
            let mut group = row?;
            group.members = load_members_locked(&conn, group.id)?;
            groups.push(group);
        }
        Ok(groups)
    }

    pub fn is_group_archived_by_canonical(&self, canonical: &str) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let exists = conn
            .query_row(
                "SELECT 1 FROM archived_groups WHERE canonical = ?1 LIMIT 1",
                params![canonical],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(exists)
    }

    pub fn find_group_by_canonical(&self, canonical: &str) -> anyhow::Result<Option<(GroupId, usize, String)>> {
        let conn = self.conn.lock().unwrap();
        let group = conn
            .query_row(
                "SELECT id, lane_size, canonical FROM groups WHERE canonical = ?1",
                params![canonical],
                |row| Ok((row.get::<_, GroupId>(0)?, row.get::<_, i64>(1)? as usize, row.get::<_, String>(2)?)),
            )
            .optional()?;
        Ok(group)
    }

    pub fn all_nonempty_lanes(&self) -> anyhow::Result<Vec<usize>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT DISTINCT lane_size FROM groups ORDER BY lane_size ASC")?;
        let rows = stmt.query_map([], |row| Ok(row.get::<_, i64>(0)? as usize))?;
        let mut lanes = Vec::new();
        for row in rows {
            lanes.push(row?);
        }
        Ok(lanes)
    }

    pub fn get_rate(&self, a: GroupId, b: GroupId) -> anyhow::Result<Option<f64>> {
        if a == b {
            return Ok(Some(50.0));
        }
        let conn = self.conn.lock().unwrap();
        let rate = conn
            .query_row(
                "SELECT win_rate_a FROM group_rates WHERE group_a = ?1 AND group_b = ?2",
                params![a, b],
                |row| row.get(0),
            )
            .optional()?;
        Ok(rate)
    }

    pub fn save_rate_pair(&self, a: GroupId, b: GroupId, win_rate_a: f64, samples: usize) -> anyhow::Result<()> {
        if a == b {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO group_rates (group_a, group_b, win_rate_a, samples, updated_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
            params![a, b, win_rate_a, samples as i64],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO group_rates (group_a, group_b, win_rate_a, samples, updated_at)
             VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
            params![b, a, 100.0 - win_rate_a, samples as i64],
        )?;
        Ok(())
    }

    pub fn save_rate_pairs_bulk(
        &self,
        rates: &[(GroupId, GroupId, f64)],
        samples: usize,
    ) -> anyhow::Result<()> {
        if rates.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        for &(a, b, win_rate_a) in rates {
            if a == b {
                continue;
            }

            tx.execute(
                "INSERT OR REPLACE INTO group_rates (group_a, group_b, win_rate_a, samples, updated_at)
                 VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
                params![a, b, win_rate_a, samples as i64],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO group_rates (group_a, group_b, win_rate_a, samples, updated_at)
                 VALUES (?1, ?2, ?3, ?4, CURRENT_TIMESTAMP)",
                params![b, a, 100.0 - win_rate_a, samples as i64],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn delete_group_and_rates(&self, group_id: GroupId) -> anyhow::Result<()> {
        self.hard_delete_group_ids(&[group_id])?;
        Ok(())
    }

    /// 物理删除一批组合。不是 archived_groups / blocked_groups 标记，而是直接从 groups 表删除；
    /// group_members、group_rates、lane_results、archived_groups、blocked_groups 会一并清掉。
    pub fn hard_delete_group_ids(&self, group_ids: &[GroupId]) -> anyhow::Result<usize> {
        if group_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut deleted = 0usize;

        for &group_id in group_ids {
            tx.execute("DELETE FROM group_rates WHERE group_a = ?1 OR group_b = ?1", params![group_id])?;
            tx.execute("DELETE FROM lane_results WHERE group_id = ?1", params![group_id])?;
            tx.execute("DELETE FROM archived_groups WHERE group_id = ?1", params![group_id])?;
            tx.execute("DELETE FROM blocked_groups WHERE group_id = ?1", params![group_id])?;
            tx.execute("DELETE FROM group_members WHERE group_id = ?1", params![group_id])?;
            deleted += tx.execute("DELETE FROM groups WHERE id = ?1", params![group_id])?;
        }

        tx.commit()?;
        Ok(deleted)
    }

    pub fn hard_delete_group_combinations(
        &self,
        candidates: &[(GroupId, String, f64)],
    ) -> anyhow::Result<usize> {
        let ids: Vec<GroupId> = candidates.iter().map(|(group_id, _, _)| *group_id).collect();
        self.hard_delete_group_ids(&ids)
    }

    /// 按已有结果/封存记录物理删除 R-Score < threshold 的组合。
    /// 注意是严格小于：R-Score == 45.000 不会被这条规则删除。
    pub fn hard_delete_groups_below_raw_score(
        &self,
        lane_size: usize,
        threshold: f64,
    ) -> anyhow::Result<usize> {
        let group_ids = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT DISTINCT g.id
                 FROM groups g
                 LEFT JOIN lane_results r
                   ON r.group_id = g.id AND r.lane_size = g.lane_size
                 LEFT JOIN archived_groups a
                   ON a.group_id = g.id
                 WHERE g.lane_size = ?1
                   AND (
                     (r.group_id IS NOT NULL AND COALESCE(r.raw_average_cqd, r.average_cqd) < ?2)
                     OR
                     (a.group_id IS NOT NULL AND a.average_cqd IS NOT NULL AND a.average_cqd < ?2)
                   )",
            )?;
            let rows = stmt.query_map(params![lane_size as i64, threshold], |row| row.get(0))?;
            let mut ids = Vec::new();
            for row in rows {
                ids.push(row?);
            }
            ids
        };

        self.hard_delete_group_ids(&group_ids)
    }

    pub fn archive_group_combination(
        &self,
        group_id: GroupId,
        reason: &str,
        average_cqd: f64,
    ) -> anyhow::Result<bool> {
        let conn = self.conn.lock().unwrap();
        let group: Option<(String, usize)> = conn
            .query_row(
                "SELECT canonical, lane_size FROM groups WHERE id = ?1",
                params![group_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
            )
            .optional()?;

        let Some((canonical, lane_size)) = group else {
            return Ok(false);
        };

        conn.execute(
            "INSERT INTO archived_groups
             (group_id, canonical, lane_size, reason, average_cqd, archived_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
             ON CONFLICT(group_id) DO UPDATE SET
               canonical = excluded.canonical,
               lane_size = excluded.lane_size,
               reason = excluded.reason,
               average_cqd = excluded.average_cqd,
               updated_at = CURRENT_TIMESTAMP",
            params![group_id, canonical, lane_size as i64, reason, average_cqd],
        )?;

        Ok(true)
    }

    pub fn archive_group_combinations(
        &self,
        candidates: &[(GroupId, String, f64)],
    ) -> anyhow::Result<usize> {
        let mut archived = 0usize;
        for (group_id, reason, average_cqd) in candidates {
            if self.archive_group_combination(*group_id, reason, *average_cqd)? {
                archived += 1;
            }
        }
        Ok(archived)
    }


    pub fn set_group_blocked(&self, group_id: GroupId, blocked: bool) -> anyhow::Result<Option<(usize, String)>> {
        let conn = self.conn.lock().unwrap();
        let group: Option<(String, usize)> = conn
            .query_row(
                "SELECT canonical, lane_size FROM groups WHERE id = ?1",
                params![group_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
            )
            .optional()?;

        let Some((canonical, lane_size)) = group else {
            return Ok(None);
        };

        if blocked {
            // 手动屏蔽的组合必须继续参与 Score/CQD 计算；如果它之前被自动封存，先从封存表恢复出来。
            conn.execute("DELETE FROM archived_groups WHERE group_id = ?1", params![group_id])?;

            conn.execute(
                "INSERT INTO blocked_groups
                 (group_id, canonical, lane_size, reason, blocked_at, updated_at)
                 VALUES (?1, ?2, ?3, 'manual', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                 ON CONFLICT(group_id) DO UPDATE SET
                   canonical = excluded.canonical,
                   lane_size = excluded.lane_size,
                   reason = 'manual',
                   updated_at = CURRENT_TIMESTAMP",
                params![group_id, canonical.as_str(), lane_size as i64],
            )?;
        } else {
            conn.execute("DELETE FROM blocked_groups WHERE group_id = ?1", params![group_id])?;
        }

        Ok(Some((lane_size, canonical)))
    }

    pub fn set_lane_status(&self, lane_size: usize, status: &str, group_count: usize) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO lane_status (lane_size, status, group_count, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)",
            params![lane_size as i64, status, group_count as i64],
        )?;
        Ok(())
    }

    pub fn set_lane_progress(
        &self,
        lane_size: usize,
        phase: &str,
        round: usize,
        total_rounds: usize,
        rate_done: usize,
        rate_total: usize,
        kicked_count: usize,
        message: &str,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO lane_progress
             (lane_size, phase, round, total_rounds, rate_done, rate_total, kicked_count, message, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, CURRENT_TIMESTAMP)",
            params![
                lane_size as i64,
                phase,
                round as i64,
                total_rounds as i64,
                rate_done as i64,
                rate_total as i64,
                kicked_count as i64,
                message,
            ],
        )?;
        Ok(())
    }

    pub fn lane_progress(&self, lane_size: usize) -> anyhow::Result<Option<LaneProgress>> {
        let conn = self.conn.lock().unwrap();
        let progress = conn
            .query_row(
                "SELECT lane_size, phase, round, total_rounds, rate_done, rate_total, kicked_count, message
                 FROM lane_progress WHERE lane_size = ?1",
                params![lane_size as i64],
                |row| {
                    Ok(LaneProgress {
                        lane_size: row.get::<_, i64>(0)? as usize,
                        phase: row.get(1)?,
                        round: row.get::<_, i64>(2)? as usize,
                        total_rounds: row.get::<_, i64>(3)? as usize,
                        rate_done: row.get::<_, i64>(4)? as usize,
                        rate_total: row.get::<_, i64>(5)? as usize,
                        kicked_count: row.get::<_, i64>(6)? as usize,
                        message: row.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(progress)
    }

    pub fn clear_lane_results(&self, lane_size: usize) -> anyhow::Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM lane_results WHERE lane_size = ?1", params![lane_size as i64])?;
        Ok(())
    }

    pub fn save_lane_results(&self, lane_size: usize, rows: &[LaneResultRow]) -> anyhow::Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM lane_results WHERE lane_size = ?1", params![lane_size as i64])?;
        for row in rows {
            tx.execute(
                "INSERT INTO lane_results
                 (lane_size, group_id, rank, average_cqd, raw_average_cqd, pair_score, pair_rank, uncertainty, raw_delta, pair_score_std, pair_rank_std, delta_std, edge_count_mean, stability_flag, marginal_value, constrained_rank, selection_status, min_cqd, max_cqd, variance_cqd, golden_rate, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, CURRENT_TIMESTAMP)",
                params![
                    lane_size as i64,
                    row.group_id,
                    row.rank as i64,
                    row.average_cqd,
                    row.raw_average_cqd,
                    row.pair_score,
                    row.pair_rank.map(|x| x as i64),
                    row.uncertainty,
                    row.raw_delta,
                    row.pair_score_std,
                    row.pair_rank_std,
                    row.delta_std,
                    row.edge_count_mean,
                    row.stability_flag,
                    row.marginal_value,
                    row.constrained_rank.map(|x| x as i64),
                    row.selection_status,
                    row.min_cqd,
                    row.max_cqd,
                    row.variance_cqd,
                    row.golden_rate,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn lane_statuses(&self) -> anyhow::Result<Vec<LaneStatus>> {
        let tuples = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT lane_size, status, group_count FROM lane_status ORDER BY lane_size ASC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? as usize,
                ))
            })?;

            let mut tuples = Vec::new();
            for row in rows {
                tuples.push(row?);
            }
            tuples
        };

        let mut out = Vec::with_capacity(tuples.len());
        for (lane_size, status, group_count) in tuples {
            let progress = self.lane_progress(lane_size)?;
            out.push(LaneStatus {
                lane_size,
                status,
                group_count,
                progress,
            });
        }
        Ok(out)
    }

    pub fn lane_top_score_group_ids(
        &self,
        lane_size: usize,
        limit: usize,
        min_average_cqd: Option<f64>,
    ) -> anyhow::Result<Vec<GroupId>> {
        let conn = self.conn.lock().unwrap();
        let mut ids = Vec::new();

        if let Some(min_average_cqd) = min_average_cqd {
            let mut stmt = conn.prepare(
                "SELECT r.group_id
                 FROM lane_results r
                 JOIN groups g ON g.id = r.group_id
                 WHERE r.lane_size = ?1
                   AND COALESCE(r.raw_average_cqd, r.average_cqd) >= ?2
                 ORDER BY COALESCE(r.raw_average_cqd, r.average_cqd) DESC, r.rank ASC
                 LIMIT ?3",
            )?;

            let rows = stmt.query_map(
                params![lane_size as i64, min_average_cqd, limit as i64],
                |row| row.get(0),
            )?;

            for row in rows {
                ids.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT r.group_id
                 FROM lane_results r
                 JOIN groups g ON g.id = r.group_id
                 WHERE r.lane_size = ?1
                 ORDER BY COALESCE(r.raw_average_cqd, r.average_cqd) DESC, r.rank ASC
                 LIMIT ?2",
            )?;

            let rows = stmt.query_map(params![lane_size as i64, limit as i64], |row| row.get(0))?;

            for row in rows {
                ids.push(row?);
            }
        }

        Ok(ids)
    }


    pub fn lane_results(&self, lane_size: usize) -> anyhow::Result<Vec<LaneResultRow>> {
        let conn = self.conn.lock().unwrap();

        let mut team_stmt = conn.prepare("SELECT name, parent FROM teams")?;
        let team_rows = team_stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut team_pairs = Vec::new();
        for row in team_rows {
            team_pairs.push(row?);
        }
        drop(team_stmt);
        let dsu = TeamDsu::from_pairs(team_pairs);

        let mut stmt = conn.prepare(
            "SELECT r.lane_size, r.group_id, r.rank, g.canonical, g.team_name,
                    r.average_cqd, COALESCE(r.raw_average_cqd, r.average_cqd) AS raw_average_cqd,
                    r.pair_score, r.pair_rank, r.uncertainty, r.raw_delta,
                    r.pair_score_std, r.pair_rank_std, r.delta_std, r.edge_count_mean,
                    COALESCE(r.stability_flag, COALESCE(r.selection_status, 'calibration_skipped')) AS stability_flag,
                    r.marginal_value, r.constrained_rank, COALESCE(r.selection_status, 'calibration_skipped') AS selection_status,
                    r.min_cqd, r.max_cqd, r.variance_cqd, r.golden_rate,
                    CASE WHEN b.group_id IS NULL THEN 0 ELSE 1 END AS is_blocked
             FROM lane_results r
             JOIN groups g ON g.id = r.group_id
             LEFT JOIN blocked_groups b ON b.group_id = r.group_id
             WHERE r.lane_size = ?1
             ORDER BY r.rank ASC",
        )?;

        let rows = stmt.query_map(params![lane_size as i64], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, GroupId>(1)?,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
                row.get::<_, f64>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, Option<i64>>(8)?.map(|x| x as usize),
                row.get::<_, Option<f64>>(9)?,
                row.get::<_, Option<f64>>(10)?,
                row.get::<_, Option<f64>>(11)?,
                row.get::<_, Option<f64>>(12)?,
                row.get::<_, Option<f64>>(13)?,
                row.get::<_, Option<f64>>(14)?,
                row.get::<_, String>(15)?,
                row.get::<_, Option<f64>>(16)?,
                row.get::<_, Option<i64>>(17)?.map(|x| x as usize),
                row.get::<_, String>(18)?,
                row.get::<_, f64>(19)?,
                row.get::<_, f64>(20)?,
                row.get::<_, f64>(21)?,
                row.get::<_, f64>(22)?,
                row.get::<_, i64>(23)? != 0,
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (
                lane_size,
                group_id,
                rank,
                canonical,
                team_name,
                average_cqd,
                raw_average_cqd,
                pair_score,
                pair_rank,
                uncertainty,
                raw_delta,
                pair_score_std,
                pair_rank_std,
                delta_std,
                edge_count_mean,
                stability_flag,
                marginal_value,
                constrained_rank,
                selection_status,
                min_cqd,
                max_cqd,
                variance_cqd,
                golden_rate,
                is_blocked,
            ) = row?;
            let root_team_name = dsu.find_readonly(&team_name);
            let skill_summary = crate::skill_eq::compute_group_skill_summary_from_canonical(&canonical);

            out.push(LaneResultRow {
                lane_size,
                group_id,
                rank,
                canonical: skill_summary.display_canonical,
                team_name,
                root_team_name,
                average_cqd,
                raw_average_cqd,
                pair_score,
                pair_rank,
                uncertainty,
                raw_delta,
                pair_score_std,
                pair_rank_std,
                delta_std,
                edge_count_mean,
                stability_flag,
                marginal_value,
                constrained_rank,
                selection_status,
                type_label: skill_summary.type_label,
                skill_totals: skill_summary.skill_totals,
                min_cqd,
                max_cqd,
                variance_cqd,
                golden_rate,
                is_blocked,
            });
        }
        Ok(out)
    }


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

#[derive(Debug, Clone, Copy)]
pub enum InsertGroupOutcome {
    Added(GroupId),
    Duplicated(GroupId),
}

fn load_members_locked(conn: &Connection, group_id: GroupId) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT member FROM group_members WHERE group_id = ?1 ORDER BY position ASC",
    )?;
    let rows = stmt.query_map(params![group_id], |row| row.get(0))?;
    let mut members = Vec::new();
    for row in rows {
        members.push(row?);
    }
    Ok(members)
}

//! 组合、胜率、封禁与归档存储。
//!
//! 插入组合时在一个事务中保存规范键、成员顺序和原始队伍；重复键复用既有 ID。
//! 胜率批次同时写 a→b 与 100-rate 的 b→a，自身查询固定为 50；读取不按采样数或
//! 引擎版本过滤。屏蔽会解除自动封存，但不会删除组合和已有胜率。

use super::*;

impl Db {
    pub fn insert_group(&self, parsed: &ParsedGroup) -> anyhow::Result<InsertGroupOutcome> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        let existing: Option<GroupId> = tx
            .query_row("SELECT id FROM groups WHERE canonical = ?1", params![parsed.canonical], |row| {
                row.get(0)
            })
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

    pub fn load_groups_by_lane_for_run(&self, lane_size: usize, skip_archived: bool) -> anyhow::Result<Vec<StoredGroup>> {
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
                |row| {
                    Ok((
                        row.get::<_, GroupId>(0)?,
                        row.get::<_, i64>(1)? as usize,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        Ok(group)
    }

    pub fn find_stored_group_by_canonical(&self, canonical: &str) -> anyhow::Result<Option<StoredGroup>> {
        let conn = self.conn.lock().unwrap();
        let group = conn
            .query_row(
                "SELECT g.id, g.canonical, g.display_raw, g.lane_size, g.team_name,
                        CASE WHEN b.group_id IS NULL THEN 0 ELSE 1 END AS is_blocked
                 FROM groups g
                 LEFT JOIN blocked_groups b ON b.group_id = g.id
                 WHERE g.canonical = ?1",
                params![canonical],
                |row| {
                    Ok(StoredGroup {
                        id: row.get(0)?,
                        canonical: row.get(1)?,
                        display_raw: row.get(2)?,
                        lane_size: row.get::<_, i64>(3)? as usize,
                        team_name: row.get(4)?,
                        members: Vec::new(),
                        is_blocked: row.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .optional()?;

        let Some(mut group) = group else {
            return Ok(None);
        };
        group.members = load_members_locked(&conn, group.id)?;
        Ok(Some(group))
    }
    pub fn get_group(&self, group_id: GroupId) -> anyhow::Result<Option<StoredGroup>> {
        let conn = self.conn.lock().unwrap();
        let group = conn
            .query_row(
                "SELECT g.id, g.canonical, g.display_raw, g.lane_size, g.team_name,
                        CASE WHEN b.group_id IS NULL THEN 0 ELSE 1 END AS is_blocked
                 FROM groups g
                 LEFT JOIN blocked_groups b ON b.group_id = g.id
                 WHERE g.id = ?1",
                params![group_id],
                |row| {
                    Ok(StoredGroup {
                        id: row.get(0)?,
                        canonical: row.get(1)?,
                        display_raw: row.get(2)?,
                        lane_size: row.get::<_, i64>(3)? as usize,
                        team_name: row.get(4)?,
                        members: Vec::new(),
                        is_blocked: row.get::<_, i64>(5)? != 0,
                    })
                },
            )
            .optional()?;

        let Some(mut group) = group else {
            return Ok(None);
        };

        group.members = load_members_locked(&conn, group.id)?;
        Ok(Some(group))
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

    pub fn save_rate_pairs_bulk(&self, rates: &[(GroupId, GroupId, f64)], samples: usize) -> anyhow::Result<()> {
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

    pub fn archive_group_combination(&self, group_id: GroupId, reason: &str, average_cqd: f64) -> anyhow::Result<bool> {
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

    pub fn archive_group_combinations(&self, candidates: &[(GroupId, String, f64)]) -> anyhow::Result<usize> {
        let mut archived = 0usize;
        for (group_id, reason, average_cqd) in candidates {
            if self.archive_group_combination(*group_id, reason, *average_cqd)? {
                archived += 1;
            }
        }
        Ok(archived)
    }

    /// 用全量复核结果在一个事务内替换指定赛道的封存集合，返回实际写入行数。
    ///
    /// 未出现在 candidates 的旧封存项会恢复参与后续计算；不存在的 ID 跳过，
    /// 其他赛道的 ID 返回错误并回滚整个替换。仅适用于已完成全量复核的调用方。
    pub fn replace_lane_archived_groups(&self, lane_size: usize, candidates: &[(GroupId, String, f64)]) -> anyhow::Result<usize> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM archived_groups WHERE lane_size = ?1", params![lane_size as i64])?;

        let mut archived = 0usize;
        for (group_id, reason, average_cqd) in candidates {
            let group: Option<(String, usize)> = tx
                .query_row(
                    "SELECT canonical, lane_size FROM groups WHERE id = ?1",
                    params![group_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize)),
                )
                .optional()?;

            let Some((canonical, group_lane_size)) = group else {
                continue;
            };
            if group_lane_size != lane_size {
                anyhow::bail!(
                    "封存候选 {} 属于 {} 人赛道，不能写入 {} 人赛道",
                    group_id,
                    group_lane_size,
                    lane_size
                );
            }

            tx.execute(
                "INSERT INTO archived_groups
                 (group_id, canonical, lane_size, reason, average_cqd, archived_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
                params![group_id, canonical, lane_size as i64, reason, average_cqd],
            )?;
            archived += 1;
        }

        tx.commit()?;
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
}

#[derive(Debug, Clone, Copy)]
pub enum InsertGroupOutcome {
    Added(GroupId),
    Duplicated(GroupId),
}

pub(super) fn load_members_locked(conn: &Connection, group_id: GroupId) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT member FROM group_members WHERE group_id = ?1 ORDER BY position ASC")?;
    let rows = stmt.query_map(params![group_id], |row| row.get(0))?;
    let mut members = Vec::new();
    for row in rows {
        members.push(row?);
    }
    Ok(members)
}

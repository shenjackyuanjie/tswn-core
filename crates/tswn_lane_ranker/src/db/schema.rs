//! SQLite 初始化、表结构和兼容迁移。
//!
//! 创建缺失表后按 PRAGMA table_info 补充结果/追溯列，供旧数据库继续读取。
//! 不删除历史列或重算历史分数；这些 ALTER 操作不是统一迁移事务，失败后可再次启动补齐。
//! 列名与类型定义均由本模块内固定调用提供，不接受 HTTP 输入拼接 SQL。

use super::*;

pub(super) fn ensure_lane_result_column(conn: &Connection, name: &str, definition: &str) -> anyhow::Result<()> {
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

pub(super) fn ensure_correct_target_trace_weight_column(conn: &Connection, name: &str, definition: &str) -> anyhow::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(correct_target_trace_weights)")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == name {
            return Ok(());
        }
    }
    conn.execute(
        &format!("ALTER TABLE correct_target_trace_weights ADD COLUMN {name} {definition}"),
        [],
    )?;
    Ok(())
}
impl Db {
    pub(super) fn init(&self) -> anyhow::Result<()> {
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

            CREATE TABLE IF NOT EXISTS correct_target_trace_meta (
                lane_size INTEGER PRIMARY KEY,
                trace_version TEXT NOT NULL,
                score_mode TEXT NOT NULL,
                metadata_json TEXT NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE IF NOT EXISTS correct_target_trace_weights (
                lane_size INTEGER NOT NULL,
                reference_scope TEXT NOT NULL,
                group_id INTEGER NOT NULL,
                reference_weight REAL NOT NULL,
                nominal_weight REAL NOT NULL,
                raw_golden_weight REAL NOT NULL,
                calibration_log_multiplier REAL NOT NULL DEFAULT 0,
                calibration_multiplier REAL NOT NULL DEFAULT 1,
                common_coefficient REAL NOT NULL DEFAULT 0,
                coefficient_mean REAL NOT NULL DEFAULT 0,
                coefficient_stddev REAL NOT NULL DEFAULT 0,
                correct_target_weight REAL NOT NULL,
                source TEXT NOT NULL,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (lane_size, reference_scope, group_id),
                FOREIGN KEY (group_id) REFERENCES groups(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_correct_target_trace_weights_lane
                ON correct_target_trace_weights(lane_size);

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
        ensure_lane_result_column(&conn, "marginal_value", "REAL")?;
        ensure_lane_result_column(&conn, "constrained_rank", "INTEGER")?;
        ensure_lane_result_column(&conn, "selection_status", "TEXT NOT NULL DEFAULT 'calibration_skipped'")?;
        ensure_lane_result_column(&conn, "winrate_type_label", "TEXT")?;
        ensure_lane_result_column(&conn, "winrate_profile_distance", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_second_distance", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_margin", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_soft_confidence", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_soft_entropy", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_soft_confidence_calibrated", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_soft_entropy_calibrated", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_bootstrap_stability", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_fixed_center_stability", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_recluster_stability", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_recluster_jaccard", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_assignment_entropy", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_recluster_ari", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_embedding_x", "REAL")?;
        ensure_lane_result_column(&conn, "winrate_profile_embedding_y", "REAL")?;
        ensure_lane_result_column(&conn, "residual_type_label", "TEXT")?;
        ensure_lane_result_column(&conn, "residual_profile_distance", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_second_distance", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_margin", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_soft_confidence_calibrated", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_soft_entropy_calibrated", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_recluster_stability", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_assignment_entropy", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_embedding_x", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_embedding_y", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_shape_rms", "REAL")?;
        ensure_lane_result_column(&conn, "residual_profile_variance_feature", "REAL")?;
        ensure_correct_target_trace_weight_column(&conn, "raw_golden_weight", "REAL NOT NULL DEFAULT 0")?;
        ensure_correct_target_trace_weight_column(&conn, "calibration_log_multiplier", "REAL NOT NULL DEFAULT 0")?;
        ensure_correct_target_trace_weight_column(&conn, "calibration_multiplier", "REAL NOT NULL DEFAULT 1")?;
        ensure_correct_target_trace_weight_column(&conn, "correct_target_weight", "REAL NOT NULL DEFAULT 0")?;
        ensure_correct_target_trace_weight_column(&conn, "common_coefficient", "REAL NOT NULL DEFAULT 0")?;
        ensure_correct_target_trace_weight_column(&conn, "coefficient_mean", "REAL NOT NULL DEFAULT 0")?;
        ensure_correct_target_trace_weight_column(&conn, "coefficient_stddev", "REAL NOT NULL DEFAULT 0")?;

        Ok(())
    }
}

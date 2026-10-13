//! PostgreSQL 玩家读取、结果持久化和排名日志。

use super::*;

/// 建立 PostgreSQL 连接并派生驱动 future，供启动和断线重连复用。
pub(super) async fn connect_postgres(database_url: &str) -> Result<Client> {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await.context("connect PostgreSQL")?;

    tokio::spawn(async move {
        if let Err(err) = connection.await {
            eprintln!("PostgreSQL连接错误: {err}");
        }
    });

    eprintln!("[db] PostgreSQL连接已创建");
    Ok(client)
}

impl Updator {
    /// 每轮/每组开始前确认连接可用；连接已关闭时重连一次。
    pub(super) async fn ensure_postgres_connected(&mut self) -> Result<()> {
        if !self.client.is_closed() {
            return Ok(());
        }

        eprintln!("[db] PostgreSQL连接已关闭，正在重连");
        self.reconnect_postgres().await
    }
    /// 仅在错误确实源于连接关闭时重连，避免对普通 SQL 错误做无谓重试。
    pub(super) async fn reconnect_after_postgres_error(&mut self, err: &anyhow::Error) {
        if !is_closed_postgres_error(err) && !self.client.is_closed() {
            return;
        }

        eprintln!("[db] 检测到PostgreSQL连接关闭，正在重连");
        if let Err(reconnect_err) = self.reconnect_postgres().await {
            eprintln!("[db] PostgreSQL重连失败: {reconnect_err:#}");
        }
    }
    async fn reconnect_postgres(&mut self) -> Result<()> {
        let database_url = self.config.database_url.clone();
        self.client = connect_postgres(&database_url).await?;
        Ok(())
    }

    pub(super) async fn load_players(&self, group: &str) -> Result<Vec<PlayerRow>> {
        let rows = self
            .client
            .query(
                r#"
                SELECT id, name, score
                FROM member
                WHERE number = $1 AND is_active = true
                ORDER BY score DESC
                "#,
                &[&group],
            )
            .await
            .context("load active players")?;

        Ok(rows
            .into_iter()
            .map(|row| PlayerRow {
                id: row.get::<_, i32>(0),
                name: row.get::<_, String>(1),
                loaded_score: numeric_to_f64(&row, 2),
                score: 0,
                original_score: 0,
                initial_rank: 0,
                rank: 0,
                total_matches: 0,
                base_matches: 0,
                second_matches: 0,
                supplement_matches: 0,
            })
            .collect())
    }

    pub(super) async fn load_handicap_data(
        &mut self,
        players: &[PlayerRow],
        group: &str,
    ) -> Result<HashMap<PairKey, HandicapData>> {
        let rows = self
            .client
            .query(
                r#"
                SELECT player1_id, player2_id, value, match_count
                FROM handicap
                WHERE number = $1
                  AND (match_count <> 0 OR ABS(value::double precision) > $2::double precision)
                "#,
                &[&group, &HANDICAP_ZERO_EPSILON],
            )
            .await
            .context("load handicap rows")?;

        let mut map = HashMap::with_capacity(players.len().saturating_mul(players.len().saturating_sub(1)) / 2);
        for i in 0..players.len() {
            for j in (i + 1)..players.len() {
                map.insert(
                    PairKey::new(players[i].id, players[j].id),
                    HandicapData {
                        basic_value: 0.0,
                        disturbed_value: self.random_in_range(1.0) * PERTURBATION_RANGE,
                        count: 0.0,
                    },
                );
            }
        }

        for row in rows {
            let id1 = row.get::<_, i32>(0);
            let id2 = row.get::<_, i32>(1);
            let value = numeric_to_f64(&row, 2);
            let match_count = numeric_to_f64(&row, 3);
            let key = PairKey::new(id1, id2);
            if let Some(data) = map.get_mut(&key) {
                data.basic_value = finite_or(value, 0.0);
                data.count = finite_or(match_count, 0.0);
                data.disturbed_value = if match_count > 100.0 {
                    finite_or(value, 0.0)
                } else {
                    finite_or(
                        value + self.random_in_range((100.0 - match_count) / 100.0) * PERTURBATION_RANGE,
                        0.0,
                    )
                };
            }
        }

        Ok(map)
    }

    pub(super) async fn persist_results(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &mut HashMap<PairKey, HandicapData>,
        changed_handicap_keys: &HashSet<PairKey>,
        group: &str,
    ) -> Result<()> {
        for batch in players.chunks(200) {
            let ids = batch.iter().map(|p| p.id).collect::<Vec<i32>>();
            let scores = batch.iter().map(|p| p.score).collect::<Vec<i32>>();
            self.client
                .execute(
                    r#"
                    UPDATE member AS m
                    SET score = u.score
                    FROM UNNEST($1::int[], $2::int[]) AS u(id, score)
                    WHERE m.id = u.id
                    "#,
                    &[&ids, &scores],
                )
                .await
                .context("update member scores")?;
        }

        let mut handicap_rows = Vec::new();
        for (key, data) in handicap_map.iter_mut() {
            let raw_value = if data.basic_value.is_finite() {
                data.basic_value
            } else {
                data.disturbed_value
            };
            let decayed_value = if changed_handicap_keys.contains(key) {
                raw_value * HANDICAP_DECAY
            } else {
                raw_value
            };
            let persisted_value = self.round_handicap(decayed_value);
            data.disturbed_value = persisted_value;
            data.basic_value = persisted_value;

            let mut count = self.round_handicap(data.count * MATCHCOUNT_DECAY);
            if count > 100.0 {
                count = 100.0;
            }

            let has_signal = persisted_value.abs() > HANDICAP_ZERO_EPSILON || count > 0.0;
            let should_persist = !HANDICAP_SPARSE_PERSIST || has_signal || changed_handicap_keys.contains(key);
            if !should_persist {
                continue;
            }

            handicap_rows.push((group.to_string(), key.0, key.1, persisted_value, count));
        }

        for batch in handicap_rows.chunks(300) {
            let numbers = batch.iter().map(|r| r.0.clone()).collect::<Vec<String>>();
            let player1_ids = batch.iter().map(|r| r.1).collect::<Vec<i32>>();
            let player2_ids = batch.iter().map(|r| r.2).collect::<Vec<i32>>();
            let values = batch.iter().map(|r| r.3).collect::<Vec<f64>>();
            let counts = batch.iter().map(|r| r.4).collect::<Vec<f64>>();

            self.client
                .execute(
                    r#"
                    INSERT INTO handicap (number, player1_id, player2_id, value, match_count)
                    SELECT u.number, u.player1_id, u.player2_id, u.value, u.match_count
                    FROM UNNEST(
                        $1::text[],
                        $2::int[],
                        $3::int[],
                        $4::double precision[],
                        $5::double precision[]
                    ) AS u(number, player1_id, player2_id, value, match_count)
                    ON CONFLICT (number, player1_id, player2_id)
                    DO UPDATE SET
                        value = EXCLUDED.value,
                        match_count = EXCLUDED.match_count
                    "#,
                    &[&numbers, &player1_ids, &player2_ids, &values, &counts],
                )
                .await
                .context("upsert handicap rows")?;
        }

        if HANDICAP_SPARSE_PERSIST {
            self.client
                .execute(
                    r#"
                    DELETE FROM handicap
                    WHERE number = $1
                      AND match_count = 0
                      AND ABS(value::double precision) <= $2::double precision
                    "#,
                    &[&group, &HANDICAP_ZERO_EPSILON],
                )
                .await
                .context("delete sparse handicap rows")?;
        }

        Ok(())
    }

    pub(super) async fn update_ranks_and_logs(
        &mut self,
        players: &mut [PlayerRow],
        mut history_records: Vec<HistoryRecord>,
        group: &str,
    ) -> Result<()> {
        players.sort_by(|a, b| b.score.cmp(&a.score));
        let final_scores = players.iter().map(|p| (p.id, p.score)).collect::<HashMap<_, _>>();

        for rec in &mut history_records {
            rec.current_score = final_scores.get(&rec.from_member_id).copied().unwrap_or(0);
        }

        for (idx, player) in players.iter_mut().enumerate() {
            player.rank = (idx + 1) as i32;
        }

        let rank_ids = players.iter().map(|p| p.id).collect::<Vec<i32>>();
        let rank_values = players.iter().map(|p| p.rank).collect::<Vec<i32>>();
        self.client
            .execute(
                r#"
                UPDATE member AS m
                SET member_rank = u.member_rank
                FROM UNNEST($1::int[], $2::int[]) AS u(id, member_rank)
                WHERE m.id = u.id
                "#,
                &[&rank_ids, &rank_values],
            )
            .await
            .context("update member ranks")?;

        if let Some(sample) = history_records.first() {
            eprintln!(
                "[db] 验证历史记录: {}#{} vs {}#{}",
                sample.from_name, sample.from_rank, sample.to_name, sample.to_rank
            );
            eprintln!(
                "   预期胜率: {:.2}, 实际结果: {}",
                sample.expected_win_prob, sample.actual_result
            );
        }

        for batch in history_records.chunks(300) {
            let win_or_lose = batch.iter().map(|r| r.win_or_lose).collect::<Vec<&str>>();
            let from_member_ids = batch.iter().map(|r| r.from_member_id).collect::<Vec<i32>>();
            let to_member_ids = batch.iter().map(|r| r.to_member_id).collect::<Vec<i32>>();
            let current_scores = batch.iter().map(|r| r.current_score).collect::<Vec<i32>>();
            let from_ranks = batch.iter().map(|r| r.from_rank).collect::<Vec<i32>>();
            let to_ranks = batch.iter().map(|r| r.to_rank).collect::<Vec<i32>>();
            let from_names = batch.iter().map(|r| r.from_name.clone()).collect::<Vec<String>>();
            let to_names = batch.iter().map(|r| r.to_name.clone()).collect::<Vec<String>>();
            let runs = batch.iter().map(|r| r.runs.clone()).collect::<Vec<String>>();
            let numbers = batch.iter().map(|r| r.number.clone()).collect::<Vec<String>>();
            let expected_win_probs = batch.iter().map(|r| r.expected_win_prob).collect::<Vec<f64>>();
            let actual_results = batch.iter().map(|r| r.actual_result).collect::<Vec<f64>>();

            self.client
                .execute(
                    r#"
                    INSERT INTO history (
                        win_or_lose, from_member_id, to_member_id, created_at,
                        current_score, from_rank, to_rank, from_name, to_name, runs, number,
                        expected_win_prob, actual_result
                    )
                    SELECT
                        u.win_or_lose, u.from_member_id, u.to_member_id, NOW(),
                        u.current_score, u.from_rank, u.to_rank, u.from_name, u.to_name, u.runs, u.number,
                        u.expected_win_prob, u.actual_result
                    FROM UNNEST(
                        $1::text[],
                        $2::int[],
                        $3::int[],
                        $4::int[],
                        $5::int[],
                        $6::int[],
                        $7::text[],
                        $8::text[],
                        $9::text[],
                        $10::text[],
                        $11::double precision[],
                        $12::double precision[]
                    ) AS u(
                        win_or_lose,
                        from_member_id,
                        to_member_id,
                        current_score,
                        from_rank,
                        to_rank,
                        from_name,
                        to_name,
                        runs,
                        number,
                        expected_win_prob,
                        actual_result
                    )
                    "#,
                    &[
                        &win_or_lose,
                        &from_member_ids,
                        &to_member_ids,
                        &current_scores,
                        &from_ranks,
                        &to_ranks,
                        &from_names,
                        &to_names,
                        &runs,
                        &numbers,
                        &expected_win_probs,
                        &actual_results,
                    ],
                )
                .await
                .context("insert history rows")?;
        }

        let rank_list = players.iter().map(|p| p.id.to_string()).collect::<Vec<_>>().join(" ");
        tokio::fs::create_dir_all(&self.config.rank_log_dir)
            .await
            .with_context(|| format!("create rank log dir {}", self.config.rank_log_dir.display()))?;
        let path = self.config.rank_log_dir.join(format!("rank_{group}.txt"));
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .with_context(|| format!("open rank log {}", path.display()))?
            .write_all(format!("{rank_list}\n").as_bytes())
            .await
            .with_context(|| format!("append rank log {}", path.display()))?;

        Ok(())
    }
}

pub(super) fn numeric_to_f64(row: &tokio_postgres::Row, idx: usize) -> f64 {
    if let Ok(value) = row.try_get::<_, f64>(idx) {
        return value;
    }
    if let Ok(value) = row.try_get::<_, f32>(idx) {
        return value as f64;
    }
    if let Ok(value) = row.try_get::<_, i32>(idx) {
        return value as f64;
    }
    if let Ok(value) = row.try_get::<_, i64>(idx) {
        return value as f64;
    }
    0.0
}

/// 识别错误链里的连接关闭原因及仅保留文本的关闭错误。
fn is_closed_postgres_error(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<tokio_postgres::Error>()
            .is_some_and(tokio_postgres::Error::is_closed)
            || cause.to_string().contains("connection closed")
    })
}

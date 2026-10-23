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
        // 只读取「本轮活跃玩家之间」的行：离开天梯的玩家留下的行对匹配没有意义，
        // 读进来只会占带宽，还会在回写时被反复重写。
        let active_ids = players.iter().map(|p| p.id).collect::<Vec<i32>>();
        let rows = self
            .client
            .query(
                r#"
                SELECT player1_id, player2_id, value, match_count
                FROM handicap
                WHERE number = $1
                  AND player1_id = ANY($2::int[])
                  AND player2_id = ANY($2::int[])
                  AND (match_count <> 0 OR value <> 0)
                "#,
                &[&group, &active_ids],
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
                        persisted_value: 0.0,
                        persisted_count: 0.0,
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
                data.persisted_value = data.basic_value;
                data.persisted_count = data.count;
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

    /// 回写分数/名次与 handicap，返回 (写入的 handicap 行数, 删除的行数)。
    ///
    /// 关键约束：**只有库里确实变了的行才写**。旧实现每轮把整张 handicap 表
    /// （活跃玩家的全部两两组合，线上约 1.8 万行）无条件 `ON CONFLICT DO UPDATE`
    /// 一遍，一分钟一次，24 GB 里绝大多数是这种反复重写留下的死元组。
    pub(super) async fn persist_results(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &mut HashMap<PairKey, HandicapData>,
        changed_handicap_keys: &HashSet<PairKey>,
        group: &str,
    ) -> Result<(usize, usize)> {
        // 名次需要最终分数，先排好再一次写回：分数和名次合成一条 UPDATE，
        // 旧实现「先写分数、再写名次」对同一批行写了两次。
        players.sort_by(|a, b| b.score.cmp(&a.score));
        for (idx, player) in players.iter_mut().enumerate() {
            player.rank = (idx + 1) as i32;
        }

        for batch in players.chunks(200) {
            let ids = batch.iter().map(|p| p.id).collect::<Vec<i32>>();
            let scores = batch.iter().map(|p| p.score).collect::<Vec<i32>>();
            let ranks = batch.iter().map(|p| p.rank).collect::<Vec<i32>>();
            if self.config.dry_run {
                continue;
            }
            self.client
                .execute(
                    r#"
                    UPDATE member AS m
                    SET score = u.score, member_rank = u.member_rank
                    FROM UNNEST($1::int[], $2::int[], $3::int[]) AS u(id, score, member_rank)
                    WHERE m.id = u.id
                      AND (m.score IS DISTINCT FROM u.score::double precision
                           OR m.member_rank IS DISTINCT FROM u.member_rank)
                    "#,
                    &[&ids, &scores, &ranks],
                )
                .await
                .context("update member scores and ranks")?;
        }

        let mut handicap_rows = Vec::new();
        let mut handicap_deletes = Vec::new();
        for (key, data) in handicap_map.iter_mut() {
            let raw_value = if data.basic_value.is_finite() {
                data.basic_value
            } else {
                data.disturbed_value
            };
            let value = if changed_handicap_keys.contains(key) {
                raw_value * HANDICAP_DECAY
            } else {
                raw_value
            };
            let count = (data.count * MATCHCOUNT_DECAY).min(100.0);

            // 内存里保留未经取舍的精确值；只有落盘值才按存储精度做概率舍入。
            data.basic_value = value;
            data.disturbed_value = value;
            data.count = count;

            let has_signal = value.abs() > HANDICAP_ZERO_EPSILON || count > 0.0;
            if !has_signal {
                // 没有信号的行直接删掉，不再依赖每轮一条全组 DELETE 语句兜底。
                if data.persisted_count > 0.0 || data.persisted_value.abs() > HANDICAP_ZERO_EPSILON {
                    handicap_deletes.push((key.0, key.1));
                }
                continue;
            }

            let persisted_value = self.round_handicap(value);
            let persisted_count = self.round_handicap(count);
            let drifted = handicap_needs_persist(
                data.persisted_value,
                data.persisted_count,
                persisted_value,
                persisted_count,
                self.config.handicap_persist_quantum,
            );
            if !drifted {
                continue;
            }

            data.persisted_value = persisted_value;
            data.persisted_count = persisted_count;
            handicap_rows.push((group.to_string(), key.0, key.1, persisted_value, persisted_count));
        }

        if self.config.dry_run {
            eprintln!(
                "[dry-run] 跳过写库：member {} 行、handicap {} 行、删除 {} 行",
                players.len(),
                handicap_rows.len(),
                handicap_deletes.len()
            );
            return Ok((handicap_rows.len(), handicap_deletes.len()));
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

        for batch in handicap_deletes.chunks(300) {
            let player1_ids = batch.iter().map(|r| r.0).collect::<Vec<i32>>();
            let player2_ids = batch.iter().map(|r| r.1).collect::<Vec<i32>>();
            self.client
                .execute(
                    r#"
                    DELETE FROM handicap h
                    USING UNNEST($2::int[], $3::int[]) AS u(player1_id, player2_id)
                    WHERE h.number = $1
                      AND h.player1_id = u.player1_id
                      AND h.player2_id = u.player2_id
                    "#,
                    &[&group, &player1_ids, &player2_ids],
                )
                .await
                .context("delete signal-less handicap rows")?;
        }

        Ok((handicap_rows.len(), handicap_deletes.len()))
    }

    /// 删除「对局双方里有已经离开天梯的玩家」的 handicap 行。
    ///
    /// 这类行不会再被匹配到，留着只会让表随历史玩家数量一直膨胀。
    /// 每 `HANDICAP_MAINTENANCE_EVERY` 轮跑一次，按 `number` 走主键前缀。
    pub(super) async fn prune_stale_handicap(&mut self, group: &str) -> Result<u64> {
        if self.config.dry_run {
            return Ok(0);
        }
        self.client
            .execute(
                r#"
                DELETE FROM handicap h
                WHERE h.number = $1
                  AND (
                        NOT EXISTS (SELECT 1 FROM member m WHERE m.id = h.player1_id AND m.is_active)
                     OR NOT EXISTS (SELECT 1 FROM member m WHERE m.id = h.player2_id AND m.is_active)
                  )
                "#,
                &[&group],
            )
            .await
            .context("prune stale handicap rows")
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
            if self.config.dry_run {
                break;
            }
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

        if self.config.dry_run {
            eprintln!("[dry-run] 跳过写库：history {} 行、rank 日志", history_records.len());
            return Ok(());
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

/// 读取累计场次快照（每行 `member_id 场次`）。
///
/// 这个数字只用于把 K 从「新玩家」平滑收缩到「老玩家」，属于可丢的缓存：文件缺失、
/// 损坏或被清掉时退回空表，所有人按新玩家 K 打几十轮就重新收敛。
pub(super) fn load_match_counts(dir: &std::path::Path) -> HashMap<i32, f64> {
    let path = dir.join(MATCHCOUNT_STATE_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return HashMap::new();
    };

    let mut counts = HashMap::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let (Some(id), Some(count)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(id), Ok(count)) = (id.parse::<i32>(), count.parse::<f64>()) else {
            continue;
        };
        if count.is_finite() && count >= 0.0 {
            counts.insert(id, count);
        }
    }
    counts
}

/// 原子写回累计场次快照；失败只打印日志，不影响天梯运行。
pub(super) fn save_match_counts(dir: &std::path::Path, counts: &HashMap<i32, f64>) {
    let path = dir.join(MATCHCOUNT_STATE_FILE);
    let tmp = dir.join(format!("{MATCHCOUNT_STATE_FILE}.tmp"));

    let mut ids = counts.keys().copied().collect::<Vec<_>>();
    ids.sort_unstable();
    let mut text = String::with_capacity(ids.len() * 16);
    for id in ids {
        let count = counts.get(&id).copied().unwrap_or(0.0);
        text.push_str(&format!("{id} {count:.0}\n"));
    }

    let saved = std::fs::create_dir_all(dir).is_ok()
        && std::fs::write(&tmp, text.as_bytes()).is_ok()
        && std::fs::rename(&tmp, &path).is_ok();
    if !saved {
        eprintln!("[state] 累计场次快照写入失败: {}", path.display());
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

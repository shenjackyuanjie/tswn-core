//! 天梯更新流程、调度、内存监控与随机辅助。
//!
//! 每轮串行处理配置中的各组，先衰减评分并生成对局，再执行战斗、分批回写和追加排名日志。
//! 组内失败会记录并继续下一组；关闭连接时尝试重连，但不会自动重放已部分写入的组。
//! 内存监控仅在能读取 Linux `/proc/self/status` 时生效，超限直接退出进程。

use super::*;

impl Updator {
    pub(super) fn new(client: Client, config: Config) -> Self {
        eprintln!("[start] Updator已启动");
        let snapshot = load_member_state(&config.state_dir);
        eprintln!("[state] 玩家状态快照 {} 人，轮次 {}", snapshot.players.len(), snapshot.round);
        Self {
            client,
            config,
            rng: fastrand::Rng::new(),
            battle_error_count: 0,
            battle_error_suppressed: 0,
            member_state: snapshot.players,
            round_index: snapshot.round,
            state_save_warned: false,
        }
    }

    /// 落盘玩家状态快照；失败只提示一次（通常是 `LADDER_STATE_DIR` 对服务用户不可写）。
    pub(super) fn flush_state(&mut self) {
        if self.config.dry_run {
            return;
        }
        if save_member_state(&self.config.state_dir, self.round_index, &self.member_state) || self.state_save_warned {
            return;
        }
        self.state_save_warned = true;
        eprintln!(
            "[state] 玩家状态快照写入失败（{}），自适应 K 会在进程重启后从 0 重新累计",
            self.config.state_dir.display()
        );
    }

    /// 自适应 K：累计场次越多，单场改分越小。
    ///
    /// 一分钟一轮、每人每轮约 9 场，一天上万场；固定 K=80 时稳态误差比真实实力差还大，
    /// 排名每分钟都在大幅互穿。这里让新玩家用完整 K 快速就位，老玩家收敛到下限。
    ///
    /// 例外：成员换号时（站点会扣 10% 分数并清 handicap），K 临时恢复满值
    /// `LADDER_K_BOOST_ROUNDS` 轮，把扣掉的分按原来 ~3 小时的节奏补回来。
    pub(super) fn k_factor(&self, member_id: i32) -> f64 {
        let state = self.member_state.get(&member_id).copied().unwrap_or_default();
        if state.boost_until > self.round_index {
            return BASE_K_FACTOR;
        }
        adaptive_k(state.matches, self.config.k_shrink_matches, self.config.k_floor)
    }

    /// 用成员名称变化识别换号，并给对应的玩家临时恢复满 K。
    ///
    /// 站点换号走的是 `app/main/team_routes.py`：改 `member.name` 的同时把分数打 9 折
    /// 并删掉该成员在两个方向上的 handicap 行。天梯这边只看得到名称变化，
    /// 所以把它当作「评分不确定性变大」的信号。
    pub(super) fn sync_member_names(&mut self, players: &[PlayerRow]) {
        let boost_rounds = self.config.k_boost_rounds;
        let round_index = self.round_index;
        for player in players {
            let current = name_hash(&player.name);
            let entry = self.member_state.entry(player.id).or_default();
            if let Some(until) = rename_boost_until(entry.name_hash, current, boost_rounds, round_index) {
                eprintln!(
                    "[state] 检测到换号: member={} 名字哈希 {:016x} -> {:016x}，K 恢复到满值直至第 {} 轮",
                    player.id, entry.name_hash, current, until
                );
                entry.boost_until = until;
            }
            entry.name_hash = current;
        }
    }

    pub(super) async fn run_forever(&mut self) -> Result<()> {
        let schedule = parse_cron_schedule(&self.config.cron_schedule)?;
        eprintln!("[cron] schedule={}", self.config.cron_schedule);

        loop {
            let now = Utc::now();
            let Some(next) = schedule.upcoming(Utc).find(|instant| *instant > now) else {
                bail!("cron schedule has no upcoming instant");
            };
            let wait = (next - now).to_std().unwrap_or_else(|_| Duration::from_secs(0));

            tokio::select! {
                _ = tokio::time::sleep(wait) => {
                    if let Err(err) = self.run_round().await {
                        eprintln!("[tick] 天梯匹配轮次异常: {err:#}");
                    }
                }
                signal = shutdown_signal() => {
                    signal?;
                    eprintln!("[shutdown] 收到退出信号");
                    self.flush_state();
                    return Ok(());
                }
            }
        }
    }

    pub(super) async fn run_round(&mut self) -> Result<()> {
        eprintln!("[tick] 天梯匹配轮次开始 {}", Utc::now().to_rfc3339());
        if let Err(err) = self.ensure_postgres_connected().await {
            eprintln!("[tick] PostgreSQL连接不可用，跳过本轮: {err:#}");
            self.check_memory_usage();
            eprintln!("[tick] 天梯匹配轮次结束");
            return Ok(());
        }

        for group in self.config.groups.clone() {
            if let Err(err) = self.ensure_postgres_connected().await {
                eprintln!("[group:{group}] 处理异常: reconnect PostgreSQL: {err:#}");
                continue;
            }

            eprintln!("[group:{group}] 开始");
            if let Err(err) = self.process_all_players(&group).await {
                eprintln!("[group:{group}] 处理异常: {err:#}");
                self.reconnect_after_postgres_error(&err).await;
            }
        }
        self.round_index = self.round_index.wrapping_add(1);

        // handicap 自维护：不依赖 autovacuum（共享集群里它可能整小时不启动）。
        // 放在所有组跑完之后做一次，而不是每组各做一次。
        if HANDICAP_VACUUM_EVERY > 0 && self.round_index % HANDICAP_VACUUM_EVERY == 0 {
            if let Err(err) = self.vacuum_handicap().await {
                eprintln!("[db] VACUUM handicap 失败: {err:#}");
            }
        }

        if MATCHCOUNT_SAVE_EVERY > 0 && self.round_index % MATCHCOUNT_SAVE_EVERY == 0 {
            self.flush_state();
        }
        self.check_memory_usage();
        eprintln!("[tick] 天梯匹配轮次结束");
        Ok(())
    }

    pub(super) fn check_memory_usage(&self) {
        if let Some(used_mb) = current_rss_mb() {
            eprintln!("[mem] {used_mb}MB");
            if used_mb > MEMORY_LIMIT_MB {
                eprintln!("内存超限 ({used_mb}MB)，准备退出程序...");
                std::process::exit(1);
            }
        }
    }

    pub(super) async fn process_all_players(&mut self, group: &str) -> Result<()> {
        // 分阶段耗时：用来判断"匹配"到底值不值得优化（线上实测对战占绝大部分）。
        let mut stage = std::time::Instant::now();
        let mut read_ms = 0.0_f64;
        let mut handicap_ms = 0.0_f64;
        let mut match_ms = 0.0_f64;
        let mut battle_ms = 0.0_f64;
        let mut write_ms = 0.0_f64;

        let mut players = self.load_players(group).await?;
        read_ms += stage.elapsed().as_secs_f64() * 1000.0;
        if players.len() < 2 {
            eprintln!("[group:{group}] 活跃玩家不足2人，跳过");
            return Ok(());
        }

        // 换号识别：站点的换号 = 改 member.name + 扣 10% 分数 + 清 handicap，
        // 天梯这边靠名称变化给该玩家临时恢复满 K，让扣掉的分按原来的节奏补回来。
        self.sync_member_names(&players);

        eprintln!("[group:{group}] 玩家={}", players.len());

        // 赛季衰减改成「每人扣掉同一个量」：等于组内平均分按 DROPRATE 会掉的那一份。
        // 原来的 `score * DROPRATE` 是按分数比例扣，扣得最多的是高分端，等于持续把
        // 分差往中间挤；分差被压缩后同样的改分噪声会造成更大的名次抖动，而且「分数
        // 尺度」和 K 绑在一起（降 K 就等于压缩全榜分数）。扣同一个量时均值同样停在
        // ~11000（它随均值自适应），但分差不受影响。
        let mean_loaded = players.iter().map(|p| p.loaded_score).sum::<f64>() / players.len() as f64;
        let decay = decay_step(mean_loaded);

        for player in &mut players {
            player.score = self.probabilistic_round(player.loaded_score - decay);
            player.original_score = player.score;
            player.total_matches = 0;
            player.base_matches = 0;
            player.second_matches = 0;
            player.supplement_matches = 0;
        }

        players.sort_by(|a, b| b.score.cmp(&a.score));
        for (idx, player) in players.iter_mut().enumerate() {
            player.initial_rank = (idx + 1) as i32;
            player.original_score = player.score;
        }

        stage = std::time::Instant::now();
        let mut handicap_map = self.load_handicap_data(&players, group).await?;
        handicap_ms += stage.elapsed().as_secs_f64() * 1000.0;

        stage = std::time::Instant::now();
        let first_matches = self.generate_base_matches(&mut players, &handicap_map);
        let base_matches = self.generate_second_matches(&mut players, &handicap_map, &first_matches);
        // 补位/退火保持关闭：实测只会把「handicap 诱导偏置」这个二阶量优化掉，
        // 天梯侧的波动与准确度没有任何变化，却要多打 21% 的对局。
        // 结论与数据见 `main.rs` 里 MAX_MATCHES 的注释。
        let supplement_matches: Vec<MatchPair> = Vec::new();
        let matches = base_matches.iter().copied().chain(supplement_matches.iter().copied()).collect::<Vec<_>>();

        match_ms += stage.elapsed().as_secs_f64() * 1000.0;
        let match_stats = self.analyze_match_distribution(&matches, &players, base_matches.len(), &handicap_map);
        eprintln!(
            "[group:{group}] 对局={} (base={}, extra={}) | 场次范围={}-{} | 均值={:.1} | loss={:.2}",
            matches.len(),
            base_matches.len(),
            supplement_matches.len(),
            match_stats.min_matches,
            match_stats.max_matches,
            match_stats.avg_matches,
            match_stats.final_squared_loss
        );

        stage = std::time::Instant::now();
        let update_results = self
            .execute_matches_with_calibrated_elo(&mut players, &matches, &mut handicap_map, group)
            .await?;
        battle_ms += stage.elapsed().as_secs_f64() * 1000.0;

        if update_results.skip_persist {
            eprintln!(
                "[group:{group}] 写入已跳过 | success={} failed={}",
                update_results.executed_matches, update_results.failed_matches
            );
            return Ok(());
        }

        if HANDICAP_MAINTENANCE_EVERY > 0 && self.round_index % HANDICAP_MAINTENANCE_EVERY == 0 {
            match self.prune_stale_handicap(group).await {
                Ok(0) => {}
                Ok(n) => eprintln!("[group:{group}] 清理失效 handicap 行 {n} 条"),
                Err(err) => eprintln!("[group:{group}] 清理 handicap 失败: {err:#}"),
            }
        }

        stage = std::time::Instant::now();
        let (handicap_written, handicap_deleted) = self
            .persist_results(&mut players, &mut handicap_map, &update_results.changed_handicaps, group)
            .await?;
        self.update_ranks_and_logs(&mut players, update_results.history_records, group).await?;
        write_ms += stage.elapsed().as_secs_f64() * 1000.0;

        eprintln!(
            "[perf:{group}] 读玩家={read_ms:.1}ms 读克制={handicap_ms:.1}ms 匹配={match_ms:.1}ms 对战={battle_ms:.1}ms 回写={write_ms:.1}ms"
        );
        eprintln!(
            "[group:{group}] 完成 | avgDelta={:.1} | quality={:.1}% | success={} failed={} | handicapAvgCount={:.1} | handicapAvgValue={:.1} | handicap写入={} 删除={}",
            update_results.avg_delta,
            update_results.match_quality,
            update_results.executed_matches,
            update_results.failed_matches,
            update_results.handicap_stats.avg_match_count,
            update_results.handicap_stats.avg_handicap,
            handicap_written,
            handicap_deleted
        );

        Ok(())
    }

    pub(super) fn probabilistic_round(&mut self, value: f64) -> i32 {
        let floor = value.floor();
        if self.rng.f64() < value - floor {
            floor as i32 + 1
        } else {
            floor as i32
        }
    }

    pub(super) fn round_handicap(&mut self, value: f64) -> f64 {
        let scale = 10_f64.powi(HANDICAP_PRECISION as i32);
        self.probabilistic_round(value * scale) as f64 / scale
    }

    pub(super) fn random_in_range(&mut self, a: f64) -> f64 { self.rng.f64() * 2.0 * a - a }

    pub(super) fn shuffle<T>(&mut self, values: &mut [T]) {
        for i in (1..values.len()).rev() {
            let j = self.rng.usize(..=i);
            values.swap(i, j);
        }
    }
}

pub(super) fn current_rss_mb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb = rest.split_whitespace().next()?.parse::<u64>().ok()?;
            return Some(kb / 1024);
        }
    }
    None
}

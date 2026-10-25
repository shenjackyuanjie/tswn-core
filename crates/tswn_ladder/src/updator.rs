//! 天梯更新流程、调度、内存监控与随机辅助。
//!
//! 每轮串行处理配置中的各组，先衰减评分并生成对局，再执行战斗、分批回写和追加排名日志。
//! 组内失败会记录并继续下一组；关闭连接时尝试重连，但不会自动重放已部分写入的组。
//! 内存监控仅在能读取 Linux `/proc/self/status` 时生效，超限直接退出进程。

use super::*;

impl Updator {
    pub(super) fn new(client: Client, config: Config) -> Self {
        eprintln!("[start] Updator已启动");
        Self {
            client,
            config,
            rng: fastrand::Rng::new(),
            battle_error_count: 0,
            battle_error_suppressed: 0,
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
        let mut players = self.load_players(group).await?;
        if players.len() < 2 {
            eprintln!("[group:{group}] 活跃玩家不足2人，跳过");
            return Ok(());
        }

        eprintln!("[group:{group}] 玩家={}", players.len());

        for player in &mut players {
            player.score = self.probabilistic_round(player.loaded_score * DROPRATE);
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

        let mut handicap_map = self.load_handicap_data(&players, group).await?;
        let first_matches = self.generate_base_matches(&mut players, &handicap_map);
        let base_matches = self.generate_second_matches(&mut players, &handicap_map, &first_matches);
        let supplement_matches: Vec<MatchPair> = Vec::new();
        let matches = base_matches.iter().copied().chain(supplement_matches.iter().copied()).collect::<Vec<_>>();

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

        let update_results = self
            .execute_matches_with_calibrated_elo(&mut players, &matches, &mut handicap_map, group)
            .await?;

        if update_results.skip_persist {
            eprintln!(
                "[group:{group}] 写入已跳过 | success={} failed={}",
                update_results.executed_matches, update_results.failed_matches
            );
            return Ok(());
        }

        self.persist_results(&mut players, &mut handicap_map, &update_results.changed_handicaps, group)
            .await?;
        self.update_ranks_and_logs(&mut players, update_results.history_records, group).await?;

        eprintln!(
            "[group:{group}] 完成 | avgDelta={:.1} | quality={:.1}% | success={} failed={} | handicapAvgCount={:.1} | handicapAvgValue={:.1}",
            update_results.avg_delta,
            update_results.match_quality,
            update_results.executed_matches,
            update_results.failed_matches,
            update_results.handicap_stats.avg_match_count,
            update_results.handicap_stats.avg_handicap
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

//! 组合导入、封禁、队伍合并与手动胜率录入。
//!
//! 导入按条区分新增、重复和忽略，收集受影响赛道后排队重算；重复导入也可触发刷新。
//! 手动胜率每两个非空组合为一对，只测同人数的已入库双方，按批保存后返回输入顺序结果。
//! 屏蔽和队伍合并先写数据库再排队，排队失败不会回滚先前修改。

use super::*;

impl AppService {
    pub fn add_groups(&self, req: AddGroupsRequest) -> anyhow::Result<AddGroupsResponse> {
        let AddGroupsRequest {
            groups,
            outer_workers,
            inner_workers,
            skip_archived,
        } = req;
        let config = self.config_with_run_options(outer_workers, inner_workers, skip_archived)?;

        let mut added = Vec::new();
        let mut duplicated = Vec::new();
        let mut ignored = Vec::new();
        let mut dirty_lanes = BTreeSet::new();

        for raw in groups {
            let raw = raw.trim().to_string();
            if raw.is_empty() {
                continue;
            }

            let parsed = match parse_group(&raw) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored.push(IgnoredGroup {
                        raw,
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            if config.skip_archived && self.db.is_group_archived_by_canonical(&parsed.canonical)? {
                ignored.push(IgnoredGroup {
                    raw,
                    reason: "archived combination skipped because 不跑封存组合 is enabled".to_string(),
                });
                continue;
            }

            match self.db.insert_group(&parsed)? {
                InsertGroupOutcome::Added(_) => {
                    dirty_lanes.insert(parsed.lane_size);
                    added.push(parsed.canonical);
                }
                InsertGroupOutcome::Duplicated(_) => {
                    // 重新导入已有组号时也触发该赛道重算，避免已有数据但结果/状态缺失时无法手动刷新。
                    dirty_lanes.insert(parsed.lane_size);
                    duplicated.push(parsed.canonical);
                }
            }
        }

        let queued_lanes = self.queue_recompute_lanes_with_config(dirty_lanes.into_iter().collect(), config)?;

        Ok(AddGroupsResponse {
            added,
            duplicated,
            ignored,
            queued_lanes,
        })
    }

    pub fn add_manual_winrates(&self, req: AddWinratesRequest) -> anyhow::Result<AddWinratesResponse> {
        let AddWinratesRequest {
            groups,
            outer_workers,
            inner_workers,
        } = req;

        let config = self.config_with_run_options(outer_workers, inner_workers, None)?;
        let samples = config.win_rate_samples;
        if samples == 0 {
            anyhow::bail!("服务端 win_rate_samples 必须是正整数");
        }

        let lines: Vec<String> = groups.into_iter().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();

        let requested_pairs = (lines.len() + 1) / 2;
        let mut ignored_pairs = Vec::new();
        let mut pairs = Vec::new();

        for (chunk_idx, chunk) in lines.chunks(2).enumerate() {
            let pair_index = chunk_idx + 1;
            if chunk.len() < 2 {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: chunk.get(0).cloned(),
                    group_b: None,
                    reason: "输入行数为奇数，最后一行没有对手；该 pair 未测也未写入数据库".to_string(),
                });
                continue;
            }

            let raw_a = chunk[0].trim().to_string();
            let raw_b = chunk[1].trim().to_string();

            let parsed_a = match parse_group(&raw_a) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored_pairs.push(IgnoredWinratePair {
                        pair_index,
                        group_a: Some(raw_a),
                        group_b: Some(raw_b),
                        reason: format!("第一行组合无法解析：{err}"),
                    });
                    continue;
                }
            };
            let parsed_b = match parse_group(&raw_b) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored_pairs.push(IgnoredWinratePair {
                        pair_index,
                        group_a: Some(raw_a),
                        group_b: Some(raw_b),
                        reason: format!("第二行组合无法解析：{err}"),
                    });
                    continue;
                }
            };

            if parsed_a.lane_size != parsed_b.lane_size {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(parsed_a.canonical),
                    group_b: Some(parsed_b.canonical),
                    reason: format!(
                        "两行组合人数不同：{} vs {}；该 pair 未写入数据库",
                        parsed_a.lane_size, parsed_b.lane_size
                    ),
                });
                continue;
            }

            let Some(group_a) = self.db.find_stored_group_by_canonical(&parsed_a.canonical)? else {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(parsed_a.canonical),
                    group_b: Some(parsed_b.canonical),
                    reason: "第一行组合不在数据库中；该 pair 未测也未写入数据库".to_string(),
                });
                continue;
            };
            let Some(group_b) = self.db.find_stored_group_by_canonical(&parsed_b.canonical)? else {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(group_a.canonical.clone()),
                    group_b: Some(parsed_b.canonical),
                    reason: "第二行组合不在数据库中；该 pair 未测也未写入数据库".to_string(),
                });
                continue;
            };

            if group_a.id == group_b.id {
                ignored_pairs.push(IgnoredWinratePair {
                    pair_index,
                    group_a: Some(group_a.canonical.clone()),
                    group_b: Some(group_b.canonical.clone()),
                    reason: "同一个组合不能手动测自身胜率；该 pair 未写入数据库".to_string(),
                });
                continue;
            }

            pairs.push(ManualWinratePair {
                pair_index,
                group_a,
                group_b,
            });
        }

        let total = pairs.len();
        if total == 0 {
            return Ok(AddWinratesResponse {
                requested_pairs,
                computed_pairs: 0,
                stored_pairs: 0,
                ignored_pairs,
                results: Vec::new(),
                samples,
                outer_workers: 0,
                mode: "no_valid_pairs".to_string(),
            });
        }

        let workers = resolve_manual_winrate_workers(config.outer_workers, total);
        let mode = if config.outer_workers == 0 {
            "dynamic_queue"
        } else {
            "static_chunks"
        }
        .to_string();
        let pairs = Arc::new(pairs);
        let done = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::with_capacity(workers);

        if config.outer_workers == 0 {
            let next_pair = Arc::new(AtomicUsize::new(0));
            for _ in 0..workers {
                let db = self.db.clone();
                let pairs = Arc::clone(&pairs);
                let next_pair = Arc::clone(&next_pair);
                let done = Arc::clone(&done);
                let inner_workers = config.inner_workers;
                handles.push(thread::spawn(move || -> anyhow::Result<Vec<ComputedManualWinrate>> {
                    let mut computed = Vec::new();
                    let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                    loop {
                        let idx = next_pair.fetch_add(1, Ordering::Relaxed);
                        let Some(pair) = pairs.get(idx) else {
                            break;
                        };
                        let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                        done.fetch_add(1, Ordering::Relaxed);
                        pending.push((pair.group_a.id, pair.group_b.id, rate));
                        persist_service_rate_checkpoint(&db, &mut pending, samples, false)?;
                        computed.push(ComputedManualWinrate::from_pair(pair, rate, samples));
                    }
                    persist_service_rate_checkpoint(&db, &mut pending, samples, true)?;
                    Ok(computed)
                }));
            }
        } else {
            for worker_id in 0..workers {
                let db = self.db.clone();
                let pairs = Arc::clone(&pairs);
                let done = Arc::clone(&done);
                let inner_workers = config.inner_workers;
                let start = total * worker_id / workers;
                let end = total * (worker_id + 1) / workers;
                handles.push(thread::spawn(move || -> anyhow::Result<Vec<ComputedManualWinrate>> {
                    let mut computed = Vec::with_capacity(end.saturating_sub(start));
                    let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                    for idx in start..end {
                        let Some(pair) = pairs.get(idx) else {
                            break;
                        };
                        let rate = compute_rate_without_db(&pair.group_a, &pair.group_b, samples, inner_workers)?;
                        done.fetch_add(1, Ordering::Relaxed);
                        pending.push((pair.group_a.id, pair.group_b.id, rate));
                        persist_service_rate_checkpoint(&db, &mut pending, samples, false)?;
                        computed.push(ComputedManualWinrate::from_pair(pair, rate, samples));
                    }
                    persist_service_rate_checkpoint(&db, &mut pending, samples, true)?;
                    Ok(computed)
                }));
            }
        }

        let mut computed = Vec::<ComputedManualWinrate>::with_capacity(total);
        for handle in handles {
            let part = handle.join().expect("manual winrate worker thread panicked")?;
            computed.extend(part);
        }
        computed.sort_by_key(|row| row.pair_index);

        let db_rates: Vec<_> = computed.iter().map(|row| (row.group_a_id, row.group_b_id, row.win_rate_a)).collect();
        self.db.save_rate_pairs_bulk(&db_rates, samples)?;

        let results = computed
            .into_iter()
            .map(|row| AddedWinrateRow {
                pair_index: row.pair_index,
                group_a: row.group_a,
                group_b: row.group_b,
                group_a_id: row.group_a_id,
                group_b_id: row.group_b_id,
                lane_size: row.lane_size,
                win_rate_a: row.win_rate_a,
                win_rate_b: 100.0 - row.win_rate_a,
                samples: row.samples,
                stored: true,
            })
            .collect::<Vec<_>>();

        Ok(AddWinratesResponse {
            requested_pairs,
            computed_pairs: results.len(),
            stored_pairs: results.len(),
            ignored_pairs,
            results,
            samples,
            outer_workers: workers,
            mode,
        })
    }

    pub fn set_group_blocked(&self, group_id: i64, blocked: bool, req: BlockGroupRequest) -> anyhow::Result<BlockGroupResponse> {
        let config = self.config_with_run_options(req.outer_workers, req.inner_workers, req.skip_archived)?;
        let Some((lane_size, canonical)) = self.db.set_group_blocked(group_id, blocked)? else {
            anyhow::bail!("group id {group_id} not found");
        };

        let queued_lanes = self.queue_recompute_lanes_with_config(vec![lane_size], config)?;
        Ok(BlockGroupResponse {
            group_id,
            lane_size,
            canonical,
            blocked,
            queued_lanes,
        })
    }

    pub fn set_groups_blocked_by_text(
        &self,
        blocked: bool,
        req: BlockGroupsByTextRequest,
    ) -> anyhow::Result<BlockGroupsByTextResponse> {
        let BlockGroupsByTextRequest {
            groups,
            outer_workers,
            inner_workers,
            skip_archived,
        } = req;
        let config = self.config_with_run_options(outer_workers, inner_workers, skip_archived)?;

        let mut blocked_groups = BTreeSet::new();
        let mut unblocked_groups = BTreeSet::new();
        let mut ignored = Vec::new();
        let mut dirty_lanes = BTreeSet::new();

        for raw in groups {
            let raw = raw.trim().to_string();
            if raw.is_empty() {
                continue;
            }

            let parsed = match parse_group(&raw) {
                Ok(parsed) => parsed,
                Err(err) => {
                    ignored.push(IgnoredGroup {
                        raw,
                        reason: err.to_string(),
                    });
                    continue;
                }
            };

            let Some((group_id, _, _)) = self.db.find_group_by_canonical(&parsed.canonical)? else {
                ignored.push(IgnoredGroup {
                    raw,
                    reason: "未找到这个组合，请先加入组合".to_string(),
                });
                continue;
            };

            let Some((changed_lane_size, changed_canonical)) = self.db.set_group_blocked(group_id, blocked)? else {
                ignored.push(IgnoredGroup {
                    raw,
                    reason: "组合更新前已经不存在".to_string(),
                });
                continue;
            };

            dirty_lanes.insert(changed_lane_size);
            if blocked {
                blocked_groups.insert(changed_canonical);
            } else {
                unblocked_groups.insert(changed_canonical);
            }
        }

        let queued_lanes = self.queue_recompute_lanes_with_config(dirty_lanes.into_iter().collect(), config)?;

        Ok(BlockGroupsByTextResponse {
            blocked: blocked_groups.into_iter().collect(),
            unblocked: unblocked_groups.into_iter().collect(),
            ignored,
            queued_lanes,
        })
    }

    pub fn merge_teams(&self, req: MergeTeamsRequest) -> anyhow::Result<MergeTeamsResponse> {
        let config = self.config_with_run_options(req.outer_workers, req.inner_workers, req.skip_archived)?;

        let mut dsu = self.db.load_team_dsu()?;
        let root = dsu.union(req.x.trim(), req.y.trim());
        self.db.save_team_dsu(&dsu)?;

        let lanes = self.db.all_nonempty_lanes()?;
        let queued_lanes = self.queue_recompute_lanes_with_config(lanes, config)?;

        Ok(MergeTeamsResponse {
            merged_root: root,
            queued_lanes,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct ManualWinratePair {
    pub(super) pair_index: usize,
    pub(super) group_a: StoredGroup,
    pub(super) group_b: StoredGroup,
}

#[derive(Debug, Clone)]
pub(super) struct ComputedManualWinrate {
    pub(super) pair_index: usize,
    pub(super) group_a: String,
    pub(super) group_b: String,
    pub(super) group_a_id: i64,
    pub(super) group_b_id: i64,
    pub(super) lane_size: usize,
    pub(super) win_rate_a: f64,
    pub(super) samples: usize,
}

impl ComputedManualWinrate {
    pub(super) fn from_pair(pair: &ManualWinratePair, win_rate_a: f64, samples: usize) -> Self {
        Self {
            pair_index: pair.pair_index,
            group_a: pair.group_a.canonical.clone(),
            group_b: pair.group_b.canonical.clone(),
            group_a_id: pair.group_a.id,
            group_b_id: pair.group_b.id,
            lane_size: pair.group_a.lane_size,
            win_rate_a,
            samples,
        }
    }
}

pub(super) fn resolve_manual_winrate_workers(requested_outer_workers: usize, total: usize) -> usize {
    if total == 0 {
        return 0;
    }
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }

    thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1).min(total.max(1))
}

pub(super) fn persist_service_rate_checkpoint(
    db: &Db,
    pending: &mut Vec<(crate::model::GroupId, crate::model::GroupId, f64)>,
    samples: usize,
    force: bool,
) -> anyhow::Result<()> {
    if pending.len() < RATE_PERSIST_CHECKPOINT_SIZE && !force {
        return Ok(());
    }
    db.save_rate_pairs_bulk(pending, samples)?;
    pending.clear();
    Ok(())
}

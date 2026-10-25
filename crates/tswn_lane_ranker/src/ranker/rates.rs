//! 胜率矩阵、缺失对局并行补算和检查点写入。
//!
//! 矩阵预留 O(n²) 槽位，实际只补算新标准列与全部候选的交叉边；对角线为 50。
//! 反向值用 100-rate 填充，缓存读取失败或计算失败返回错误；部分检查点可能已提交。
//! covered_standard_indices 仅在该列全部就绪后更新，get 要求调用方先完成对应加载。

use super::*;

pub(super) struct RateMatrix {
    pub(super) rates: Vec<Vec<Option<f64>>>,
    pub(super) covered_standard_indices: HashSet<usize>,
    pub(super) loaded_pairs: usize,
}

impl RateMatrix {
    pub(super) fn new(size: usize) -> Self {
        let mut rates = vec![vec![None; size]; size];
        for i in 0..size {
            rates[i][i] = Some(50.0);
        }

        Self {
            rates,
            covered_standard_indices: HashSet::new(),
            loaded_pairs: 0,
        }
    }

    pub(super) fn get(&self, i: usize, j: usize) -> f64 { self.rates[i][j].expect("rate should be loaded before fight") }

    pub(super) fn loaded_pair_count(&self) -> usize { self.loaded_pairs }

    pub(super) fn set_pair(&mut self, i: usize, j: usize, rate_i_to_j: f64) {
        if i == j {
            self.rates[i][j] = Some(50.0);
            return;
        }

        if self.rates[i][j].is_none() {
            self.loaded_pairs += 1;
        }

        self.rates[i][j] = Some(rate_i_to_j);
        self.rates[j][i] = Some(100.0 - rate_i_to_j);
    }

    pub(super) fn ensure_rates_for_standard(
        &mut self,
        db: &Db,
        lane_size: usize,
        round: usize,
        nodes: &[RankNode],
        standard: &[usize],
        config: &RankerConfig,
    ) -> anyhow::Result<()> {
        let new_standard: Vec<usize> = standard
            .iter()
            .copied()
            .filter(|idx| !self.covered_standard_indices.contains(idx))
            .collect();

        if new_standard.is_empty() {
            return Ok(());
        }

        let mut missing_pairs = Vec::<(usize, usize, StoredGroup, StoredGroup)>::new();
        let mut seen_pairs = HashSet::<(usize, usize)>::new();

        for &std_idx in &new_standard {
            for i in 0..nodes.len() {
                if i == std_idx {
                    continue;
                }

                if self.rates[i][std_idx].is_some() {
                    continue;
                }

                let (a_idx, b_idx) = ordered_index_pair(i, std_idx);
                if !seen_pairs.insert((a_idx, b_idx)) {
                    continue;
                }

                if let Some(rate) = db.get_rate(nodes[i].group.id, nodes[std_idx].group.id)? {
                    self.set_pair(i, std_idx, rate);
                    continue;
                }

                if let Some(reverse_rate) = db.get_rate(nodes[std_idx].group.id, nodes[i].group.id)? {
                    self.set_pair(i, std_idx, 100.0 - reverse_rate);
                    continue;
                }

                missing_pairs.push((i, std_idx, nodes[i].group.clone(), nodes[std_idx].group.clone()));
            }
        }

        if missing_pairs.is_empty() {
            for idx in new_standard {
                self.covered_standard_indices.insert(idx);
            }

            db.set_lane_progress(
                lane_size,
                "rates_ready",
                round,
                config.total_rounds,
                self.loaded_pair_count(),
                0,
                0,
                &format!(
                    "rates for current target already cached, loaded_pairs={}, mode=lazy_rate_matrix_dynamic_workers",
                    self.loaded_pair_count()
                ),
            )?;

            return Ok(());
        }

        let total = missing_pairs.len();
        let requested_outer_workers = config.outer_workers;
        let inner_workers = config.inner_workers;
        let workers = resolve_rate_pair_workers(requested_outer_workers, total);
        let outer_label = format_outer_workers(requested_outer_workers, workers);
        let inner_label = format_inner_workers(inner_workers);
        let mode = if requested_outer_workers == 0 {
            "dynamic_queue"
        } else {
            "static_chunks"
        };
        let done = Arc::new(AtomicUsize::new(0));
        let rate_started = Arc::new(Instant::now());

        db.set_lane_progress(
            lane_size,
            "computing_rates",
            round,
            config.total_rounds,
            0,
            total,
            0,
            &format!(
                "computing missing rates for current target 0/{total}, 0.00 pair/s, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"
            ),
        )?;

        let missing_pairs = Arc::new(missing_pairs);
        let mut handles = Vec::with_capacity(workers);

        if requested_outer_workers == 0 {
            let next_pair = Arc::new(AtomicUsize::new(0));

            for _worker_id in 0..workers {
                let db = db.clone();
                let missing_pairs = Arc::clone(&missing_pairs);
                let next_pair = Arc::clone(&next_pair);
                let done = Arc::clone(&done);
                let rate_started = Arc::clone(&rate_started);
                let samples = config.win_rate_samples;
                let total_rounds = config.total_rounds;
                let outer_label = outer_label.clone();
                let inner_label = inner_label.clone();

                handles.push(thread::spawn(move || -> anyhow::Result<Vec<(usize, usize, GroupId, GroupId, f64)>> {
                    let mut computed = Vec::new();
                    let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);

                    loop {
                        let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                        let Some((i, j, a, b)) = missing_pairs.get(pair_idx) else {
                            break;
                        };

                        let rate = compute_rate_without_db(a, b, samples, inner_workers)?;
                        computed.push((*i, *j, a.id, b.id, rate));
                        pending.push((a.id, b.id, rate));
                        persist_rate_checkpoint(&db, &mut pending, samples, false)?;

                        let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if should_report_rate_progress(current, total) {
                            let elapsed = rate_started.elapsed().as_secs_f64().max(0.001);
                            let pairs_per_sec = current as f64 / elapsed;
                            let eta_sec = if pairs_per_sec > 0.0 {
                                total.saturating_sub(current) as f64 / pairs_per_sec
                            } else {
                                0.0
                            };

                            db.set_lane_progress(
                                lane_size,
                                "computing_rates",
                                round,
                                total_rounds,
                                current,
                                total,
                                0,
                                &format!(
                                    "computing missing rates for current target {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_label}, mode=dynamic_queue",
                                    pairs_per_sec,
                                    format_duration(elapsed),
                                    format_duration(eta_sec)
                                ),
                            )?;
                        }
                    }
                    persist_rate_checkpoint(&db, &mut pending, samples, true)?;

                    Ok(computed)
                }));
            }
        } else {
            for worker_id in 0..workers {
                let db = db.clone();
                let missing_pairs = Arc::clone(&missing_pairs);
                let done = Arc::clone(&done);
                let rate_started = Arc::clone(&rate_started);
                let samples = config.win_rate_samples;
                let total_rounds = config.total_rounds;
                let outer_label = outer_label.clone();
                let inner_label = inner_label.clone();
                let start = total * worker_id / workers;
                let end = total * (worker_id + 1) / workers;

                handles.push(thread::spawn(move || -> anyhow::Result<Vec<(usize, usize, GroupId, GroupId, f64)>> {
                    let mut computed = Vec::with_capacity(end.saturating_sub(start));
                    let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);

                    for pair_idx in start..end {
                        let Some((i, j, a, b)) = missing_pairs.get(pair_idx) else {
                            break;
                        };

                        let rate = compute_rate_without_db(a, b, samples, inner_workers)?;
                        computed.push((*i, *j, a.id, b.id, rate));
                        pending.push((a.id, b.id, rate));
                        persist_rate_checkpoint(&db, &mut pending, samples, false)?;

                        let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if should_report_rate_progress(current, total) {
                            let elapsed = rate_started.elapsed().as_secs_f64().max(0.001);
                            let pairs_per_sec = current as f64 / elapsed;
                            let eta_sec = if pairs_per_sec > 0.0 {
                                total.saturating_sub(current) as f64 / pairs_per_sec
                            } else {
                                0.0
                            };

                            db.set_lane_progress(
                                lane_size,
                                "computing_rates",
                                round,
                                total_rounds,
                                current,
                                total,
                                0,
                                &format!(
                                    "computing missing rates for current target {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_label}, mode=static_chunks",
                                    pairs_per_sec,
                                    format_duration(elapsed),
                                    format_duration(eta_sec)
                                ),
                            )?;
                        }
                    }
                    persist_rate_checkpoint(&db, &mut pending, samples, true)?;

                    Ok(computed)
                }));
            }
        }

        let mut db_rates = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
        let mut matrix_rates = Vec::<(usize, usize, f64)>::with_capacity(total);

        for handle in handles {
            let part = handle.join().expect("rate pair worker thread panicked")?;
            for (i, j, a_id, b_id, rate) in part {
                db_rates.push((a_id, b_id, rate));
                matrix_rates.push((i, j, rate));
            }
        }

        db.set_lane_progress(
            lane_size,
            "saving_rates",
            round,
            config.total_rounds,
            total,
            total,
            0,
            &format!("saving {total} computed current-target rates in one transaction, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"),
        )?;

        db.save_rate_pairs_bulk(&db_rates, config.win_rate_samples)?;

        for (i, j, rate) in matrix_rates {
            self.set_pair(i, j, rate);
        }

        for idx in new_standard {
            self.covered_standard_indices.insert(idx);
        }

        let elapsed = rate_started.elapsed().as_secs_f64().max(0.001);
        let pairs_per_sec = total as f64 / elapsed;

        db.set_lane_progress(
            lane_size,
            "rates_ready",
            round,
            config.total_rounds,
            total,
            total,
            0,
            &format!(
                "computed and saved current-target rates {total}/{total}, {:.2} pair/s, elapsed {}, loaded_pairs={}, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}",
                pairs_per_sec,
                format_duration(elapsed),
                self.loaded_pair_count()
            ),
        )?;

        Ok(())
    }
}

pub(super) fn ordered_index_pair(a: usize, b: usize) -> (usize, usize) { if a <= b { (a, b) } else { (b, a) } }

pub(super) fn should_report_rate_progress(done: usize, total: usize) -> bool { done == total || done % 10 == 0 }

pub(super) const RATE_PERSIST_CHECKPOINT_SIZE: usize = 100;

pub(super) fn persist_rate_checkpoint(
    db: &Db,
    pending: &mut Vec<(GroupId, GroupId, f64)>,
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

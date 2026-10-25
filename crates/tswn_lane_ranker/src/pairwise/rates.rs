//! 校准所需胜率的检查、并行补算和持久化。
//!
//! 按无序 group_id 对查找已存观测，缺失边采用动态队列或静态分块计算，按百条检查点保存。
//! 已存边只要求 samples>0，不按本次采样数重测；更换统计或引擎版本须由宿主管理缓存。
//! worker 失败可在已有批次提交后返回，其他已启动 worker 不具备统一取消协议。

use super::*;

pub(super) fn compute_rate_pairs_by_id_request(
    db: &Db,
    lane_size: usize,
    config: &RankerConfig,
    pairs: &[(GroupId, GroupId)],
    context: &str,
) -> anyhow::Result<usize> {
    let existing_rates = load_existing_rate_pairs_for_python(db)?;
    let stored_by_id: HashMap<GroupId, StoredGroup> = db.load_groups_by_lane(lane_size)?.into_iter().map(|g| (g.id, g)).collect();
    let mut missing = Vec::<RequiredRatePair>::new();
    let mut required = HashSet::<(GroupId, GroupId)>::new();
    for &(a_id, b_id) in pairs {
        if a_id == b_id {
            continue;
        }
        let pair = ordered_group_id_pair(a_id, b_id);
        required.insert(pair);
        if existing_rates.contains(&pair) {
            continue;
        }
        let a = stored_by_id
            .get(&pair.0)
            .cloned()
            .with_context(|| format!("dynamic strict Python rate request references missing group_id={}", pair.0))?;
        let b = stored_by_id
            .get(&pair.1)
            .cloned()
            .with_context(|| format!("dynamic strict Python rate request references missing group_id={}", pair.1))?;
        missing.push(RequiredRatePair { a, b });
    }
    if missing.is_empty() {
        return Ok(0);
    }

    let total = missing.len();
    let workers = resolve_calibration_rate_pair_workers(config.outer_workers, total);
    let outer_label = format_calibration_outer_workers(config.outer_workers, workers);
    let inner_label = format_calibration_inner_workers(config.inner_workers);
    let mode = if config.outer_workers == 0 {
        "dynamic_queue"
    } else {
        "static_chunks"
    };
    let done = Arc::new(AtomicUsize::new(0));
    let rate_started = Arc::new(Instant::now());
    let missing = Arc::new(missing);

    db.set_lane_progress(
        lane_size,
        "calibration_dynamic_missing_rates",
        0,
        config.total_rounds,
        0,
        total,
        0,
        &format!(
            "computing dynamic strict Python missing rates for {context}: 0/{total}, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"
        ),
    )?;

    let mut handles = Vec::with_capacity(workers);
    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let next_pair = Arc::clone(&next_pair);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            let context = context.to_string();
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::new();
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute dynamic strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            &context,
                            rate_started.elapsed().as_secs_f64(),
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
            let missing = Arc::clone(&missing);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            let context = context.to_string();
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::with_capacity(end.saturating_sub(start));
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute dynamic strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            &context,
                            rate_started.elapsed().as_secs_f64(),
                        )?;
                    }
                }
                persist_rate_checkpoint(&db, &mut pending, samples, true)?;
                Ok(computed)
            }));
        }
    }

    let mut computed = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
    for handle in handles {
        let mut rows = handle
            .join()
            .map_err(|_| anyhow::anyhow!("dynamic strict calibration rate worker panicked"))??;
        computed.append(&mut rows);
    }
    if computed.len() != total {
        anyhow::bail!(
            "dynamic strict calibration rate fill computed {} pairs but expected {}; refusing to continue",
            computed.len(),
            total
        );
    }
    db.save_rate_pairs_bulk(&computed, config.win_rate_samples)?;
    Ok(computed.len())
}

pub(super) fn ensure_required_winrates_for_strict_python(
    db: &Db,
    lane_size: usize,
    groups: &[CalibGroup],
    config: &RankerConfig,
    cqd_threshold: f64,
) -> anyhow::Result<usize> {
    let train_ids: Vec<GroupId> = groups.iter().filter(|g| g.train_eligible).map(|g| g.group_id).collect();
    if train_ids.len() < 2 {
        anyhow::bail!(
            "strict Python calibration aborted before rate fill: not_enough_fit_candidates; fit_candidates={}",
            train_ids.len()
        );
    }

    // Python 拟合 Raw >= --raw-min 的所有未阻塞行。阻塞行被排除在拟合外，但 Python 仅评分分支需要每条
    // Raw >= --raw-min 的阻塞行均拥有对冻结训练池的边。
    let blocked_score_only_ids: Vec<GroupId> = groups
        .iter()
        .filter(|g| g.is_blocked && g.raw_score >= cqd_threshold)
        .map(|g| g.group_id)
        .collect();

    let mut required = HashSet::<(GroupId, GroupId)>::new();
    for i in 0..train_ids.len() {
        for j in (i + 1)..train_ids.len() {
            required.insert(ordered_group_id_pair(train_ids[i], train_ids[j]));
        }
    }
    for &blocked_id in &blocked_score_only_ids {
        for &train_id in &train_ids {
            if blocked_id != train_id {
                required.insert(ordered_group_id_pair(blocked_id, train_id));
            }
        }
    }

    let existing_rates = load_existing_rate_pairs_for_python(db)?;
    let stored_by_id: HashMap<GroupId, StoredGroup> = db.load_groups_by_lane(lane_size)?.into_iter().map(|g| (g.id, g)).collect();
    let mut missing = Vec::<RequiredRatePair>::new();
    for (a_id, b_id) in required.iter().copied() {
        if existing_rates.contains(&ordered_group_id_pair(a_id, b_id)) {
            continue;
        }
        let a = stored_by_id
            .get(&a_id)
            .cloned()
            .with_context(|| format!("required calibration rate references missing group_id={a_id}"))?;
        let b = stored_by_id
            .get(&b_id)
            .cloned()
            .with_context(|| format!("required calibration rate references missing group_id={b_id}"))?;
        missing.push(RequiredRatePair { a, b });
    }

    if missing.is_empty() {
        db.set_lane_progress(
            lane_size,
            "calibration_rates_ready",
            0,
            config.total_rounds,
            required.len(),
            required.len(),
            0,
            &format!(
                "all strict Python calibration win-rate pairs already cached: required_pairs={}, train_pairs={}, blocked_score_only_pairs={}",
                required.len(),
                train_ids.len().saturating_mul(train_ids.len().saturating_sub(1)) / 2,
                blocked_score_only_ids.len().saturating_mul(train_ids.len())
            ),
        )?;
        return Ok(0);
    }

    let total = missing.len();
    let workers = resolve_calibration_rate_pair_workers(config.outer_workers, total);
    let outer_label = format_calibration_outer_workers(config.outer_workers, workers);
    let inner_label = format_calibration_inner_workers(config.inner_workers);
    let mode = if config.outer_workers == 0 {
        "dynamic_queue"
    } else {
        "static_chunks"
    };
    let done = Arc::new(AtomicUsize::new(0));
    let rate_started = Arc::new(Instant::now());
    let missing = Arc::new(missing);

    db.set_lane_progress(
        lane_size,
        "calibration_computing_missing_rates",
        0,
        config.total_rounds,
        0,
        total,
        0,
        &format!(
            "computing missing strict Python calibration rates 0/{total}, 0.00 pair/s, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}"
        ),
    )?;

    let mut handles = Vec::with_capacity(workers);
    if config.outer_workers == 0 {
        let next_pair = Arc::new(AtomicUsize::new(0));
        for _worker_id in 0..workers {
            let db = db.clone();
            let missing = Arc::clone(&missing);
            let next_pair = Arc::clone(&next_pair);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::new();
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                loop {
                    let pair_idx = next_pair.fetch_add(1, Ordering::Relaxed);
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            "dynamic_queue",
                            rate_started.elapsed().as_secs_f64(),
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
            let missing = Arc::clone(&missing);
            let done = Arc::clone(&done);
            let rate_started = Arc::clone(&rate_started);
            let samples = config.win_rate_samples;
            let inner_workers = config.inner_workers;
            let total_rounds = config.total_rounds;
            let outer_label = outer_label.clone();
            let inner_label = inner_label.clone();
            let start = total * worker_id / workers;
            let end = total * (worker_id + 1) / workers;
            handles.push(thread::spawn(move || -> anyhow::Result<Vec<(GroupId, GroupId, f64)>> {
                let mut computed = Vec::with_capacity(end.saturating_sub(start));
                let mut pending = Vec::with_capacity(RATE_PERSIST_CHECKPOINT_SIZE);
                for pair_idx in start..end {
                    let Some(pair) = missing.get(pair_idx) else {
                        break;
                    };
                    let rate = compute_rate_without_db(&pair.a, &pair.b, samples, inner_workers).with_context(|| {
                        format!(
                            "compute strict calibration win rate: {} vs {}",
                            pair.a.canonical, pair.b.canonical
                        )
                    })?;
                    computed.push((pair.a.id, pair.b.id, rate));
                    pending.push((pair.a.id, pair.b.id, rate));
                    persist_rate_checkpoint(&db, &mut pending, samples, false)?;
                    let current = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if should_report_calibration_rate_progress(current, total) {
                        report_calibration_rate_progress(
                            &db,
                            lane_size,
                            total_rounds,
                            current,
                            total,
                            &outer_label,
                            &inner_label,
                            "static_chunks",
                            rate_started.elapsed().as_secs_f64(),
                        )?;
                    }
                }
                persist_rate_checkpoint(&db, &mut pending, samples, true)?;
                Ok(computed)
            }));
        }
    }

    let mut computed = Vec::<(GroupId, GroupId, f64)>::with_capacity(total);
    for handle in handles {
        let mut rows = handle.join().map_err(|_| anyhow::anyhow!("strict calibration rate worker panicked"))??;
        computed.append(&mut rows);
    }
    if computed.len() != total {
        anyhow::bail!(
            "strict calibration rate fill computed {} pairs but expected {}; refusing to call Python with incomplete win-rate data",
            computed.len(),
            total
        );
    }
    db.save_rate_pairs_bulk(&computed, config.win_rate_samples)?;
    db.set_lane_progress(
        lane_size,
        "calibration_rates_ready",
        0,
        config.total_rounds,
        required.len(),
        required.len(),
        0,
        &format!(
            "strict Python calibration rates ready: newly_computed_pairs={}, total_required_pairs={}",
            computed.len(),
            required.len()
        ),
    )?;
    Ok(computed.len())
}

pub(super) fn load_existing_rate_pairs_for_python(db: &Db) -> anyhow::Result<HashSet<(GroupId, GroupId)>> {
    let conn = Connection::open(db.path()).with_context(|| format!("open sqlite database: {}", db.path()))?;
    let mut stmt = conn.prepare("SELECT group_a, group_b FROM group_rates WHERE samples > 0 AND win_rate_a IS NOT NULL")?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, GroupId>(0)?, row.get::<_, GroupId>(1)?)))?;
    let mut out = HashSet::new();
    for row in rows {
        let (a, b) = row?;
        if a != b {
            out.insert(ordered_group_id_pair(a, b));
        }
    }
    Ok(out)
}

pub(super) fn ordered_group_id_pair(a: GroupId, b: GroupId) -> (GroupId, GroupId) { if a <= b { (a, b) } else { (b, a) } }

pub(super) fn resolve_calibration_rate_pair_workers(requested_outer_workers: usize, total: usize) -> usize {
    if requested_outer_workers > 0 {
        return requested_outer_workers.max(1).min(total.max(1));
    }
    let available = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    available.max(1).min(total.max(1))
}

pub(super) fn format_calibration_outer_workers(requested_outer_workers: usize, actual_workers: usize) -> String {
    if requested_outer_workers == 0 {
        format!("auto({actual_workers})")
    } else {
        format!("static({actual_workers})")
    }
}

pub(super) fn format_calibration_inner_workers(inner_workers: u32) -> String {
    if inner_workers == 0 {
        "auto(0)".to_string()
    } else {
        inner_workers.to_string()
    }
}

pub(super) fn should_report_calibration_rate_progress(done: usize, total: usize) -> bool { done == total || done % 10 == 0 }

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

pub(super) fn report_calibration_rate_progress(
    db: &Db,
    lane_size: usize,
    total_rounds: usize,
    current: usize,
    total: usize,
    outer_label: &str,
    inner_label: &str,
    mode: &str,
    elapsed_secs: f64,
) -> anyhow::Result<()> {
    let elapsed = elapsed_secs.max(0.001);
    let pairs_per_sec = current as f64 / elapsed;
    let eta_sec = if pairs_per_sec > 0.0 {
        total.saturating_sub(current) as f64 / pairs_per_sec
    } else {
        0.0
    };
    db.set_lane_progress(
        lane_size,
        "calibration_computing_missing_rates",
        0,
        total_rounds,
        current,
        total,
        0,
        &format!(
            "computing missing strict Python calibration rates {current}/{total}, {:.2} pair/s, elapsed {}, eta {}, outer_workers={outer_label}, inner_threads={inner_label}, mode={mode}",
            pairs_per_sec,
            format_calibration_duration(elapsed),
            format_calibration_duration(eta_sec)
        ),
    )
}

pub(super) fn format_calibration_duration(seconds: f64) -> String {
    if seconds < 60.0 {
        format!("{seconds:.1}s")
    } else if seconds < 3600.0 {
        format!("{:.1}m", seconds / 60.0)
    } else {
        format!("{:.1}h", seconds / 3600.0)
    }
}

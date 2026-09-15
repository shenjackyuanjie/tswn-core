//! Runtime 的 CQP/CQD 矩阵执行器。
//!
//! 将 matchup 和轮次区间统一动态派发给一组 worker：大量短 matchup 不再重复
//! 建线程，少量长 matchup 也不会被 matchup 数限制并行度。每个 worker 只缓存
//! 当前 matchup 的准备结果，避免为整个矩阵长期持有全部 Runtime roster。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    CustomRuntimeImportConfig, PreparedRuntimeRunner, RuntimeBatchError, RuntimeBatchSummary,
    default_custom_runtime_import_config, prepared_runtime_win_rate_range,
};

/// 一个已经拆成队伍的 CQP/CQD matchup。
#[derive(Debug, Clone)]
pub struct RuntimeCqpMatchup {
    pub groups: Vec<Vec<String>>,
}

impl RuntimeCqpMatchup {
    pub fn new(groups: Vec<Vec<String>>) -> Self { Self { groups } }
}

/// 单个 matchup 的计算结果；耗时为最早分片开始到最后分片完成的墙钟时间。
#[derive(Debug)]
pub struct RuntimeCqpMatchupResult {
    pub summary: Result<RuntimeBatchSummary, RuntimeBatchError>,
    pub elapsed: Duration,
}

/// 整个矩阵的执行结果。
#[derive(Debug)]
pub struct RuntimeCqpBatchResult {
    /// 与输入 matchup 顺序一一对应；取消后未完整完成的项保持 `None`。
    pub matchups: Vec<Option<RuntimeCqpMatchupResult>>,
    pub completed: usize,
}

struct RangeEvent {
    index: usize,
    summary: Result<RuntimeBatchSummary, RuntimeBatchError>,
    started: Instant,
    finished: Instant,
}

#[derive(Debug, Clone, Copy)]
struct RangePlan {
    parts: usize,
    tasks: usize,
    workers: usize,
}

impl RangePlan {
    fn new(thread: u32, matchups: usize, n: usize) -> Self {
        if matchups == 0 {
            return Self {
                parts: 1,
                tasks: 0,
                workers: 1,
            };
        }
        let requested = resolve_cqp_workers(thread, usize::MAX, n);
        // 尽量提供每个 worker 四份工作；同时限制每次不可取消的批量区间。
        // 短任务不拆得过细，避免 roster/runner 初始化反而成为主开销。
        let parts = requested
            .saturating_mul(4)
            .div_ceil(matchups)
            .min(n.div_ceil(64).max(1))
            .max(n.div_ceil(256))
            .max(1)
            .min(usize::MAX / matchups);
        let tasks = matchups * parts;
        Self {
            parts,
            tasks,
            workers: requested.min(tasks).max(1),
        }
    }

    fn bounds(self, part: usize, n: usize) -> (usize, usize) {
        let base = n / self.parts;
        let extra = n % self.parts;
        let start = part * base + part.min(extra);
        (start, start + base + usize::from(part < extra))
    }
}

struct Accumulator {
    parts: usize,
    summary: Result<RuntimeBatchSummary, RuntimeBatchError>,
    started: Option<Instant>,
    finished: Option<Instant>,
}

impl Accumulator {
    fn new() -> Self {
        Self {
            parts: 0,
            summary: Ok(RuntimeBatchSummary::default()),
            started: None,
            finished: None,
        }
    }

    fn record(&mut self, event: RangeEvent) {
        self.parts += 1;
        self.started = Some(self.started.map_or(event.started, |old| old.min(event.started)));
        self.finished = Some(self.finished.map_or(event.finished, |old| old.max(event.finished)));
        match (&mut self.summary, event.summary) {
            (Ok(total), Ok(part)) => total.merge(part),
            (total @ Ok(_), Err(err)) => *total = Err(err),
            (Err(_), _) => {}
        }
    }
}

/// 动态调度 matchup × 轮次区间，不嵌套启动内层线程。
///
/// 每个完整 matchup 只调用一次 `on_complete`，且回调只在调用线程执行；
/// 结果按输入顺序保存。分片始终使用原始轮次编号，因此线程数不改变 seed。
pub fn runtime_cqp_matchups(
    matchups: &[RuntimeCqpMatchup],
    n: usize,
    eval_rq: f64,
    thread: u32,
    cancel: &AtomicBool,
    mut on_complete: impl FnMut(),
) -> Result<RuntimeCqpBatchResult, RuntimeBatchError> {
    let mut ordered = (0..matchups.len()).map(|_| None).collect::<Vec<_>>();
    if matchups.is_empty() || cancel.load(Ordering::Relaxed) {
        return Ok(RuntimeCqpBatchResult {
            matchups: ordered,
            completed: 0,
        });
    }
    let config = default_custom_runtime_import_config()?;
    let plan = RangePlan::new(thread, matchups.len(), n);
    let next = AtomicUsize::new(0);
    let mut accumulators = (0..matchups.len()).map(|_| Accumulator::new()).collect::<Vec<_>>();
    let mut completed = 0;
    let mut record = |event: RangeEvent| {
        let index = event.index;
        let accumulator = &mut accumulators[index];
        accumulator.record(event);
        if accumulator.parts == plan.parts {
            ordered[index] = Some(RuntimeCqpMatchupResult {
                summary: std::mem::replace(&mut accumulator.summary, Ok(RuntimeBatchSummary::default())),
                elapsed: accumulator.finished.unwrap().duration_since(accumulator.started.unwrap()),
            });
            completed += 1;
            on_complete();
        }
    };

    if plan.workers <= 1 {
        run_range_jobs(matchups, n, eval_rq, &config, plan, &next, cancel, |event| {
            record(event);
            true
        });
    } else {
        // 限制完成事件队列；GUI 慢时不无限积压结果。进度不是逐场事件。
        let (tx, rx) = mpsc::sync_channel::<RangeEvent>(plan.workers.saturating_mul(2));
        std::thread::scope(|scope| {
            // 接收器归 scope 闭包所有：回调 panic 时必须先断开通道再 join，
            // 否则阻塞在有界 send 的 worker 会与 scope 的析构互相等待。
            let rx = rx;
            for _ in 0..plan.workers {
                let tx = tx.clone();
                let config = &config;
                let next = &next;
                scope.spawn(move || {
                    run_range_jobs(matchups, n, eval_rq, config, plan, next, cancel, |event| tx.send(event).is_ok());
                });
            }
            drop(tx);
            for event in rx {
                record(event);
            }
        });
    }
    Ok(RuntimeCqpBatchResult {
        matchups: ordered,
        completed,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_range_jobs(
    matchups: &[RuntimeCqpMatchup],
    n: usize,
    eval_rq: f64,
    config: &CustomRuntimeImportConfig<'static>,
    plan: RangePlan,
    next: &AtomicUsize,
    cancel: &AtomicBool,
    mut send: impl FnMut(RangeEvent) -> bool,
) {
    let mut cached_index = usize::MAX;
    let mut prepared = None;
    loop {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let task = next.fetch_add(1, Ordering::Relaxed);
        if task >= plan.tasks {
            break;
        }
        let index = task / plan.parts;
        let (start, end) = plan.bounds(task % plan.parts, n);
        let started = Instant::now();
        if cached_index != index {
            prepared = Some(
                PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&matchups[index].groups, eval_rq, config.clone())
                    .map_err(RuntimeBatchError::from),
            );
            cached_index = index;
        }
        let summary = prepared
            .as_ref()
            .unwrap()
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|runner| prepared_runtime_win_rate_range(runner, start, end));
        if !send(RangeEvent {
            index,
            summary,
            started,
            finished: Instant::now(),
        }) {
            break;
        }
    }
}

/// 解析 worker 预算，`total` 是可调度任务数，而非必须是 matchup 数。
/// 自动模式保持原来的短任务 1.5 倍、中长任务 2 倍逻辑核策略。
pub fn resolve_cqp_workers(thread: u32, total: usize, n: usize) -> usize {
    if cfg!(target_family = "wasm") {
        return 1;
    }
    let workers = if thread == 0 {
        platform_default_cqp_workers(n)
    } else {
        thread as usize
    };
    workers.max(1).min(total.max(1))
}

#[cfg(target_family = "wasm")]
fn platform_default_cqp_workers(_n: usize) -> usize { 1 }

#[cfg(not(target_family = "wasm"))]
fn platform_default_cqp_workers(n: usize) -> usize {
    let logical = std::thread::available_parallelism().map_or(1, |value| value.get());
    if n <= 100 {
        logical.saturating_mul(3).div_ceil(2)
    } else {
        logical.saturating_mul(2)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cqp_matrix_keeps_input_order_and_win_totals() {
        let eval_rq = crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
        let matchups = vec![
            RuntimeCqpMatchup::new(vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]]),
            RuntimeCqpMatchup::new(vec![vec!["alpha@red".to_owned()], vec!["beta@blue".to_owned()]]),
        ];
        let cancel = AtomicBool::new(false);
        let mut progress = 0;
        let batch = runtime_cqp_matchups(&matchups, 24, eval_rq, 4, &cancel, || progress += 1).expect("CQP 矩阵应成功执行");

        assert_eq!(batch.completed, 2);
        assert_eq!(progress, 2);
        for (matchup, actual) in matchups.iter().zip(batch.matchups) {
            let actual = actual.expect("未取消的 matchup 应有结果").summary.expect("matchup 应成功");
            let expected = super::super::runtime_groups_win_rate(&matchup.groups, 24, eval_rq, 1).expect("单 matchup 应成功");
            assert_eq!((actual.wins, actual.total), (expected.wins, expected.total));
        }
    }

    #[test]
    fn cqp_matrix_honors_preexisting_cancellation() {
        let matchups = vec![RuntimeCqpMatchup::new(vec![
            vec!["left@red".to_owned()],
            vec!["right@blue".to_owned()],
        ])];
        let cancel = AtomicBool::new(true);
        let batch = runtime_cqp_matchups(&matchups, 24, 6.0, 4, &cancel, || {}).expect("取消不应被视为执行错误");
        assert_eq!(batch.completed, 0);
        assert!(batch.matchups[0].is_none());
    }
}

#[cfg(test)]
mod range_regressions {
    use super::*;

    fn matchup() -> RuntimeCqpMatchup { RuntimeCqpMatchup::new(vec![vec!["alpha@red".to_owned()], vec!["beta@blue".to_owned()]]) }

    #[test]
    fn range_plan_covers_every_round_once_and_parallelizes_single_matchup() {
        for n in [0, 1, 24, 63, 64, 100, 129, 513, 10_000] {
            for m in [1, 2, 20, 1000] {
                let plan = RangePlan::new(4, m, n);
                let mut previous = 0;
                for part in 0..plan.parts {
                    let (start, end) = plan.bounds(part, n);
                    assert_eq!(start, previous);
                    assert!(end >= start && end - start <= 256);
                    previous = end;
                }
                assert_eq!(previous, n);
                assert_eq!(plan.tasks, plan.parts * m);
                assert!(plan.workers <= 4 && plan.workers <= plan.tasks);
            }
        }
        if !cfg!(target_family = "wasm") {
            assert_eq!(RangePlan::new(4, 1, 1024).workers, 4);
        }
    }

    #[test]
    fn split_matrix_matches_unsplit_runtime_including_zero_rounds() {
        let cancel = AtomicBool::new(false);
        let eval = crate::namerena::eval_name::WIN_RATE_EVAL_RQ;
        for n in [0, 1, 24, 129, 513] {
            let matchups = vec![matchup(), matchup()];
            let expected = super::super::runtime_groups_win_rate(&matchups[0].groups, n, eval, 1).unwrap();
            for threads in [1, 2, 4] {
                let mut callbacks = 0;
                let batch = runtime_cqp_matchups(&matchups, n, eval, threads, &cancel, || callbacks += 1).unwrap();
                assert_eq!(callbacks, 2);
                assert_eq!(batch.completed, 2);
                for actual in batch.matchups {
                    let actual = actual.unwrap().summary.unwrap();
                    assert_eq!(
                        (actual.wins, actual.total, actual.errors, actual.guard_exhausted),
                        (expected.wins, expected.total, expected.errors, expected.guard_exhausted)
                    );
                }
            }
        }
    }

    #[test]
    fn cancellation_is_checked_between_fragments() {
        let cancel = AtomicBool::new(false);
        let matchups = vec![matchup()];
        let config = default_custom_runtime_import_config().unwrap();
        let plan = RangePlan::new(1, 1, 1024);
        let next = AtomicUsize::new(0);
        let mut received = 0;
        run_range_jobs(&matchups, 1024, 6.0, &config, plan, &next, &cancel, |event| {
            received += 1;
            assert!(event.summary.unwrap().total <= 256);
            cancel.store(true, Ordering::Relaxed);
            true
        });
        assert_eq!(received, 1);
        assert!(received < plan.parts);
    }

    #[test]
    fn cancelling_from_callback_keeps_only_complete_results() {
        let cancel = AtomicBool::new(false);
        let matchups = vec![matchup(); 4];
        let batch = runtime_cqp_matchups(&matchups, 513, 6.0, 1, &cancel, || cancel.store(true, Ordering::Relaxed)).unwrap();
        assert_eq!(batch.completed, 1);
        assert_eq!(batch.matchups[0].as_ref().unwrap().summary.as_ref().unwrap().total, 513);
        assert!(batch.matchups[1..].iter().all(Option::is_none));
    }

    #[test]
    fn callback_panic_disconnects_bounded_queue() {
        let matchups = vec![matchup(); 128];
        let result = std::panic::catch_unwind(|| {
            runtime_cqp_matchups(&matchups, 0, 6.0, 4, &AtomicBool::new(false), || panic!("测试回调异常")).unwrap();
        });
        assert!(result.is_err());
    }
}

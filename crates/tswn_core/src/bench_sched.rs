//! 低精度批量任务的“外层并行”调度器。
//!
//! 当单局场数较小（1%/10% 这类低精度档位）时，内层 win-rate 的并行收益会被
//! 反复 `thread::spawn` 的开销吃掉。这时改成“外层 item 并行、内层单线程”往往更快：
//! 每个 worker 拿一个完整 item（namer-pf 的一组名字 / cqd-cqp 的一个选手）单线程跑完，
//! 多个 item 之间并行。
//!
//! 本模块只负责通用调度，不掺杂任何业务格式：
//! - [`low_accuracy_outer_workers`] 决定该不该走外层并行、用几个 worker；
//! - [`run_outer_parallel_ordered`] 把 items 派发给 worker 计算，并按 **item 原始顺序**
//!   回调 `emit`，从而让上层输出顺序与串行版本保持一致。
//!
//! 之所以放到 `tswn_core` 而不是各 bin 内部，是因为 CLI 与 openbox GUI 都需要这套调度，
//! 否则同一份 work-stealing + 有序回传逻辑会被复制好几份。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use crate::win_rate::resolve_win_rate_workers;

/// 低精度外层并行的场数上限：单局场数 ≤ 此值时才考虑走外层并行。
pub const LOW_ACCURACY_OUTER_PARALLEL_LIMIT: usize = 1000;

/// 进度回调最迟每个间隔被调用线程排空，worker 不为每个 tick 等待 channel。
const PROGRESS_POLL_INTERVAL: Duration = Duration::from_millis(40);

/// 计算外层并行应使用的 worker 数。
///
/// 返回 `1` 表示“无需外层并行，上层应回退到原有内层并行/串行路径”。
/// 低精度或 item 足够填满 worker 时使用外层并行；
/// `thread` 的语义与 [`resolve_win_rate_workers`] 一致（`0` = 自动）。
pub fn low_accuracy_outer_workers(n: usize, item_count: usize, thread: u32) -> usize {
    if item_count <= 1 {
        return 1;
    }
    let requested = resolve_win_rate_workers(thread, usize::MAX);
    // 高精度且 item 足够多时同样使用外层池，避免每个 item/评分项反复建线程。
    // item 不足以填满 worker 时保留内层并行回退。
    if n > LOW_ACCURACY_OUTER_PARALLEL_LIMIT && item_count < requested {
        return 1;
    }
    requested.min(item_count)
}

struct OuterEvent<R> {
    index: usize,
    result: R,
}

/// 外层并行执行器：把 `items` 派发给 `workers` 个 worker 并行计算，按 item 原始顺序 emit。
///
/// - `compute(index, item, tick)`：在 worker 线程上计算单个 item，内部应保持单线程。
///   `tick` 是一个细粒度进度回调，`compute` 可按需调用任意次（例如每完成一个 matchup 调一次），
///   这些调用会被汇集到主线程的 `on_tick`。
/// - `on_tick()`：在主线程被调用，用于推进总体进度（每次对应一次 `tick`）。
/// - `emit(result)`：在主线程**按 item 原始顺序**被调用；返回 `Err` 会触发取消并短路后续 emit。
///
/// 取消语义：传入的 `cancel` 被 worker 在每个 item 前检查；`emit` 返回 `Err` 时也会置位
/// `cancel`，让仍在运行的 worker 尽快收敛。返回值是已完成（产出 `Done`）的 item 数，
/// 或第一个 `emit` 错误。
pub fn run_outer_parallel_ordered<I, R>(
    items: &[I],
    workers: usize,
    cancel: &AtomicBool,
    compute: impl Fn(usize, &I, &dyn Fn()) -> R + Sync,
    mut on_tick: impl FnMut(),
    mut emit: impl FnMut(R) -> Result<(), String>,
) -> Result<usize, String>
where
    I: Sync,
    R: Send,
{
    let len = items.len();
    if len == 0 {
        return Ok(0);
    }

    let worker_count = if cfg!(target_family = "wasm") {
        1
    } else {
        workers.min(len).max(1)
    };
    if worker_count == 1 {
        // 单线程直接在调用线程执行，避免纯单线程/WASM 路径也创建 OS 线程。
        let ticks = std::cell::RefCell::new(&mut on_tick);
        let tick = || (*ticks.borrow_mut())();
        let mut completed = 0;
        for (index, item) in items.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let result = compute(index, item, &tick);
            completed += 1;
            if let Err(err) = emit(result) {
                cancel.store(true, Ordering::Relaxed);
                return Err(err);
            }
        }
        return Ok(completed);
    }

    let next = AtomicUsize::new(0);
    let pending_ticks = AtomicUsize::new(0);
    let (tx, rx) = mpsc::sync_channel::<OuterEvent<R>>(worker_count.saturating_mul(2));
    std::thread::scope(|scope| {
        // 回调 panic 时先析构接收器，防止 worker 卡在有界 send 而 scope 等待 join。
        let rx = rx;
        for _ in 0..worker_count {
            let tx = tx.clone();
            let next = &next;
            let pending_ticks = &pending_ticks;
            let compute = &compute;
            scope.spawn(move || {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= len {
                        break;
                    }
                    let tick = || {
                        pending_ticks.fetch_add(1, Ordering::Relaxed);
                    };
                    let result = compute(index, &items[index], &tick);
                    if tx.send(OuterEvent { index, result }).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);
        let mut pending: Vec<Option<R>> = (0..len).map(|_| None).collect();
        let mut next_emit = 0;
        let mut completed = 0;
        let mut first_error = None;
        loop {
            let event = rx.recv_timeout(PROGRESS_POLL_INTERVAL);
            // 保持一次 tick 对应一次回调的既有契约，但不让 worker 阻塞于 GUI 进度。
            for _ in 0..pending_ticks.swap(0, Ordering::Relaxed) {
                on_tick();
            }
            match event {
                Ok(OuterEvent { index, result }) => {
                    completed += 1;
                    pending[index] = Some(result);
                    while next_emit < len {
                        let Some(result) = pending[next_emit].take() else {
                            break;
                        };
                        if first_error.is_none()
                            && let Err(err) = emit(result)
                        {
                            first_error = Some(err);
                            cancel.store(true, Ordering::Relaxed);
                        }
                        next_emit += 1;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        match first_error {
            Some(err) => Err(err),
            None => Ok(completed),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn emits_in_item_order_regardless_of_completion() {
        let items: Vec<usize> = (0..50).collect();
        let cancel = AtomicBool::new(false);
        let ticks = AtomicUsize::new(0);
        let mut emitted = Vec::new();

        let completed = run_outer_parallel_ordered(
            &items,
            8,
            &cancel,
            |_, item, tick| {
                tick();
                *item * 2
            },
            || {
                ticks.fetch_add(1, Ordering::Relaxed);
            },
            |result| {
                emitted.push(result);
                Ok(())
            },
        )
        .expect("scheduler should succeed");

        assert_eq!(completed, 50);
        assert_eq!(ticks.load(Ordering::Relaxed), 50);
        assert_eq!(emitted, (0..50).map(|x| x * 2).collect::<Vec<_>>());
    }

    #[test]
    fn single_worker_handles_empty_and_single_item() {
        let cancel = AtomicBool::new(false);
        let empty: Vec<usize> = Vec::new();
        let count = run_outer_parallel_ordered(&empty, 4, &cancel, |_, item: &usize, _| *item, || {}, |_| Ok(())).unwrap();
        assert_eq!(count, 0);

        let one = vec![7usize];
        let mut seen = Vec::new();
        let count = run_outer_parallel_ordered(
            &one,
            4,
            &cancel,
            |_, item, _| *item,
            || {},
            |result| {
                seen.push(result);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(count, 1);
        assert_eq!(seen, vec![7]);
    }

    #[test]
    fn emit_error_short_circuits_and_cancels() {
        let items: Vec<usize> = (0..100).collect();
        let cancel = AtomicBool::new(false);
        let mut emitted = 0usize;

        let result = run_outer_parallel_ordered(
            &items,
            4,
            &cancel,
            |_, item, _| *item,
            || {},
            |value| {
                emitted += 1;
                if value == 0 { Err("boom".to_string()) } else { Ok(()) }
            },
        );

        assert_eq!(result, Err("boom".to_string()));
        assert!(cancel.load(Ordering::Relaxed));
    }
}

#[cfg(test)]
mod parallel_regressions {
    use super::*;
    #[test]
    fn high_accuracy_uses_outer_workers_when_items_fill_budget() {
        if !cfg!(target_family = "wasm") {
            assert_eq!(low_accuracy_outer_workers(10_000, 32, 4), 4);
        }
        assert_eq!(low_accuracy_outer_workers(10_000, 2, 4), 1);
        assert_eq!(low_accuracy_outer_workers(100, 1, 4), 1);
    }
    #[test]
    fn single_worker_runs_on_caller_and_preserves_ticks() {
        let caller = std::thread::current().id();
        let mut ticks = 0;
        run_outer_parallel_ordered(
            &[1, 2],
            1,
            &AtomicBool::new(false),
            |_, item, tick| {
                assert_eq!(std::thread::current().id(), caller);
                tick();
                tick();
                *item
            },
            || ticks += 1,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(ticks, 4);
    }
    #[test]
    fn heavy_ticks_are_not_lost() {
        let mut ticks = 0;
        let completed = run_outer_parallel_ordered(
            &[1; 16],
            4,
            &AtomicBool::new(false),
            |_, _, tick| {
                for _ in 0..5000 {
                    tick();
                }
            },
            || ticks += 1,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(completed, 16);
        assert_eq!(ticks, 80_000);
    }
    #[test]
    fn progress_panic_does_not_deadlock_bounded_results() {
        let result = std::panic::catch_unwind(|| {
            run_outer_parallel_ordered(
                &[1; 128],
                4,
                &AtomicBool::new(false),
                |_, _, tick| {
                    for _ in 0..100 {
                        tick();
                    }
                },
                || panic!("测试进度异常"),
                |_| Ok(()),
            )
            .unwrap();
        });
        assert!(result.is_err());
    }
}

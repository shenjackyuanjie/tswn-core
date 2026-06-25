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

use crate::win_rate::resolve_win_rate_workers;

/// 低精度外层并行的场数上限：单局场数 ≤ 此值时才考虑走外层并行。
pub const LOW_ACCURACY_OUTER_PARALLEL_LIMIT: usize = 1000;

/// worker 事件 channel 容量。给细粒度 tick 留足缓冲，避免 worker 频繁阻塞在 `send` 上。
const OUTER_EVENT_CHANNEL_CAPACITY: usize = 4096;

/// 计算外层并行应使用的 worker 数。
///
/// 返回 `1` 表示“无需外层并行，上层应回退到原有内层并行/串行路径”。
/// 仅当单局场数足够小（`n <= LOW_ACCURACY_OUTER_PARALLEL_LIMIT`）且 item 数大于 1 时才并行；
/// `thread` 的语义与 [`resolve_win_rate_workers`] 一致（`0` = 自动）。
pub fn low_accuracy_outer_workers(n: usize, item_count: usize, thread: u32) -> usize {
    if n > LOW_ACCURACY_OUTER_PARALLEL_LIMIT || item_count <= 1 {
        return 1;
    }
    resolve_win_rate_workers(thread, item_count)
}

/// worker → 主线程的事件。
enum OuterEvent<R> {
    /// 细粒度进度 +1（由 `compute` 主动调用 tick 触发）。
    Tick,
    /// 某个 item 计算完成。
    Done { index: usize, result: R },
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

    let next = AtomicUsize::new(0);
    let worker_count = workers.min(len).max(1);
    let (tx, rx) = mpsc::sync_channel::<OuterEvent<R>>(OUTER_EVENT_CHANNEL_CAPACITY);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let tx = tx.clone();
            let next = &next;
            let compute = &compute;
            let cancel = &*cancel;
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
                        let _ = tx.send(OuterEvent::Tick);
                    };
                    let result = compute(index, &items[index], &tick);
                    if tx.send(OuterEvent::Done { index, result }).is_err() {
                        break;
                    }
                }
            });
        }
        // 主线程不再持有 tx；channel 在所有 worker 结束、丢弃各自 tx 后自然关闭。
        drop(tx);

        let mut pending: Vec<Option<R>> = (0..len).map(|_| None).collect();
        let mut next_emit = 0usize;
        let mut completed = 0usize;
        let mut first_error: Option<String> = None;

        while let Ok(event) = rx.recv() {
            match event {
                OuterEvent::Tick => on_tick(),
                OuterEvent::Done { index, result } => {
                    completed += 1;
                    if index < pending.len() {
                        pending[index] = Some(result);
                    }
                    while next_emit < pending.len() {
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
        let count = run_outer_parallel_ordered(
            &empty,
            4,
            &cancel,
            |_, item: &usize, _| *item,
            || {},
            |_| Ok(()),
        )
        .unwrap();
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
                if value == 0 {
                    Err("boom".to_string())
                } else {
                    Ok(())
                }
            },
        );

        assert_eq!(result, Err("boom".to_string()));
        assert!(cancel.load(Ordering::Relaxed));
    }
}

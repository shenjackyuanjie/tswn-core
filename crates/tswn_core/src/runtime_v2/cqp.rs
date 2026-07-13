//! Runtime v2 的 CQP/CQD 矩阵执行器。
//!
//! CQP 的自然任务是 `player × target` 矩阵。旧调度把一个 player 的全部 target
//! 固定交给同一个 worker，复杂名字形成长尾时会让其他核心提前空闲；高精度档还会在
//! 每个 matchup 内重复创建线程。本模块把 matchup 直接动态派发给一组持久 worker，
//! 同时只构建一次默认 Runtime v2 profile。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use super::{
    CustomRuntimeV2ImportConfig, PreparedRuntimeV2Runner, RuntimeV2BatchError, RuntimeV2BatchSummary,
    default_custom_runtime_v2_import_config, prepared_runtime_v2_win_rate,
};

/// 一个已经拆成队伍的 CQP/CQD matchup。
#[derive(Debug, Clone)]
pub struct RuntimeV2CqpMatchup {
    pub groups: Vec<Vec<String>>,
}

impl RuntimeV2CqpMatchup {
    pub fn new(groups: Vec<Vec<String>>) -> Self { Self { groups } }
}

/// 单个 matchup 的计算结果；`elapsed` 包含准备 roster 与执行全部轮次的耗时。
#[derive(Debug)]
pub struct RuntimeV2CqpMatchupResult {
    pub summary: Result<RuntimeV2BatchSummary, RuntimeV2BatchError>,
    pub elapsed: Duration,
}

/// 整个矩阵的执行结果。
#[derive(Debug)]
pub struct RuntimeV2CqpBatchResult {
    /// 与输入 matchup 顺序一一对应；取消后尚未开始的项保持 `None`。
    pub matchups: Vec<Option<RuntimeV2CqpMatchupResult>>,
    pub completed: usize,
}

struct MatchupEvent {
    index: usize,
    result: RuntimeV2CqpMatchupResult,
}

/// 动态调度一组 CQP/CQD matchup。
///
/// `on_complete` 始终在调用线程执行，因此 GUI/CLI 可以直接在回调里更新进度状态；
/// 返回数组仍保持输入顺序，不能让 worker 完成顺序影响最终输出。
pub fn runtime_v2_cqp_matchups(
    matchups: &[RuntimeV2CqpMatchup],
    n: usize,
    eval_rq: f64,
    thread: u32,
    cancel: &AtomicBool,
    mut on_complete: impl FnMut(),
) -> Result<RuntimeV2CqpBatchResult, RuntimeV2BatchError> {
    let mut ordered = (0..matchups.len()).map(|_| None).collect::<Vec<_>>();
    if matchups.is_empty() || cancel.load(Ordering::Relaxed) {
        return Ok(RuntimeV2CqpBatchResult {
            matchups: ordered,
            completed: 0,
        });
    }

    let config = default_custom_runtime_v2_import_config()?;
    let workers = resolve_cqp_workers(thread, matchups.len(), n);
    if workers <= 1 {
        let mut completed = 0;
        for (index, matchup) in matchups.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            ordered[index] = Some(run_matchup(matchup, n, eval_rq, &config));
            completed += 1;
            on_complete();
        }
        return Ok(RuntimeV2CqpBatchResult {
            matchups: ordered,
            completed,
        });
    }

    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<MatchupEvent>();
    let completed = std::thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let config = &config;
            let next = &next;
            scope.spawn(move || {
                loop {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= matchups.len() {
                        break;
                    }
                    let result = run_matchup(&matchups[index], n, eval_rq, config);
                    if tx.send(MatchupEvent { index, result }).is_err() {
                        break;
                    }
                }
            });
        }
        drop(tx);

        let mut completed = 0;
        while let Ok(event) = rx.recv() {
            ordered[event.index] = Some(event.result);
            completed += 1;
            on_complete();
        }
        completed
    });

    Ok(RuntimeV2CqpBatchResult {
        matchups: ordered,
        completed,
    })
}

/// 解析 CQP/CQD 矩阵 worker 数。
///
/// matchup 内部保持单线程后，每个 worker 会频繁等待内存分配和不规则技能分支；
/// 自动模式因此允许轻度超卖。短任务使用 1.5 倍逻辑核，减少线程启动成本；
/// 中长任务使用 2 倍逻辑核，改善复杂 matchup 造成的尾部空洞。显式线程数原样保留。
pub fn resolve_cqp_workers(thread: u32, total: usize, n: usize) -> usize {
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

fn run_matchup(
    matchup: &RuntimeV2CqpMatchup,
    n: usize,
    eval_rq: f64,
    config: &CustomRuntimeV2ImportConfig<'static>,
) -> RuntimeV2CqpMatchupResult {
    let started = Instant::now();
    let summary = (|| {
        let prepared = PreparedRuntimeV2Runner::from_custom_mixed_roster_with_eval_rq(&matchup.groups, eval_rq, config.clone())?;
        prepared_runtime_v2_win_rate(&prepared, n, 1)
    })();
    RuntimeV2CqpMatchupResult {
        summary,
        elapsed: started.elapsed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cqp_matrix_keeps_input_order_and_win_totals() {
        let eval_rq = crate::player::eval_name::WIN_RATE_EVAL_RQ;
        let matchups = vec![
            RuntimeV2CqpMatchup::new(vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]]),
            RuntimeV2CqpMatchup::new(vec![vec!["alpha@red".to_owned()], vec!["beta@blue".to_owned()]]),
        ];
        let cancel = AtomicBool::new(false);
        let mut progress = 0;
        let batch = runtime_v2_cqp_matchups(&matchups, 24, eval_rq, 4, &cancel, || progress += 1).expect("CQP 矩阵应成功执行");

        assert_eq!(batch.completed, 2);
        assert_eq!(progress, 2);
        for (matchup, actual) in matchups.iter().zip(batch.matchups) {
            let actual = actual.expect("未取消的 matchup 应有结果").summary.expect("matchup 应成功");
            let expected = super::super::runtime_v2_groups_win_rate(&matchup.groups, 24, eval_rq, 1).expect("单 matchup 应成功");
            assert_eq!((actual.wins, actual.total), (expected.wins, expected.total));
        }
    }

    #[test]
    fn cqp_matrix_honors_preexisting_cancellation() {
        let matchups = vec![RuntimeV2CqpMatchup::new(vec![
            vec!["left@red".to_owned()],
            vec!["right@blue".to_owned()],
        ])];
        let cancel = AtomicBool::new(true);
        let batch = runtime_v2_cqp_matchups(&matchups, 24, 6.0, 4, &cancel, || {}).expect("取消不应被视为执行错误");
        assert_eq!(batch.completed, 0);
        assert!(batch.matchups[0].is_none());
    }
}

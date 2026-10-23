//! 主 Runtime 胜率批量计算接口，复用阵容模板而不收集完整回放。
//!
//! 先用 [`crate::Runner::prepare_groups_with_eval_rq`] 构造 [`PreparedRunner`]，再调用
//! [`prepared_win_rate`]；`eval_rq` 在准备时固定，换口径时重新准备阵容。
//! 统计口径为第一输入组获胜比例，返回百分比。
//! `thread=0` 自动选择，`1` 强制单线程；WASM 平台限制为单 worker。
//! 范围接口使用绝对局号生成 seed，可将同一批任务分段而不改变采样身份。
//! `_timed` 版本额外逐场计时；普通统计使用非计时入口可节省计时开销。
//!
//! ```
//! use tswn_core::{Runner, win_rate};
//! let groups = vec![vec!["left@red".into()], vec!["right@blue".into()]];
//! let rq = tswn_core::namerena::eval_name::WIN_RATE_EVAL_RQ;
//! let prepared = Runner::prepare_groups_with_eval_rq(&groups, rq)?;
//! let summary = win_rate::prepared_win_rate(&prepared, 16, rq, 1)?;
//! assert_eq!(summary.total, 16);
//! assert!((0.0..=100.0).contains(&summary.win_rate_percent()));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::PreparedRunner;
use crate::runtime::{
    RuntimeBatchError, RuntimeBatchSummary, prepared_runtime_win_rate, prepared_runtime_win_rate_range,
    prepared_runtime_win_rate_range_timed, prepared_runtime_win_rate_timed, runtime_groups_win_rate,
    runtime_groups_win_rate_timed,
};

#[cfg(target_family = "wasm")]
fn platform_default_win_rate_workers() -> usize { 1 }

#[cfg(not(target_family = "wasm"))]
fn platform_default_win_rate_workers() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().saturating_mul(5).div_ceil(4))
        .unwrap_or(1)
}

#[cfg(target_family = "wasm")]
fn platform_limit_win_rate_workers(_workers: usize) -> usize { 1 }

#[cfg(not(target_family = "wasm"))]
fn platform_limit_win_rate_workers(workers: usize) -> usize { workers.max(1) }

/// 累加批次内各局初始化与战斗阶段的诊断耗时，单位为纳秒。
///
/// 多 worker 的值是各 worker 耗时之和，并非批次墙钟时间；非 `_timed` 入口保持为零。
/// 计时精度由平台决定，WASM 使用毫秒级时钟，不参与胜负判定或 seed 生成。
#[derive(Debug, Clone, Copy, Default)]
pub struct WinRateTiming {
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

impl WinRateTiming {
    /// 将另一批次的两个阶段耗时分别累加到当前统计。
    ///
    /// 适用于合并互不重叠的任务分片；相同分片重复合并会重复计时。
    pub fn merge(&mut self, other: Self) {
        self.init_nanos += other.init_nanos;
        self.fight_nanos += other.fight_nanos;
    }
}

/// 第一输入组的胜场数、已执行局数及可选阶段耗时。
///
/// 统计范围由调用入口确定；范围分片可以累加 `wins`、`total` 与 `timing`，
/// 再统一计算百分比，避免直接平均不同大小分片的胜率。
#[derive(Debug, Clone, Copy, Default)]
pub struct WinRateSummary {
    pub wins: usize,
    pub total: usize,
    pub timing: WinRateTiming,
}

impl WinRateSummary {
    /// 将胜场数换算为 `[0, 100]` 百分比，空统计返回 `0.0`。
    ///
    /// 返回范围以有效统计的 `wins <= total` 为前提；手工构造结构体时由调用方维持该约束。
    pub fn win_rate_percent(self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }
}

impl From<RuntimeBatchSummary> for WinRateSummary {
    fn from(summary: RuntimeBatchSummary) -> Self {
        Self {
            wins: summary.wins,
            total: summary.total,
            timing: WinRateTiming {
                init_nanos: summary.timing.init_nanos,
                fight_nanos: summary.timing.fight_nanos,
            },
        }
    }
}

/// 将线程参数与任务数量解析为平台允许的 worker 上限。
///
/// `0` 按可用并行度自动选择，`1` 强制单 worker，其余值指定上限；
/// 返回值至少为 `1` 且不超过 `total.max(1)`，WASM 始终返回 `1`。
/// 实际执行层仍可针对小批次选择串行，返回值不保证创建同样数量的线程。
pub fn resolve_win_rate_workers(thread: u32, total: usize) -> usize {
    let workers = match thread {
        0 => platform_default_win_rate_workers(),
        1 => 1,
        count => count as usize,
    };
    platform_limit_win_rate_workers(workers).min(total.max(1))
}

/// 在已准备的阵容上执行局号 `0..n`，仅收集胜率而不保存逐回合回放。
///
/// `_eval_rq` 为兼容旧调用保留，实际数值已固定在 `prepared` 中；切换口径须重新准备阵容。
/// `n == 0` 返回空统计，`thread` 按 [`resolve_win_rate_workers`] 解析。
/// 普通入口不逐局计时；导入或准备相关错误通过 [`RuntimeBatchError`] 返回。
pub fn prepared_win_rate(
    prepared: &PreparedRunner,
    n: usize,
    _eval_rq: f64,
    thread: u32,
) -> Result<WinRateSummary, RuntimeBatchError> {
    prepared_runtime_win_rate(prepared, n, thread).map(Into::into)
}

/// 与 [`prepared_win_rate`] 相同，但填充 [`WinRateTiming`]。
///
/// 统计为各局耗时之和，包含计时调用自身开销，不代表批次墙钟时间。
/// 阵容、`_eval_rq` 兼容参数和线程语义与非计时入口一致。
pub fn prepared_win_rate_timed(
    prepared: &PreparedRunner,
    n: usize,
    _eval_rq: f64,
    thread: u32,
) -> Result<WinRateSummary, RuntimeBatchError> {
    prepared_runtime_win_rate_timed(prepared, n, thread).map(Into::into)
}

/// 按 `eval_rq` 准备输入分组，再执行 `n` 局第一组胜率统计。
///
/// 输入解析、profile 或阵容导入错误通过 [`RuntimeBatchError`] 返回；即使 `n == 0` 也会校验输入。
/// 同一阵容重复统计时优先复用 [`PreparedRunner`] 与 [`prepared_win_rate`]，避免重复准备。
pub fn groups_win_rate(groups: &[Vec<String>], n: usize, eval_rq: f64, thread: u32) -> Result<WinRateSummary, RuntimeBatchError> {
    runtime_groups_win_rate(groups, n, eval_rq, thread).map(Into::into)
}

/// 与 [`groups_win_rate`] 相同，但填充 [`WinRateTiming`]。
///
/// 输入仍先准备一次，计时字段累加每局初始化与战斗阶段，不包含完整入口的准备和调度耗时。
/// 无需阶段诊断时使用非计时入口，避免额外读取时钟。
pub fn groups_win_rate_timed(
    groups: &[Vec<String>],
    n: usize,
    eval_rq: f64,
    thread: u32,
) -> Result<WinRateSummary, RuntimeBatchError> {
    runtime_groups_win_rate_timed(groups, n, eval_rq, thread).map(Into::into)
}

/// 单 worker 执行半开局号区间 `start..end`，用于可复现的分片统计。
///
/// seed 使用绝对局号；将同一区间拆分后累加胜场与总局数，不改变采样身份。
/// `start >= end` 返回空统计，不执行战斗；此入口不逐局计时，也不创建额外 worker。
pub fn run_prepared_win_rate_range(
    prepared: &PreparedRunner,
    start: usize,
    end: usize,
) -> Result<WinRateSummary, RuntimeBatchError> {
    prepared_runtime_win_rate_range(prepared, start, end).map(Into::into)
}

/// 与 [`run_prepared_win_rate_range`] 相同，但填充 [`WinRateTiming`]。
///
/// 仍以绝对局号派生 seed，空区间计数与耗时均为零；用于分片诊断而不改变战斗结果。
/// 合并分片时分别累加初始化与战斗耗时，不将它们视为并行执行的墙钟耗时。
pub fn run_prepared_win_rate_range_timed(
    prepared: &PreparedRunner,
    start: usize,
    end: usize,
) -> Result<WinRateSummary, RuntimeBatchError> {
    prepared_runtime_win_rate_range_timed(prepared, start, end).map(Into::into)
}

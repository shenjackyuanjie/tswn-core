//! 主 Runtime 胜率批量计算接口。

use crate::PreparedRunner;
use crate::runtime::{
    RuntimeBatchError, RuntimeBatchSummary, prepared_runtime_win_rate, prepared_runtime_win_rate_range, runtime_groups_win_rate,
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

#[derive(Debug, Clone, Copy, Default)]
pub struct WinRateTiming {
    pub init_nanos: u128,
    pub fight_nanos: u128,
}

impl WinRateTiming {
    pub fn merge(&mut self, other: Self) {
        self.init_nanos += other.init_nanos;
        self.fight_nanos += other.fight_nanos;
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WinRateSummary {
    pub wins: usize,
    pub total: usize,
    pub timing: WinRateTiming,
}

impl WinRateSummary {
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

pub fn resolve_win_rate_workers(thread: u32, total: usize) -> usize {
    let workers = match thread {
        0 => platform_default_win_rate_workers(),
        1 => 1,
        count => count as usize,
    };
    platform_limit_win_rate_workers(workers).min(total.max(1))
}

pub fn prepared_win_rate(
    prepared: &PreparedRunner,
    n: usize,
    _eval_rq: f64,
    thread: u32,
) -> Result<WinRateSummary, RuntimeBatchError> {
    prepared_runtime_win_rate(prepared, n, thread).map(Into::into)
}

pub fn groups_win_rate(groups: &[Vec<String>], n: usize, eval_rq: f64, thread: u32) -> Result<WinRateSummary, RuntimeBatchError> {
    runtime_groups_win_rate(groups, n, eval_rq, thread).map(Into::into)
}

pub fn run_prepared_win_rate_range(
    prepared: &PreparedRunner,
    start: usize,
    end: usize,
) -> Result<WinRateSummary, RuntimeBatchError> {
    prepared_runtime_win_rate_range(prepared, start, end).map(Into::into)
}

//! benchmark 共享的轻量类型。
//!
//! 这里故意只保留“多条 benchmark 路径都会依赖”的最小公共抽象：
//! - 胜率/评分摘要；
//! - 给批量任务进度条复用的耗时格式化。
//!
//! 这样做可以避免把具体业务逻辑硬塞进公共模块，最终又回到“大杂烩 bench.rs”的老路。

use std::time::Duration;

use tswn_core::win_rate::WinRateTiming;

/// 把 `Option<usize>` 线程设置转换成 win-rate / 调度器使用的 `thread` 语义（`0` = 自动）。
pub(super) fn thread_spec(threads: Option<usize>) -> u32 { threads.and_then(|x| u32::try_from(x).ok()).unwrap_or(0) }

/// 将秒数格式化成人类可读的时间字符串。
pub(super) fn format_duration(secs: f64) -> String {
    if secs < 0.0 || secs.is_nan() || secs.is_infinite() {
        return "--".to_string();
    }
    let s = secs.round() as u64;
    if s < 60 {
        format!("{s}s")
    } else if s < 3600 {
        format!("{}m{}s", s / 60, s % 60)
    } else {
        format!("{}h{}m{}s", s / 3600, (s % 3600) / 60, s % 60)
    }
}

/// 单个评分或胜率任务的统一摘要。
#[derive(Debug, Clone, Copy)]
pub(super) struct BenchSummary {
    pub(super) wins: usize,
    pub(super) total: usize,
    pub(super) timing: WinRateTiming,
    pub(super) elapsed: Duration,
}

impl BenchSummary {
    /// 把 wins/total 转换成百分比。
    pub(super) fn win_rate_percent(self) -> f64 { self.wins as f64 * 100.0 / self.total.max(1) as f64 }
}

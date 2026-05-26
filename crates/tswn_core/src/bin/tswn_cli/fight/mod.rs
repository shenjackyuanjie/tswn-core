//! `fight` 子模块门面。
//!
//! 原文件同时包含：
//! - 普通对战与 diff 的命令入口；
//! - `raw` 子命令里对 `!test!` benchmark 的特殊分流；
//! - 大量 trace 格式化、归一化、召唤物命名细节。
//!
//! 这三块修改动机完全不同，因此拆成三个子模块：
//! - `driver.rs`：普通入口与 `Runner` 构建；
//! - `raw_bench.rs`：`raw` 命令的 benchmark 分流；
//! - `trace.rs`：日志格式化、归一化与 raw 聚合输出。
//!
//! 现在门面文件放在 `fight/mod.rs`，默认模块解析就足够，不需要 `#[path = ...]`。

mod driver;
mod raw_bench;
mod trace;

pub use driver::{run, run_diff};
pub use raw_bench::run_raw;

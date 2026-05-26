//! `bench` 子模块门面。
//!
//! 旧版 `bench.rs` 同时塞进了五类完全不同的东西：
//! - 自动 / 普通胜率 benchmark 入口；
//! - score benchmark 与 `namer-pf`；
//! - batch-rate / pair 这类大循环任务；
//! - 文件输出与 JSONL / log 文本格式化；
//! - 共享摘要类型和少量时间格式化工具。
//!
//! 现在改成目录级 `mod.rs` 后，`batch.rs`、`common.rs`、`output.rs`、`score.rs`、`winrate.rs`
//! 都能按默认规则被加载，不需要再写 `#[path = ...]`。

mod batch;
mod common;
mod output;
mod score;
mod winrate;

pub use batch::{run_bench_batch_rate, run_bench_pair};
pub use score::run_namer_pf;
pub use winrate::{run_bench_group_win_rate, run_bench_winrate, run_benchmark};

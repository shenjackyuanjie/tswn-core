//! `bench` 子模块门面。
//!
//! 旧版 `bench.rs` 同时塞进了五类完全不同的东西：
//! - 自动 / 普通胜率 benchmark 入口；
//! - score benchmark 与 `namer-pf`；
//! - batch-rate / pair 这类大循环任务；
//! - 文件输出与 JSONL / log 文本格式化；
//! - 共享摘要类型和少量时间格式化工具。
//!
//! 现在改成目录级 `mod.rs` 后，`batch_rate.rs`、`common.rs`、`output.rs`、
//! `pair.rs`、`progress.rs`、`score.rs`、`skill_board.rs`、`win_rate.rs`
//! 都能按默认规则被加载，不需要再写 `#[path = ...]`。batch-rate 与 pair 曾是
//! 同一个 `batch.rs`，随 openbox 对齐功能膨胀到 1300+ 行后按职责拆分。

/// `bench batch-rate` / `bench cqp` 的批量评估实现。
mod batch_rate;
/// benchmark 共享的摘要与耗时格式化类型。
mod common;
/// benchmark 输出、文件交互与文本格式化 helper。
mod output;
/// `bench pair` 的二人组评估实现。
mod pair;
/// 批量评估共用的终端进度条。
mod progress;
/// 评分 benchmark 与 `namer-pf` 的实现。
mod score;
/// `namer-pf` 技能榜。
mod skill_board;
/// 胜率 benchmark 的入口与调度。
mod win_rate;

pub use batch_rate::run_bench_batch_rate;
pub use output::ScoreOutputOptions;
pub use pair::{PairOptions, run_bench_pair};
pub use score::{NamerPfOutputOptions, run_namer_pf};
pub use win_rate::{run_bench_group_win_rate, run_bench_win_rate, run_benchmark};

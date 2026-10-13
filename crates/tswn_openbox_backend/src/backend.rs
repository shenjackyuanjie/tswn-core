//! 共用业务任务门面：DIY、namer-pf、批量胜率与 pair。
//!
//! 输入通过 [`BatchRateInput`]、[`PairInput`] 等明确结构表达，传统任务入口回传
//! [`ProgressEvent`]，`run_*_observed` 还提供 [`live`] 的结构化增量结果。
//! 所有计算同步执行，GUI 将其放在后台线程，CLI 根据输出模式消费事件。
//! 输出与筛选策略属于业务层，实际角色构建、评分和战斗复用 core。

mod batch;
mod format;
pub mod live;
#[cfg(test)]
mod live_tests;
mod namer_pf;
mod output;
mod pair;
mod parse;
mod score;
mod skill_board;
mod to_diy;
mod types;

pub use batch::{run_batch_rate, run_batch_rate_observed};
pub use namer_pf::{run_namer_pf, run_namer_pf_observed};
pub use pair::{run_pair, run_pair_observed};
pub use to_diy::{run_to_diy, run_to_diy_observed};
pub use types::{
    BatchRateInput, CommonBenchOptions, NamerPfInput, NamerPfMetric, NamerPfMetricOptions, NamerPfSkillBoardOptions, OutputMode,
    PairDetailMode, PairInput, ProgressEvent,
};

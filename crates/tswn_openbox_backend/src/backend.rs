//! 共用业务任务门面：DIY、namer-pf、批量胜率、pair 与 base。
//!
//! 输入通过 [`BatchRateInput`]、[`PairInput`] 等明确结构表达，传统任务入口回传
//! [`ProgressEvent`]，`run_*_observed` 还提供 [`live`] 的结构化增量结果。
//! 所有计算同步执行，GUI 将其放在后台线程，CLI 根据输出模式消费事件。
//! 输出与筛选策略属于业务层，实际角色构建、评分和战斗复用 core。
//!
//! # 回调、取消与错误
//!
//! 输入按值交给批任务；字符串与路径由任务持有，取消令牌通过 Arc 与调用方共享。
//! 任务返回前会同步调用事件/观察回调，观察者可能从计算 worker 并发收到增量结果，
//! 因此只转发自有数据，不直接操作 UI；慢回调也会延迟计算，回调 panic 不在此门面捕获。
//! run_namer_pf/batch_rate/pair 通过 Done(Result) 报终态，DIY 直接返回 Result。
//! 取消是协作式请求，已完成明细保留，已开始的工作可能继续；文件写入不是事务，
//! 出错或取消不会恢复旧文件，输出路径不应指向需要保留的输入或配置。
//!
//! `base` 是单名字的同步渲染入口 [`run_base`]：它不产生进度事件，GUI 把它放进
//! 后台线程只为保持界面响应，文本本身与 `openbox-cli base` 完全一致。

mod base;
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

pub use base::run_base;
pub use batch::{run_batch_rate, run_batch_rate_observed};
pub use namer_pf::{run_namer_pf, run_namer_pf_observed};
pub use pair::{run_pair, run_pair_observed};
pub use to_diy::{run_to_diy, run_to_diy_observed};
pub use types::{
    BatchRateInput, CommonBenchOptions, NamerPfInput, NamerPfMetric, NamerPfMetricOptions, NamerPfSkillBoardOptions, OutputMode,
    PairDetailMode, PairInput, ProgressEvent,
};

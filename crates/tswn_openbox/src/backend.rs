//! 后端业务逻辑模块。
//!
//! 重新导出各子模块的公开任务函数（`run_to_diy`、`run_namer_pf` 等）及类型，
//! 供 `app::actions` 在独立线程中调用后端计算并通过 channel 回传进度事件。

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

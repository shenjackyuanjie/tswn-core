//! 后端业务逻辑模块。
//!
//! 重新导出各子模块的公开任务函数（`run_to_diy`、`run_namer_pf` 等）及类型，
//! 供 `app::actions` 在独立线程中调用后端计算并通过 channel 回传进度事件。

mod format;
mod parse;
mod score;
mod tasks;
mod types;

pub use tasks::{run_batch_rate, run_namer_pf, run_pair, run_to_diy};
pub use types::{BatchRateInput, CommonBenchOptions, OutputMode, PairInput, ProgressEvent};

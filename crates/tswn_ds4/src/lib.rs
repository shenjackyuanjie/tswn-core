//! DS4 单人评分、增量配对和筛选流程的 Rust API。
//!
//! 使用 [`Config::load_from_root`] 读取配置，再调用 [`run`]；CLI 和界面共用同一执行路径。
//! 每个工作目录保存独立的历史缓存，同一目录不应并发执行。

mod abcp;
mod cli;
mod compat;
mod config;
mod error;
mod input;
mod model;
mod openbox;
mod ops;
mod output;
mod pairing;
mod pipeline;
mod sp1;
mod sp1_coeffs;
mod store;
mod three;

pub use config::{Config, PairMode, PairThreshold, SingleMode, SingleThreshold, Sp1Threshold, ThreeThreshold};
pub use error::{Ds4Error, Ds4Result};
pub use openbox::screen_pairs as screen_openbox_pairs;
pub use ops::dedup::DedupStats;
pub use pipeline::run_full as run;
pub use pipeline::{FullRunReport, PairReport, SingleReport, Stage1Report};
pub use pipeline::{RunStage, run_with_progress};

/// DS4 新工作目录使用的 JSON 配置模板。
pub const DEFAULT_CONFIG_JSON: &str = include_str!("../config.example.json");

/// 执行统一 CLI 入口；嵌入其他 Rust 应用时优先使用 [`run`]。
pub fn run_cli(args: impl IntoIterator<Item = String>) -> Ds4Result<()> { cli::execute(args) }

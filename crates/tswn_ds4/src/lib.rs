//! DS4 文件工作流：单人评分、增量配对、外部 ABCP5 与实战筛选。
//!
//! # 快速接入
//!
//! ```no_run
//! use std::path::Path;
//! use tswn_ds4::{Config, Ds4Result};
//!
//! let root = Path::new("ds4-workspace");
//! let config = Config::load_from_root(root)?;
//! let report = tswn_ds4::run(root, &config)?;
//! println!("本轮有效输入：{}", report.stage1.dedup.remaining);
//! # Ok::<(), tswn_ds4::Ds4Error>(())
//! ```
//!
//! [`run`] / [`run_with_progress`] 与 CLI 共用执行路径，返回 [`FullRunReport`] 或
//! [`Ds4Error`]，由宿主统一处理失败。调用同步执行，进度回调报告 [`RunStage`]；
//! 当前流程运行到完成或错误，同一工作目录按任务串行使用。
//!
//! # 哪些计算来自哪里
//!
//! | 阶段 | 实现来源 | 注意事项 |
//! | --- | --- | --- |
//! | 去重、单人/配对模型、排序和归档 | 本 crate | 更新工作目录和历史缓存 |
//! | ABCP5 二人/三人预测 | 外部程序与模型资源 | 本 crate 不分发模型或运行库 |
//! | Openbox CQP 实战筛选 | `tswn_openbox_backend → tswn_core` | 直接调用共用 library |
//!
//! [`Config::load_from_root`] 优先 JSON，仍支持旧 TOML/TXT；
//! [`DEFAULT_CONFIG_JSON`] 可用于创建配置。模型或阈值改变后，使用新的工作目录
//! 重新生成对应缓存。文件布局、ABCP5 资源与局部命令见
//! [DS4 README](https://github.com/shenjackyuanjie/tswn-core/tree/main/crates/tswn_ds4)。

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

/// DS4 新工作目录使用的内嵌 JSON 配置模板，可复制到配置文件或交给 Config::from_json。
///
/// 此常量只提供文本，不创建目录或检查模板所启用阶段的模型、靶子等外部资源。
pub const DEFAULT_CONFIG_JSON: &str = include_str!("../config.example.json");

/// 执行不含程序名的 CLI 参数序列，向 stdout/stderr 输出状态或帮助。
///
/// 空参数会在当前目录执行文件工作流，而非仅显示帮助；用法、配置、I/O 或模型失败
/// 通过 Result 返回，不直接退出宿主进程。嵌入应用需要结构化统计时优先使用 [`run`]。
pub fn run_cli(args: impl IntoIterator<Item = String>) -> Ds4Result<()> { cli::execute(args) }

//! `tswn-cli` 参数入口。
//!
//! 按处理阶段组织参数：`cli` 定义 Clap 命令树，`input` 统一 stdin / 文件 / raw 文本，
//! `parsed` 形成执行阶段的 [`ParsedCommand`]。公开解析入口为 [`parse`]。
//! 文本切分和选项校验在执行前完成，fight、bench、icon 等模块消费归一化后的命令。

mod cli;
mod input;
mod parsed;

pub use cli::parse;
pub use parsed::{BenchThreadMode, NamerPfMetric, NamerPfMetricSpec, NamerPfMode, PairDetailMode, ParsedCommand};

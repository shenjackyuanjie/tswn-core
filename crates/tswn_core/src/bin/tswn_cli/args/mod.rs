//! `tswn-cli` 参数入口。
//!
//! 按处理阶段组织参数：`cli` 定义 Clap 命令树，`input` 统一 stdin / 文件 / raw 文本，
//! `parsed` 形成执行阶段的 [`ParsedCommand`]。公开解析入口为 `parse`（由 `cli` 模块再导出）。
//! 文本切分和选项校验在执行前完成，fight、bench、icon 等模块消费归一化后的命令。

/// clap 命令定义：声明 CLI 参数结构并转换为内部命令表示。
mod cli;
/// 输入来源读取与参数值解析的公共工具。
mod input;
/// 解析后的内部命令表示与分派执行。
mod parsed;

pub use cli::parse;
pub use parsed::{BenchThreadMode, NamerPfMetric, NamerPfMetricSpec, NamerPfMode, PairDetailMode, ParsedCommand};

//! `tswn-cli` 参数入口。
//!
//! 旧版 `args.rs` 同时承担三层职责：
//! - 定义 `clap` 命令树；
//! - 把 `clap` 结果转换成执行阶段使用的 `ParsedCommand`；
//! - 负责 stdin / 文件读取，以及多种文本切分与参数校验规则。
//!
//! 这三层的修改频率完全不同，继续堆在一个文件里时，任何 CLI 改动都会把输入规则、
//! 文本切分 helper、测试用例混在同一页里，阅读和维护成本都会持续上涨。
//!
//! 现在改成标准 `mod.rs` 布局后，Rust 会默认从同目录加载 `cli.rs`、`input.rs`、`parsed.rs`，
//! 因此不再需要 `#[path = ...]` 来手动指定子模块文件位置。

mod cli;
mod input;
mod parsed;

pub use cli::parse;
pub use parsed::{BenchThreadMode, ParsedCommand};
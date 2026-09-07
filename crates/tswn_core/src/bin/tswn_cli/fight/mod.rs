//! 普通战斗和 Runtime 诊断命令入口。
mod driver;
mod runtime;
mod trace;

pub use driver::{run, run_diff};
pub use runtime::run_runtime_normalized;

//! 普通战斗和 Runtime 诊断命令入口。
//!
//! `run` 消费规范战斗流，提供人类可读或 JSONL 输出；`run_diff` 与
//! `run_runtime_normalized` 保留 Runtime 诊断格式。参数读取和 banner 策略在上层完成，
//! 本模块转交已归一化 raw 与回合预算，执行错误由各命令入口报告。
mod driver;
mod runtime;
mod trace;

pub use driver::{run, run_diff};
pub use runtime::run_runtime_normalized;

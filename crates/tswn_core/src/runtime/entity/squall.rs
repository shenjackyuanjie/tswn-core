//! 科研 Boss 的可克隆运行态；计时全部采用本体实际行动次数。
//!
//! 占位骨架：当前为空结构体，字段与机制由后续 worker 填充。
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SquallRuntime {}

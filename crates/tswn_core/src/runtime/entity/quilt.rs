//! 被子衍生物的计数器与临时标记；随实体一起克隆与恢复。
//!
//! 占位骨架：当前为空结构体，字段与机制由后续 worker 填充。
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuiltMinionState {}

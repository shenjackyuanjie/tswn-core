//! FeatureEncoder：把 [`BattleModelState`] 编码为固定形状张量的纯 Rust 实现。
//!
//! 设计规格见 [feature-encoder-spec.md](../../../docs/design/feature-encoder-spec.md)；
//! 本文档只说明模块边界，不重复规格内容。
//!
//! **落位约束**（外部评审结论）：encoder 只在 Rust 实现一次，供离线导出器（`tswn_pwp`）、
//! Python/WASM 绑定与网页推理共享，因此本模块**不依赖 Arrow/Parquet、文件系统或模型参数**，
//! 也必须能在 `wasm32-unknown-unknown` 上编译。数据读取、校准命令与离线写入器留在 `tswn_pwp`。
//!
//! 容量常量与计费规则的唯一权威在 [`capacity`]（`BASELINE_64`）；`tswn_pwp::capacity`
//! 只是再导出，不再保留第二份实现。

pub mod batch;
pub mod capacity;
pub mod error;
pub mod manifest;
pub mod numeric;
pub mod vocab;

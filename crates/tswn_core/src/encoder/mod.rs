//! FeatureEncoder：把 [`BattleModelState`](crate::runtime::model_state::BattleModelState)
//! 编码为固定形状张量的纯 Rust 实现。
//!
//! 输入来自 [`crate::cli_api::battle::BattleModelSession`] 或
//! [`crate::runtime::RuntimeRunner::model_state`]。首先用有训练集校准证据的
//! [`manifest::EncoderManifest`] 构造 [`encode::FeatureEncoder`]，再调用 `encode`
//! 或向可复用 [`batch::EncodedBatch`] 的一个槽位 `encode_into`。
//! 设计规格见仓库 `docs/design/feature-encoder-spec.md`；当前签名以本 rustdoc 为准。
//!
//! encoder 在 Rust 中保持唯一实现，离线导出器 `tswn_pwp` 直接调用，其他语言绑定
//! 与网页推理可在此基础上接入。纯机制编码适配 `wasm32-unknown-unknown`；数据读取、
//! Arrow/Parquet、校准命令和离线写入由 `tswn_pwp` 处理。
//!
//! 容量常量与计费规则的唯一权威在 [`capacity`]（`BASELINE_64`）；`tswn_pwp::capacity`
//! 再导出同一套定义。
//!
//! # 模块职责
//!
//! | 模块 | 内容 |
//! | --- | --- |
//! | [`manifest`] / [`vocab`] | 编码契约、校准证据、摘要与分类词表 |
//! | [`capacity`] | 容量档位与上界计费；超限返回结构化错误 |
//! | [`encode`] | 审计状态、局内 ID 重映射、写张量 |
//! | [`batch`] | 连续批缓冲、mask 和精确旁路 |
//! | [`numeric`] / [`slots`] | 数值变换、扩展槽的已审计语义 |
//! | [`error`] | schema、校准、引用和容量等结构化失败 |
//!
//! 当前提供确定性的编码输入，模型训练、推理和在线接入可沿此继续建设。
//! feature 张量保存机制数据，名字、seed、标签与审计列由各自数据通道保存。

pub mod batch;
pub mod capacity;
pub mod encode;
pub mod error;
pub mod manifest;
pub mod numeric;
pub mod slots;
pub mod vocab;

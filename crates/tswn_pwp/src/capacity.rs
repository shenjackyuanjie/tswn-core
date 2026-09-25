//! 容量权威的再导出。
//!
//! FeatureEncoder 容量 profile 与计费已随 encoder 落位到 `tswn_core::encoder::capacity`
//! （外部评审要求：常量与计费规则只保留唯一权威实现，不在 core / pwp / WASM 各复制一份）。
//! 本模块仅再导出，`stats` 等既有调用方无需改动。

pub use tswn_core::encoder::capacity::{BASELINE_64, CAPACITY_DIMS, CapacityMeasure, EncoderProfile};

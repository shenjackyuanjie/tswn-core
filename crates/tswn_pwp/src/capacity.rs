//! 容量权威的再导出。
//!
//! FeatureEncoder 容量 profile 与计费统一来自 `tswn_core::encoder::capacity`，
//! 统计和实际编码应共享同一上界口径，不能在工具侧复制另一套常量或公式。
//! 本模块只再导出 BASELINE_64、维度表和测量类型，不分配张量，也不裁剪超限样本。

pub use tswn_core::encoder::capacity::{BASELINE_64, CAPACITY_DIMS, CapacityMeasure};

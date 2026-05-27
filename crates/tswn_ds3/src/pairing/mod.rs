//! 字对配对模型集合。
//!
//! FC、RH、WC 三种配对模式共享 `common` 中的候选加载、组合遍历和二次模型打分逻辑；
//! 各模式文件只负责选择左右两侧使用的模型系数与配对公式。

#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::excessive_precision
)]

pub mod coeffs;
pub mod common;
pub mod fc;
pub mod rh;
pub mod wc;

//! 单字评分模型集合。
//!
//! 本模块把 DS3 原版的五套单字模型按模式拆分为 `bc`、`fz`、`wc`、`fs`、`pj`，
//! 并通过 `common` / `engine` 复用特征提取、二次模型求值、阈值过滤和多线程文件评分逻辑。

#![allow(
    dead_code,
    clippy::too_many_arguments,
    clippy::excessive_precision
)]

pub mod bc;
pub mod coeffs;
pub mod common;
pub mod engine;
pub mod fs;
pub mod fz;
pub mod pj;
pub mod wc;

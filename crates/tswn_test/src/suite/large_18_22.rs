//! 大型回放测试分片 18-22。
//!
//! 保存由真实/采样输入生成的长回放 fixture，按编号拆分以降低单文件体积并方便定位失败 case。

use super::*;

/// large_18 用例子模块。
mod large_18;
/// large_19 用例子模块。
mod large_19;
/// large_20 用例子模块。
mod large_20;
/// large_21 用例子模块。
mod large_21;
/// large_22 用例子模块。
mod large_22;

pub use large_18::large_18;
pub use large_19::large_19;
pub use large_20::large_20;
pub use large_21::large_21;
pub use large_22::large_22;

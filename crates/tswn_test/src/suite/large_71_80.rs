//! 大型回放测试分片 71-80。
//!
//! 保存由真实/采样输入生成的长回放 fixture，按编号拆分以降低单文件体积并方便定位失败 case。

use super::*;

mod large_71;
mod large_72;

pub use large_71::large_71;
pub use large_72::large_72;

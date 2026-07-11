//! 大型回放测试分片 01-10。
//!
//! 每个 case 独立成子模块，保持单文件体积可控。

use super::*;

mod large_01;
mod large_02;
mod large_03;
mod large_04;
mod large_05;
mod large_06;
mod large_07;
mod large_08;
mod large_09;
mod large_10;

pub use large_01::large_01;
pub use large_02::large_02;
pub use large_03::large_03;
pub use large_04::large_04;
pub use large_05::large_05;
pub use large_06::large_06;
pub use large_07::large_07;
pub use large_08::large_08;
pub use large_09::large_09;
pub use large_10::large_10;

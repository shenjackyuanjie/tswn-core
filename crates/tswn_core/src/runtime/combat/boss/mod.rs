//! Boss 专属机制族：每只 Boss 一个文件，文件名表达具体 Boss 对象。
//!
//! 与 [`super::skills::boss`]（Boss 技能的三个 match 派发）是不同层次：这里是单只
//! Boss 的专属实现，`skills/boss.rs` 是「技能 → Boss 技能」的派发逻辑。

pub mod conan;
pub mod fish;
#[cfg(test)]
pub mod fish_tests;
pub mod mario;
pub mod quilt;
#[cfg(test)]
pub mod quilt_tests;
pub mod slime;
pub mod smdd;
#[cfg(test)]
pub mod smdd_tests;
pub mod squall;
#[cfg(test)]
pub mod squall_tests;
pub mod thanos;
pub mod tiger;
#[cfg(test)]
pub mod tiger_tests;
pub mod wheel;
pub mod xjy;
#[cfg(test)]
pub mod xjy_tests;
pub mod yi;
#[cfg(test)]
pub mod yi_tests;
pub mod yuri;

//! Boss 专属机制族：每只 Boss 一个文件，文件名表达具体 Boss 对象。
//!
//! 与 [`super::skills::boss`]（Boss 技能的三个 match 派发）是不同层次：这里是单只
//! Boss 的专属实现，`skills/boss.rs` 是「技能 → Boss 技能」的派发逻辑。

// 第二、三批才接入命中驱散、烧伤、五分之一吸血，及共享行动计时、
// 驱散/护盾/减伤、冻结延长、有效速度和威力覆盖；第一批没有这些调用方。
pub mod cirno;
#[cfg(test)]
pub mod cirno_tests;
pub mod conan;
pub mod fish;
#[cfg(test)]
pub mod fish_tests;
pub mod mario;
pub mod quilt;
#[cfg(test)]
pub mod quilt_tests;
pub mod rosmontic;
#[cfg(test)]
pub mod rosmontic_tests;
pub mod rumia;
pub mod slime;
pub mod smdd;
#[cfg(test)]
pub mod smdd_tests;
pub mod squall;
#[cfg(test)]
pub mod squall_tests;
pub mod testsubject;
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

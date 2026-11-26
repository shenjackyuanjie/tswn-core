//! Boss 专属机制族：每只 Boss 一个文件，文件名表达具体 Boss 对象。
//!
//! 与 [`super::skills::boss`]（Boss 技能的三个 match 派发）是不同层次：这里是单只
//! Boss 的专属实现，`skills/boss.rs` 是「技能 → Boss 技能」的派发逻辑。

pub mod alice_ran;
pub mod cirno;
#[cfg(test)]
pub mod cirno_tests;
pub mod conan;
pub mod fish;
#[cfg(test)]
pub mod fish_tests;
pub mod flandre_letty_chen;
pub mod mario;
pub mod pcb_hooks;
pub mod pcb_summons;
#[cfg(test)]
pub mod pcb_tests;
pub mod phantom_youmu_yuyuko;
pub mod quilt;
#[cfg(test)]
pub mod quilt_tests;
pub mod reimu_marisa;
pub mod rosmontic;
#[cfg(test)]
pub mod rosmontic_tests;
pub mod rumia;
pub mod scarlet_mansion;
pub mod scarlet_support;
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
pub mod touhou;
#[cfg(test)]
pub mod touhou_tests;
pub mod wheel;
pub mod xjy;
#[cfg(test)]
pub mod xjy_tests;
pub mod yi;
#[cfg(test)]
pub mod yi_tests;
pub mod yuri;

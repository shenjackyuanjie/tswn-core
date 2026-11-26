//! 技能族：按技能类别切分，文件名表达具体技能类别。
//!
//! 这里的 `boss.rs` 是 Boss 技能的三个 match 派发（select / prepare / probability）
//! 与 `drain_plain_boss_skill_into`，与 [`super::boss`]（每只 Boss 的专属实现）是不同层次。

pub mod assassinate;
mod boss;
mod control;
mod debuff;
mod lifecycle;
mod offense;
mod summon;
mod team;
mod weapon;
mod zombie;

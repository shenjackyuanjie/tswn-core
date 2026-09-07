//! 规范的面向用户战斗数据与执行 API。
mod dto;
mod replay;
mod session;

pub use dto::*;
pub use replay::battle_replay;

pub use session::BattleSession;

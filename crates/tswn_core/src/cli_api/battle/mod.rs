//! 规范的面向用户战斗数据与执行 API。
mod driver;
mod dto;
mod model_session;
mod replay;
mod session;

pub use dto::*;
pub use replay::battle_replay;

pub use session::BattleSession;

pub use model_session::{BattleModelFrame, BattleModelOutcome, BattleModelSession};

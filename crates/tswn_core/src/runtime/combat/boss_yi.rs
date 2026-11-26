//! 咦与拜谢的行动结算入口。
//!
//! 占位骨架：drain 入口为空实现，真实逻辑由后续 worker 填充。
use super::*;

/// 咦 Boss 的身份名。
pub const ID: &str = "咦";

impl CombatRuntime {
    /// 咦专属行动结算占位；空实现，避免未完成时主线 panic。
    pub fn drain_yi_action_into(&mut self, _actor: EntityIdx, _updates: &mut RunUpdates) {}
}

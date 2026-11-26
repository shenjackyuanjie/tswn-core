//! 被子的专属行动结算入口与本体判定。
//!
//! 占位骨架：drain 入口为空实现，真实逻辑由后续 worker 填充。
use super::*;

/// 被子 Boss 的身份名。
pub const ID: &str = "quilt";

impl CombatRuntime {
    /// 判断实体是否为被子本体；占位仅识别 Boss 身份名，衍生物路由由 worker 补充。
    pub fn is_quilt(&self, idx: EntityIdx) -> bool {
        self.entities
            .get(idx)
            .is_some_and(|e| e.template.name == ID && e.runtime.flags.contains(PlayerKindFlags::BOSS))
    }

    /// 被子专属行动结算占位；空实现，避免未完成时主线 panic。
    pub fn drain_quilt_action_into(&mut self, _actor: EntityIdx, _updates: &mut RunUpdates) {}
}

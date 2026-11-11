//! 能力卡：按根角色的队伍查询卡牌减耗与倍率，并为技能与被动建立独立作用域。
//!
//! 整个模块只随 cards feature 编译；关闭时 CombatRuntime 上不存在这些方法，
//! 各调用点也一并被 cfg 去掉，热路径与普通引擎对局完全一致。
use super::*;
impl CombatRuntime {
    // 卡牌玩家及其派生单位即使经吞噬提高原始幻术，也只按 5 计算；普通引擎对局不变。
    pub(crate) fn card_skill_level(&self, actor: EntityIdx, skill: BuiltinActiveSkill, level: u32) -> u32 {
        let root = self.entities.get(actor).and_then(|entity| self.entities.get(entity.runtime.root_owner));
        if skill == BuiltinActiveSkill::Shadow
            && root.is_some_and(|entity| matches!(crate::runtime::forced_action::controlled_team(), Some(team) if team == entity.template.team || team == u32::MAX as usize))
        {
            level.min(5)
        } else {
            level
        }
    }
    // 根角色的原始队伍决定能力归属，敌方与被魅惑角色不会借用玩家能力。
    pub(crate) fn ability_value(&self, actor: EntityIdx, skill: &str) -> (u32, f64) {
        let Some(entity) = self.entities.get(actor) else {
            return (0, 1.0);
        };
        let root = self.entities.get(entity.runtime.root_owner).unwrap_or(entity);
        crate::runtime::forced_action::ability_value(root.template.team, skill)
    }
    pub(crate) fn ability_scope(&self, actor: EntityIdx, skill: &str) -> crate::runtime::forced_action::AbilityScope {
        let (cost, power) = self.ability_value(actor, skill);
        let root = self.entities.get(actor).map_or(actor, |e| e.runtime.root_owner);
        crate::runtime::forced_action::scope_ability(root.0, power, cost)
    }
}

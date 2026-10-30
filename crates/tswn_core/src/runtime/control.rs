//! 外部行动脚本：按实体排队“这一次行动用哪个技能”，并在受控单位缺指令时暂停。
//!
//! 卡牌等玩法需要由前端决定每个单位这一次的行动，而不是按熟练度自动挑选。这里提供纯数据
//! 的队列与暂停标记：状态挂在 [`CombatRuntime`] 上，每场战斗独立，同一进程里并行的多场
//! 对局互不影响；脚本为空且未设受控队伍时，执行链与自动模式完全一致。
//!
//! core 只认 [`BuiltinActiveSkill`]；前端 overlay 的编号（例如 `0..34`）由绑定层翻译，
//! 不进入机制层。暂停不是终局：会话层读到 [`BattleControl::take_awaiting`] 后截断回放
//! 并返回 `paused_actor`，调用方补上指令后重新模拟。

use super::{BuiltinActiveSkill, CombatRuntime, PlrId};
use std::fmt;

/// 一次行动要用的技能。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedSkill {
    /// 使用已装备的内置主动技能；未装备或等级为 0 时整条指令失败。
    Builtin(BuiltinActiveSkill),
    /// 明确要求普通攻击。
    BasicAttack,
}

/// 一条外部行动指令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedAction {
    /// 行动者实体 ID，与 `BattlePlayerState.id` 相同。
    pub actor: PlrId,
    /// 这一次行动使用的技能。
    pub skill: PlannedSkill,
    /// 抽牌时已掷好的 MP 消耗；`None` 表示按 tswn 规则现场掷。
    ///
    /// 给出具体值时不再消耗那一次随机数，因此脚本回放与自动回放的随机流不同，
    /// 但同一份脚本的重复模拟仍然确定。
    pub magic_cost: Option<u8>,
}

/// 脚本指令无法执行的原因；会话层把它映射成 `INVALID_ARGUMENT`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedActionError {
    /// 该单位没有装备这个技能，或技能等级为 0。
    SkillNotEquipped { actor: PlrId, skill: BuiltinActiveSkill },
    /// 当前 MP 不足以支付这一手。
    InsufficientMagicPoint { actor: PlrId, required: i32, available: i32 },
    /// 按该技能的目标口径选不出任何目标。
    NoValidTarget { actor: PlrId, skill: PlannedSkill },
}

impl fmt::Display for PlannedActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SkillNotEquipped { actor, skill } => {
                write!(f, "actor {actor} has not equipped {}", skill.export_name())
            }
            Self::InsufficientMagicPoint {
                actor,
                required,
                available,
            } => write!(f, "actor {actor} needs {required} MP but only {available} is available"),
            Self::NoValidTarget { actor, skill } => {
                let name = match skill {
                    PlannedSkill::Builtin(skill) => skill.export_name(),
                    PlannedSkill::BasicAttack => "basic attack",
                };
                write!(f, "actor {actor} has no valid target for {name}")
            }
        }
    }
}

impl std::error::Error for PlannedActionError {}

/// 一场战斗的外部控制状态：受控队伍、指令队列、暂停标记与首条执行错误。
#[derive(Debug, Clone, Default)]
pub struct BattleControl {
    controlled_team: Option<usize>,
    queue: Vec<PlannedAction>,
    awaiting: Option<PlrId>,
    error: Option<PlannedActionError>,
}

impl BattleControl {
    /// 设定“由玩家操控”的队伍编号；`None` 表示完全回到自动执行。
    ///
    /// 只有该队伍的本体（默认 kind）与分身会暂停；幻影、使魔、丧尸等召唤物仍自动行动。
    pub fn set_controlled_team(&mut self, team: Option<usize>) {
        self.controlled_team = team;
        self.awaiting = None;
    }

    pub fn controlled_team(&self) -> Option<usize> { self.controlled_team }

    /// 追加一条指令；同一实体的多条指令按入队顺序消费。
    pub fn queue(&mut self, action: PlannedAction) { self.queue.push(action); }

    pub fn queue_all(&mut self, actions: impl IntoIterator<Item = PlannedAction>) { self.queue.extend(actions); }

    /// 尚未消费的指令条数。
    pub fn pending(&self) -> usize { self.queue.len() }

    /// 取走属于 `actor` 的第一条指令。
    pub fn take_for(&mut self, actor: PlrId) -> Option<PlannedAction> {
        let index = self.queue.iter().position(|action| action.actor == actor)?;
        Some(self.queue.remove(index))
    }

    pub fn clear_queue(&mut self) { self.queue.clear(); }

    /// 复位到“无脚本、无受控队伍、无暂停”的初始状态；复用 runner 开始下一场时调用。
    pub fn reset(&mut self) { *self = Self::default(); }

    /// 记录“这个单位在等玩家指令”。
    pub fn set_awaiting(&mut self, actor: PlrId) { self.awaiting = Some(actor); }

    pub fn awaiting(&self) -> Option<PlrId> { self.awaiting }

    /// 取走暂停标记；会话层每个边界只读一次，避免残留到下一场战斗。
    pub fn take_awaiting(&mut self) -> Option<PlrId> { self.awaiting.take() }

    /// 记录一条指令的执行失败；会话层据此返回 `INVALID_ARGUMENT`。
    pub fn set_error(&mut self, error: PlannedActionError) { self.error = Some(error); }

    pub fn take_error(&mut self) -> Option<PlannedActionError> { self.error.take() }
}

impl CombatRuntime {
    /// 设定受控队伍；只有该队伍的本体与分身会暂停等指令。
    pub fn set_controlled_team(&mut self, team: Option<usize>) { self.control.set_controlled_team(team); }

    /// 排队一条外部指令，可与 [`Self::set_controlled_team`] 配合逐次驱动对局。
    pub fn queue_planned_action(&mut self, action: PlannedAction) { self.control.queue(action); }

    pub fn queue_planned_actions(&mut self, actions: impl IntoIterator<Item = PlannedAction>) { self.control.queue_all(actions); }

    /// 当前在等玩家指令的单位；非暂停边界为 `None`。
    pub fn awaiting_actor(&self) -> Option<PlrId> { self.control.awaiting() }

    /// 取走“等待指令”的标记。
    pub fn take_awaiting_actor(&mut self) -> Option<PlrId> { self.control.take_awaiting() }

    /// 取走脚本指令的执行错误。
    pub fn take_planned_action_error(&mut self) -> Option<PlannedActionError> { self.control.take_error() }

    /// 尚未消费的指令条数。
    pub fn planned_actions_pending(&self) -> usize { self.control.pending() }

    /// 清空尚未消费的指令，保留受控队伍设置。
    pub fn clear_planned_actions(&mut self) { self.control.clear_queue(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_consumes_per_actor_in_order_and_reset_clears_everything() {
        let mut control = BattleControl::default();
        control.set_controlled_team(Some(0));
        control.queue(PlannedAction {
            actor: 1,
            skill: PlannedSkill::BasicAttack,
            magic_cost: None,
        });
        control.queue(PlannedAction {
            actor: 0,
            skill: PlannedSkill::BasicAttack,
            magic_cost: Some(12),
        });
        control.queue(PlannedAction {
            actor: 0,
            skill: PlannedSkill::BasicAttack,
            magic_cost: None,
        });
        assert_eq!(control.pending(), 3);

        assert_eq!(control.take_for(0).map(|action| action.magic_cost), Some(Some(12)));
        assert_eq!(control.take_for(0).map(|action| action.magic_cost), Some(None));
        assert_eq!(control.take_for(0), None);
        // 属于其他实体的指令不受影响。
        assert_eq!(control.take_for(1).map(|action| action.actor), Some(1));

        control.set_awaiting(0);
        control.set_error(PlannedActionError::InsufficientMagicPoint {
            actor: 0,
            required: 8,
            available: 0,
        });
        control.reset();
        assert_eq!(control.controlled_team(), None);
        assert_eq!(control.pending(), 0);
        assert_eq!(control.awaiting(), None);
        assert_eq!(control.take_error(), None);
    }

    #[test]
    fn planned_action_error_describes_the_failure() {
        let error = PlannedActionError::SkillNotEquipped {
            actor: 2,
            skill: BuiltinActiveSkill::Fire,
        };
        assert_eq!(error.to_string(), "actor 2 has not equipped core.skill.fire");
    }
}

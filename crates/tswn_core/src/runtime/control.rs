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

/// 即时对局自动技能的概率倍率。1x 保留原单字节随机流，其他倍率用 24 位精度判定。
pub(crate) fn roll_scaled_skill(rng: &mut crate::rc4::RC4, level: u32, range: u32, multiplier: f64) -> bool {
    if multiplier == 1.0 {
        return (u32::from(rng.next_u8()) & (range - 1)) < level;
    }
    let chance = ((level.min(range) as f64 / range as f64) * multiplier).clamp(0.0, 1.0);
    (rng.rFFFFFF() as f64 / 16_777_216.0) < chance
}

/// 一次行动要用的技能。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedSkill {
    /// 使用已装备的内置主动技能；未装备或等级为 0 时整条指令失败。
    Builtin(BuiltinActiveSkill),
    /// 明确要求普通物理攻击。
    BasicAttack,
    /// 明确要求普通魔法攻击。
    MagicAttack,
}

/// 缺少外部指令的暂停边界上，引擎仍将接管的行动类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForcedAction {
    Berserk,
}

impl ForcedAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Berserk => "berserk",
        }
    }
}

/// 被动 hook 暂停时使用的 panic 标记；避免把内部借用状态暴露给调用方。
#[derive(Debug, Clone, Copy)]
pub struct PassivePause;

/// 一次伤害解析的显式决策边界。询问与结算顺序由该类型集中声明，
/// 具体 hook 可以跨多次回放消费同一个 batch 的决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageDecisionPhase {
    Evade,
    Reflect,
    Protect,
    Defend,
}

impl DamageDecisionPhase {
    pub const ASK_ORDER: [Self; 4] = [Self::Evade, Self::Reflect, Self::Protect, Self::Defend];
    pub const RESOLVE_ORDER: [Self; 4] = [Self::Reflect, Self::Protect, Self::Evade, Self::Defend];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageDecisionBatch {
    pub id: u64,
    pub attacker: PlrId,
    pub target: PlrId,
    pub requested: Vec<DamageDecisionPhase>,
    /// 回避在成为目标时提前预演的结果；只属于当前批次，不能跨伤害复用。
    pub evade: Option<EvadeDecision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvadeDecision {
    pub activate: bool,
    pub success: bool,
    pub unavoidable: bool,
}

/// 同一次雷击的暂停上下文；嵌套防御/反击暂停也保留归属，但不属于雷击回避本身。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThunderContext {
    pub id: u64,
    pub attacker: PlrId,
    pub target: PlrId,
    pub hit: u32,
    pub is_evade: bool,
    /// 上一段的原始回避阈值，首段为 None；用于只累计概率差值。
    pub previous_level: Option<u32>,
}

impl DamageDecisionBatch {
    pub fn new(id: u64, attacker: PlrId, target: PlrId) -> Self {
        Self {
            id,
            attacker,
            target,
            requested: Vec::new(),
            evade: None,
        }
    }

    pub fn record(&mut self, phase: DamageDecisionPhase) {
        if !self.requested.contains(&phase) {
            self.requested.push(phase);
        }
    }

    pub fn set_evade(&mut self, decision: EvadeDecision) { self.evade = Some(decision); }

    pub fn take_evade(&mut self) -> Option<EvadeDecision> { self.evade.take() }
}

/// 一条被动触发决策：服务器已经完成成功率判定，只把结果交给回放。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassiveDecision {
    /// 被动所属实体。
    pub actor: PlrId,
    /// 被动技能导出名，例如 `core.skill.reraise`。
    pub skill: String,
    /// 玩家是否选择发动。
    pub activate: bool,
    /// 服务器按累计成功率判定的结果；`activate=false` 时忽略。
    pub success: bool,
    /// 守护目标；其他被动为 `None`。
    pub target: Option<PlrId>,
}

/// 引擎在被动 hook 处缺少决策时返回的待确认信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassiveRequest {
    /// 当前响应的实际发起者；反击和反弹使用嵌套伤害的攻击者。
    pub initiator: Option<PlrId>,
    /// 当前可选技能实际需要支付的 MP；无消耗的技能不在表内。
    pub mp_costs: std::collections::BTreeMap<String, u32>,
    pub actor: PlrId,
    /// 该触发指向的角色：回避是回避者，守护拦截是被守护者。
    pub target: Option<PlrId>,
    pub skill: String,
    pub level: u32,
    /// 守护候选：存活、非召唤物、不含自己。
    pub candidates: Vec<PlrId>,
    /// 回避请求是否属于强命技能（即使回避成功也无效）。
    pub unavoidable: bool,
    /// 守护请求是否为“即将受伤时是否进行守护”，而不是选择守护目标。
    pub intercept: bool,
    /// 所属伤害批次；非伤害被动为 None。
    pub batch_id: Option<u64>,
    /// 该请求在伤害批次中的阶段。
    pub phase: Option<DamageDecisionPhase>,
}

/// 一条外部行动指令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannedAction {
    /// 本次首段索敌的机制队伍；None 沿用全场自动索敌。
    pub target_team: Option<usize>,
    /// 行动者实体 ID，与 `BattlePlayerState.id` 相同。
    pub actor: PlrId,
    /// 这一次行动使用的技能。
    pub skill: PlannedSkill,
    /// 抽牌时已掷好的 MP 消耗；`None` 表示按 tswn 规则现场掷。
    ///
    /// 给出具体值时不再消耗那一次随机数，因此脚本回放与自动回放的随机流不同，
    /// 但同一份脚本的重复模拟仍然确定。
    pub magic_cost: Option<u8>,
    /// 这一手附带的药水编号：`0` 表示不喝药，`1..=8` 是属性下标加一，
    /// `9` 是能量药水、`10` 是体力药水。药水不占行动，先于技能结算。
    pub potion: u32,
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
                    PlannedSkill::MagicAttack => "magic attack",
                };
                write!(f, "actor {actor} has no valid target for {name}")
            }
        }
    }
}

impl std::error::Error for PlannedActionError {}

/// 卡牌前端使用的公开技能编号（overlay 里的 `0..24`）→ 引擎内置主动技能。
///
/// `25..34` 是防御/触发类技能，不参与「主动出招」，返回 `None`。
pub fn public_builtin_skill(id: u32) -> Option<BuiltinActiveSkill> {
    Some(match id {
        0 => BuiltinActiveSkill::Fire,
        1 => BuiltinActiveSkill::Ice,
        2 => BuiltinActiveSkill::Thunder,
        3 => BuiltinActiveSkill::Quake,
        4 => BuiltinActiveSkill::Absorb,
        5 => BuiltinActiveSkill::Poison,
        6 => BuiltinActiveSkill::Rapid,
        7 => BuiltinActiveSkill::Critical,
        8 => BuiltinActiveSkill::Half,
        9 => BuiltinActiveSkill::Exchange,
        10 => BuiltinActiveSkill::Berserk,
        11 => BuiltinActiveSkill::Charm,
        12 => BuiltinActiveSkill::Haste,
        13 => BuiltinActiveSkill::Slow,
        14 => BuiltinActiveSkill::Curse,
        15 => BuiltinActiveSkill::Heal,
        16 => BuiltinActiveSkill::Revive,
        17 => BuiltinActiveSkill::Disperse,
        18 => BuiltinActiveSkill::Iron,
        19 => BuiltinActiveSkill::Charge,
        20 => BuiltinActiveSkill::Accumulate,
        21 => BuiltinActiveSkill::Assassinate,
        22 => BuiltinActiveSkill::Summon,
        23 => BuiltinActiveSkill::Clone,
        24 => BuiltinActiveSkill::Shadow,
        _ => return None,
    })
}

/// 公开编号里的「普通物理攻击」；其余编号按 [`public_builtin_skill`] 翻译。
pub const PUBLIC_BASIC_ATTACK: u32 = 255;

/// 解码前端线格式 `[actor, skill, packed]` 三元组。
///
/// `skill` 是公开主动技能编号或 [`PUBLIC_BASIC_ATTACK`]；`packed` 低字节是这一手的
/// MP 消耗（`0` 表示现场掷），高字节是药水编号。长度不完整、编号越界或药水码非法
/// 都在推进之前整体拒绝，不静默丢弃尾部数据。
pub fn decode_actions(actions: &[u32]) -> Result<Vec<PlannedAction>, &'static str> {
    if !actions.len().is_multiple_of(3) {
        return Err("actions must contain complete actor/skill/mp triples");
    }
    actions
        .chunks_exact(3)
        .map(|triple| {
            let skill = if triple[1] == PUBLIC_BASIC_ATTACK {
                PlannedSkill::BasicAttack
            } else {
                PlannedSkill::Builtin(
                    public_builtin_skill(triple[1]).ok_or("forced skill must be active id 0..24 or normal attack 255")?,
                )
            };
            let potion = triple[2] >> 8;
            if triple[2] > 0xffff || !matches!(potion, 0..=7 | 9 | 10) {
                return Err("invalid packed potion code");
            }
            let cost = triple[2] & 0xff;
            Ok(PlannedAction {
                target_team: None,
                actor: triple[0] as PlrId,
                skill,
                magic_cost: u8::try_from(cost).ok().filter(|cost| *cost > 0),
                potion,
            })
        })
        .collect()
}

/// 一场战斗的外部控制状态：受控队伍、指令队列、暂停标记与首条执行错误。
#[derive(Debug, Clone, Default)]
pub struct BattleControl {
    controlled_teams: Vec<usize>,
    queue: Vec<PlannedAction>,
    awaiting: Option<(PlrId, Option<ForcedAction>)>,
    error: Option<PlannedActionError>,
    passive_decisions: Vec<PassiveDecision>,
    passive_cursor: usize,
    /// 回避脚本索引的执行回执；无键表示仍等待该伤害批次结算。
    evade_results: std::collections::BTreeMap<usize, bool>,
    evade_indices: std::collections::BTreeMap<u64, usize>,
    pending_passive: Option<PassiveRequest>,
    pending_updates: Option<crate::runtime::update::RunUpdates>,
    passive_decision_mode: bool,
    controlled_minions: bool,
    /// 药水加成：实体 →（属性下标, 增量）。持续到服用者的下一次行动前还原。
    active_potions: std::collections::BTreeMap<PlrId, (u8, i32)>,
    damage_batches: Vec<DamageDecisionBatch>,
    next_damage_batch_id: u64,
    thunder: Option<ThunderContext>,
    next_thunder_id: u64,
    skill_multiplier: Option<f64>,
}

impl BattleControl {
    /// 设定“由玩家操控”的队伍编号；`None` 表示完全回到自动执行。
    ///
    /// 只有受控队伍的本体（默认 kind）与分身会暂停；幻影、使魔、丧尸等召唤物仍自动行动。
    pub fn set_controlled_team(&mut self, team: Option<usize>) {
        self.controlled_teams.clear();
        if let Some(team) = team {
            self.controlled_teams.push(team);
        }
        self.awaiting = None;
    }

    /// 设定多支由玩家操控的队伍；空列表表示完全回到自动执行。
    pub fn set_controlled_teams(&mut self, teams: impl IntoIterator<Item = usize>) {
        self.controlled_teams.clear();
        for team in teams {
            if !self.controlled_teams.contains(&team) {
                self.controlled_teams.push(team);
            }
        }
        self.awaiting = None;
    }

    pub fn controlled_team(&self) -> Option<usize> { self.controlled_teams.first().copied() }

    pub fn controlled_teams(&self) -> &[usize] { &self.controlled_teams }

    /// 表示「双方本体与分身都由玩家控制」的特殊队伍编号，沿用 `u32::MAX` 线格式。
    pub const CONTROLLED_ALL_TEAMS: usize = u32::MAX as usize;

    /// 受控判定；`usize::MAX` 表示双方本体与分身都由玩家控制。
    pub fn is_controlled_team(&self, team: usize) -> bool {
        self.controlled_teams
            .iter()
            .any(|controlled| *controlled == Self::CONTROLLED_ALL_TEAMS || *controlled == team)
    }
    pub fn queue(&mut self, action: PlannedAction) { self.queue.push(action); }

    pub fn queue_all(&mut self, actions: impl IntoIterator<Item = PlannedAction>) { self.queue.extend(actions); }

    /// 尚未消费的指令条数。
    pub fn pending(&self) -> usize { self.queue.len() }

    /// 取走属于 `actor` 的第一条指令。
    pub fn take_for(&mut self, actor: PlrId) -> Option<PlannedAction> {
        let index = self.queue.iter().position(|action| action.actor == actor)?;
        // 诊断计数：绑定层据此确认脚本确实被消费，而不是靠回放外观推断。
        crate::runtime::forced_action::note_taken();
        Some(self.queue.remove(index))
    }

    /// 按原始脚本顺序附加席位选择；在首次推进前设置。
    pub(crate) fn set_action_target_teams(&mut self, teams: &[Option<usize>]) {
        for (action, team) in self.queue.iter_mut().zip(teams) {
            action.target_team = *team;
        }
    }

    pub fn clear_queue(&mut self) { self.queue.clear(); }

    /// 复位到“无脚本、无受控队伍、无暂停”的初始状态；复用 runner 开始下一场时调用。
    pub fn reset(&mut self) { *self = Self::default(); }

    /// 记录“这个单位在等玩家指令”。
    pub fn set_awaiting(&mut self, actor: PlrId) { self.set_awaiting_forced_action(actor, None); }

    /// 仅在实际缺指令暂停的位置写入，不能用角色状态标签推测。
    pub(crate) fn set_awaiting_forced_action(&mut self, actor: PlrId, forced_action: Option<ForcedAction>) {
        self.awaiting = Some((actor, forced_action));
    }

    pub fn awaiting(&self) -> Option<PlrId> { self.awaiting.map(|(actor, _)| actor) }

    pub fn awaiting_forced_action(&self) -> Option<ForcedAction> { self.awaiting.and_then(|(_, forced)| forced) }

    /// 取走暂停标记及附属元数据；会话层每个边界只读一次，避免残留到下一场战斗。
    pub fn take_awaiting(&mut self) -> Option<PlrId> { self.awaiting.take().map(|(actor, _)| actor) }

    /// 设置本次回放可消费的被动决策脚本，按触发顺序消费。
    pub fn set_passive_decisions(&mut self, decisions: impl IntoIterator<Item = PassiveDecision>) {
        self.passive_decisions = decisions.into_iter().collect();
        self.passive_cursor = 0;
        self.evade_results.clear();
        self.evade_indices.clear();
    }

    pub fn set_passive_decision_mode(&mut self, enabled: bool) { self.passive_decision_mode = enabled; }

    pub fn passive_decision_mode(&self) -> bool { self.passive_decision_mode }

    pub fn set_skill_multiplier(&mut self, multiplier: f64) { self.skill_multiplier = Some(multiplier); }

    pub fn skill_multiplier(&self) -> f64 {
        if self.passive_decision_mode {
            self.skill_multiplier.unwrap_or(1.0)
        } else {
            1.0
        }
    }

    pub fn set_controlled_minions(&mut self, enabled: bool) { self.controlled_minions = enabled; }

    pub fn controlled_minions(&self) -> bool { self.controlled_minions }

    pub fn next_passive_decision(&mut self) -> Option<PassiveDecision> {
        let decision = self.passive_decisions.get(self.passive_cursor).cloned()?;
        self.passive_cursor += 1;
        Some(decision)
    }

    /// 仅消费匹配的旧决策；自动护身符不能拿走后续其他被动的脚本。
    pub(crate) fn next_passive_decision_for(&mut self, actor: PlrId, skill: &str) -> Option<PassiveDecision> {
        let next = self.passive_decisions.get(self.passive_cursor)?;
        if next.actor != actor || next.skill != skill {
            return None;
        }
        self.next_passive_decision()
    }

    pub fn set_pending_passive(&mut self, request: PassiveRequest) { self.pending_passive = Some(request); }

    pub fn take_pending_passive(&mut self) -> Option<PassiveRequest> { self.pending_passive.take() }

    pub fn pending_passive(&self) -> Option<&PassiveRequest> { self.pending_passive.as_ref() }

    pub fn thunder_context(&self) -> Option<ThunderContext> { self.thunder }

    pub(crate) fn begin_thunder(&mut self, attacker: PlrId, target: PlrId) {
        if !self.passive_decision_mode {
            return;
        }
        self.next_thunder_id += 1;
        self.thunder = Some(ThunderContext {
            id: self.next_thunder_id,
            attacker,
            target,
            hit: 0,
            is_evade: false,
            previous_level: None,
        });
    }

    pub(crate) fn set_thunder_hit(&mut self, hit: u32, is_evade: bool, previous_level: Option<u32>) {
        if let Some(thunder) = self.thunder.as_mut() {
            thunder.hit = hit;
            thunder.is_evade = is_evade;
            thunder.previous_level = previous_level;
        }
    }

    pub(crate) fn finish_thunder(&mut self) { self.thunder = None; }

    /// Preserve updates already emitted before a passive pause unwinds the round.
    pub fn set_pending_updates(&mut self, updates: &crate::runtime::update::RunUpdates) {
        self.pending_updates = Some(updates.clone());
    }

    pub fn take_pending_updates(&mut self) -> Option<crate::runtime::update::RunUpdates> { self.pending_updates.take() }

    pub fn begin_damage_batch(&mut self, attacker: PlrId, target: PlrId) -> u64 {
        self.next_damage_batch_id = self.next_damage_batch_id.saturating_add(1);
        let id = self.next_damage_batch_id;
        self.damage_batches.push(DamageDecisionBatch::new(id, attacker, target));
        id
    }

    pub fn ensure_damage_batch(&mut self, attacker: PlrId, target: PlrId) -> u64 {
        if let Some(batch) = self.damage_batches.last()
            && batch.attacker == attacker
            && batch.target == target
        {
            return batch.id;
        }
        self.begin_damage_batch(attacker, target)
    }

    pub fn damage_batch(&self) -> Option<&DamageDecisionBatch> { self.damage_batches.last() }

    pub fn finish_damage_batch(&mut self, id: u64) -> bool {
        if self.damage_batches.last().is_some_and(|batch| batch.id == id) {
            self.damage_batches.pop();
            if let Some(index) = self.evade_indices.remove(&id) {
                // 反弹/守护已覆盖伤害，未执行缓存回避；网站应返还预留资格。
                self.evade_results.insert(index, false);
            }
            true
        } else {
            false
        }
    }

    pub fn record_damage_phase(&mut self, phase: DamageDecisionPhase) -> Option<u64> {
        let batch = self.damage_batches.last_mut()?;
        batch.record(phase);
        Some(batch.id)
    }

    pub fn set_evade_decision(&mut self, decision: EvadeDecision) -> Option<u64> {
        let batch = self.damage_batches.last_mut()?;
        batch.set_evade(decision);
        let id = batch.id;
        if let Some(index) = self.passive_cursor.checked_sub(1) {
            self.evade_indices.insert(id, index);
        }
        Some(id)
    }

    pub fn take_evade_decision(&mut self) -> Option<EvadeDecision> {
        let batch = self.damage_batches.last_mut()?;
        let decision = batch.take_evade()?;
        if let Some(index) = self.evade_indices.remove(&batch.id) {
            self.evade_results
                .insert(index, !decision.unavoidable && decision.activate && decision.success);
        }
        Some(decision)
    }

    /// 雷击等直接执行回避的路径，不经过预演缓存。
    pub(crate) fn record_direct_evade_result(&mut self, success: bool) {
        if let Some(index) = self.passive_cursor.checked_sub(1) {
            self.evade_results.insert(index, success);
        }
    }

    pub fn evade_result(&self, decision_index: usize) -> Option<bool> { self.evade_results.get(&decision_index).copied() }

    /// 记录一条指令的执行失败；会话层据此返回 `INVALID_ARGUMENT`。
    /// 记录这一回合生效的属性药水，供下一次行动前还原。
    pub fn set_active_potion(&mut self, actor: PlrId, field: u8, delta: i32) {
        self.active_potions.insert(actor, (field, delta));
    }

    /// 取走并清空该实体的药水加成记录。
    pub fn take_active_potion(&mut self, actor: PlrId) -> Option<(u8, i32)> { self.active_potions.remove(&actor) }

    pub fn set_error(&mut self, error: PlannedActionError) { self.error = Some(error); }

    pub fn take_error(&mut self) -> Option<PlannedActionError> { self.error.take() }
}

impl CombatRuntime {
    /// 设定受控队伍；只有该队伍的本体与分身会暂停等指令。
    pub fn set_controlled_team(&mut self, team: Option<usize>) { self.control.set_controlled_team(team); }

    /// 设定多支受控队伍；只有这些队伍的本体与分身会暂停等指令。
    pub fn set_controlled_teams(&mut self, teams: impl IntoIterator<Item = usize>) { self.control.set_controlled_teams(teams); }

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
    fn awaiting_forced_metadata_is_bound_to_the_active_pause() {
        let mut control = BattleControl::default();
        control.set_awaiting_forced_action(3, Some(ForcedAction::Berserk));
        assert_eq!(control.awaiting(), Some(3));
        assert_eq!(control.awaiting_forced_action(), Some(ForcedAction::Berserk));
        assert_eq!(control.take_awaiting(), Some(3));
        assert_eq!(control.awaiting_forced_action(), None);
        for reset in 0..4 {
            control.set_awaiting_forced_action(3, Some(ForcedAction::Berserk));
            match reset {
                0 => control.set_awaiting(4),
                1 => control.set_controlled_team(None),
                2 => control.set_controlled_teams([0, 1]),
                _ => control.reset(),
            }
            assert_eq!(control.awaiting_forced_action(), None);
        }
    }

    #[test]
    fn damage_decision_batch_declares_boundary_and_orders() {
        assert_eq!(
            DamageDecisionPhase::ASK_ORDER,
            [
                DamageDecisionPhase::Evade,
                DamageDecisionPhase::Reflect,
                DamageDecisionPhase::Protect,
                DamageDecisionPhase::Defend,
            ]
        );
        assert_eq!(
            DamageDecisionPhase::RESOLVE_ORDER,
            [
                DamageDecisionPhase::Reflect,
                DamageDecisionPhase::Protect,
                DamageDecisionPhase::Evade,
                DamageDecisionPhase::Defend,
            ]
        );
        let mut control = BattleControl::default();
        let id = control.begin_damage_batch(7, 9);
        assert_eq!(id, 1);
        assert_eq!(control.ensure_damage_batch(7, 9), id);
        assert_eq!(control.record_damage_phase(DamageDecisionPhase::Evade), Some(id));
        assert_eq!(control.damage_batch().unwrap().requested, vec![DamageDecisionPhase::Evade]);
        assert_eq!(control.damage_batch().unwrap().evade, None);
        control.set_evade_decision(EvadeDecision {
            activate: true,
            success: true,
            unavoidable: false,
        });
        assert_eq!(control.damage_batch().unwrap().evade.unwrap().success, true);
        assert_eq!(control.take_evade_decision().unwrap().activate, true);
        assert_eq!(control.begin_damage_batch(7, 9), 2);
        assert_eq!(control.damage_batch().unwrap().id, 2);
        assert!(control.finish_damage_batch(2));
        assert_eq!(control.damage_batch().unwrap().id, 1);
        assert!(control.finish_damage_batch(1));
        assert_eq!(control.damage_batch(), None);
    }

    #[test]
    fn queue_consumes_per_actor_in_order_and_reset_clears_everything() {
        let mut control = BattleControl::default();
        control.set_controlled_team(Some(0));
        control.queue(PlannedAction {
            target_team: None,
            actor: 1,
            skill: PlannedSkill::BasicAttack,
            magic_cost: None,
            potion: 0,
        });
        control.queue(PlannedAction {
            target_team: None,
            actor: 0,
            skill: PlannedSkill::BasicAttack,
            magic_cost: Some(12),
            potion: 0,
        });
        control.queue(PlannedAction {
            target_team: None,
            actor: 0,
            skill: PlannedSkill::BasicAttack,
            magic_cost: None,
            potion: 0,
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
    #[test]
    fn evade_receipts_distinguish_execution_coverage_and_nested_batches() {
        let mut control = BattleControl::default();
        control.set_passive_decisions((0..2).map(|actor| PassiveDecision {
            actor,
            skill: "evade".into(),
            activate: true,
            success: true,
            target: None,
        }));
        let outer = control.begin_damage_batch(1, 0);
        control.next_passive_decision().unwrap();
        control.set_evade_decision(EvadeDecision {
            activate: true,
            success: true,
            unavoidable: false,
        });
        assert_eq!(control.evade_result(0), None);
        let inner = control.begin_damage_batch(0, 1);
        control.next_passive_decision().unwrap();
        control.set_evade_decision(EvadeDecision {
            activate: true,
            success: true,
            unavoidable: false,
        });
        control.take_evade_decision().unwrap();
        assert_eq!(control.evade_result(1), Some(true));
        assert_eq!(control.evade_result(0), None);
        control.finish_damage_batch(inner);
        control.finish_damage_batch(outer);
        assert_eq!(control.evade_result(0), Some(false));
        assert_eq!(control.evade_result(1), Some(true));
        assert_eq!(control.evade_result(99), None);
    }

    #[test]
    fn unavoidable_evade_receipt_does_not_report_consumption() {
        let mut control = BattleControl::default();
        control.set_passive_decisions([PassiveDecision {
            actor: 0,
            skill: "evade".into(),
            activate: true,
            success: true,
            target: None,
        }]);
        let batch = control.begin_damage_batch(1, 0);
        control.next_passive_decision().unwrap();
        control.set_evade_decision(EvadeDecision {
            activate: true,
            success: true,
            unavoidable: true,
        });
        control.take_evade_decision().unwrap();
        control.finish_damage_batch(batch);
        assert_eq!(control.evade_result(0), Some(false));
    }
}

//! 共用的状态与召唤 handler：在受控上下文中修改载荷或加入效果。
//!
//! 状态 handler 由当前 hook 与载荷驱动；召唤辅助从已注册模板槽读取蓝图，
//! 再提交生成或重施效果。缺少模板、能力或非法引用时返回对应 handler 错误。
//! 默认规则由 profile 绑定这些函数，低层扩展也可按相同协议使用。

use super::*;

mod minions;
mod skills;
mod states;

pub use minions::*;
pub use skills::*;
pub use states::*;

/// 召唤 handler 错误：上下文失败、模板槽缺失/非法，或重施目标仍然存活。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSummonHandlerError {
    Context(EffectContextError),
    MissingTemplateSlot(TemplateSlotId),
    InvalidTemplateSlot(TemplateSlotId),
    RememberedSummonAlive(EntityIdx),
}

impl From<EffectContextError> for RuntimeSummonHandlerError {
    /// 把上下文错误转换为 [`RuntimeSummonHandlerError::Context`] 变体。
    fn from(error: EffectContextError) -> Self { Self::Context(error) }
}

/// 使魔 handler 错误：上下文失败、模板槽或计数槽缺失/非法、计数溢出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMinionHandlerError {
    Context(EffectContextError),
    MissingTemplateSlot(TemplateSlotId),
    InvalidTemplateSlot(TemplateSlotId),
    InvalidCounterSlot(EntitySlotId),
    CounterOverflow(EntitySlotId),
}

impl From<EffectContextError> for RuntimeMinionHandlerError {
    /// 把上下文错误转换为 [`RuntimeMinionHandlerError::Context`] 变体。
    fn from(error: EffectContextError) -> Self { Self::Context(error) }
}

/// 从模板槽读取召唤蓝图，并按默认消息入队生成。
pub fn push_summon_from_template_slot(
    context: &mut SkillContext<'_>,
    template_slot: TemplateSlotId,
) -> Result<(), RuntimeSummonHandlerError> {
    push_summon_from_template_slot_with_message(context, template_slot, "出现一个新的[1]")
}

/// 从模板槽读取召唤蓝图，并按自定义消息入队生成。
pub fn push_summon_from_template_slot_with_message(
    context: &mut SkillContext<'_>,
    template_slot: TemplateSlotId,
    message: impl Into<String>,
) -> Result<(), RuntimeSummonHandlerError> {
    let summon_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeSummonHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeSummonHandlerError::MissingTemplateSlot(template_slot)),
    };
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: context.owner_idx(),
        template: Box::new(summon_template),
        message: std::borrow::Cow::Owned(message.into()),
    });
    Ok(())
}

/// 重施召唤物：已记录且已死亡则复活，否则按默认消息生成并记录下标。
pub fn push_summon_recast_from_entity_slot(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
) -> Result<EntityIdx, RuntimeSummonHandlerError> {
    push_summon_recast_from_entity_slot_with_messages(
        context,
        entity_slot,
        summon_template,
        revive_hp,
        "出现一个新的[1]",
        "[1][复活]了",
    )
}

/// 重施召唤物，生成与复活共用同一条消息。
pub fn push_summon_recast_from_entity_slot_with_message(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeSummonHandlerError> {
    let message = message.into();
    push_summon_recast_from_entity_slot_with_messages(context, entity_slot, summon_template, revive_hp, message.clone(), message)
}

/// 重施召唤物，可分别指定生成与复活消息。
pub fn push_summon_recast_from_entity_slot_with_messages(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
    spawn_message: impl Into<String>,
    revive_message: impl Into<String>,
) -> Result<EntityIdx, RuntimeSummonHandlerError> {
    let owner = context.owner_idx();
    let spawn_message = spawn_message.into();
    let revive_message = revive_message.into();
    let remembered = context
        .owner()
        .and_then(|entity| entity.slots.get(entity_slot))
        .and_then(|value| match value {
            SlotValue::U64(idx) => Some(EntityIdx(*idx as u32)),
            _ => None,
        });
    if let Some(summon) = remembered {
        let entity = context.entity(summon)?;
        if entity.runtime.alive {
            return Err(RuntimeSummonHandlerError::RememberedSummonAlive(summon));
        }
        context.push_nested(QueuedEffect::ReviveWithMessage {
            caster: owner,
            target: summon,
            hp: revive_hp,
            message: revive_message,
        });
        return Ok(summon);
    }

    let next_entity = EntityArena::next_spawn_idx_from_slot_count(context.entity_count(), &summon_template);
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: owner,
        template: Box::new(summon_template),
        message: std::borrow::Cow::Owned(spawn_message),
    });
    context.set_entity_slot(owner, entity_slot, SlotValue::U64(u64::from(next_entity.0)))?;
    Ok(next_entity)
}

/// 从模板槽取蓝图后重施召唤物，可分别指定生成与复活消息。
pub fn push_summon_recast_from_template_slot_with_messages(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
    spawn_message: impl Into<String>,
    revive_message: impl Into<String>,
) -> Result<EntityIdx, RuntimeSummonHandlerError> {
    let summon_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeSummonHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeSummonHandlerError::MissingTemplateSlot(template_slot)),
    };
    push_summon_recast_from_entity_slot_with_messages(
        context,
        entity_slot,
        summon_template,
        revive_hp,
        spawn_message,
        revive_message,
    )
}

/// 从模板槽取蓝图后按默认消息重施召唤物。
pub fn push_summon_recast_from_template_slot(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
) -> Result<EntityIdx, RuntimeSummonHandlerError> {
    push_summon_recast_from_template_slot_with_messages(
        context,
        entity_slot,
        template_slot,
        revive_hp,
        "出现一个新的[1]",
        "[1][复活]了",
    )
}

/// 从模板槽取蓝图后重施召唤物，生成与复活共用同一条消息。
pub fn push_summon_recast_from_template_slot_with_message(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeSummonHandlerError> {
    let message = message.into();
    push_summon_recast_from_template_slot_with_messages(context, entity_slot, template_slot, revive_hp, message.clone(), message)
}

/// 血祭重施 handler：播报后生成或复活模板槽中的召唤物。
pub fn run_summon_recast_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0]使用[血祭]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    push_summon_recast_from_template_slot_with_messages(context, entity_slot, template_slot, revive_hp, "召唤出[1]", "召唤出[1]")
        .expect("summon recast handler should spawn or revive template-slot summon");
}

/// 血祭重施 handler 的默认槽位与复活体力绑定。
pub fn run_summon_recast_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_summon_recast_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0), 10);
}

/// 构造召唤物默认技能装载：两个火球加一个自爆，按给定行动序排列。
pub fn summon_default_skill_loadout(fire_skill: SkillId, explode_skill: SkillId, active_order: [usize; 3]) -> SkillLoadout {
    SkillLoadout::from_skills([fire_skill, fire_skill, explode_skill]).with_active_order(active_order)
}

/// 入队一次召唤物火球攻击，附带火焰状态键。
pub fn push_summon_fire(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::FireAttack {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

/// 召唤物火球技能 handler：对选中目标入队火球攻击。
pub fn run_summon_fire_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_summon_fire(context, target, 91);
}

/// 入队一次召唤物自爆，附带火焰状态键。
pub fn push_summon_explode(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::SummonExplode {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

/// 召唤物自爆技能 handler：对选中目标入队自爆。
pub fn run_summon_explode_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_summon_explode(context, target, 91);
}

/// 入队一次驱散攻击。
pub fn push_disperse_attack(context: &mut SkillContext<'_>, target: EntityIdx) {
    context.push_nested(QueuedEffect::DisperseAttack {
        caster: context.owner_idx(),
        target,
    });
}

/// 驱散技能 handler：对选中目标入队驱散攻击。
pub fn run_disperse_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_disperse_attack(context, target);
}

/// 附体技能 handler：移除自身，并使选中目标进入狂暴状态。
pub fn run_possess_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0]使用[附体]",
        context.owner_idx().0 as usize,
        target.0 as usize,
        0,
    ));
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[1]进入[狂暴]状态",
        context.owner_idx().0 as usize,
        target.0 as usize,
        0,
    ));
    context.push_nested(QueuedEffect::Remove {
        caster: context.owner_idx(),
        target: context.owner_idx(),
    });
    context.push_nested(QueuedEffect::AddBerserkState {
        target,
        legacy_order_key: 10,
        step: 4,
    });
}

/// 驱散选目标评分：聪明按血量与吸引力计算，普通掷随机加吸引力；实体不存在返回负无穷。
pub fn score_disperse_target(entities: &EntityArena, world: &WorldArena, target: EntityIdx, smart: bool, rng: &mut RC4) -> f64 {
    let Some(target_entity) = entities.get(target) else {
        return f64::MIN;
    };
    let rate_hi_hp = |hp: i32| -> f64 {
        if hp < 20 {
            30.0
        } else if hp > 300 {
            300.0
        } else {
            hp as f64
        }
    };
    let target_runtime = &target_entity.runtime;
    let mut score = if smart {
        if world.alive_group_count() > 2 {
            rate_hi_hp(target_runtime.hp) * world.alive_group_len_containing(target) as f64 * target_runtime.attract()
        } else {
            (1.0 / rate_hi_hp(target_runtime.hp)) * target_runtime.atk_sum as f64 * target_runtime.attract()
        }
    } else {
        rng.rFFFF() as f64 + target_runtime.attract()
    };
    if smart && target_runtime.is_combat_minion() && target_runtime.hp > 100 {
        score *= 2.0;
    }
    score
}

/// 驱散的选目标逻辑：收集非同队存活者作为候选，按评分挑选后返回。
pub fn select_disperse_targets(
    entities: &EntityArena,
    world: &WorldArena,
    actor: EntityIdx,
    smart: bool,
    rng: &mut RC4,
) -> Vec<EntityIdx> {
    let Some(actor_entity) = entities.get(actor) else {
        return Vec::new();
    };
    let candidates = world
        .flat_alive()
        .iter()
        .copied()
        .filter(|target| {
            entities
                .get(*target)
                .is_some_and(|target_entity| target_entity.runtime.team != actor_entity.runtime.team)
        })
        .collect::<Vec<_>>();
    select_disperse_targets_from_candidates(entities, world, &candidates, smart, rng)
}

/// 从候选集合中抽取驱散目标：smart 选 3 个、否则 2 个，重复与无效抽样有次数上限；多选时按评分降序返回。
fn select_disperse_targets_from_candidates(
    entities: &EntityArena,
    world: &WorldArena,
    candidates: &[EntityIdx],
    smart: bool,
    rng: &mut RC4,
) -> Vec<EntityIdx> {
    let select_count = if smart { 3 } else { 2 };
    let mut selected = Vec::new();
    let mut dup = 0usize;
    let mut invalid = -(select_count as i32);
    while dup <= select_count && invalid <= select_count as i32 {
        let Some(idx) = rng.pick(candidates) else {
            return Vec::new();
        };
        let target = candidates[idx];
        if entities.get(target).is_none() {
            invalid += 1;
            continue;
        }
        if selected.contains(&target) {
            dup += 1;
            continue;
        }
        selected.push(target);
        if selected.len() >= select_count {
            break;
        }
    }
    if selected.is_empty() {
        return Vec::new();
    }
    if selected.len() == 1 {
        let target = selected[0];
        let _ = score_disperse_target(entities, world, target, smart, rng);
        return vec![target];
    }

    let mut scored = selected
        .into_iter()
        .map(|target| (target, score_disperse_target(entities, world, target, smart, rng)))
        .collect::<Vec<_>>();
    scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(target, _)| target).collect()
}

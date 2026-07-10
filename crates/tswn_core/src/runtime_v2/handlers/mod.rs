use super::*;

mod minions;
mod states;

pub use minions::*;
pub use states::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeV2SummonHandlerError {
    Context(EffectContextError),
    MissingTemplateSlot(TemplateSlotId),
    InvalidTemplateSlot(TemplateSlotId),
    RememberedSummonAlive(EntityIdx),
}

impl From<EffectContextError> for RuntimeV2SummonHandlerError {
    fn from(error: EffectContextError) -> Self { Self::Context(error) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeV2MinionHandlerError {
    Context(EffectContextError),
    MissingTemplateSlot(TemplateSlotId),
    InvalidTemplateSlot(TemplateSlotId),
    InvalidCounterSlot(EntitySlotId),
    CounterOverflow(EntitySlotId),
}

impl From<EffectContextError> for RuntimeV2MinionHandlerError {
    fn from(error: EffectContextError) -> Self { Self::Context(error) }
}

pub fn push_summon_from_template_slot(
    context: &mut SkillContext<'_>,
    template_slot: TemplateSlotId,
) -> Result<(), RuntimeV2SummonHandlerError> {
    push_summon_from_template_slot_with_message(context, template_slot, "出现一个新的[1]")
}

pub fn push_summon_from_template_slot_with_message(
    context: &mut SkillContext<'_>,
    template_slot: TemplateSlotId,
    message: impl Into<String>,
) -> Result<(), RuntimeV2SummonHandlerError> {
    let summon_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeV2SummonHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeV2SummonHandlerError::MissingTemplateSlot(template_slot)),
    };
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: context.owner_idx(),
        template: summon_template,
        message: message.into(),
    });
    Ok(())
}

pub fn push_summon_recast_from_entity_slot(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    push_summon_recast_from_entity_slot_with_messages(
        context,
        entity_slot,
        summon_template,
        revive_hp,
        "出现一个新的[1]",
        "[1][复活]了",
    )
}

pub fn push_summon_recast_from_entity_slot_with_message(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let message = message.into();
    push_summon_recast_from_entity_slot_with_messages(context, entity_slot, summon_template, revive_hp, message.clone(), message)
}

pub fn push_summon_recast_from_entity_slot_with_messages(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
    spawn_message: impl Into<String>,
    revive_message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
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
            return Err(RuntimeV2SummonHandlerError::RememberedSummonAlive(summon));
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
        template: summon_template,
        message: spawn_message,
    });
    context.set_entity_slot(owner, entity_slot, SlotValue::U64(u64::from(next_entity.0)))?;
    Ok(next_entity)
}

pub fn push_summon_recast_from_template_slot_with_messages(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
    spawn_message: impl Into<String>,
    revive_message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let summon_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeV2SummonHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeV2SummonHandlerError::MissingTemplateSlot(template_slot)),
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

pub fn push_summon_recast_from_template_slot(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    push_summon_recast_from_template_slot_with_messages(
        context,
        entity_slot,
        template_slot,
        revive_hp,
        "出现一个新的[1]",
        "[1][复活]了",
    )
}

pub fn push_summon_recast_from_template_slot_with_message(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let message = message.into();
    push_summon_recast_from_template_slot_with_messages(context, entity_slot, template_slot, revive_hp, message.clone(), message)
}

pub fn run_legacy_summon_recast_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
) {
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]使用[血祭]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    push_summon_recast_from_template_slot_with_messages(context, entity_slot, template_slot, revive_hp, "召唤出[1]", "召唤出[1]")
        .expect("legacy summon recast handler should spawn or revive template-slot summon");
}

pub fn run_legacy_summon_recast_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_legacy_summon_recast_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0), 10);
}

pub fn summon_default_skill_loadout(fire_skill: SkillId, explode_skill: SkillId, active_order: [usize; 3]) -> SkillLoadout {
    SkillLoadout::from_skills([fire_skill, fire_skill, explode_skill]).with_active_order(active_order)
}

pub fn push_summon_fire(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::FireAttack {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

pub fn run_summon_fire_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_summon_fire(context, target, 91);
}

pub fn push_summon_explode(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::SummonExplode {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

pub fn run_summon_explode_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_summon_explode(context, target, 91);
}

pub fn push_disperse_attack(context: &mut SkillContext<'_>, target: EntityIdx) {
    context.push_nested(QueuedEffect::DisperseAttack {
        caster: context.owner_idx(),
        target,
    });
}

pub fn run_disperse_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_disperse_attack(context, target);
}

pub fn run_possess_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]使用[附体]",
        context.owner_idx().0 as usize,
        target.0 as usize,
        0,
    ));
    context.add_update(crate::engine::update::RunUpdate::new(
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

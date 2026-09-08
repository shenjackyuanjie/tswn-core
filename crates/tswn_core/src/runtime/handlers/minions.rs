use super::*;

pub fn next_minion_name_from_entity_slot(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
) -> Result<String, RuntimeMinionHandlerError> {
    let owner = context.owner().ok_or(EffectContextError::UnknownEntity(context.owner_idx()))?;
    let root_owner_idx = owner.runtime.root_owner;
    let root_owner = context.entity(root_owner_idx)?;
    let root_name = root_owner.template.name.clone();
    let next = match root_owner.slots.get(counter_slot) {
        Some(SlotValue::U64(next)) => *next,
        Some(_) => return Err(RuntimeMinionHandlerError::InvalidCounterSlot(counter_slot)),
        None => 0,
    };
    let following = next.checked_add(1).ok_or(RuntimeMinionHandlerError::CounterOverflow(counter_slot))?;
    context.set_entity_slot(root_owner_idx, counter_slot, SlotValue::U64(following))?;
    Ok(format!("{root_name}?{next}"))
}

pub fn push_minion_from_template_with_allocated_name(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    mut minion_template: PlayerTemplate,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeMinionHandlerError> {
    let minion_name = next_minion_name_from_entity_slot(context, counter_slot)?;
    let next_entity = EntityArena::next_spawn_idx_from_slot_count(context.entity_count(), &minion_template);
    minion_template.name = minion_name;
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: context.owner_idx(),
        template: minion_template,
        message: message.into(),
    });
    Ok(next_entity)
}

pub fn push_minion_from_template_with_allocated_name_silent(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    mut minion_template: PlayerTemplate,
) -> Result<EntityIdx, RuntimeMinionHandlerError> {
    let minion_name = next_minion_name_from_entity_slot(context, counter_slot)?;
    let next_entity = EntityArena::next_spawn_idx_from_slot_count(context.entity_count(), &minion_template);
    minion_template.name = minion_name;
    context.push_nested(QueuedEffect::SpawnSilent {
        caster: context.owner_idx(),
        template: minion_template,
    });
    Ok(next_entity)
}

pub fn push_minion_from_template_slot_with_allocated_name(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeMinionHandlerError> {
    let minion_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeMinionHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeMinionHandlerError::MissingTemplateSlot(template_slot)),
    };
    push_minion_from_template_with_allocated_name(context, counter_slot, minion_template, message)
}

pub fn push_minion_from_template_slot_with_allocated_name_silent(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
) -> Result<EntityIdx, RuntimeMinionHandlerError> {
    let minion_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeMinionHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeMinionHandlerError::MissingTemplateSlot(template_slot)),
    };
    push_minion_from_template_with_allocated_name_silent(context, counter_slot, minion_template)
}

pub fn run_shadow_minion_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0]使用[幻术]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    push_minion_from_template_slot_with_allocated_name(context, counter_slot, template_slot, "召唤出[1]")
        .expect("shadow minion handler should spawn template-slot minion");
}

pub fn run_shadow_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_shadow_minion_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0));
}

pub fn run_zombie_minion_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    killed_target: EntityIdx,
) {
    let zombie = push_minion_from_template_slot_with_allocated_name_silent(context, counter_slot, template_slot)
        .expect("zombie minion handler should spawn template-slot minion");
    context.add_update(crate::runtime::update::RunUpdate::new_newline());
    let mut summon_update =
        crate::runtime::update::RunUpdate::new("[0][召唤亡灵]", context.owner_idx().0 as usize, killed_target.0 as usize, 60);
    summon_update.delay0 = 1500;
    context.add_update(summon_update);
    let mut zombied =
        crate::runtime::update::RunUpdate::new("[2]变成了[1]", context.owner_idx().0 as usize, zombie.0 as usize, 0);
    zombied.targets.push(killed_target.0 as usize);
    context.add_update(zombied);
}

pub fn run_zombie_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let killed_target = context.selected_target().unwrap_or(EntityIdx(1));
    run_zombie_minion_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0), killed_target);
}

pub fn minion_display_index_for_entity(entity: Option<&EntityRecord>) -> usize {
    let Some(entity) = entity else {
        return 0;
    };
    if !entity.runtime.is_minion() {
        return 0;
    }
    entity
        .template
        .name
        .rsplit_once('?')
        .and_then(|(_, index)| index.parse::<usize>().ok())
        .map(|index| index + 1)
        .unwrap_or(1)
}

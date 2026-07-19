use super::*;

mod custom_bed2_fixture_tests;
mod custom_bed2_roster_import_tests;
mod custom_minion_name_tests;
mod custom_minion_spawn_tests;
mod custom_mixed_import_tests;
mod custom_runner_fixture_diff_tests;
mod custom_runner_import_tests;
mod custom_runner_round_diff_tests;
mod custom_summon_handler_tests;
mod effect_pipeline_tests;
mod entity_identity_tests;
mod plain_action_scheduler_tests;
mod plain_assassinate_skill_tests;
mod plain_at_boost_precision_tests;
mod plain_attack_skill_tests;
mod plain_charge_accumulate_tests;
mod plain_charm_targeting_tests;
mod plain_clone_skill_tests;
mod plain_counter_skill_tests;
mod plain_damage_short_circuit_tests;
mod plain_disperse_effect_tests;
mod plain_disperse_skill_tests;
mod plain_haste_scheduler_tests;
mod plain_haste_targeting_tests;
mod plain_heal_skill_tests;
mod plain_ice_scheduler_tests;
mod plain_ice_skill_tests;
mod plain_kill_hook_tests;
mod plain_linked_minion_cleanup_tests;
mod plain_poison_state_tests;
mod plain_probability_tests;
mod plain_protect_skill_tests;
mod plain_raw_import_tests;
mod plain_reflect_skill_tests;
mod plain_revive_lifecycle_tests;
mod plain_status_skill_tests;
mod plain_summon_share_damage_tests;
mod plain_summon_skill_tests;
mod plain_terminal_round_tests;
mod plain_zombie_skill_tests;
mod prepared_init_tests;
mod runtime_core_tests;
mod runtime_render_tests;
mod skill_hook_dispatch_tests;
mod state_hook_dispatch_tests;
mod state_hook_generation_tests;
mod summon_explode_post_defend_tests;
mod summon_explode_pre_defend_tests;
mod summon_explode_scheduler_tests;
mod summon_explode_tests;

fn assert_rng_state_eq(actual: &RC4, expected: &RC4) {
    assert_eq!(actual.i, expected.i);
    assert_eq!(actual.j, expected.j);
    assert_eq!(actual.main_val, expected.main_val);
}

fn custom_marks_update(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Text(message) = &effect.payload else {
        panic!("custom test effect expects text payload");
    };
    context.add_update(crate::runtime::update::RunUpdate::new(
        message.clone(),
        effect.caster.0 as usize,
        effect.target.unwrap().0 as usize,
        0,
    ));
}

fn custom_spawns_nested_damage(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(amount) = effect.payload else {
        panic!("custom test effect expects int payload");
    };
    context.push_nested(QueuedEffect::Damage {
        caster: effect.caster,
        target: effect.target.expect("custom test effect needs target"),
        amount,
    });
}

fn custom_spawns_nested_heal(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(amount) = effect.payload else {
        panic!("custom test effect expects int payload");
    };
    context.push_nested(QueuedEffect::Heal {
        caster: effect.caster,
        target: effect.target.expect("custom test effect needs target"),
        amount,
    });
}

fn custom_rejects_cross_entity_read(context: &mut EffectContext<'_>, _: &CustomEffect) {
    assert_eq!(
        context.entity(EntityIdx(2)),
        Err(EffectContextError::MissingCapability(ExtensionCapability::ReadEnemies))
    );
    context.add_update(crate::runtime::update::RunUpdate::new("read denied", 0, 0, 0));
}

fn custom_reads_cross_entity(context: &mut EffectContext<'_>, _: &CustomEffect) {
    let observed = context.entity(EntityIdx(2)).expect("capability should allow cross-entity read");
    context.add_update(crate::runtime::update::RunUpdate::new(observed.template.name.clone(), 0, 2, 0));
}

fn custom_mutates_entity_slot(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(slot) = effect.payload else {
        panic!("custom test effect expects entity slot id payload");
    };
    context
        .set_entity_slot(
            effect.target.expect("custom test effect needs target"),
            EntitySlotId(slot as u32),
            SlotValue::Bool(true),
        )
        .expect("capability should allow entity slot mutation");
    context.add_update(crate::runtime::update::RunUpdate::new("slot set", 0, 0, 0));
}

fn custom_consumes_rng(context: &mut EffectContext<'_>, effect: &CustomEffect) {
    let CustomEffectPayload::Int(max) = effect.payload else {
        panic!("custom test effect expects rng max payload");
    };
    let value = context.rng_next_i32(max);
    let next_byte = context.rng_next_u8();
    context.add_update(crate::runtime::update::RunUpdate::new(
        format!("rng:{value}:{next_byte}"),
        effect.caster.0 as usize,
        effect.target.unwrap().0 as usize,
        value as u32,
    ));
}

fn skill_marks_update(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "skill mark",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.skill_id.0,
    ));
}

fn skill_marks_selected_target(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let target = context.selected_target().expect("skill should receive selected target");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "selected target",
        entry.owner.0 as usize,
        target.0 as usize,
        target.0,
    ));
}

fn state_marks_charge_boost(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let owner = context.owner().expect("state owner should exist");
    let message = if owner.runtime.charge.active && owner.runtime.at_boost_millionths == 3_000_000 {
        "charge boosted"
    } else {
        "charge inactive"
    };
    context.add_update(crate::runtime::update::RunUpdate::new(
        message,
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
}

fn skill_clears_positive_runtime(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let messages = context
        .clear_owner_positive_runtime_messages()
        .expect("clear-positive owner should exist");
    let owner = context.owner_idx();
    for (priority, message) in messages {
        context.add_update(crate::runtime::update::RunUpdate::new(
            message,
            owner.0 as usize,
            owner.0 as usize,
            priority as u32,
        ));
    }
}

fn skill_clears_positive_states(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let messages = context.clear_owner_positive_state_messages().expect("clear-positive owner should exist");
    let owner = context.owner_idx();
    for (priority, message) in messages {
        context.add_update(crate::runtime::update::RunUpdate::new(
            message,
            owner.0 as usize,
            owner.0 as usize,
            priority as u32,
        ));
    }
}

fn skill_clears_positive(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let messages = context.clear_owner_positive_messages().expect("clear-positive owner should exist");
    let owner = context.owner_idx();
    for (priority, message) in messages {
        context.add_update(crate::runtime::update::RunUpdate::new(
            message,
            owner.0 as usize,
            owner.0 as usize,
            priority as u32,
        ));
    }
}

fn skill_noop(_: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {}

fn skill_pushes_nested_damage(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.push_nested(QueuedEffect::Damage {
        caster: context.owner_idx(),
        target: EntityIdx(1),
        amount: 2,
    });
}

fn skill_halves_defend_atp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let atp = context.defend_atp().expect("pre-defend skill should receive atp");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "pre defend skill",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        atp as u32,
    ));
    context.set_defend_atp(atp / 2.0);
}

fn skill_zeroes_defend_atp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    assert!(context.defend_atp().expect("pre-defend skill should receive atp") > 0.0);
    context.add_update(crate::runtime::update::RunUpdate::new(
        "pre defend zero",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.skill_id.0,
    ));
    context.set_defend_atp(0.0);
}

fn skill_marks_defend_replay(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let caster = context.defend_caster().expect("post-defend skill should receive incoming caster");
    let target = context.defend_target().expect("post-defend skill should receive incoming target");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0][防御]",
        target.0 as usize,
        caster.0 as usize,
        0,
    ));
}

fn skill_halves_defend_damage(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let damage = context.defend_damage().expect("post-defend skill should receive damage");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "post defend skill",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        damage as u32,
    ));
    context.set_defend_damage(damage / 2);
}

fn skill_bed2_template_slot_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    push_summon_from_template_slot(context, TemplateSlotId(0)).expect("bed2 summon handler should read template slot payload");
}

fn skill_bed2_template_slot_legacy_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    push_summon_from_template_slot_with_message(context, TemplateSlotId(0), "召唤出[1]")
        .expect("bed2 summon handler should read template slot payload");
}

fn skill_records_missing_template_slot_error(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    assert_eq!(
        push_summon_from_template_slot(context, TemplateSlotId(0)),
        Err(RuntimeSummonHandlerError::MissingTemplateSlot(TemplateSlotId(0)))
    );
    context.add_update(crate::runtime::update::RunUpdate::new(
        "missing summon template",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        0,
    ));
}

fn skill_consumes_rng(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let value = context.rng_next_i32(10);
    let next_byte = context.rng_next_u8();
    context.add_update(crate::runtime::update::RunUpdate::new(
        format!("skill-rng:{value}:{next_byte}"),
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        value as u32,
    ));
}

fn skill_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10)
        .expect("summon recast fixture should spawn or revive summon");
}

fn skill_legacy_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "[0]使用[血祭]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    push_summon_recast_from_entity_slot_with_messages(context, EntitySlotId(0), summon_template, 10, "召唤出[1]", "召唤出[1]")
        .expect("legacy summon recast fixture should spawn or revive summon");
}

fn skill_configured_summon_recast_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_summon_recast_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(1), 7);
}

fn skill_records_alive_summon_recast_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    assert_eq!(
        push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10),
        Err(RuntimeSummonHandlerError::RememberedSummonAlive(EntityIdx(2)))
    );
}

fn skill_records_missing_recast_read_allies_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([SkillId(0)]);
    assert_eq!(
        push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10),
        Err(RuntimeSummonHandlerError::Context(EffectContextError::MissingCapability(
            ExtensionCapability::ReadAllies
        )))
    );
}

fn skill_records_next_minion_name(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let name = next_minion_name_from_entity_slot(context, EntitySlotId(0)).expect("minion name helper should allocate a name");
    context.add_update(crate::runtime::update::RunUpdate::new(
        name,
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
}

fn skill_records_missing_minion_name_read_allies_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    assert_eq!(
        next_minion_name_from_entity_slot(context, EntitySlotId(0)),
        Err(RuntimeMinionHandlerError::Context(EffectContextError::MissingCapability(
            ExtensionCapability::ReadAllies
        )))
    );
}

fn skill_pushes_named_minion_spawn(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let minion_template = PlayerTemplate::with_kind(3, "placeholder", PlayerKindId(0), 0, 5, 1);
    assert_eq!(
        push_minion_from_template_with_allocated_name(context, EntitySlotId(0), minion_template, "召唤出[1]"),
        Ok(EntityIdx(2))
    );
}

fn skill_pushes_named_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    assert_eq!(
        push_minion_from_template_slot_with_allocated_name(context, EntitySlotId(0), TemplateSlotId(0), "召唤出[1]"),
        Ok(EntityIdx(2))
    );
}

fn skill_configured_shadow_minion_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_shadow_minion_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(1));
}

fn skill_configured_zombie_minion_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_zombie_minion_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(2), EntityIdx(2));
}

fn state_marks_update(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    context.add_update(crate::runtime::update::RunUpdate::new(
        "state mark",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
}

fn state_pushes_nested_heal(context: &mut StateContext<'_>, _: &StateHookPlanEntry) {
    context.push_nested(QueuedEffect::Heal {
        caster: context.owner_idx(),
        target: context.owner_idx(),
        amount: 2,
    });
}

fn state_consumes_rng(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let value = context.rng_next_i32(10);
    let next_byte = context.rng_next_u8();
    context.add_update(crate::runtime::update::RunUpdate::new(
        format!("state-rng:{value}:{next_byte}"),
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        value as u32,
    ));
}

fn state_adds_defend_damage(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let damage = context.defend_damage().expect("post-defend state should receive damage");
    context.add_update(crate::runtime::update::RunUpdate::new(
        "post defend state",
        entry.owner.0 as usize,
        entry.owner.0 as usize,
        entry.legacy_order_key,
    ));
    context.set_defend_damage(damage + 3);
}

fn render_first_message_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
    Some(RenderedReplay::new(
        ReplayRendererId(0),
        frame.updates.updates.first()?.message.to_string(),
    ))
}

fn render_update_count_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
    Some(RenderedReplay::new(
        ReplayRendererId(1),
        frame.updates.updates.len().to_string(),
    ))
}

fn render_first_message_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
    Some(RenderedShow::new(
        ShowRendererId(0),
        frame.updates.updates.first()?.message.to_string(),
    ))
}

fn render_hp_marker_bar_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
    let hp_report = frame.updates.updates.iter().find(|update| update.message == "[0]还剩[2]点血")?;
    Some(RenderedShow::new(
        ShowRendererId(0),
        format!(
            "hp-bar:actor={}:value={}:text={}",
            hp_report.caster,
            hp_report.param.unwrap_or(hp_report.score),
            hp_report.msg()
        ),
    ))
}

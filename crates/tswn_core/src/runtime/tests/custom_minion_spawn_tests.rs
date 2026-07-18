use super::*;

#[test]
fn push_minion_from_template_with_allocated_name_sets_name_and_spawns() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-spawn",
            "custom.minion_spawn",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion spawn skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_pushes_named_minion_spawn,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("named minion spawn should emit update");

    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[0].caster, 0);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let minion = runtime.entities.get(EntityIdx(2)).expect("minion should spawn");
    assert_eq!(minion.template.name, "owner?0");
    assert_eq!(minion.template.kind, minion_kind);
    assert_eq!(minion.runtime.owner, EntityIdx(0));
    assert_eq!(minion.runtime.root_owner, EntityIdx(0));
}

#[test]
fn push_minion_from_template_slot_with_allocated_name_reads_template_and_spawns() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "shadow-template", "custom.minion.shadow_template")
        .expect("shadow template slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-template-spawn",
            "custom.minion_template_spawn",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion template spawn skill should register");
    let inherited_skill = builder
        .register_skill(
            "custom",
            "possess",
            "custom.minion.possess",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("inherited minion skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "shadow",
            "custom.minion.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("shadow minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "placeholder-shadow", minion_kind, 0, 5, 1)
        .with_def_res(2, 3)
        .with_skills([inherited_skill])
        .with_speed_points(-2048);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("shadow template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_pushes_named_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("named minion template spawn should emit update");

    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let minion = runtime.entities.get(EntityIdx(2)).expect("template minion should spawn");
    assert_eq!(minion.template.name, "owner?0");
    assert_eq!(minion.template.kind, minion_kind);
    assert_eq!(minion.template.max_hp, payload.max_hp);
    assert_eq!(minion.template.defense, payload.defense);
    assert_eq!(minion.template.resistance, payload.resistance);
    assert_eq!(minion.template.skills.skills(), &[inherited_skill]);
    assert_eq!(minion.runtime.owner, EntityIdx(0));
    assert_eq!(minion.runtime.root_owner, EntityIdx(0));
    assert_eq!(minion.template.move_state, payload.move_state);
    assert_eq!(minion.runtime.move_state, payload.move_state);
}

#[test]
fn shadow_style_minion_template_handler_emits_legacy_replay_sequence() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "shadow-template", "custom.minion.shadow_template")
        .expect("shadow template slot should reserve");
    let shadow_skill = builder
        .register_skill_with_hooks(
            "custom",
            "shadow",
            "custom.minion.shadow_skill",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("shadow skill should register");
    let possess_skill = builder
        .register_skill(
            "custom",
            "possess",
            "custom.minion.possess",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("possess skill should register");
    let shadow_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "shadow",
            "custom.minion.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("shadow minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "owner?shadow", shadow_kind, 0, 5, 1)
        .with_skills([possess_skill])
        .with_speed_points(-2048);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([shadow_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
        .expect("shadow template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        shadow_skill,
        run_shadow_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("shadow-style minion spawn should emit legacy updates");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[幻术]");
    assert_eq!(frame.updates.updates[0].caster, 0);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].score, 60);
    assert_eq!(frame.updates.updates[1].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let shadow = runtime.entities.get(EntityIdx(2)).expect("shadow minion should spawn");
    assert_eq!(shadow.template.name, "owner?0");
    assert_eq!(shadow.template.kind, shadow_kind);
    assert_eq!(shadow.template.skills.skills(), &[possess_skill]);
    assert_eq!(shadow.runtime.owner, EntityIdx(0));
    assert_eq!(shadow.runtime.root_owner, EntityIdx(0));
    assert_eq!(shadow.runtime.move_state, MoveState { speed_points: -2048 });
}

#[test]
fn possess_skill_berserks_target_and_removes_shadow_caster() {
    let mut builder = ExtensionRegistryBuilder::default();
    let possess_skill = builder
        .register_skill_with_hooks(
            "custom",
            "possess",
            "custom.minion.possess",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("possess skill should register");
    let shadow_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "shadow",
            "custom.minion.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("shadow minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::with_kind(2, "owner?0", shadow_kind, 0, 5, 1).with_skills([possess_skill]),
            PlayerTemplate::new(3, "target", 1, 20, 1),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    runtime.set_skill_handler(possess_skill, run_possess_skill);

    let plan = runtime
        .scheduler
        .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(1), ProcMask::PRE_ACTION);
    let mut updates = RunUpdates::new();
    runtime.drain_skill_hook_plan_with_selected_target_into(&plan, &mut updates, Some(EntityIdx(2)));
    let frame = RuntimeFrame { updates };

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[附体]");
    assert_eq!(frame.updates.updates[0].caster, 1);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].message, "[1]进入[狂暴]状态");
    assert_eq!(frame.updates.updates[1].caster, 1);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    assert_eq!(frame.updates.updates[2].caster, 1);
    assert_eq!(frame.updates.updates[2].target, 1);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(2))
            .unwrap()
            .states
            .entry(10)
            .map(|entry| entry.payload.clone()),
        Some(StatePayload::Berserk { step: 4 })
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2)]);
}

#[test]
fn possess_skill_extends_existing_berserk_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let possess_skill = builder
        .register_skill_with_hooks(
            "custom",
            "possess",
            "custom.minion.possess",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("possess skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "shadow", 0, 5, 1).with_skills([possess_skill]),
            PlayerTemplate::new(2, "target", 1, 20, 1),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::berserk(10, 2));
    runtime.set_skill_handler(possess_skill, run_possess_skill);

    let plan = runtime
        .scheduler
        .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(0), ProcMask::PRE_ACTION);
    let mut updates = RunUpdates::new();
    runtime.drain_skill_hook_plan_with_selected_target_into(&plan, &mut updates, Some(EntityIdx(1)));

    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(10)
            .map(|entry| entry.payload.clone()),
        Some(StatePayload::Berserk { step: 6 })
    );
}

#[test]
fn zombie_style_minion_template_handler_emits_legacy_replay_sequence() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "zombie-template", "custom.minion.zombie_template")
        .expect("zombie template slot should reserve");
    let zombie_skill = builder
        .register_skill_with_hooks(
            "custom",
            "zombie",
            "custom.minion.zombie_skill",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("zombie skill should register");
    let zombie_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "zombie",
            "custom.minion.zombie",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("zombie minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "owner?zombie", zombie_kind, 0, 4, 1).with_speed_points(1020);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([zombie_skill]),
            PlayerTemplate::new(2, "victim", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
        .expect("zombie template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        zombie_skill,
        run_zombie_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::KILL)
        .expect("zombie-style minion spawn should emit legacy updates");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "\n");
    assert_eq!(frame.updates.updates[1].message, "[0][召唤亡灵]");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 1);
    assert_eq!(frame.updates.updates[1].score, 60);
    assert_eq!(frame.updates.updates[1].delay0, 1500);
    assert_eq!(frame.updates.updates[2].message, "[2]变成了[1]");
    assert_eq!(frame.updates.updates[2].caster, 0);
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[2].targets.as_slice(), &[1]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let zombie = runtime.entities.get(EntityIdx(2)).expect("zombie minion should spawn");
    assert_eq!(zombie.template.name, "owner?0");
    assert_eq!(zombie.template.kind, zombie_kind);
    assert_eq!(zombie.runtime.owner, EntityIdx(0));
    assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
    assert_eq!(zombie.runtime.move_state, MoveState { speed_points: 1020 });
}

#[test]
fn zombie_style_minion_handler_uses_lethal_damage_killed_target() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "zombie-template", "custom.minion.zombie_template")
        .expect("zombie template slot should reserve");
    let zombie_skill = builder
        .register_skill_with_hooks(
            "custom",
            "zombie",
            "custom.minion.zombie_skill",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("zombie skill should register");
    let zombie_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "zombie",
            "custom.minion.zombie",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("zombie minion kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(4, "owner?zombie", zombie_kind, 0, 4, 1);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([zombie_skill]),
            PlayerTemplate::new(2, "first-target", 1, 20, 1),
            PlayerTemplate::new(3, "killed-target", 1, 3, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
        .expect("zombie template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        zombie_skill,
        run_zombie_minion_from_template_slot,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(2),
        amount: 3,
    });

    let frame = runtime.flush_effects().expect("lethal damage should drive zombie-style minion spawn");

    assert_eq!(frame.updates.updates.len(), 4);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].message, "\n");
    assert_eq!(frame.updates.updates[2].message, "[0][召唤亡灵]");
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[3].message, "[2]变成了[1]");
    assert_eq!(frame.updates.updates[3].target, 3);
    assert_eq!(frame.updates.updates[3].targets.as_slice(), &[2]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    let zombie = runtime.entities.get(EntityIdx(3)).expect("zombie minion should spawn");
    assert_eq!(zombie.template.name, "owner?0");
    assert_eq!(zombie.template.kind, zombie_kind);
    assert_eq!(zombie.runtime.owner, EntityIdx(0));
    assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
    assert!(!runtime.world.flat_alive().contains(&EntityIdx(2)));
    assert!(runtime.world.flat_alive().contains(&EntityIdx(3)));
}

#[test]
fn configured_minion_handlers_use_non_default_slots_and_targets() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .reserve_entity_slot("custom", "unused-counter", "custom.minion.unused_counter")
        .expect("unused counter slot should reserve");
    let counter_slot = builder
        .reserve_entity_slot("custom", "configured-counter", "custom.minion.configured_counter")
        .expect("configured counter slot should reserve");
    builder
        .reserve_template_slot("custom", "unused-template", "custom.minion.unused_template")
        .expect("unused template slot should reserve");
    let shadow_template_slot = builder
        .reserve_template_slot(
            "custom",
            "configured-shadow-template",
            "custom.minion.configured_shadow_template",
        )
        .expect("configured shadow template slot should reserve");
    let zombie_template_slot = builder
        .reserve_template_slot(
            "custom",
            "configured-zombie-template",
            "custom.minion.configured_zombie_template",
        )
        .expect("configured zombie template slot should reserve");
    let shadow_skill = builder
        .register_skill_with_hooks(
            "custom",
            "configured-shadow",
            "custom.configured_shadow",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("configured shadow skill should register");
    let zombie_skill = builder
        .register_skill_with_hooks(
            "custom",
            "configured-zombie",
            "custom.configured_zombie",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("configured zombie skill should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "configured-minion",
            "custom.configured_minion",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("configured minion kind should register");
    let registry = builder.build();
    let shadow_payload = PlayerTemplate::with_kind(3, "shadow-template", minion_kind, 0, 5, 1);
    let zombie_payload = PlayerTemplate::with_kind(4, "zombie-template", minion_kind, 0, 6, 1);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([shadow_skill, zombie_skill]),
            PlayerTemplate::new(2, "victim-a", 1, 10, 1),
            PlayerTemplate::new(3, "victim-b", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(shadow_template_slot, SlotValue::PlayerTemplate(Box::new(shadow_payload)))
        .expect("configured shadow template slot should write");
    template
        .slots
        .set(zombie_template_slot, SlotValue::PlayerTemplate(Box::new(zombie_payload)))
        .expect("configured zombie template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        shadow_skill,
        skill_configured_shadow_minion_handler,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );
    runtime.set_skill_handler_with_capabilities(
        zombie_skill,
        skill_configured_zombie_minion_handler,
        &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
    );

    let shadow_frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("configured shadow minion should emit updates");

    assert_eq!(shadow_frame.updates.updates[0].message, "[0]使用[幻术]");
    assert_eq!(shadow_frame.updates.updates[1].target, 3);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(EntitySlotId(0)), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(1))
    );
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.name, "owner?0");

    let zombie_frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::KILL)
        .expect("configured zombie minion should emit updates");

    assert_eq!(zombie_frame.updates.updates[1].message, "[0][召唤亡灵]");
    assert_eq!(zombie_frame.updates.updates[1].target, 2);
    assert_eq!(zombie_frame.updates.updates[2].message, "[2]变成了[1]");
    assert_eq!(zombie_frame.updates.updates[2].target, 4);
    assert_eq!(zombie_frame.updates.updates[2].targets.as_slice(), &[2]);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().template.name, "owner?1");
}

#[test]
fn custom_minion_heal_fixture_does_not_share_with_owner_or_summons() {
    let mut builder = ExtensionRegistryBuilder::default();
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion-owner",
            "custom.minion_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3),
            PlayerTemplate::new(2, "healer", 0, 10, 1),
            PlayerTemplate::new(3, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "summon-a", summon_kind, 0, 10, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(5, "summon-b", summon_kind, 0, 10, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(2),
        target: EntityIdx(0),
        amount: 4,
    });
    let shared_damage = runtime.flush_effects().expect("owner damage should share to minions");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
    assert_eq!(shared_damage.updates.updates.len(), 3);
    assert_eq!(shared_damage.updates.updates[0].target, 0);
    assert_eq!(shared_damage.updates.updates[1].target, 3);
    assert_eq!(shared_damage.updates.updates[2].target, 4);

    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(1),
        target: EntityIdx(3),
        amount: 3,
    });
    let minion_heal = runtime.flush_effects().expect("minion heal should emit one update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 9);
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
    assert_eq!(minion_heal.updates.updates.len(), 1);
    assert_eq!(minion_heal.updates.updates[0].message, "[1]回复体力[2]点");
    assert_eq!(minion_heal.updates.updates[0].target, 3);
    assert_eq!(minion_heal.updates.updates[0].score, 3);
}

#[test]
fn custom_minion_owner_death_removes_linked_minions_in_entity_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 10,
    });
    let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(frame.updates.updates.len(), 5);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    assert_eq!(frame.updates.updates[2].score, 50);
    assert_eq!(
        frame.updates.updates[3].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[4].target, 3);
    assert_eq!(frame.updates.updates[4].message, "[1]消失了");
    assert_eq!(frame.updates.updates[4].score, 50);
}

#[test]
fn custom_minion_owner_remove_cleans_linked_minions_in_entity_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION | PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
    });
    runtime.flush_effects().expect("minion spawns should emit updates");

    runtime.effects.push(QueuedEffect::Remove {
        caster: EntityIdx(1),
        target: EntityIdx(0),
    });
    let frame = runtime.flush_effects().expect("owner remove should cleanup linked minions");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(frame.updates.updates.len(), 5);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].message, "[1]消失了");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    assert_eq!(frame.updates.updates[2].score, 50);
    assert_eq!(
        frame.updates.updates[3].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[4].target, 3);
    assert_eq!(frame.updates.updates[4].message, "[1]消失了");
    assert_eq!(frame.updates.updates[4].score, 50);
}

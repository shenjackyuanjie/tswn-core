use super::*;

#[test]
fn custom_summon_fixture_combines_owner_route_share_and_skill_reuse() {
    let mut builder = ExtensionRegistryBuilder::default();
    let recast_skill = builder
        .register_skill(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1).with_skills([recast_skill]),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(4, "summon-recast", summon_kind, 0, 5, 1).with_skills([recast_skill]),
    });
    runtime.flush_effects().expect("summon spawns should emit updates");

    assert_eq!(
        runtime.entities.get(EntityIdx(2)).unwrap().template.skills.skills(),
        &[recast_skill]
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(3)).unwrap().template.skills.skills(),
        &[recast_skill]
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.root_owner, EntityIdx(0));

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });
    let routed = runtime.flush_effects().expect("summon/root owner damage should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 5);
    assert_eq!(routed.updates.updates.len(), 1);
    assert_eq!(routed.updates.updates[0].target, 0);
    assert_eq!(routed.updates.updates[0].score, 4);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 2,
    });
    let shared = runtime.flush_effects().expect("owner damage should share to summons");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 4);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 3);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 3);
    assert_eq!(shared.updates.updates.len(), 3);
    assert_eq!(shared.updates.updates[0].target, 0);
    assert_eq!(shared.updates.updates[1].target, 2);
    assert_eq!(shared.updates.updates[2].target, 3);
}

#[test]
fn custom_summon_fixture_inherits_owner_defense_and_resistance() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_def_res(77, 88),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));

    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
    });
    runtime.flush_effects().expect("summon spawn should emit update");

    let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
    assert_eq!(summon.template.defense, 77);
    assert_eq!(summon.template.resistance, 88);
    assert_eq!(summon.runtime.defense, 77);
    assert_eq!(summon.runtime.resistance, 88);
}

#[test]
fn summon_default_skill_loadout_keeps_fixed_lanes_and_active_order() {
    let fire = SkillId(11);
    let explode = SkillId(12);
    let loadout = summon_default_skill_loadout(fire, explode, [2, 0, 1]);

    assert_eq!(loadout.skills(), &[fire, fire, explode]);
    assert_eq!(loadout.active_order(), &[2, 0, 1]);
}

#[test]
fn charged_summon_template_can_disable_share_damage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "charged-summon", summon_kind, 0, 5, 1)
            .with_damage_share_policy(DamageSharePolicy::None)
            .with_speed_points(2048),
    });
    runtime.flush_effects().expect("charged summon spawn should emit update");

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });
    let frame = runtime.flush_effects().expect("summon damage should emit update");

    let summon = runtime.entities.get(EntityIdx(2)).expect("charged summon should exist");
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 10);
    assert_eq!(summon.runtime.hp, 1);
    assert_eq!(summon.runtime.move_state, MoveState { speed_points: 2048 });
    assert_eq!(summon.runtime.policies.damage_share, DamageSharePolicy::None);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[0].score, 4);
}

#[test]
fn custom_summon_recast_handler_revives_existing_summon_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                .with_def_res(77, 88)
                .with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_summon_recast_fixture_handler,
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("first summon cast should emit spawn update");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(first.updates.updates.len(), 1);
    assert_eq!(first.updates.updates[0].message, "出现一个新的[1]");
    assert_eq!(first.updates.updates[0].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    let summon = runtime.entities.get(EntityIdx(2)).expect("summon should exist");
    assert_eq!(summon.template.kind, summon_kind);
    assert_eq!(summon.template.skills.skills(), &[recast_skill]);
    assert_eq!(summon.runtime.owner, EntityIdx(0));
    assert_eq!(summon.runtime.root_owner, EntityIdx(0));
    assert_eq!(summon.runtime.defense, 77);
    assert_eq!(summon.runtime.resistance, 88);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 10,
    });
    runtime.flush_effects().expect("lethal summon damage should emit update");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("summon recast should revive existing entity");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates.len(), 1);
    assert_eq!(recast.updates.updates[0].message, "[1][复活]了");
    assert_eq!(recast.updates.updates[0].target, 2);
    let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
    assert!(revived.runtime.alive);
    assert_eq!(revived.runtime.hp, 10);
    assert_eq!(revived.template.skills.skills(), &[recast_skill]);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
}

#[test]
fn custom_summon_recast_handler_can_emit_legacy_summon_messages() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_legacy_summon_recast_fixture_handler,
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("legacy summon cast should emit updates");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(first.updates.updates.len(), 2);
    assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(first.updates.updates[0].score, 60);
    assert_eq!(first.updates.updates[1].message, "召唤出[1]");
    assert_eq!(first.updates.updates[1].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.kind, summon_kind);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 10,
    });
    runtime.flush_effects().expect("lethal summon damage should emit update");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("legacy summon recast should emit updates");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates.len(), 2);
    assert_eq!(recast.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(recast.updates.updates[0].score, 60);
    assert_eq!(recast.updates.updates[1].message, "召唤出[1]");
    assert_eq!(recast.updates.updates[1].target, 2);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
}

#[test]
fn summon_recast_from_template_slot_uses_payload_and_revives_existing_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "summon-template", "custom.summon.template")
        .expect("summon template slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon-owner",
            "custom.summon_owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon owner kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "summon-template", summon_kind, 0, 10, 1)
        .with_def_res(11, 22)
        .with_skills([recast_skill])
        .with_speed_points(2048);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                .with_def_res(77, 88)
                .with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("summon template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        run_summon_recast_from_template_slot,
        &[
            ExtensionCapability::ReadTemplateSlots,
            ExtensionCapability::ReadAllies,
            ExtensionCapability::MutateEntitySlots,
        ],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("template-slot summon cast should emit updates");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(first.updates.updates.len(), 2);
    assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(first.updates.updates[0].score, 60);
    assert_eq!(first.updates.updates[1].message, "召唤出[1]");
    assert_eq!(first.updates.updates[1].target, 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
    assert_eq!(summon.template.name, payload.name);
    assert_eq!(summon.template.kind, summon_kind);
    assert_eq!(summon.template.skills.skills(), &[recast_skill]);
    assert_eq!(summon.runtime.owner, EntityIdx(0));
    assert_eq!(summon.runtime.root_owner, EntityIdx(0));
    assert_eq!(summon.runtime.defense, 77);
    assert_eq!(summon.runtime.resistance, 88);
    assert_eq!(summon.template.move_state, payload.move_state);
    assert_eq!(summon.runtime.move_state, payload.move_state);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 10,
    });
    runtime.flush_effects().expect("lethal summon damage should emit update");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("template-slot summon recast should revive existing entity");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates.len(), 2);
    assert_eq!(recast.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(recast.updates.updates[1].message, "召唤出[1]");
    assert_eq!(recast.updates.updates[1].target, 2);
    let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
    assert!(revived.runtime.alive);
    assert_eq!(revived.runtime.hp, 10);
    assert_eq!(revived.template.skills.skills(), &[recast_skill]);
    assert_eq!(revived.runtime.move_state, payload.move_state);
}

#[test]
fn configured_summon_recast_handler_uses_non_default_slots() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .reserve_entity_slot("custom", "unused-entity-slot", "custom.summon.unused_entity")
        .expect("unused entity slot should reserve");
    let summoned_slot = builder
        .reserve_entity_slot("custom", "configured-summon-slot", "custom.summon.configured_entity")
        .expect("configured summoned entity slot should reserve");
    builder
        .reserve_template_slot("custom", "unused-template-slot", "custom.summon.unused_template")
        .expect("unused template slot should reserve");
    let template_slot = builder
        .reserve_template_slot("custom", "configured-summon-template", "custom.summon.configured_template")
        .expect("configured summon template slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "configured-summon-recast",
            "custom.configured_summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("configured summon recast skill should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "configured-summon",
            "custom.configured_summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("configured summon kind should register");
    let registry = builder.build();
    let payload = PlayerTemplate::with_kind(3, "configured-summon", summon_kind, 0, 7, 1);
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    );
    template
        .slots
        .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("configured summon template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_configured_summon_recast_handler,
        &[
            ExtensionCapability::ReadTemplateSlots,
            ExtensionCapability::ReadAllies,
            ExtensionCapability::MutateEntitySlots,
        ],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("configured summon recast should emit updates");

    assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
    assert_eq!(first.updates.updates[1].message, "召唤出[1]");
    assert_eq!(first.updates.updates[1].target, 2);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(EntitySlotId(0)), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, payload.name);

    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 7,
    });
    runtime.flush_effects().expect("configured summon damage should flush");
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

    let recast = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("configured summon recast should revive existing entity");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(recast.updates.updates[1].target, 2);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 7);
}

#[test]
fn push_summon_recast_from_entity_slot_reports_alive_remembered_summon() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
            PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(summoned_slot, SlotValue::U64(2))
        .expect("remembered summon slot should write");
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_records_alive_summon_recast_error,
        &[ExtensionCapability::ReadAllies],
    );

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.len(), 3);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
}

#[test]
fn push_summon_recast_from_entity_slot_reports_missing_read_allies_capability() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summoned_slot = builder
        .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
        .expect("summoned entity slot should reserve");
    let recast_skill = builder
        .register_skill_with_hooks(
            "custom",
            "summon-recast",
            "custom.summon_recast",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon recast skill should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::None,
                inherit_owner_def_res: true,
            },
        )
        .expect("summon kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
            PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(summoned_slot, SlotValue::U64(2))
        .expect("remembered summon slot should write");
    runtime.set_skill_handler_with_capabilities(
        recast_skill,
        skill_records_missing_recast_read_allies_error,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.len(), 3);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
}

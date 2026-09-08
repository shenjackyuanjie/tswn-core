use super::*;

#[test]
fn next_minion_name_from_entity_slot_allocates_from_root_owner() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-name",
            "custom.minion_name",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion name skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill])],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_records_next_minion_name,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("first minion name should emit update");
    let second = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("second minion name should emit update");

    assert_eq!(first.updates.updates[0].message, "owner?0");
    assert_eq!(second.updates.updates[0].message, "owner?1");
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(2))
    );
}

#[test]
fn next_minion_name_from_entity_slot_uses_root_owner_for_child_minions() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-name",
            "custom.minion_name",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion name skill should register");
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
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?shadow", minion_kind, 0, 5, 1).with_skills([minion_skill]),
    });
    runtime.flush_effects().expect("child minion spawn should emit update");
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_records_next_minion_name,
        &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
    );

    let first = runtime
        .run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION)
        .expect("first child minion name should emit update");
    let second = runtime
        .run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION)
        .expect("second child minion name should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(first.updates.updates[0].message, "owner?0");
    assert_eq!(second.updates.updates[0].message, "owner?1");
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
        Some(&SlotValue::U64(2))
    );
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().slots.get(counter_slot), None);
}

#[test]
fn next_minion_name_from_entity_slot_requires_root_owner_read_capability() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter_slot = builder
        .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
        .expect("minion counter slot should reserve");
    let minion_skill = builder
        .register_skill_with_hooks(
            "custom",
            "minion-name",
            "custom.minion_name",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("minion name skill should register");
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
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 1),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "owner?shadow", minion_kind, 0, 5, 1).with_skills([minion_skill]),
    });
    runtime.flush_effects().expect("child minion spawn should emit update");
    runtime.set_skill_handler_with_capabilities(
        minion_skill,
        skill_records_missing_minion_name_read_allies_error,
        &[ExtensionCapability::MutateEntitySlots],
    );

    let frame = runtime.run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot), None);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().slots.get(counter_slot), None);
}

#[test]
fn minion_display_index_for_entity_matches_legacy_name_suffix() {
    let mut builder = ExtensionRegistryBuilder::default();
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
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 3),
            PlayerTemplate::with_kind(2, "owner?0", minion_kind, 0, 5, 1),
            PlayerTemplate::with_kind(3, "owner?12", minion_kind, 0, 5, 1),
            PlayerTemplate::with_kind(4, "owner?shadow", minion_kind, 0, 5, 1),
            PlayerTemplate::with_kind(5, "shadow", minion_kind, 0, 5, 1),
        ],
        registry,
    ));

    assert_eq!(minion_display_index_for_entity(None), 0);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(0))), 0);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(1))), 1);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(2))), 13);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(3))), 1);
    assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(4))), 1);
}

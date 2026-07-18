use super::*;

#[test]
fn flush_effects_routes_root_owner_damage_to_owner_entity() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::None,
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
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("routed damage should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].score, 4);
}

#[test]
fn flush_effects_runs_die_hook_on_resolved_root_owner() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("summon kind should register");
    let die_state = builder
        .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
        .expect("die state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 4, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 99,
        extension_state_id: Some(die_state),
        hook_mask: ProcMask::DIE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(die_state, state_marks_update);
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("lethal routed damage should emit hooks");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[1].message, "state mark");
    assert_eq!(frame.updates.updates[1].score, 99);
}

#[test]
fn flush_effects_shares_summon_damage_to_owner_entity() {
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
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("shared damage should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 1);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[0].score, 4);
    assert_eq!(frame.updates.updates[1].target, 0);
    assert_eq!(frame.updates.updates[1].score, 4);
}

#[test]
fn flush_effects_runs_die_hook_on_damage_share_owner() {
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
    let die_state = builder
        .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
        .expect("die state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 4, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 101,
        extension_state_id: Some(die_state),
        hook_mask: ProcMask::DIE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(die_state, state_marks_update);
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
    });
    runtime.flush_effects().expect("spawn should emit update");
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(2),
        amount: 4,
    });

    let frame = runtime.flush_effects().expect("shared lethal damage should emit hooks");

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].target, 0);
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 101);
}

#[test]
fn flush_effects_removes_lethal_damage_target_from_alive_views() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 4,
    });

    runtime.flush_effects().expect("lethal damage should emit update");

    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities), None);
}

#[test]
fn flush_effects_shares_owner_damage_to_alive_summons() {
    let mut builder = ExtensionRegistryBuilder::default();
    let owner_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "owner",
            "custom.owner",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::ShareToSummons,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("owner kind should register");
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
        template: PlayerTemplate::new(3, "summon-a", 0, 5, 1),
    });
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(4, "summon-b", 0, 5, 1),
    });
    runtime.flush_effects().expect("spawns should emit updates");
    runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.alive = false;
    runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.hp = 0;
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(1),
        target: EntityIdx(0),
        amount: 3,
    });

    let frame = runtime.flush_effects().expect("shared summon damage should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 7);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 2);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].target, 0);
    assert_eq!(frame.updates.updates[0].score, 3);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[1].score, 3);
}

#[test]
fn flush_effects_dispatches_custom_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(marker, custom_marks_update);

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        marker,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("custom mark".to_owned()),
    )));

    let frame = runtime.flush_effects().expect("custom handler should emit update");
    assert_eq!(frame.updates.updates[0].message, "custom mark");
}

#[test]
fn flush_effects_exposes_controlled_rng_to_custom_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let rng_handler = builder
        .register_effect_handler("custom", "rng", "custom.rng", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(rng_handler, custom_consumes_rng);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        rng_handler,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(10),
    )));
    let mut expected_rng = RC4::default();
    let expected_value = expected_rng.next_i32(10);
    let expected_byte = expected_rng.next_u8();

    let frame = runtime.flush_effects().expect("custom rng handler should emit update");

    assert_eq!(
        frame.updates.updates[0].message,
        format!("rng:{expected_value}:{expected_byte}")
    );
    assert_eq!(frame.updates.updates[0].score, expected_value as u32);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
}

#[test]
fn flush_effects_runs_nested_custom_effect_before_older_siblings() {
    let mut builder = ExtensionRegistryBuilder::default();
    let nested_damage = builder
        .register_effect_handler("custom", "nested-damage", "custom.nested_damage", SkillPriority(0))
        .expect("handler should register");
    let marker = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(nested_damage, custom_spawns_nested_damage);
    runtime.set_effect_handler(marker, custom_marks_update);

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        nested_damage,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(4),
    )));
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        marker,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("after nested".to_owned()),
    )));

    let frame = runtime.flush_effects().expect("nested damage should emit update");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 6);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "after nested");
}

#[test]
fn flush_effects_applies_heal_without_exceeding_max_hp() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 4;
    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 20,
    });

    let frame = runtime.flush_effects().expect("heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
    assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
}

#[test]
fn flush_effects_readds_healed_dead_target_to_alive_views() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 4,
    });
    runtime.flush_effects().expect("lethal damage should emit update");
    runtime.effects.push(QueuedEffect::Heal {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        amount: 2,
    });

    runtime.flush_effects().expect("heal should emit update");

    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 2);
    assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert_eq!(
        runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities),
        Some(EntityIdx(1))
    );
}

#[test]
fn flush_effects_spawns_entity_and_adds_it_to_round_order() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(3, "summoned", 0, 5, 2),
    });

    let frame = runtime.flush_effects().expect("spawn should emit update");

    assert_eq!(runtime.entities.len(), 3);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, "summoned");
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
    assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
}

#[test]
fn owner_spawn_ignores_blueprint_team_and_does_not_create_enemy() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        ExtensionRegistry::default(),
    ));
    runtime.effects.push(QueuedEffect::Spawn {
        caster: EntityIdx(0),
        template: PlayerTemplate::new(2, "enemy", 1, 8, 4),
    });
    runtime.flush_effects().expect("spawn should emit update");

    let spawned = runtime.entities.get(EntityIdx(1)).expect("spawned entity should exist");
    assert_eq!(spawned.template.team, 0);
    assert_eq!(spawned.runtime.team, 0);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.sync_winner(&runtime.entities), Some(0));
    assert_eq!(
        runtime.scheduler.select_minimal_action(&mut runtime.world, &runtime.entities),
        None
    );
}

#[test]
fn flush_effects_adds_and_clears_state_entries() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    let state = StateEntry {
        legacy_order_key: 77,
        extension_state_id: Some(StateId(1)),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(5),
        registration_order: RegistrationOrder(2),
        payload: StatePayload::None,
    };

    runtime.effects.push(QueuedEffect::AddState {
        target: EntityIdx(1),
        state: state.clone(),
    });
    let add_frame = runtime.flush_effects().expect("add state should emit update");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), Some(&state));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 1);
    assert_eq!(add_frame.updates.updates[0].message, "[1]状态改变");

    runtime.effects.push(QueuedEffect::ClearState {
        target: EntityIdx(1),
        legacy_order_key: 77,
    });
    let clear_frame = runtime.flush_effects().expect("clear state should emit update");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 2);
    assert_eq!(clear_frame.updates.updates[0].message, "[1]状态解除");
}

#[test]
fn flush_effects_runs_nested_heal_before_older_siblings() {
    let mut builder = ExtensionRegistryBuilder::default();
    let nested_heal = builder
        .register_effect_handler("custom", "nested-heal", "custom.nested_heal", SkillPriority(0))
        .expect("handler should register");
    let marker = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 3;
    runtime.set_effect_handler(nested_heal, custom_spawns_nested_heal);
    runtime.set_effect_handler(marker, custom_marks_update);

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        nested_heal,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(4),
    )));
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        marker,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("after heal".to_owned()),
    )));

    let frame = runtime.flush_effects().expect("nested heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[1].message, "after heal");
}

#[test]
fn flush_effects_revives_dead_entity_with_capped_hp() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.world.remove_round_actor(EntityIdx(1));
    let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
    target.runtime.hp = 0;
    target.runtime.alive = false;
    runtime.effects.push(QueuedEffect::Revive {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        hp: 20,
    });

    let frame = runtime.flush_effects().expect("revive should emit update");

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.hp, 10);
    assert!(target.runtime.alive);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(frame.updates.updates[0].message, "[1][复活]了");
}

#[test]
fn flush_effects_removes_entity_from_alive_set() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Remove {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("remove should emit update");

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.hp, 0);
    assert!(!target.runtime.alive);
    assert_eq!(runtime.world.round_order(), &[EntityIdx(0)]);
    assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
    assert_eq!(frame.updates.updates[0].message, "[1]消失了");
}

#[test]
fn flush_effects_emits_replay_effect_without_state_mutation() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Replay {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        message: "custom replay".to_owned(),
        score: 7,
    });

    let frame = runtime.flush_effects().expect("replay should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
    assert_eq!(frame.updates.updates[0].message, "custom replay");
    assert_eq!(frame.updates.updates[0].score, 7);
}

#[test]
fn flush_effects_merges_fixed_lane_skill_loadout() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill_a = builder
        .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
        .expect("skill should register");
    let skill_b = builder
        .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
        .expect("skill should register");
    let skill_c = builder
        .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
        .expect("skill should register");
    let merge_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "merge",
            "custom.merge",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("merge kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)])),
            PlayerTemplate::new(2, "right", 1, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("merge should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[2]);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[0][吞噬]了[1]");
    assert_eq!(frame.updates.updates[1].score, 60);
    assert_eq!(frame.updates.updates[2].message, "[0]属性上升");
    assert_eq!(frame.updates.updates[2].score, 0);
}

#[test]
fn flush_effects_merge_drops_unmapped_skills_when_policy_requires() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill_a = builder
        .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
        .expect("skill should register");
    let skill_b = builder
        .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
        .expect("skill should register");
    let skill_c = builder
        .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
        .expect("skill should register");
    let merge_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "merge",
            "custom.merge",
            PlayerKindFlags::default(),
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::DropUnmappedSkills,
                inherit_owner_def_res: false,
            },
        )
        .expect("merge kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)]).with_fixed_lane_keys([1])),
            PlayerTemplate::new(2, "right", 1, 10, 3)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
        ],
        registry,
    ));
    runtime.effects.push(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    runtime.flush_effects().expect("merge should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[3]);
}

#[test]
fn flush_effects_panics_on_unknown_damage_target() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        amount: 1,
    });

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(result.is_err());
}

fn assert_effect_panics(effect: QueuedEffect) {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
    runtime.effects.push(effect);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(result.is_err());
}

fn dummy_state_entry() -> StateEntry {
    StateEntry {
        legacy_order_key: 999,
        extension_state_id: None,
        hook_mask: ProcMask::NONE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    }
}

#[test]
fn flush_effects_panics_on_unknown_effect_entities() {
    assert_effect_panics(QueuedEffect::Damage {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        amount: 1,
    });
    assert_effect_panics(QueuedEffect::SummonExplode {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        fire_state_key: 91,
    });
    assert_effect_panics(QueuedEffect::SummonExplode {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        fire_state_key: 91,
    });
    assert_effect_panics(QueuedEffect::DisperseAttack {
        caster: EntityIdx(99),
        target: EntityIdx(1),
    });
    assert_effect_panics(QueuedEffect::DisperseAttack {
        caster: EntityIdx(0),
        target: EntityIdx(99),
    });
    assert_effect_panics(QueuedEffect::DisperseHit {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        damage: 1,
    });
    assert_effect_panics(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        damage: 1,
    });
    assert_effect_panics(QueuedEffect::Heal {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        amount: 1,
    });
    assert_effect_panics(QueuedEffect::Heal {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        amount: 1,
    });
    assert_effect_panics(QueuedEffect::Spawn {
        caster: EntityIdx(99),
        template: PlayerTemplate::new(3, "ghost", 1, 1, 0),
    });
    assert_effect_panics(QueuedEffect::AddState {
        target: EntityIdx(99),
        state: dummy_state_entry(),
    });
    assert_effect_panics(QueuedEffect::ClearState {
        target: EntityIdx(99),
        legacy_order_key: 999,
    });
    assert_effect_panics(QueuedEffect::Revive {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        hp: 1,
    });
    assert_effect_panics(QueuedEffect::Revive {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        hp: 1,
    });
    assert_effect_panics(QueuedEffect::Remove {
        caster: EntityIdx(99),
        target: EntityIdx(1),
    });
    assert_effect_panics(QueuedEffect::Remove {
        caster: EntityIdx(0),
        target: EntityIdx(99),
    });
    assert_effect_panics(QueuedEffect::Merge {
        caster: EntityIdx(99),
        target: EntityIdx(1),
    });
    assert_effect_panics(QueuedEffect::Merge {
        caster: EntityIdx(0),
        target: EntityIdx(99),
    });
    assert_effect_panics(QueuedEffect::Replay {
        caster: EntityIdx(99),
        target: EntityIdx(1),
        message: "bad caster".to_owned(),
        score: 0,
    });
    assert_effect_panics(QueuedEffect::Replay {
        caster: EntityIdx(0),
        target: EntityIdx(99),
        message: "bad target".to_owned(),
        score: 0,
    });
}

#[test]
fn flush_effects_panics_on_unknown_custom_effect_entities() {
    let mut builder = ExtensionRegistryBuilder::default();
    let handler = builder
        .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(handler, custom_marks_update);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        handler,
        EntityIdx(99),
        Some(EntityIdx(1)),
        CustomEffectPayload::Text("bad caster".to_owned()),
    )));

    let bad_caster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(bad_caster.is_err());

    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        handler,
        EntityIdx(0),
        Some(EntityIdx(99)),
        CustomEffectPayload::Text("bad target".to_owned()),
    )));

    let bad_target = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

    assert!(bad_target.is_err());
}

#[test]
fn custom_context_restricts_cross_entity_reads_by_capability() {
    let mut builder = ExtensionRegistryBuilder::default();
    let reader = builder
        .register_effect_handler("custom", "reader", "custom.reader", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
            PlayerTemplate::new(3, "third", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler(reader, custom_rejects_cross_entity_read);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        reader,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::None,
    )));
    let denied = runtime.flush_effects().expect("denied read handler should emit update");
    assert_eq!(denied.updates.updates[0].message, "read denied");

    runtime.set_effect_handler_with_capabilities(reader, custom_reads_cross_entity, &[ExtensionCapability::ReadEnemies]);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        reader,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::None,
    )));
    let allowed = runtime.flush_effects().expect("allowed read handler should emit update");
    assert_eq!(allowed.updates.updates[0].message, "third");
}

#[test]
fn custom_context_requires_capability_for_entity_slot_mutation() {
    let mut builder = ExtensionRegistryBuilder::default();
    let slot = builder
        .reserve_entity_slot("custom", "flag", "custom.flag")
        .expect("entity slot should reserve");
    let mutator = builder
        .register_effect_handler("custom", "mutator", "custom.mutator", SkillPriority(0))
        .expect("handler should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_effect_handler_with_capabilities(mutator, custom_mutates_entity_slot, &[ExtensionCapability::MutateEntitySlots]);
    runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
        mutator,
        EntityIdx(0),
        Some(EntityIdx(1)),
        CustomEffectPayload::Int(slot.0 as i32),
    )));

    let frame = runtime.flush_effects().expect("slot mutation should emit update");

    assert_eq!(frame.updates.updates[0].message, "slot set");
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().slots.get(slot),
        Some(&SlotValue::Bool(true))
    );
}

use super::*;

fn kill_rng_runtime(players: Vec<PlayerTemplate>) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let kill_skill = builder
        .register_skill_with_hooks(
            "custom",
            "kill-rng",
            "custom.kill_rng",
            ProcMask::KILL,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("kill rng skill should register");
    let registry = builder.build();
    let mut players = players;
    players[0].skills = SkillLoadout::from_skill_levels([(kill_skill, 1)]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(players, registry));
    runtime.set_skill_handler(kill_skill, skill_consumes_rng);
    runtime
}

fn queue_lethal_damage(runtime: &mut CombatRuntime, target: EntityIdx) {
    runtime.effects.push(QueuedEffect::Damage {
        caster: EntityIdx(0),
        target,
        amount: 3,
    });
}

fn zombie_merge_kill_runtime(zombie_level: u32, target_zombie_level: u32) -> CombatRuntime {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let zombie = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_ZOMBIE_SKILL_EXPORT)
        .expect("default profile should register zombie");
    let merge = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_MERGE_SKILL_EXPORT)
        .expect("default profile should register merge");
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "killer", 0, 100, 10)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(zombie, zombie_level), (merge, 64)])),
            PlayerTemplate::new(2, "victim", 1, 1, 10)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(zombie, target_zombie_level), (merge, 1)])),
            PlayerTemplate::new(3, "enemy", 1, 100, 10),
        ],
        config.registry,
    ))
}

fn zombie_first_kill_plan(runtime: &CombatRuntime) -> SkillHookPlan {
    let mut plan = runtime
        .scheduler
        .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(0), ProcMask::KILL);
    plan.entries.sort_by_key(|entry| {
        let export_name = runtime.registry.skill(entry.skill_id).unwrap().export_name.as_str();
        usize::from(export_name != DEFAULT_CORE_ZOMBIE_SKILL_EXPORT)
    });
    plan
}

#[test]
fn zombie_trigger_stops_merge_kill_hook_before_second_rng_roll() {
    let mut runtime = zombie_merge_kill_runtime(64, 64);
    let mut expected_rng = runtime.rng.clone();
    assert!(expected_rng.r63() < 64);
    let plan = zombie_first_kill_plan(&runtime);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_kill_skill_plan_into(&plan, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.corpse,
        RuntimeCorpseKind::Zombie
    );
}

#[test]
fn zombie_failure_allows_merge_kill_hook_to_consume_next_rng_roll() {
    let mut runtime = zombie_merge_kill_runtime(1, 64);
    while {
        let mut probe = runtime.rng.clone();
        probe.r63() < 1
    } {
        runtime.rng.next_u8();
    }
    let mut expected_rng = runtime.rng.clone();
    assert!(expected_rng.r63() >= 1);
    assert!(expected_rng.r63() < 64);
    let plan = zombie_first_kill_plan(&runtime);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_kill_skill_plan_into(&plan, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.corpse,
        RuntimeCorpseKind::Merge
    );
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["\n", "[0][吞噬]了[1]", "[0]属性上升"]
    );
}

#[test]
fn terminal_kill_skips_kill_hooks_without_consuming_rng() {
    let mut runtime = kill_rng_runtime(vec![
        PlayerTemplate::new(1, "killer", 0, 10, 3),
        PlayerTemplate::new(2, "last-enemy", 1, 3, 3),
    ]);
    let expected_rng = runtime.rng.clone();
    queue_lethal_damage(&mut runtime, EntityIdx(1));

    let frame = runtime.flush_effects().expect("lethal damage should emit replay");

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]攻击[1]"]
    );
}

#[test]
fn kill_hooks_run_while_another_enemy_is_alive() {
    let mut runtime = kill_rng_runtime(vec![
        PlayerTemplate::new(1, "killer", 0, 10, 3),
        PlayerTemplate::new(2, "victim", 1, 3, 3),
        PlayerTemplate::new(3, "enemy", 1, 10, 3),
    ]);
    let expected_rng = runtime.rng.clone();
    queue_lethal_damage(&mut runtime, EntityIdx(1));

    let frame = runtime.flush_effects().expect("lethal damage should emit kill hook");

    assert_ne!((runtime.rng.i, runtime.rng.j), (expected_rng.i, expected_rng.j));
    assert!(frame.updates.updates.iter().any(|update| update.message.starts_with("skill-rng:")));
}

#[test]
fn frozen_enemy_still_allows_kill_hooks() {
    let mut runtime = kill_rng_runtime(vec![
        PlayerTemplate::new(1, "killer", 0, 10, 3),
        PlayerTemplate::new(2, "victim", 1, 3, 3),
        PlayerTemplate::new(3, "frozen-enemy", 1, 10, 3),
    ]);
    runtime
        .entities
        .get_mut(EntityIdx(2))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().is_active());
    queue_lethal_damage(&mut runtime, EntityIdx(1));

    let frame = runtime.flush_effects().expect("frozen enemy should keep kill hooks active");

    assert!(frame.updates.updates.iter().any(|update| update.message.starts_with("skill-rng:")));
}

#[test]
fn pending_enemy_spawn_allows_kill_hooks() {
    let mut runtime = kill_rng_runtime(vec![
        PlayerTemplate::new(1, "killer", 0, 10, 3),
        PlayerTemplate::new(2, "last-visible-enemy", 1, 3, 3),
    ]);
    queue_lethal_damage(&mut runtime, EntityIdx(1));
    runtime.effects.push(QueuedEffect::SpawnSilent {
        caster: EntityIdx(1),
        template: PlayerTemplate::new(3, "pending-enemy", 0, 10, 3),
    });

    let frame = runtime.flush_effects().expect("pending enemy spawn should keep kill hooks active");

    assert!(frame.updates.updates.iter().any(|update| update.message.starts_with("skill-rng:")));
    let spawned = runtime.entities.get(EntityIdx(2)).expect("pending enemy should spawn");
    assert_eq!(spawned.runtime.team, 1);
    assert!(spawned.runtime.alive);
}

#[test]
fn run_minimal_round_dispatches_die_and_kill_state_hooks_while_enemy_remains() {
    let mut builder = ExtensionRegistryBuilder::default();
    let die_state = builder
        .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
        .expect("die state should register");
    let kill_state = builder
        .register_state("custom", "kill", "custom.kill", ProcMask::KILL, SkillPriority(0))
        .expect("kill state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 3, 3),
            PlayerTemplate::new(3, "right-ally", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 11,
        extension_state_id: Some(kill_state),
        hook_mask: ProcMask::KILL,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 22,
        extension_state_id: Some(die_state),
        hook_mask: ProcMask::DIE,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(1),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(die_state, state_marks_update);
    runtime.set_state_handler(kill_state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("lethal attack should emit hooks");

    assert_eq!(outcome.winner_team, None);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "state mark");
    assert_eq!(frame.updates.updates[1].score, 22);
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 11);
}

#[test]
fn flush_effects_dispatches_die_and_kill_skill_hooks_while_enemy_remains() {
    let mut builder = ExtensionRegistryBuilder::default();
    let die_skill = builder
        .register_skill_with_hooks(
            "custom",
            "die-skill",
            "custom.die_skill",
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("die skill should register");
    let kill_skill = builder
        .register_skill_with_hooks(
            "custom",
            "kill-skill",
            "custom.kill_skill",
            ProcMask::KILL,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("kill skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([kill_skill]),
            PlayerTemplate::new(2, "right", 1, 3, 3).with_skills([die_skill]),
            PlayerTemplate::new(3, "right-ally", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(die_skill, skill_marks_update);
    runtime.set_skill_handler(kill_skill, skill_marks_update);
    queue_lethal_damage(&mut runtime, EntityIdx(1));

    let frame = runtime.flush_effects().expect("lethal damage should emit hooks");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "skill mark");
    assert_eq!(frame.updates.updates[1].score, die_skill.0);
    assert_eq!(frame.updates.updates[2].message, "skill mark");
    assert_eq!(frame.updates.updates[2].score, kill_skill.0);
}

#[test]
fn flush_effects_passes_killed_target_to_kill_skill_hooks() {
    let mut builder = ExtensionRegistryBuilder::default();
    let kill_skill = builder
        .register_skill_with_hooks(
            "custom",
            "kill-skill",
            "custom.kill_skill",
            ProcMask::KILL,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("kill skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([kill_skill]),
            PlayerTemplate::new(2, "first-target", 1, 10, 3),
            PlayerTemplate::new(3, "killed-target", 1, 3, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(kill_skill, skill_marks_selected_target);
    queue_lethal_damage(&mut runtime, EntityIdx(2));

    let frame = runtime.flush_effects().expect("lethal damage should emit kill hook");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].target, 2);
    assert_eq!(frame.updates.updates[1].message, "selected target");
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[1].score, 2);
}

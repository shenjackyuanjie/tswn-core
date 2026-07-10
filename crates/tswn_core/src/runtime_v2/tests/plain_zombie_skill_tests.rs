use super::*;

fn zombie_runtime(level: u32, target_kind: PlayerKindId, with_blueprint: bool) -> CombatRuntime {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let registry = config.registry;
    let zombie_skill = registry
        .skill_id_by_export_name(DEFAULT_CORE_ZOMBIE_SKILL_EXPORT)
        .expect("default profile should register core zombie skill");
    let zombie_kind = registry
        .player_kind_id_by_export_name(DEFAULT_CORE_ZOMBIE_KIND_EXPORT)
        .expect("default profile should register core zombie kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 10)
                .with_magic_point(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(zombie_skill, level)])),
            PlayerTemplate::with_kind(2, "target", target_kind, 1, 100, 10),
        ],
        registry,
    ));
    if with_blueprint {
        let blueprint_slot = runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT)
            .expect("default profile should reserve core zombie blueprint slot");
        let blueprint = PlayerTemplate::with_kind(0, "zombie-blueprint", zombie_kind, 0, 24, 0)
            .with_display_name("丧尸")
            .with_reserved_player_ids_before_spawn(1);
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(blueprint_slot, SlotValue::PlayerTemplate(Box::new(blueprint)))
            .expect("zombie blueprint slot should accept template");
    }
    runtime
}

#[test]
fn plain_zombie_skips_combat_minions_without_consuming_rng() {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let minion_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_ZOMBIE_KIND_EXPORT)
        .expect("default profile should register core zombie kind");
    let mut runtime = zombie_runtime(64, minion_kind, true);
    let expected_rng = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_zombie_kill_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.len(), 2);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.corpse,
        RuntimeCorpseKind::None
    );
    assert!(updates.updates.is_empty());
}

#[test]
fn plain_zombie_probability_failure_consumes_only_r63() {
    let mut runtime = zombie_runtime(1, PlayerTemplate::DEFAULT_KIND, true);
    while {
        let mut probe = runtime.rng.clone();
        probe.r63() < 1
    } {
        runtime.rng.next_u8();
    }
    let mut expected_rng = runtime.rng.clone();
    let roll = expected_rng.r63();
    assert!(roll >= 1);
    let mp_before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_zombie_kill_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point, mp_before);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.corpse,
        RuntimeCorpseKind::None
    );
    assert_eq!(runtime.entities.len(), 2);
    assert!(updates.updates.is_empty());
}

#[test]
fn plain_zombie_marks_corpse_spawns_after_reserved_id_and_emits_legacy_updates() {
    let mut runtime = zombie_runtime(64, PlayerTemplate::DEFAULT_KIND, true);
    let mut expected_rng = runtime.rng.clone();
    assert!(expected_rng.r63() < 64);
    let required_mp = expected_rng.r3x3() as i32;
    let expected_move_points = expected_rng.r255() as i32 * 4;
    let mp_before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_zombie_kill_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point,
        mp_before - required_mp
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.corpse,
        RuntimeCorpseKind::Zombie
    );
    assert_eq!(runtime.entities.len(), 4);
    assert!(runtime.entities.get(EntityIdx(2)).is_none());
    let zombie = runtime.entities.get(EntityIdx(3)).expect("zombie should spawn after reserved id");
    assert_eq!(zombie.template.id, 4);
    assert_eq!(zombie.template.name, "owner?0");
    assert_eq!(zombie.template.display_name, "丧尸");
    assert_eq!(zombie.runtime.owner, EntityIdx(0));
    assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
    assert_eq!(zombie.runtime.move_state.speed_points, expected_move_points);
    assert!(zombie.runtime.is_combat_minion());
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["\n", "[0][召唤亡灵]", "[2]变成了[1]"]
    );
    assert_eq!(updates.updates[1].delay0, 1500);
    assert_eq!(updates.updates[1].target, 1);
    assert_eq!(updates.updates[2].target, 3);
    assert_eq!(updates.updates[2].targets.as_slice(), &[1]);
}

#[test]
fn plain_zombie_without_spawnable_owner_still_marks_the_corpse() {
    let mut runtime = zombie_runtime(64, PlayerTemplate::DEFAULT_KIND, false);
    let mut expected_rng = runtime.rng.clone();
    assert!(expected_rng.r63() < 64);
    let mp_before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_zombie_kill_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point, mp_before);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.corpse,
        RuntimeCorpseKind::Zombie
    );
    assert_eq!(runtime.entities.len(), 2);
    assert!(updates.updates.is_empty());
}

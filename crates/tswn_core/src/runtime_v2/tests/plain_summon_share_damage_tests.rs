use super::*;

#[test]
fn summon_share_damage_emits_owner_death_replay_without_removing_the_active_summon() {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 1),
            PlayerTemplate::new(2, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 50, EntityIdx(1), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.runtime.hp, 0);
    assert!(!owner.runtime.alive);
    assert!(runtime.entities.get(summoned).unwrap().runtime.alive);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了"]
    );
    assert_eq!(updates.updates[2].caster, 1);
    assert_eq!(updates.updates[2].target, 0);
    assert_eq!(updates.updates[2].score, 50);
}

#[test]
fn summon_share_damage_reraise_keeps_owner_in_round_order() {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let reraise = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT)
        .expect("default profile should register reraise");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 1).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
            PlayerTemplate::new(2, "caster", 1, 100, 1),
            PlayerTemplate::new(3, "tail", 1, 100, 1),
        ],
        config.registry,
    ));
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(0)));
    let before_order = runtime.world.round_order().to_vec();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 50, EntityIdx(1), &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.alive);
    assert!(owner.runtime.hp > 0);
    assert_eq!(runtime.world.round_order(), before_order.as_slice());
    assert!(runtime.entities.get(summoned).unwrap().runtime.alive);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(1)));
}

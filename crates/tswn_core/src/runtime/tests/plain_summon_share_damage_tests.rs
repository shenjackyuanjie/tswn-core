use super::*;

#[test]
fn summon_share_damage_owner_death_marks_active_summon_for_outer_lethal_chain() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
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
    let summoned_entity = runtime.entities.get(summoned).unwrap();
    assert_eq!(summoned_entity.runtime.hp, 0);
    assert!(summoned_entity.runtime.alive);
    assert!(runtime.world.round_order().contains(&summoned));
    assert!(runtime.world.flat_alive().contains(&summoned));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了"]
    );
    assert_eq!(updates.updates[2].caster, 1);
    assert_eq!(updates.updates[2].target, 0);
    assert_eq!(updates.updates[2].score, 50);
}

#[test]
fn half_skill_finishes_active_summon_lethal_chain_after_owner_share_death() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let summon_share = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT)
        .expect("default profile should register summon share damage");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 20, 1),
            PlayerTemplate::new(2, "caster", 1, 100, 1).with_magic(100),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().activate_charge_runtime();
    let summoned = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 100, 0)
            .with_skill_loadout(SkillLoadout::from_skill_levels([(summon_share, 1)])),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );
    let summon_team = runtime.entities.get(summoned).unwrap().runtime.team;
    runtime.world.add_spawned_alive(summoned, summon_team);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_half_skill_into(EntityIdx(1), summoned, &mut updates);

    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    let summoned_entity = runtime.entities.get(summoned).unwrap();
    assert_eq!(summoned_entity.runtime.hp, 0);
    assert!(!summoned_entity.runtime.alive);
    assert!(!runtime.world.round_order().contains(&summoned));
    assert!(!runtime.world.flat_alive().contains(&summoned));
    assert_eq!(runtime.world.alive_group_count(), 1);
    assert!(
        updates
            .updates
            .iter()
            .any(|update| update.target == summoned.0 as usize && update.message == "[1]消失了")
    );
}

#[test]
fn summon_share_damage_hide_counts_zero_hp_summon_before_outer_lethal_chain() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let hide = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT)
        .expect("default profile should register hide skill");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 100, 1).with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 20)])),
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
    runtime.entities.get_mut(summoned).unwrap().runtime.hp = 0;
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r63();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(summoned, 1, 10, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn summon_share_damage_owner_death_removes_root_owned_sibling_minion_when_direct_owner_is_dead() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "root", 0, 100, 1),
            PlayerTemplate::new(2, "owner", 0, 20, 1),
            PlayerTemplate::new(3, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    let active_summon = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(1)),
        Some(EntityIdx(0)),
    );
    let sibling = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "root?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(1)),
    );
    let active_team = runtime.entities.get(active_summon).unwrap().runtime.team;
    let sibling_team = runtime.entities.get(sibling).unwrap().runtime.team;
    runtime.world.add_spawned_alive(active_summon, active_team);
    runtime.world.add_spawned_alive(sibling, sibling_team);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
    runtime.world.mark_dead(EntityIdx(0), 0);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(active_summon, 1, 50, EntityIdx(2), &mut updates);

    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    let active_summon_entity = runtime.entities.get(active_summon).unwrap();
    assert_eq!(active_summon_entity.runtime.hp, 0);
    assert!(active_summon_entity.runtime.alive);
    assert!(!runtime.entities.get(sibling).unwrap().runtime.alive);
    assert!(runtime.world.round_order().contains(&active_summon));
    assert!(!runtime.world.round_order().contains(&sibling));
    assert!(runtime.world.flat_alive().contains(&active_summon));
    assert!(!runtime.world.flat_alive().contains(&sibling));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了", "\n", "[1]消失了"]
    );
    assert_eq!(updates.updates[4].caster, 1);
    assert_eq!(updates.updates[4].target, sibling.0 as usize);
    assert_eq!(updates.updates[4].score, 50);
}

#[test]
fn summon_share_damage_owner_death_removes_direct_shadow_but_keeps_root_owned_shadow() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let summon_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
        .expect("default profile should register core summon kind");
    let shadow_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
        .expect("default profile should register core shadow kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "root", 0, 100, 1),
            PlayerTemplate::new(2, "owner", 0, 20, 1),
            PlayerTemplate::new(3, "caster", 1, 100, 1),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    let active_summon = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?0", summon_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(1)),
        Some(EntityIdx(0)),
    );
    let sibling_shadow = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "root?shadow", shadow_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(1)),
    );
    let direct_shadow = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(0, "owner?shadow", shadow_kind, 0, 40, 0),
        &runtime.registry,
        Some(EntityIdx(1)),
        Some(EntityIdx(0)),
    );
    let active_team = runtime.entities.get(active_summon).unwrap().runtime.team;
    let shadow_team = runtime.entities.get(sibling_shadow).unwrap().runtime.team;
    let direct_shadow_team = runtime.entities.get(direct_shadow).unwrap().runtime.team;
    runtime.world.add_spawned_alive(active_summon, active_team);
    runtime.world.add_spawned_alive(sibling_shadow, shadow_team);
    runtime.world.add_spawned_alive(direct_shadow, direct_shadow_team);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_summon_share_damage_into(active_summon, 1, 50, EntityIdx(2), &mut updates);

    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert!(runtime.entities.get(sibling_shadow).unwrap().runtime.alive);
    assert!(runtime.world.round_order().contains(&sibling_shadow));
    assert!(runtime.world.flat_alive().contains(&sibling_shadow));
    assert!(!runtime.entities.get(direct_shadow).unwrap().runtime.alive);
    assert!(!runtime.world.round_order().contains(&direct_shadow));
    assert!(!runtime.world.flat_alive().contains(&direct_shadow));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "\n", "[1]被击倒了", "\n", "[1]消失了"]
    );
    assert_eq!(updates.updates[4].target, direct_shadow.0 as usize);
}

#[test]
fn summon_share_damage_reraise_keeps_owner_in_round_order() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
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

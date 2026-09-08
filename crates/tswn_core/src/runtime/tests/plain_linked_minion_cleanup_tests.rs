use super::*;

#[test]
fn owner_death_removes_linked_minion_before_owner_for_round_cursor() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "linked-minion",
            "custom.linked_minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("linked minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 1),
            PlayerTemplate::new(2, "current", 1, 10, 1),
            PlayerTemplate::new(3, "next", 2, 10, 1),
        ],
        registry,
    ));
    let owner = EntityIdx(0);
    let current = EntityIdx(1);
    let next = EntityIdx(2);
    let minion = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(4, "owner?0", minion_kind, 0, 5, 1),
        &runtime.registry,
        Some(owner),
        Some(owner),
    );
    runtime.world.add_spawned_alive(minion, 0);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(owner));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(current));

    let mut updates = RunUpdates::new();
    assert!(runtime.kill_entity_without_damage_into(owner, &mut updates));

    assert_eq!(runtime.world.round_order(), &[current, next]);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(current));
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["\n", "[1]消失了"]
    );
}

#[test]
fn plain_fire_lethal_owner_replays_knockout_before_linked_minion_cleanup() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "linked-minion",
            "custom.linked_minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::SelfEntity,
                damage_share: DamageSharePolicy::None,
                merge: MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("linked minion kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(1_000_000),
            PlayerTemplate::new(2, "owner", 1, 3, 3).with_def_res(0, 0),
        ],
        builder.build(),
    ));
    let owner = EntityIdx(1);
    let minion = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(3, "owner?0", minion_kind, 1, 5, 1),
        &runtime.registry,
        Some(owner),
        Some(owner),
    );
    runtime.world.add_spawned_alive(minion, 1);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_fire_skill_into(EntityIdx(0), owner, &mut updates);

    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[火球术]", "[1]受到[2]点伤害", "\n", "[1]被击倒了", "\n", "[1]消失了",]
    );
    assert!(!runtime.entities.get(owner).unwrap().runtime.alive);
    assert!(!runtime.entities.get(minion).unwrap().runtime.alive);
}

#[test]
fn owner_death_does_not_cleanup_root_owned_shadow_for_child_owner() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let shadow_kind = config
        .registry
        .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
        .expect("default profile should register core shadow kind");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "root", 0, 100, 1),
            PlayerTemplate::new(2, "child", 0, 10, 1),
            PlayerTemplate::new(3, "tail", 1, 10, 1),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
    let shadow = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(4, "root?shadow", shadow_kind, 0, 5, 1),
        &runtime.registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(1)),
    );
    runtime.world.add_spawned_alive(shadow, 0);
    let mut updates = RunUpdates::new();

    assert!(runtime.kill_entity_without_damage_into(EntityIdx(1), &mut updates));

    assert!(runtime.entities.get(shadow).unwrap().runtime.alive);
    assert!(runtime.world.round_order().contains(&shadow));
    assert_eq!(updates.updates.len(), 0);
}

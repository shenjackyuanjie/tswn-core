use super::*;

fn protect_runtime(target_kind_export: &str) -> CombatRuntime {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let protect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        .expect("default profile should register protect skill");
    let target_kind = config
        .registry
        .player_kind_id_by_export_name(target_kind_export)
        .expect("default profile should register target kind");
    let CustomRuntimeV2ImportConfig {
        registry,
        skill_handlers,
        ..
    } = config;
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "protector", 0, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(protect, 64)])),
            PlayerTemplate::with_kind(2, "ally", target_kind, 0, 100, 3),
            PlayerTemplate::new(3, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    for binding in skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    runtime
}

#[test]
fn plain_protect_can_target_clone_and_preserves_legacy_rng_consumption() {
    let mut runtime = protect_runtime(DEFAULT_CORE_CLONE_KIND_EXPORT);
    let candidates = runtime.world.team_alive(0).unwrap().to_vec();
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r127();
    for _ in 0..4 {
        assert_eq!(expected_rng.pick_skip(&candidates, 0), Some(1));
    }
    expected_rng.rFFFF();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_protect_post_action_into(EntityIdx(0), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.protect_to,
        Some(EntityIdx(1))
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.protect_from,
        vec![ProtectLinkRuntime {
            owner: EntityIdx(0),
            level: 64,
        }]
    );
    assert!(updates.updates.is_empty());
}

#[test]
fn plain_protect_rejects_combat_minion_after_legacy_retry_budget() {
    let mut runtime = protect_runtime(DEFAULT_CORE_SUMMON_KIND_EXPORT);
    let candidates = runtime.world.team_alive(0).unwrap().to_vec();
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r127();
    for _ in 0..5 {
        assert_eq!(expected_rng.pick_skip(&candidates, 0), Some(1));
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_protect_post_action_into(EntityIdx(0), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.protect_to, None);
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.protect_from.is_empty());
    assert!(updates.updates.is_empty());
}

use super::*;
use crate::runtime::combat::PlainAttackOnDamage;

fn protect_runtime(target_kind_export: &str) -> CombatRuntime {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let protect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        .expect("default profile should register protect skill");
    let target_kind = config
        .registry
        .player_kind_id_by_export_name(target_kind_export)
        .expect("default profile should register target kind");
    let CustomRuntimeImportConfig {
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
fn deferred_plain_protect_keeps_expiring_charm_team_until_state_chain_tail() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let protect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        .expect("default profile should register protect skill");
    let charm = config
        .registry
        .state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT)
        .expect("default profile should register charm state");
    let CustomRuntimeImportConfig {
        registry,
        skill_handlers,
        state_handlers,
        ..
    } = config;
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "protector", 0, 100, 3)
                .with_wisdom(256)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(protect, 64)])),
            PlayerTemplate::new(2, "original ally", 0, 100, 3),
            PlayerTemplate::new(3, "charm ally", 1, 100, 3),
        ],
        registry,
    ));
    for binding in skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    for binding in state_handlers {
        runtime.set_state_handler_with_capabilities(binding.state_id, binding.handler, &binding.capabilities);
    }
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        assert!(owner.states.add_entry(StateEntry::charm(
            76,
            charm,
            2,
            Some(1),
            Some(1),
            Some(2),
            1,
            SkillPriority(210),
        )));
        let state_cursor = owner.states.post_action_registration_cursor();
        owner.template.skills.register_post_action_after_states(0, state_cursor);
    }

    let outcome = runtime.run_minimal_round();

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.protect_to,
        Some(EntityIdx(2))
    );
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76).is_none());
    assert!(
        outcome
            .frame
            .unwrap()
            .updates
            .updates
            .iter()
            .any(|update| update.message == "[1]从[魅惑]中解除")
    );
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

#[test]
fn plain_protect_keeps_zero_hp_alive_combat_minion_in_legacy_retry_candidates() {
    let mut runtime = protect_runtime(DEFAULT_CORE_SUMMON_KIND_EXPORT);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;
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

#[test]
fn plain_protect_freezes_existing_pre_defend_skill_count_on_first_link() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let protect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        .expect("default profile should register protect skill");
    let reflect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_REFLECT_SKILL_EXPORT)
        .expect("default profile should register reflect skill");
    let CustomRuntimeImportConfig {
        registry,
        skill_handlers,
        ..
    } = config;
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "protector", 0, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(protect, 64)])),
            PlayerTemplate::new(2, "ally", 0, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 64)])),
            PlayerTemplate::new(3, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    for binding in skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_protect_post_action_into(EntityIdx(0), &mut updates);

    let ally = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(ally.runtime.protect_pre_defend_skill_count, Some(1));
    assert_eq!(
        ally.runtime.protect_from,
        vec![ProtectLinkRuntime {
            owner: EntityIdx(0),
            level: 64,
        }]
    );
}

#[test]
fn plain_protect_redirect_uses_magic_resistance_for_magic_attacks() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let protect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        .expect("default profile should register protect skill");
    let CustomRuntimeImportConfig {
        registry,
        skill_handlers,
        ..
    } = config;
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "protector", 0, 100, 100)
                .with_def_res(0, 1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(protect, 256)])),
            PlayerTemplate::new(2, "ally", 0, 100, 100),
            PlayerTemplate::new(3, "caster", 1, 100, 100),
        ],
        registry,
    ));
    for binding in skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    let mut updates = RunUpdates::new();
    runtime.drain_plain_protect_post_action_into(EntityIdx(0), &mut updates);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.protect_to,
        Some(EntityIdx(1))
    );
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.magic_point = 100;
    let mut defend_value = RuntimeDefendValue::Atp {
        value: 5_000.0,
        caster: EntityIdx(2),
        target: EntityIdx(1),
        is_magic: true,
    };

    runtime.drain_pre_defend_hooks_into(EntityIdx(1), &mut updates, &mut defend_value);

    assert_eq!(defend_value.atp(), Some(0.0));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 98);
}

#[test]
fn plain_protect_redirect_preserves_absorb_on_damage_heal() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let protect = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        .expect("default profile should register protect skill");
    let CustomRuntimeImportConfig {
        registry,
        skill_handlers,
        ..
    } = config;
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "protector", 0, 100, 100)
                .with_def_res(0, 0)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(protect, 256)])),
            PlayerTemplate::new(2, "ally", 0, 100, 100),
            PlayerTemplate::new(3, "absorber", 1, 100, 100),
        ],
        registry,
    ));
    for binding in skill_handlers {
        runtime.set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
    }
    runtime.entities.get_mut(EntityIdx(2)).unwrap().runtime.hp = 20;
    let mut updates = RunUpdates::new();
    runtime.drain_plain_protect_post_action_into(EntityIdx(0), &mut updates);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.magic_point = 100;

    let amount = runtime.drain_plain_attack_with_atp_and_on_damage_into(
        EntityIdx(2),
        EntityIdx(1),
        true,
        4_352.0,
        PlainAttackOnDamage::Absorb,
        &mut updates,
    );

    assert_eq!(amount, 0);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 66);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 37);
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[0][守护][1]", 40), ("[1]受到[2]点伤害", 34), ("[1]回复体力[2]点", 17)]
    );
}

#[test]
fn nested_plain_protect_redirect_preserves_poison_on_damage() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "target", 0, 1_000, 100),
            PlayerTemplate::new(2, "first protector", 0, 1_000, 100),
            PlayerTemplate::new(3, "second protector", 0, 1_000, 100),
            PlayerTemplate::new(4, "poisoner", 1, 1_000, 100).with_magic(100),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.magic_point = 100;
    runtime.entities.get_mut(EntityIdx(2)).unwrap().runtime.magic_point = 100;
    {
        let target = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime;
        target.protect_from.push(ProtectLinkRuntime {
            owner: EntityIdx(1),
            level: 256,
        });
        target.protect_pre_defend_skill_count = Some(0);
    }
    {
        let first = &mut runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime;
        first.protect_from.push(ProtectLinkRuntime {
            owner: EntityIdx(2),
            level: 256,
        });
        first.protect_pre_defend_skill_count = Some(0);
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_attack_with_atp_and_on_damage_into(
        EntityIdx(3),
        EntityIdx(0),
        true,
        10_000.0,
        PlainAttackOnDamage::Poison,
        &mut updates,
    );

    assert!(runtime.entities.get(EntityIdx(2)).unwrap().states.entry(PLAIN_POISON_STATE_KEY).is_some());
    assert!(updates.updates.iter().any(|update| update.message == "[1][中毒]"));
}

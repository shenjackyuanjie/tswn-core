use super::*;

#[test]
fn plain_disperse_uses_builtin_static_dispatch_without_handler() {
    let mut builder = ExtensionRegistryBuilder::default();
    let disperse = builder
        .register_skill(
            "core",
            "disperse",
            BuiltinActiveSkill::Disperse.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(17),
        )
        .expect("disperse skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(1_000)
                .with_wisdom(128)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(disperse, 128)])),
            PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16),
        ],
        registry,
    ));

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("disperse should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Disperse);
    assert_eq!(prepared.targets.as_slice(), &[EntityIdx(1)]);

    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[净化]");
}

#[test]
fn plain_disperse_skips_pre_defend_and_dodge_rng() {
    let mut builder = ExtensionRegistryBuilder::default();
    let reflect = builder
        .register_skill_with_hooks(
            "core",
            "reflect",
            DEFAULT_CORE_REFLECT_SKILL_EXPORT,
            ProcMask::PRE_DEFEND,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("reflect skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(1_000),
            PlayerTemplate::new(2, "target", 1, 1_000, 3)
                .with_def_res(0, 10_000)
                .with_agility(10_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 256)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(reflect, run_reflect_pre_defend_skill);

    let caster_hp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp;
    let target_hp = runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp;
    let mut expected_rng = runtime.rng.clone();
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    let damage = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_disperse_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, caster_hp);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, target_hp - damage);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[净化]", "[1]受到[2]点伤害"]
    );
}

#[test]
fn plain_disperse_damage_then_clears_positive_state_and_magic_point() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(80),
            PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16).with_magic_point(96),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    runtime.effects.push(QueuedEffect::DisperseAttack {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("disperse attack should emit updates");
    let target = runtime.entities.get(EntityIdx(1)).unwrap();

    assert_eq!(target.runtime.hp, 1_000 - expected_amount);
    assert_eq!(target.runtime.magic_point, 32);
    assert_eq!(target.states.entry(77), None);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(frame.updates.updates.len(), 4);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[净化]");
    assert_eq!(frame.updates.updates[0].score, 20);
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
    assert_eq!(
        frame.updates.updates[2].update_type,
        crate::engine::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[3].message, "[1]从[疾走]中解除");
}

#[test]
fn plain_disperse_refreshes_runtime_attract_after_clearing_iron() {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let iron = config
        .registry
        .skill_id_by_export_name(BuiltinActiveSkill::Iron.export_name())
        .expect("default profile should register iron skill");
    let CustomRuntimeV2ImportConfig {
        registry,
        state_handlers,
        ..
    } = config;
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(80),
            PlayerTemplate::new(2, "target", 1, 1_000, 3)
                .with_def_res(0, 16)
                .with_magic(20)
                .with_magic_point(96)
                .with_target_score_stats(100, 100, 32_768.0)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(iron, 128)])),
        ],
        registry,
    ));
    for binding in state_handlers {
        runtime.set_state_handler_with_capabilities(binding.state_id, binding.handler, &binding.capabilities);
    }
    runtime.drain_plain_iron_skill_into(EntityIdx(1), &mut RunUpdates::new());
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.attract(),
        32_768.0 * 1.1200000047683716
    );
    let mut updates = RunUpdates::new();

    assert!(!runtime.apply_disperse_attack_damage_into(EntityIdx(0), EntityIdx(1), 1, &mut updates));

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.hp, 999);
    assert_eq!(target.runtime.magic_point, 32);
    assert_eq!(target.states.entry(PLAIN_IRON_STATE_KEY), None);
    assert_eq!(target.runtime.attract(), 32_768.0);
    assert_eq!(
        updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "[1]的[铁壁]被打消了"]
    );
}

#[test]
fn plain_disperse_clears_before_post_damage_upgrade_activation() {
    let mut builder = ExtensionRegistryBuilder::default();
    let upgrade = builder
        .register_skill_with_hooks(
            "core",
            "upgrade",
            DEFAULT_CORE_UPGRADE_SKILL_EXPORT,
            ProcMask::POST_DAMAGE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("upgrade skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(80),
            PlayerTemplate::new(2, "target", 1, 100, 3)
                .with_def_res(0, 16)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(upgrade, 128)])),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();

    assert!(!runtime.apply_disperse_attack_damage_into(EntityIdx(0), EntityIdx(1), 90, &mut updates,));

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.hp, 10);
    assert!(target.runtime.upgrade_active);
    assert_eq!(
        updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec!["[1]受到[2]点伤害", "[0]做出[垂死]抗争", "[0]所有属性上升"]
    );
}

#[test]
fn lethal_disperse_emits_knockout_before_reraise_without_state_cancel_message() {
    let mut builder = ExtensionRegistryBuilder::default();
    let reraise = builder
        .register_skill_with_hooks(
            "core",
            "reraise",
            DEFAULT_CORE_RERAISE_SKILL_EXPORT,
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("reraise skill should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(1_000),
            PlayerTemplate::new(2, "target", 1, 1, 3)
                .with_def_res(0, 16)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    runtime.effects.push(QueuedEffect::DisperseAttack {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("lethal disperse with reraise should emit updates");
    let target = runtime.entities.get(EntityIdx(1)).unwrap();

    assert!(target.runtime.alive);
    assert!(target.runtime.hp > 0);
    assert_eq!(target.states.entry(77), None);
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec![
            "[0]使用[净化]",
            "[1]受到[2]点伤害",
            "[1]被击倒了",
            "[0]使用[护身符]抵挡了一次死亡",
            "[1]回复体力[2]点",
        ]
    );
}

#[test]
fn plain_disperse_runs_before_default_attack_in_minimal_round() {
    let mut builder = ExtensionRegistryBuilder::default();
    let disperse = builder
        .register_skill_with_hooks(
            "core",
            "disperse",
            "core.disperse",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("disperse skill should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3)
                .with_magic(80)
                .with_wisdom(64)
                .with_skills([disperse]),
            PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16).with_magic_point(96),
        ],
        registry,
    ));
    runtime.set_skill_handler(disperse, run_disperse_skill);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    let mut expected_rng = RC4::default();
    expected_rng.next_u8();
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    let expected_disperse_damage =
        (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("disperse plus attack should emit update");

    assert_eq!(outcome.action.unwrap().target, EntityIdx(1));
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp,
        1_000 - expected_disperse_damage - 3
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, 32);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec!["[0]使用[净化]", "[1]受到[2]点伤害", "[1]从[疾走]中解除", "[0]攻击[1]"]
    );
}

#[test]
fn plain_disperse_scores_multiple_enemy_targets_without_dodge_rng() {
    let mut builder = ExtensionRegistryBuilder::default();
    let disperse = builder
        .register_skill_with_hooks(
            "core",
            "disperse",
            "core.disperse",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("disperse skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 0)
                .with_magic(80)
                .with_wisdom(64)
                .with_skills([disperse]),
            PlayerTemplate::new(2, "first", 1, 200, 0)
                .with_def_res(0, 16)
                .with_magic_point(96)
                .with_target_score_stats(0, 30, 1.0),
            PlayerTemplate::new(3, "best", 1, 200, 0)
                .with_def_res(0, 16)
                .with_magic_point(96)
                .with_target_score_stats(0, 300, 1.0),
            PlayerTemplate::new(4, "also-picked", 1, 200, 0)
                .with_def_res(0, 16)
                .with_magic_point(96)
                .with_target_score_stats(0, 60, 1.0),
        ],
        registry,
    ));
    runtime.set_skill_handler(disperse, run_disperse_skill);
    let mut expected_rng = RC4::default();
    expected_rng.next_u8();
    let selected_targets = select_disperse_targets(&runtime.entities, &runtime.world, EntityIdx(0), true, &mut expected_rng);
    let selected_target = selected_targets[0];
    assert_eq!(selected_target, EntityIdx(2));
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    let expected_disperse_damage =
        (atp / runtime.entities.get(selected_target).unwrap().runtime.magic_defense() as f64).ceil() as i32;

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("disperse should emit update");

    assert_eq!(outcome.action.unwrap().target, EntityIdx(1));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 200);
    assert_eq!(
        runtime.entities.get(selected_target).unwrap().runtime.hp,
        200 - expected_disperse_damage
    );
    assert_eq!(runtime.entities.get(selected_target).unwrap().runtime.magic_point, 32);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| (update.message.as_ref(), update.target))
            .collect::<Vec<_>>(),
        vec![("[0]使用[净化]", 2), ("[1]受到[2]点伤害[s_dmg120]", 2), ("[0]攻击[1]", 1)]
    );
}

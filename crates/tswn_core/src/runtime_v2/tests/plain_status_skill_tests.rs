use super::*;

#[test]
fn plain_berserk_static_dispatch_applies_and_extends_forced_action_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let berserk = builder
        .register_skill(
            "core",
            "berserk",
            BuiltinActiveSkill::Berserk.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(10),
        )
        .expect("berserk skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(10_000)
                .with_agility(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(berserk, 128)])),
            PlayerTemplate::new(2, "target", 1, 1_000_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("berserk should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Berserk);
    assert_eq!(prepared.targets, vec![EntityIdx(1)]);
    while {
        let mut probe = runtime.rng.clone();
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut probe);
        PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut probe,
        )
    } {
        runtime.rng.next_u8();
    }
    let mut expected_rng = runtime.rng.clone();
    runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng,
    ));
    let mut updates = RunUpdates::new();

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(PLAIN_BERSERK_STATE_KEY)
            .and_then(|entry| match &entry.payload {
                StatePayload::Berserk { step } => Some(*step),
                _ => None,
            }),
        Some(1)
    );
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]使用[狂暴术]", "[1]受到[2]点伤害[s_dmg160]", "[1]进入[狂暴]状态"]
    );

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.at_boost_millionths = 3_000_000;
    let update_count = updates.updates.len();
    runtime.apply_berserk_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(PLAIN_BERSERK_STATE_KEY)
            .and_then(|entry| match &entry.payload {
                StatePayload::Berserk { step } => Some(*step),
                _ => None,
            }),
        Some(3)
    );
    assert_eq!(updates.updates.len(), update_count);
}

#[test]
fn plain_haste_static_dispatch_applies_speed_state_and_charge_extension() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_skill(
            "core",
            "haste",
            BuiltinActiveSkill::Haste.export_name(),
            TargetPolicy::Ally,
            SkillPriority(12),
        )
        .expect("haste skill should register");
    builder
        .register_state(
            "core",
            "haste",
            DEFAULT_CORE_HASTE_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_speed(40)
                .with_speed_points(100)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(haste, 128)])),
            PlayerTemplate::new(2, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("haste should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Haste);
    assert_eq!(prepared.targets, vec![EntityIdx(0)]);
    let expected_rng = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(PLAIN_HASTE_STATE_KEY)
            .and_then(StateEntry::haste_value),
        Some((2, 3))
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 140);
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[0]使用[加速术]", 60), ("[1]进入[疾走]状态", 0)]
    );

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.at_boost_millionths = 3_000_000;
    runtime.drain_plain_haste_skill_into(EntityIdx(0), EntityIdx(0), &mut updates);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(PLAIN_HASTE_STATE_KEY)
            .and_then(StateEntry::haste_value),
        Some((4, 7))
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 220);
}

#[test]
fn plain_heal_clears_negative_states_restores_derived_stats_and_decays_level() {
    let mut builder = ExtensionRegistryBuilder::default();
    let heal = builder
        .register_skill(
            "core",
            "heal",
            BuiltinActiveSkill::Heal.export_name(),
            TargetPolicy::Ally,
            SkillPriority(15),
        )
        .expect("heal skill should register");
    let curse = builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    let poison = builder
        .register_state("core", "poison", "core.state.poison", ProcMask::POST_ACTION, SkillPriority(150))
        .expect("poison state should register");
    let haste = builder
        .register_state("core", "haste", "core.state.haste", ProcMask::POST_ACTION, SkillPriority(210))
        .expect("haste state should register");
    let charm = builder
        .register_state(
            "core",
            "charm",
            DEFAULT_CORE_CHARM_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("charm state should register");
    let slow = builder
        .register_state(
            "core",
            "slow",
            DEFAULT_CORE_SLOW_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("slow state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "healer", 0, 1_000, 3)
                .with_magic(6_000)
                .with_wisdom(128)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(heal, 9)])),
            PlayerTemplate::new(2, "target", 0, 1_000, 3)
                .with_speed(40)
                .with_target_score_stats(10, 7, 1.0),
        ],
        registry,
    ));
    {
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 500;
        target.runtime.atk_sum = 28;
        target.runtime.speed = 40;
        target.states.add_entry(StateEntry::fire_mag(0, 2));
        target.states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
        target
            .states
            .add_entry(StateEntry::curse(PLAIN_CURSE_STATE_KEY, curse, 42, 2, SkillPriority(10_000)));
        target
            .states
            .add_entry(StateEntry::poison(75, poison, Some(0), Some(1), 10.0, 2, SkillPriority(150)));
        target.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(210)));
        target.states.add_entry(StateEntry::berserk(10, 2));
        target.states.add_entry(StateEntry::charm(
            76,
            charm,
            0,
            Some(0),
            Some(0),
            Some(1),
            2,
            SkillPriority(210),
        ));
        target.states.add_entry(StateEntry::slow(78, slow, 2, SkillPriority(210)));
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_heal_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.runtime.hp > 500);
    assert_eq!(target.runtime.atk_sum, 7);
    assert_eq!(target.runtime.speed, 40);
    assert_eq!(target.states.effective_speed(target.runtime.speed), 80);
    assert_eq!(target.states.entry(77).and_then(StateEntry::haste_value), Some((2, 3)));
    assert_eq!(target.states.entries().len(), 1);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.level_at(0), Some(8));
    assert_eq!(
        updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec![
            "[0]使用[治愈魔法]",
            "[1]回复体力[2]点",
            "[1]从[狂暴]中解除",
            "[1]从[魅惑]中解除",
            "[1]从[诅咒]中解除",
            "[1]从[冰冻]中解除",
            "[1]从[中毒]中解除",
            "[1]从[迟缓]中解除",
        ]
    );
}

#[test]
fn plain_iron_static_dispatch_gates_active_state_and_uses_charge_formula() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron = builder
        .register_skill(
            "core",
            "iron",
            BuiltinActiveSkill::Iron.export_name(),
            TargetPolicy::None,
            SkillPriority(18),
        )
        .expect("iron skill should register");
    builder
        .register_state(
            "core",
            "iron",
            DEFAULT_CORE_IRON_STATE_EXPORT,
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(20)
                .with_speed_points(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(iron, 128)])),
            PlayerTemplate::new(2, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("iron should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Iron);
    assert_eq!(prepared.targets, vec![EntityIdx(0)]);
    let expected_rng = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(PLAIN_IRON_STATE_KEY)
            .and_then(StateEntry::iron_value),
        Some((130, 3))
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 744);
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[0]发动[铁壁]", 60), ("[0]防御力大幅上升", 0)]
    );

    let gated_rng = runtime.rng.clone();
    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Iron, 128, false));
    assert_rng_state_eq(&runtime.rng, &gated_rng);

    assert!(
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .states
            .set_payload(PLAIN_IRON_STATE_KEY, StatePayload::Iron { protect: 0, step: 0 })
    );
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.at_boost_millionths = 3_000_000;
    runtime.drain_plain_iron_skill_into(EntityIdx(0), &mut updates);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(PLAIN_IRON_STATE_KEY)
            .and_then(StateEntry::iron_value),
        Some((450, 7))
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 488);
}

#[test]
fn plain_iron_refreshes_runtime_attract_after_shield_break() {
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
            PlayerTemplate::new(1, "iron", 0, 100, 3)
                .with_magic(20)
                .with_target_score_stats(100, 100, 32_768.0)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(iron, 128)])),
            PlayerTemplate::new(2, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    for binding in state_handlers {
        runtime.set_state_handler_with_capabilities(binding.state_id, binding.handler, &binding.capabilities);
    }
    runtime.drain_plain_iron_skill_into(EntityIdx(0), &mut RunUpdates::new());
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.attract(),
        32_768.0 * 1.1200000047683716
    );
    let mut defend_value = RuntimeDefendValue::Damage {
        value: 200,
        caster: EntityIdx(1),
        target: EntityIdx(0),
    };
    let mut updates = RunUpdates::new();

    runtime.drain_post_defend_hooks_into(EntityIdx(0), &mut updates, &mut defend_value);

    assert_eq!(defend_value.damage(), Some(70));
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(PLAIN_IRON_STATE_KEY)
            .and_then(StateEntry::iron_value),
        Some((0, 0))
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.attract(), 32_768.0);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["\n", "[1]的[铁壁]被打消了"]
    );
}

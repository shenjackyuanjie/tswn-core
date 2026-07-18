use super::*;

#[test]
fn score_disperse_target_matches_legacy_smart_two_team_formula() {
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 10, 3),
        PlayerTemplate::new(2, "target", 1, 80, 3).with_target_score_stats(77, 120, 2.5),
    ]));
    let mut rng = RC4::default();

    let score = score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), true, &mut rng);

    assert_eq!(score, (1.0 / 80.0) * 120.0 * 2.5);
    let expected_rng = RC4::default();
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
}

#[test]
fn score_disperse_target_matches_legacy_smart_multi_team_and_minion_formula() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::with_kind(2, "target", minion_kind, 1, 400, 3).with_target_score_stats(77, 120, 2.5),
            PlayerTemplate::new(3, "team-2", 2, 10, 3),
            PlayerTemplate::new(4, "team-1-ally", 1, 10, 3),
        ],
        registry,
    ));
    let mut rng = RC4::default();

    let score = score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), true, &mut rng);

    assert_eq!(score, 300.0 * 2.0 * 2.5 * 2.0);
    let expected_rng = RC4::default();
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
}

#[test]
fn score_disperse_target_matches_legacy_random_formula_and_unknown_target() {
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 10, 3),
        PlayerTemplate::new(2, "target", 1, 80, 3).with_target_score_stats(77, 120, 2.5),
    ]));
    let mut rng = RC4::default();
    let mut expected_rng = RC4::default();
    let expected = expected_rng.rFFFF() as f64 + 2.5;

    assert_eq!(
        score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), false, &mut rng),
        expected
    );
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
    assert_eq!(
        score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(99), false, &mut rng),
        f64::MIN
    );
}

#[test]
fn flush_effects_disperse_hit_clears_positives_and_spends_mp() {
    let mut builder = ExtensionRegistryBuilder::default();
    let shield = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::new(2, "target", 1, 10, 3).with_magic_point(96),
        ],
        registry,
    ));
    {
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.activate_charge_runtime();
        target.activate_accumulate_runtime();
        target.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        target.states.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
        target.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    }
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        damage: 1,
    });

    let frame = runtime.flush_effects().expect("disperse hit should emit clear-positive messages");
    let target = runtime.entities.get(EntityIdx(1)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec![
            "[1]的[聚气]被打消了",
            "[1]的[蓄力]被中止了",
            "[1]从[疾走]中解除",
            "[1]的[铁壁]被打消了"
        ]
    );
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .count(),
        4
    );
    assert_eq!(target.runtime.magic_point, 32);
    assert!(!target.runtime.accumulate.active);
    assert!(!target.runtime.charge.active);
    assert_eq!(target.runtime.at_boost_millionths, 1_000_000);
    assert_eq!(target.states.entry(74), None);
    assert_eq!(target.states.entry(77), None);
    assert_eq!(target.states.entry(79), None);
}

#[test]
fn flush_effects_disperse_hit_uses_legacy_mp_thresholds_and_skips_zero_damage() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::new(2, "high", 1, 10, 3).with_magic_point(65),
            PlayerTemplate::new(3, "mid", 1, 10, 3).with_magic_point(33),
            PlayerTemplate::new(4, "low", 1, 10, 3).with_magic_point(32),
            PlayerTemplate::new(5, "missed", 1, 10, 3).with_magic_point(96),
        ],
        ExtensionRegistry::default(),
    ));
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        damage: 1,
    });
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(2),
        damage: 1,
    });
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(3),
        damage: 1,
    });
    runtime.effects.push(QueuedEffect::DisperseHit {
        caster: EntityIdx(0),
        target: EntityIdx(4),
        damage: 0,
    });

    let frame = runtime.flush_effects();

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, 1);
    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_point, 0);
    assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.magic_point, 0);
    assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.magic_point, 96);
}

#[test]
fn flush_effects_disperse_attack_doubles_atp_against_minion_targets() {
    let mut builder = ExtensionRegistryBuilder::default();
    let minion_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "minion",
            "custom.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("minion kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(80),
            PlayerTemplate::with_kind(2, "minion", minion_kind, 1, 10_000, 3).with_def_res(0, 16),
        ],
        registry,
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 2.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    runtime.effects.push(QueuedEffect::DisperseAttack {
        caster: EntityIdx(0),
        target: EntityIdx(1),
    });

    let frame = runtime.flush_effects().expect("minion disperse should emit damage");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
}

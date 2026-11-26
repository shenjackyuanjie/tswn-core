//! 伤害短路与概率门：必中交换、零伤跳过与技能概率 RNG 语义。
use super::*;

/// 按 charged 构造注册了换血/交换等技能的 CombatRuntime，用于必中控制测试。
fn control_runtime(charged: bool) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let exchange = builder
        .register_skill(
            "core",
            "exchange",
            BuiltinActiveSkill::Exchange.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .unwrap();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(60)
                .with_at_boost(if charged { 3.0 } else { 1.0 })
                .with_skill_loadout(SkillLoadout::from_skill_levels([(exchange, 64)])),
            PlayerTemplate::new(2, "target", 1, 200, 3).with_def_res(1000, 1000).with_agility(1000),
        ],
        builder.build(),
    ))
}

/// 按 exchange 选择换血或半血技能路径，把技能效果 drain 进 updates。
fn cast(runtime: &mut CombatRuntime, exchange: bool) {
    let mut updates = RunUpdates::new();
    if exchange {
        runtime.drain_plain_exchange_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);
    } else {
        runtime.drain_plain_half_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);
    }
}

#[test]
fn charged_exchange_and_half_bypass_manual_evade_without_consuming_its_script() {
    for exchange in [false, true] {
        let mut runtime = control_runtime(true);
        runtime.control.set_passive_decision_mode(true);
        runtime.control.set_passive_decisions([PassiveDecision {
            actor: 1,
            skill: "evade".into(),
            activate: true,
            success: true,
            target: None,
        }]);
        let before_rng = runtime.rng.clone();
        cast(&mut runtime, exchange);
        assert!(runtime.control.pending_passive().is_none());
        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < 200);
        assert_rng_state_eq(&runtime.rng, &before_rng);
        assert_eq!(runtime.control.next_passive_decision().unwrap().skill, "evade");
    }
}

#[test]
fn uncharged_exchange_and_half_still_request_and_respect_manual_evade() {
    for exchange in [false, true] {
        let mut runtime = control_runtime(false);
        runtime.control.set_passive_decision_mode(true);
        let paused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cast(&mut runtime, exchange)));
        assert!(paused.is_err());
        let pending = runtime.control.pending_passive().unwrap();
        assert_eq!(pending.skill, "evade");
        assert!(!pending.unavoidable);

        let mut runtime = control_runtime(false);
        runtime.control.set_passive_decision_mode(true);
        runtime.control.set_passive_decisions([PassiveDecision {
            actor: 1,
            skill: "evade".into(),
            activate: true,
            success: true,
            target: None,
        }]);
        cast(&mut runtime, exchange);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 200);
    }
}

#[test]
fn zero_plain_attack_damage_skips_on_damage_and_post_damage_rng() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter = builder
        .register_skill_with_hooks(
            "core",
            "counter",
            DEFAULT_CORE_COUNTER_SKILL_EXPORT,
            ProcMask::POST_DAMAGE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("counter skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 1).with_skill_loadout(SkillLoadout::from_skill_levels([(counter, 128)])),
        ],
        registry,
    ));
    let expected_rng = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    assert!(!runtime.apply_plain_attack_damage_into(EntityIdx(0), EntityIdx(1), 0, &mut updates,));

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100);
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.counter.pending);
    assert!(updates.on_update_end.is_empty());
    assert_eq!(updates.updates.len(), 1);
    assert_eq!(updates.updates[0].message, "[0]受到[2]点伤害[s_dmg0]");
    assert_eq!(updates.updates[0].score, 10);
}

#[test]
fn zero_disperse_damage_skips_post_damage_rng() {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter = builder
        .register_skill_with_hooks(
            "core",
            "counter",
            DEFAULT_CORE_COUNTER_SKILL_EXPORT,
            ProcMask::POST_DAMAGE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("counter skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 1).with_skill_loadout(SkillLoadout::from_skill_levels([(counter, 128)])),
        ],
        registry,
    ));
    let expected_rng = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    assert!(!runtime.apply_disperse_attack_damage_into(EntityIdx(0), EntityIdx(1), 0, &mut updates,));

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100);
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.counter.pending);
    assert!(updates.on_update_end.is_empty());
    assert_eq!(updates.updates.len(), 1);
    assert_eq!(updates.updates[0].message, "[0]受到[2]点伤害[s_dmg0]");
    assert_eq!(updates.updates[0].score, 10);
}

#[cfg(feature = "no_debug")]
#[test]
fn uncaptured_attack_matches_captured_dodge_and_damage() {
    let base = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3),
        PlayerTemplate::new(2, "target", 1, 100, 1),
    ]));
    let (actor, target) = (EntityIdx(0), EntityIdx(1));

    for use_magic in [false, true] {
        for (atp, shield) in [(0.0, 0), (640.0, 0), (640.0, 7), (-640.0, 0), (640_000.0, 0)] {
            let mut saw_dodge = false;
            let mut saw_hit = false;
            for seed in 0..24 {
                let mut captured = base.clone();
                captured.rng = RC4::new(&[seed], 1);
                let target_runtime = &mut captured.entities.get_mut(target).unwrap().runtime;
                target_runtime.hp = 70;
                target_runtime.shield = shield;
                let mut uncaptured = captured.clone();

                let mut replay = RunUpdates::new();
                let mut silent = RunUpdates::new_no_capture();
                let expected = captured.drain_plain_attack_with_atp_into(actor, target, use_magic, atp, &mut replay);
                let actual = uncaptured.drain_plain_attack_with_atp_into(actor, target, use_magic, atp, &mut silent);
                assert_eq!(actual, expected);
                assert_rng_state_eq(&uncaptured.rng, &captured.rng);
                assert_eq!(uncaptured.entities, captured.entities);
                assert_eq!(uncaptured.world, captured.world);
                assert_eq!(uncaptured.effects, captured.effects);
                assert_eq!(uncaptured.control.damage_batch(), None);
                assert_eq!(
                    uncaptured.control.begin_damage_batch(0, 1),
                    captured.control.begin_damage_batch(0, 1)
                );
                assert_eq!(silent.had_updates(), replay.had_updates());
                assert_eq!(
                    silent.last_update_was_plain_defense(target.0 as usize, actor.0 as usize),
                    replay.last_update_was_plain_defense(target.0 as usize, actor.0 as usize)
                );
                assert!(silent.updates.is_empty());
                saw_dodge |= replay.updates.iter().any(|update| update.message == "[0][回避]了攻击");
                saw_hit |= expected != 0;
            }
            if atp != 0.0 {
                assert!(saw_dodge);
                assert!(saw_hit);
            }
        }
    }
}

use crate::runtime::combat::PlainAttackOnDamage;

#[test]
fn plain_absorb_smart_low_missing_hp_skips_probability_rng() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3),
        ],
        registry,
    ));
    let expected_rng = runtime.rng.clone();

    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Absorb, 128, true));
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_accumulate_gates_skip_probability_rng() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 200, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 119;
    let expected_rng = runtime.rng.clone();

    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Accumulate, 128, true));
    assert_rng_state_eq(&runtime.rng, &expected_rng);

    {
        let actor = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        actor.runtime.hp = actor.template.max_hp;
        assert!(actor.activate_accumulate_runtime());
    }
    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Accumulate, 128, false));
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_curse_empty_smart_targets_still_consume_sampling_rng() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "low-hp-target", 1, 100, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 79;
    let all_alive = runtime.world.flat_alive().to_vec();
    let mut expected_rng = runtime.rng.clone();
    for _ in 0..7 {
        assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0]), Some(1));
    }

    assert!(runtime.select_plain_curse_targets(EntityIdx(0), true).is_empty());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn reflect_failed_level_roll_only_consumes_r255() {
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
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "reflector", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 1)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(reflect, run_reflect_pre_defend_skill);
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r255();
    let mut updates = RunUpdates::new();
    let mut defend_value = RuntimeDefendValue::Atp {
        value: 50.0,
        caster: EntityIdx(0),
        target: EntityIdx(1),
        is_magic: true,
        kind: DamageKind::Default,
    };

    runtime.drain_pre_defend_hooks_into(EntityIdx(1), &mut updates, &mut defend_value);

    assert_eq!(defend_value.atp(), Some(50.0));
    assert!(updates.updates.is_empty());
    assert!(runtime.effects.is_empty());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn reflected_attack_applies_damage_before_move_penalty_finishes() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "reflector", 0, 100, 3)
                .with_magic(10_000)
                .with_wisdom(10_000)
                .with_speed_points(1_000),
            PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16),
        ],
        registry,
    ));
    while {
        let mut probe = runtime.rng.clone();
        probe.next_u8() <= 7
    } {
        runtime.rng.next_u8();
    }
    runtime.effects.push(QueuedEffect::ReflectedAttack {
        caster: EntityIdx(0),
        target: EntityIdx(1),
        atp_bits: 50.0_f64.to_bits(),
        kind: DamageKind::Default,
        on_damage: PlainAttackOnDamage::None,
    });

    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        1_000
    );
    let frame = runtime.flush_effects().expect("reflected attack should emit damage");

    assert!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < 1_000,
        "reflected damage must resolve before the queued effect completes"
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 520);
    assert!(
        frame
            .updates
            .updates
            .iter()
            .any(|update| update.caster == 0 && update.target == 1 && update.score > 0)
    );
}

#[test]
fn plain_curse_skill_applies_state_after_damage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let curse_state = builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(80).with_wisdom(64),
            PlayerTemplate::new(2, "target", 1, 1_000, 3)
                .with_def_res(0, 16)
                .with_target_score_stats(0, 7, 1.0),
        ],
        registry,
    ));
    runtime.set_state_handler(curse_state, run_curse_post_defend_state);
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
    let mut updates = RunUpdates::new();

    runtime.drain_plain_curse_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!((1..1_000).contains(&target.runtime.hp));
    assert_eq!(target.runtime.atk_sum, 28);
    assert_eq!(
        target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
        Some(StatePayload::Curse { prob: 42, multiply: 2 })
    );
    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[诅咒]");
    assert!(updates.updates[1].message.starts_with("[1]受到[2]点伤害"));
    assert_eq!(updates.updates.last().unwrap().message, "[1]被[诅咒]了");
}

#[test]
fn plain_curse_on_damage_stacks_caster_charge_bonus_without_reapplying_atk_sum() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            // 蓄力来源由 curse_caster_charge 决定：默认看目标，开启后看施法者。
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3).with_target_score_stats(0, 7, 1.0),
        ],
        registry,
    ));
    // 默认读目标的蓄力状态；`curse_caster_charge` 打开后改读施法者。
    let charged = if cfg!(feature = "curse_caster_charge") {
        EntityIdx(0)
    } else {
        EntityIdx(1)
    };
    runtime.entities.get_mut(charged).unwrap().activate_charge_runtime();
    let mut updates = RunUpdates::new();

    runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);
    runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(target.runtime.atk_sum, 28);
    assert_eq!(
        target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
        Some(StatePayload::Curse { prob: 72, multiply: 5 })
    );
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[1]被[诅咒]了", 60), ("[1]被[诅咒]了", 60)]
    );
}

#[test]
fn plain_curse_target_charge_bonus_follows_compat_feature() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            // 目标蓄力：默认口径下会加成诅咒，`curse_caster_charge` 打开后不再生效。
            PlayerTemplate::new(2, "charged-target", 1, 100, 3).with_target_score_stats(0, 7, 1.0),
        ],
        registry,
    ));
    // 蓄力口径是运行时 charge 状态。
    runtime.entities.get_mut(EntityIdx(1)).unwrap().activate_charge_runtime();
    let mut updates = RunUpdates::new();

    runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);
    runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);

    // 默认沿用上游口径：读目标蓄力，连叠两次得到 72/5。
    // `curse_caster_charge` 打开后只看施法者，目标蓄力不再改变强度（52/3）。
    let expected = if cfg!(feature = "curse_caster_charge") {
        (52, 3)
    } else {
        (72, 5)
    };
    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    let payload = target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| match entry.payload {
        StatePayload::Curse { prob, multiply } => (prob, multiply),
        _ => panic!("curse state payload changed"),
    });
    assert_eq!(payload, Some(expected));
}

#[test]
fn plain_default_enemy_target_selection_matches_legacy_rng_for_single_enemy() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3),
        PlayerTemplate::new(2, "ally", 0, 100, 3),
        PlayerTemplate::new(3, "enemy", 1, 100, 3),
    ]));
    let all_alive = runtime.world.flat_alive().to_vec();
    let mut expected_rng = runtime.rng.clone();
    for _ in 0..4 {
        assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0, 1]), Some(2));
    }
    let _ = expected_rng.rFFFF();

    let selected = runtime.select_plain_default_enemy_targets(EntityIdx(0), false);

    assert_eq!(selected.as_slice(), &[EntityIdx(2)]);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_poison_static_dispatch_applies_threshold_and_stacking_semantics() {
    let mut builder = ExtensionRegistryBuilder::default();
    let poison = builder
        .register_skill(
            "core",
            "poison",
            BuiltinActiveSkill::Poison.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(5),
        )
        .expect("poison skill should register");
    let poison_state = builder
        .register_state(
            "core",
            "poison",
            DEFAULT_CORE_POISON_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(150),
        )
        .expect("poison state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(1_000_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(poison, 128)])),
            PlayerTemplate::new(2, "target", 1, 100_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    let mut updates = RunUpdates::new();
    let threshold_rng = runtime.rng.clone();

    runtime.apply_poison_on_damage(EntityIdx(0), EntityIdx(1), 4, &mut updates);

    assert_rng_state_eq(&runtime.rng, &threshold_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().states.entry(PLAIN_POISON_STATE_KEY),
        None
    );
    assert!(updates.updates.is_empty());

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("poison should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Poison);
    assert_eq!(prepared.targets.as_slice(), &[EntityIdx(1)]);

    let mut expected_rng = runtime.rng.clone();
    let attack_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng,
    ));
    let damage = (attack_atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(damage > 4);
    let first_poison_atp =
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2000000476837158;

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(PLAIN_POISON_STATE_KEY)
            .and_then(StateEntry::poison_value),
        Some((Some(0), Some(1), first_poison_atp, 4))
    );
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0][投毒]", "[1]受到[2]点伤害[s_dmg160]", "[1][中毒]"]
    );

    let mut expected_rng = runtime.rng.clone();
    let second_poison_atp =
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2000000476837158;

    runtime.apply_poison_on_damage(EntityIdx(0), EntityIdx(1), 5, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(
        target.states.entry(PLAIN_POISON_STATE_KEY).and_then(StateEntry::poison_value),
        Some((Some(0), Some(1), first_poison_atp + second_poison_atp, 4))
    );
    assert_eq!(
        target.states.entry(PLAIN_POISON_STATE_KEY).and_then(|entry| entry.extension_state_id),
        Some(poison_state)
    );
    assert_eq!(updates.updates.last().unwrap().message, "[1][中毒]");
}

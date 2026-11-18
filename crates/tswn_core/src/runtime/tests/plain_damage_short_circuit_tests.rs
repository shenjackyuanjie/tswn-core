use super::*;

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

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

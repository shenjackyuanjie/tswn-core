use super::*;

#[test]
fn plain_reflect_pre_defend_skips_frozen_owner_without_mp_rng() {
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
            PlayerTemplate::new(2, "reflector", 1, 100, 3)
                .with_magic_point(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 255)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(reflect, run_reflect_pre_defend_skill);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.active());
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().is_active());
    let mut expected_rng = runtime.rng.clone();
    expected_rng.r255();
    expected_rng.c50();
    let mp_before = runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point;
    let mut updates = RunUpdates::new();
    let mut defend_value = RuntimeDefendValue::Atp {
        value: 50.0,
        caster: EntityIdx(0),
        target: EntityIdx(1),
        is_magic: true,
    };

    runtime.drain_pre_defend_hooks_into(EntityIdx(1), &mut updates, &mut defend_value);

    assert_eq!(defend_value.atp(), Some(50.0));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, mp_before);
    assert!(updates.updates.is_empty());
    assert!(runtime.effects.is_empty());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

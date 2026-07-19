use super::*;
use crate::runtime::combat::PlainAttackOnDamage;

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

#[test]
fn reflected_fire_attack_keeps_fire_on_damage_callback() {
    let registry = ExtensionRegistryBuilder::default().build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "reflector", 0, 100, 3).with_magic(10_000),
            PlayerTemplate::new(2, "caster", 1, 1_000, 3).with_def_res(0, 16),
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
        on_damage: PlainAttackOnDamage::Fire(91),
    });

    runtime.flush_effects().expect("reflected fire attack should emit damage");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
}

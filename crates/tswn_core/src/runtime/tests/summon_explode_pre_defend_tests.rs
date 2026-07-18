use super::*;

#[test]
fn summon_explode_runs_pre_defend_before_dodge_and_damage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let pre_defend = builder
        .register_skill_with_hooks(
            "custom",
            "pre-defend",
            "custom.pre_defend",
            ProcMask::PRE_DEFEND,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("pre-defend skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                .with_def_res(0, 16)
                .with_skills([pre_defend]),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_skill_handler(pre_defend, skill_halves_defend_atp);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::fire_mag(91, 3));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 5.5 / 2.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("pre-defend summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 2.0);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "pre defend skill");
    assert_eq!(frame.updates.updates[1].caster, 1);
    assert_eq!(frame.updates.updates[2].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[2].score, expected_amount as u32);
}

#[test]
fn summon_explode_pre_defend_zero_stops_before_dodge_damage_and_fire() {
    let mut builder = ExtensionRegistryBuilder::default();
    let pre_defend = builder
        .register_skill_with_hooks(
            "custom",
            "pre-defend-zero",
            "custom.pre_defend_zero",
            ProcMask::PRE_DEFEND,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("pre-defend skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                .with_def_res(0, 512)
                .with_agility(512)
                .with_skills([pre_defend]),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(0),
        ],
        registry,
    ));
    runtime.set_skill_handler(pre_defend, skill_zeroes_defend_atp);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::fire_mag(91, 3));
    let mut expected_rng = RC4::default();
    let _ = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("pre-defend zero should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 1.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "pre defend zero");
}

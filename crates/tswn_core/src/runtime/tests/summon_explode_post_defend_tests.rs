use super::*;

#[test]
fn summon_explode_runs_post_defend_skill_and_state_in_priority_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    let post_skill = builder
        .register_skill_with_hooks(
            "custom",
            "post-defend-skill",
            "custom.post_defend_skill",
            ProcMask::POST_DEFEND,
            TargetPolicy::None,
            SkillPriority(2000),
        )
        .expect("post-defend skill should register");
    let post_state = builder
        .register_state(
            "custom",
            "post-defend-state",
            "custom.post_defend_state",
            ProcMask::POST_DEFEND,
            SkillPriority(1000),
        )
        .expect("post-defend state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                .with_def_res(0, 16)
                .with_skills([post_skill]),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_skill_handler(post_skill, skill_halves_defend_damage);
    runtime.set_state_handler(post_state, state_adds_defend_damage);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 77,
        extension_state_id: Some(post_state),
        hook_mask: ProcMask::POST_DEFEND,
        priority: SkillPriority(1000),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let expected_amount = (raw_amount + 3) / 2;
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("post-defend summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 4);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "post defend state");
    assert_eq!(frame.updates.updates[2].message, "post defend skill");
    assert_eq!(frame.updates.updates[3].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[3].score, expected_amount as u32);
}

#[test]
fn summon_explode_post_defend_shield_absorbs_damage_and_consumes_payload() {
    let mut builder = ExtensionRegistryBuilder::default();
    let shield_state = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(shield_state, run_shield_post_defend_state);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
        77,
        shield_state,
        500,
        SkillPriority(6000),
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(raw_amount < 500);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("shielded summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(77)
            .and_then(StateEntry::shield_value),
        Some(500 - raw_amount)
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[0]受到[2]点伤害[s_dmg0]");
    assert_eq!(frame.updates.updates[1].score, 10);
}

#[test]
fn summon_explode_post_defend_shield_breaks_before_remaining_damage() {
    let mut builder = ExtensionRegistryBuilder::default();
    let shield_state = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(shield_state, run_shield_post_defend_state);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
        77,
        shield_state,
        3,
        SkillPriority(6000),
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(raw_amount > 3);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("shield break summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - raw_amount);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(77)
            .and_then(StateEntry::shield_value),
        Some(0)
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, raw_amount as u32);
}

#[test]
fn summon_explode_post_defend_iron_reduces_absorbed_damage_to_one() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
        .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 500, 3, SkillPriority(10)));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!((1..=500).contains(&raw_amount));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("iron absorbed summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 9_999);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(79)
            .and_then(StateEntry::iron_value),
        Some((500, 3))
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, 1);
}

#[test]
fn summon_explode_post_defend_iron_reduces_defended_damage_to_zero() {
    let mut builder = ExtensionRegistryBuilder::default();
    let defend_skill = builder
        .register_skill_with_hooks(
            "custom",
            "defend-marker",
            "custom.defend_marker",
            ProcMask::POST_DEFEND,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("defend marker skill should register");
    let iron_state = builder
        .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                .with_def_res(0, 16)
                .with_skills([defend_skill]),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_skill_handler(defend_skill, skill_marks_defend_replay);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 500, 3, SkillPriority(10)));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!((1..=500).contains(&raw_amount));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("defended iron summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(79)
            .and_then(StateEntry::iron_value),
        Some((500, 3))
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[0][防御]");
    assert_eq!(frame.updates.updates[1].caster, 1);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[2].message, "[0]受到[2]点伤害[s_dmg0]");
    assert_eq!(frame.updates.updates[2].score, 10);
}

#[test]
fn summon_explode_post_defend_iron_breaks_and_emits_cancel_replay() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
        .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 3, 3, SkillPriority(10)));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(raw_amount > 3);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("broken iron summon explode should emit updates");

    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp,
        10_000 - (raw_amount - 3)
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(79), None);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 4);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].message, "[1]的[铁壁]被打消了");
    assert_eq!(frame.updates.updates[2].caster, 2);
    assert_eq!(frame.updates.updates[2].target, 1);
    assert_eq!(frame.updates.updates[3].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[3].score, (raw_amount - 3) as u32);
}

#[test]
fn summon_explode_post_defend_iron_skips_state_change_when_damage_is_zero() {
    let mut builder = ExtensionRegistryBuilder::default();
    let shield_state = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(0))
        .expect("shield state should register");
    let iron_state = builder
        .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(shield_state, run_shield_post_defend_state);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::shield(77, shield_state, 500, SkillPriority(0)));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 3, SkillPriority(10)));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(raw_amount < 500);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("zero damage iron summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(79)
            .and_then(StateEntry::iron_value),
        Some((300, 3))
    );
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[0]受到[2]点伤害[s_dmg0]");
    assert_eq!(frame.updates.updates[1].score, 10);
}

#[test]
fn summon_explode_post_defend_curse_doubles_damage_and_emits_replay() {
    let mut builder = ExtensionRegistryBuilder::default();
    let curse_state = builder
        .register_state("core", "curse", "core.curse", ProcMask::POST_DEFEND, SkillPriority(10_000))
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(curse_state, run_curse_post_defend_state);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::curse(
        78,
        curse_state,
        64,
        2,
        SkillPriority(10_000),
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let curse_roll = expected_rng.next_u8() as u32 & 63;
    assert!(curse_roll < 64);
    let expected_amount = raw_amount * 2;
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("curse summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[诅咒]使伤害加倍");
    assert_eq!(frame.updates.updates[1].caster, 2);
    assert_eq!(frame.updates.updates[1].target, 1);
    assert_eq!(frame.updates.updates[1].score, 0);
    assert_eq!(frame.updates.updates[2].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[2].score, expected_amount as u32);
}

#[test]
fn summon_explode_post_defend_curse_consumes_rng_without_trigger() {
    let mut builder = ExtensionRegistryBuilder::default();
    let curse_state = builder
        .register_state("core", "curse", "core.curse", ProcMask::POST_DEFEND, SkillPriority(10_000))
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(curse_state, run_curse_post_defend_state);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::curse(
        78,
        curse_state,
        0,
        2,
        SkillPriority(10_000),
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let _curse_roll = expected_rng.next_u8() as u32 & 63;
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("curse miss summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - raw_amount);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, raw_amount as u32);
}

#[test]
fn summon_explode_post_defend_curse_skips_rng_when_damage_is_zero() {
    let mut builder = ExtensionRegistryBuilder::default();
    let shield_state = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let curse_state = builder
        .register_state("core", "curse", "core.curse", ProcMask::POST_DEFEND, SkillPriority(10_000))
        .expect("curse state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime.set_state_handler(shield_state, run_shield_post_defend_state);
    runtime.set_state_handler(curse_state, run_curse_post_defend_state);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
        77,
        shield_state,
        500,
        SkillPriority(6000),
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::curse(
        78,
        curse_state,
        64,
        2,
        SkillPriority(10_000),
    ));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    assert!(raw_amount < 500);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("shielded curse summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[0]受到[2]点伤害[s_dmg0]");
    assert_eq!(frame.updates.updates[1].score, 10);
}

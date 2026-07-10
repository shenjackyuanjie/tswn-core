use super::*;

#[test]
fn plain_thunder_static_dispatch_matches_multihit_rng_and_delay() {
    let mut builder = ExtensionRegistryBuilder::default();
    let thunder = builder
        .register_skill(
            "core",
            "thunder",
            BuiltinActiveSkill::Thunder.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("thunder skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(10_000)
                .with_agility(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(thunder, 128)])),
            PlayerTemplate::new(2, "target", 1, 1_000_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("thunder should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Thunder);
    assert_eq!(prepared.targets, vec![EntityIdx(1)]);

    while {
        let mut probe = runtime.rng.clone();
        let count = 3 + probe.r3() as usize;
        let mut accuracy = 100 + runtime.entities.get(EntityIdx(0)).unwrap().runtime.agility;
        (0..count).any(|_| {
            if PlayerRuntime::dodge(
                accuracy,
                runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
                &mut probe,
            ) {
                return true;
            }
            accuracy -= 10;
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut probe);
            false
        })
    } {
        runtime.rng.next_u8();
    }

    let mut expected_rng = runtime.rng.clone();
    let count = 3 + expected_rng.r3() as usize;
    let mut accuracy = 100 + runtime.entities.get(EntityIdx(0)).unwrap().runtime.agility;
    let mut total_damage = 0;
    for _ in 0..count {
        assert!(!PlayerRuntime::dodge(
            accuracy,
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng,
        ));
        accuracy -= 10;
        let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 0.36000001430511475;
        total_damage += (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    }

    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 1_000_000 - total_damage);
    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[雷击术]");
    let damage_updates = updates
        .updates
        .iter()
        .filter(|update| update.message.starts_with("[1]受到[2]点伤害"))
        .collect::<Vec<_>>();
    assert_eq!(damage_updates.len(), count);
    assert!(damage_updates.iter().all(|update| update.delay0 == 300));
}

#[test]
fn plain_thunder_dodge_happens_before_attack_rng_and_stops_skill() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3),
        PlayerTemplate::new(2, "target", 1, 100, 3).with_def_res(0, 10_000).with_agility(10_000),
    ]));
    let mut expected_rng = runtime.rng.clone();
    let _ = expected_rng.r3();
    assert!(PlayerRuntime::dodge(
        100,
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng,
    ));
    let mut updates = RunUpdates::new();

    runtime.drain_plain_thunder_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[0]使用[雷击术]", 1), ("\n", 0), ("[0][回避]了攻击", 0)]
    );
}

#[test]
fn plain_quake_static_dispatch_consumes_dead_target_attack_rng() {
    let mut builder = ExtensionRegistryBuilder::default();
    let quake = builder
        .register_skill(
            "core",
            "quake",
            BuiltinActiveSkill::Quake.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(2),
        )
        .expect("quake skill should register");
    let registry = builder.build();
    let mut templates = vec![
        PlayerTemplate::new(1, "caster", 0, 100, 3)
            .with_magic(10_000)
            .with_agility(1_000)
            .with_skill_loadout(SkillLoadout::from_skill_levels([(quake, 128)])),
    ];
    templates.extend(
        (0..12).map(|index| PlayerTemplate::new(index + 2, format!("target-{index}"), 1, 1_000_000, 3).with_def_res(0, 0)),
    );
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(templates, registry));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("quake should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Quake);
    assert_eq!(prepared.targets.len(), 5);

    let smart_targets = runtime.select_plain_default_enemy_targets_with_count(EntityIdx(0), true, 6);
    assert_eq!(smart_targets.len(), 6);

    let dead_target = prepared.targets[0];
    {
        let dead = runtime.entities.get_mut(dead_target).unwrap();
        dead.runtime.hp = 0;
        dead.runtime.alive = false;
    }
    while {
        let mut probe = runtime.rng.clone();
        let round = if probe.c50() { 5 } else { 4 };
        prepared.targets[..round.min(prepared.targets.len())].iter().copied().any(|target| {
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut probe);
            if runtime.entities.get(target).unwrap().runtime.hp <= 0 {
                return false;
            }
            PlayerRuntime::dodge(
                runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
                runtime.entities.get(target).unwrap().runtime.magic_dodge(),
                &mut probe,
            )
        })
    } {
        runtime.rng.next_u8();
    }

    let mut expected_rng = runtime.rng.clone();
    let round = if expected_rng.c50() { 5 } else { 4 };
    let picked = &prepared.targets[..round.min(prepared.targets.len())];
    let picked_len = picked.len();
    let divisor = picked.len() as f64 + 0.6000000238418579;
    let mut expected_hp = picked
        .iter()
        .map(|target| (*target, runtime.entities.get(*target).unwrap().runtime.hp))
        .collect::<Vec<_>>();
    for (target, hp) in &mut expected_hp {
        let atp =
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 2.440000057220459 / divisor;
        if *hp <= 0 {
            continue;
        }
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(*target).unwrap().runtime.magic_dodge(),
            &mut expected_rng,
        ));
        *hp -= (atp / runtime.entities.get(*target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    }

    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(dead_target).unwrap().runtime.hp, 0);
    for (target, hp) in expected_hp {
        assert_eq!(runtime.entities.get(target).unwrap().runtime.hp, hp);
    }
    assert_eq!(updates.updates.first().unwrap().message, "[0]使用[地裂术]");
    let damage_updates = updates
        .updates
        .iter()
        .filter(|update| update.message.starts_with("[1]受到[2]点伤害"))
        .collect::<Vec<_>>();
    assert_eq!(damage_updates.len(), picked_len - 1);
    assert!(damage_updates.iter().all(|update| update.delay0 == 300));
}

#[test]
fn plain_absorb_static_dispatch_heals_after_damage_and_preserves_zero_heal_frame() {
    let mut builder = ExtensionRegistryBuilder::default();
    let absorb = builder
        .register_skill(
            "core",
            "absorb",
            BuiltinActiveSkill::Absorb.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(3),
        )
        .expect("absorb skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3)
                .with_magic(10_000)
                .with_agility(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(absorb, 128)])),
            PlayerTemplate::new(2, "target", 1, 1_000_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 20;
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("absorb should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Absorb);

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
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2999999523162842;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng,
    ));
    let damage = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let healed = ((damage + 1) / 2).min(80);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 20 + healed);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]发起[吸血攻击]", "[1]受到[2]点伤害[s_dmg160]", "[1]回复体力[2]点"]
    );

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 90;
    runtime.apply_absorb_on_damage(EntityIdx(0), 5, &mut updates);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 93);
    assert_eq!(updates.updates.last().unwrap().score, 3);

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 100;
    runtime.apply_absorb_on_damage(EntityIdx(0), 5, &mut updates);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 100);
    assert_eq!(updates.updates.last().unwrap().score, 0);

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
    let update_count = updates.updates.len();
    runtime.apply_absorb_on_damage(EntityIdx(0), 5, &mut updates);
    assert_eq!(updates.updates.len(), update_count);
}

#[test]
fn plain_critical_static_dispatch_uses_three_physical_rolls_and_maximum() {
    let mut builder = ExtensionRegistryBuilder::default();
    let critical = builder
        .register_skill(
            "core",
            "critical",
            BuiltinActiveSkill::Critical.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(6),
        )
        .expect("critical skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 10_000)
                .with_agility(1_000)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(critical, 128)])),
            PlayerTemplate::new(2, "target", 1, 1_000_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("critical should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Critical);

    while {
        let mut probe = runtime.rng.clone();
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut probe);
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut probe);
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut probe);
        PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.attack
                + runtime.entities.get(EntityIdx(0)).unwrap().runtime.agility,
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.defense
                + runtime.entities.get(EntityIdx(1)).unwrap().runtime.agility,
            &mut probe,
        )
    } {
        runtime.rng.next_u8();
    }
    let mut expected_rng = runtime.rng.clone();
    let atp0 = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut expected_rng) * 1.149999976158142;
    let atp1 = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut expected_rng) * 1.2000000476837158;
    let atp2 = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut expected_rng) * 1.25;
    let atp = atp0.max(atp1).max(atp2);
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.attack + runtime.entities.get(EntityIdx(0)).unwrap().runtime.agility,
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.defense + runtime.entities.get(EntityIdx(1)).unwrap().runtime.agility,
        &mut expected_rng,
    ));
    let damage = (atp / (runtime.entities.get(EntityIdx(1)).unwrap().runtime.defense + 64) as f64).ceil() as i32;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 1_000_000 - damage);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]发动[会心一击]", "[1]受到[2]点伤害[s_dmg160]"]
    );
}

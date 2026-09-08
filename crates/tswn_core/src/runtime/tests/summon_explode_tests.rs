use super::*;

#[test]
fn summon_explode_combat_minion_emits_disappear_after_target_resolution() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "core",
            "summon",
            "core.kind.test-summon",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("summon kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 0),
            PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1).with_magic(80),
        ],
        builder.build(),
    ));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("summon explode should emit target and self-death frames");

    let messages = frame
        .updates
        .updates
        .iter()
        .map(|update| (update.message.as_ref(), update.score))
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        vec![
            ("[0]使用[自爆]", 0),
            ("[1]受到[2]点伤害", frame.updates.updates[1].score),
            ("\n", 0),
            ("[1]消失了", 50),
        ]
    );
}

#[test]
fn summon_explode_protect_redirect_applies_fire_stack_to_protector() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "target", 0, 1_000, 100),
            PlayerTemplate::new(2, "protector", 0, 1_000, 100),
            PlayerTemplate::new(3, "summon", 1, 5, 1).with_magic(100),
            PlayerTemplate::new(4, "summon owner", 1, 1_000, 100),
        ],
        config.registry,
    ));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.magic_point = 100;
    {
        let target = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime;
        target.protect_from.push(ProtectLinkRuntime {
            owner: EntityIdx(1),
            level: 256,
        });
        target.protect_pre_defend_skill_count = Some(0);
    }
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(0),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("protected summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.fire_mag(91), 0.0);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
    assert!(
        frame
            .updates
            .updates
            .iter()
            .any(|update| update.message == "[0][守护][1]" && update.caster == 1 && update.target == 0)
    );
}

#[test]
fn summon_explode_can_be_dodged_after_self_death() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "owner", 0, 10, 3),
        PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 512).with_agility(512),
        PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(0),
    ]));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::fire_mag(91, 3));
    let mut expected_rng = RC4::default();
    let _ = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng);
    assert!(PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("dodged summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 1.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
    assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[0][回避]了攻击");
    assert_eq!(frame.updates.updates[1].caster, 1);
    assert_eq!(frame.updates.updates[1].target, 2);
    assert_eq!(frame.updates.updates[1].score, 20);
}

#[test]
fn summon_explode_fire_stack_respects_boss_fire_immune() {
    let mut builder = ExtensionRegistryBuilder::default();
    let boss_kind = builder
        .register_player_kind_with_policies(
            "core",
            "boss",
            "core.boss",
            PlayerKindFlags::BOSS,
            PlayerKindPolicies::default(),
        )
        .expect("boss kind should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::with_kind(2, "saitama", boss_kind, 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::fire_mag(91, 3));
    let mut expected_rng = RC4::default();
    let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 5.5;
    assert!(!PlayerRuntime::dodge(
        runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
        &mut expected_rng
    ));
    let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let threshold = crate::namerena::boss_immune_threshold("saitama", "fire");
    assert!((expected_rng.next_u8() as i32) < threshold);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("boss immune summon explode should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 1.5);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
}

#[test]
fn summon_explode_skips_kill_hook_after_summon_self_death() {
    let mut builder = ExtensionRegistryBuilder::default();
    let die_skill = builder
        .register_skill_with_hooks(
            "custom",
            "die-skill",
            "custom.die_skill",
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("die skill should register");
    let kill_skill = builder
        .register_skill_with_hooks(
            "custom",
            "kill-skill",
            "custom.kill_skill",
            ProcMask::KILL,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("kill skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 3, 3).with_def_res(0, 0).with_skills([die_skill]),
            PlayerTemplate::new(3, "summon", 0, 5, 1)
                .with_magic(80)
                .with_skills([die_skill, kill_skill]),
            PlayerTemplate::new(4, "enemy-ally", 1, 100, 1),
        ],
        registry,
    ));
    runtime.set_skill_handler(die_skill, skill_marks_update);
    runtime.set_skill_handler(kill_skill, skill_marks_update);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("summon explode should emit hook updates");

    assert_eq!(frame.updates.updates.len(), 6);
    assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
    assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
    assert_eq!(frame.updates.updates[1].target, 1);
    assert_eq!(frame.updates.updates[3].message, "[1]被击倒了");
    assert_eq!(frame.updates.updates[4].message, "skill mark");
    assert_eq!(frame.updates.updates[4].caster, 1);
    assert_eq!(frame.updates.updates[4].score, die_skill.0);
    assert_eq!(frame.updates.updates[5].message, "skill mark");
    assert_eq!(frame.updates.updates[5].caster, 2);
    assert_eq!(frame.updates.updates[5].target, 2);
    assert_eq!(frame.updates.updates[5].score, die_skill.0);
    assert!(!frame.updates.updates.iter().any(|update| update.score == kill_skill.0));
}

#[test]
fn summon_explode_terminal_knockout_suppresses_self_disappear_replay() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "core",
            "summon",
            "core.kind.test-summon",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("summon kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 3, 3).with_def_res(0, 0),
            PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1).with_magic(80),
        ],
        builder.build(),
    ));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: EntityIdx(2),
        target: EntityIdx(1),
        fire_state_key: 91,
    });

    let frame = runtime
        .flush_effects()
        .expect("summon explode should emit lethal target and caster updates");

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![
            ("[0]使用[自爆]", 0),
            ("[1]受到[2]点伤害", frame.updates.updates[1].score),
            ("\n", 0),
            ("[1]被击倒了", 50),
        ]
    );
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
}

#[test]
fn summon_explode_owner_death_does_not_emit_duplicate_self_disappear() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "core",
            "summon",
            "core.kind.test-summon",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("summon kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 3, 3).with_def_res(0, 0),
            PlayerTemplate::new(2, "enemy-next", 1, 100, 3),
            PlayerTemplate::new(3, "enemy-tail", 1, 100, 3),
        ],
        builder.build(),
    ));
    let owner = EntityIdx(0);
    let summon = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(4, "summon", summon_kind, 0, 5, 1).with_magic(80),
        &runtime.registry,
        Some(owner),
        Some(owner),
    );
    runtime.world.add_spawned_alive(summon, 0);
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(owner));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(1)));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(2)));
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(summon));
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster: summon,
        target: owner,
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("summon explode should kill owner and summon");

    assert!(!runtime.entities.get(owner).unwrap().runtime.alive);
    assert!(!runtime.entities.get(summon).unwrap().runtime.alive);
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| update.message.as_ref() == "[1]消失了")
            .count(),
        1
    );
    assert_eq!(runtime.world.next_actor(&runtime.entities), Some(EntityIdx(1)));
}

#[test]
fn summon_explode_owner_death_cleans_alive_linked_minion_before_deferred_caster() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon_kind = builder
        .register_player_kind_with_policies(
            "core",
            "summon",
            "core.kind.test-summon",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("summon kind should register");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "owner", 0, 3, 3).with_def_res(0, 0),
            PlayerTemplate::new(2, "enemy", 1, 100, 3),
        ],
        builder.build(),
    ));
    let owner = EntityIdx(0);
    let caster = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(3, "self-destruct caster", summon_kind, 0, 5, 1).with_magic(80),
        &runtime.registry,
        Some(owner),
        Some(owner),
    );
    runtime.world.add_spawned_alive(caster, 0);
    let linked = runtime.entities.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(4, "linked minion", summon_kind, 0, 5, 1),
        &runtime.registry,
        Some(owner),
        Some(owner),
    );
    runtime.world.add_spawned_alive(linked, 0);
    runtime.effects.push(QueuedEffect::SummonExplode {
        caster,
        target: owner,
        fire_state_key: 91,
    });

    let frame = runtime.flush_effects().expect("summon explode should clean both linked minions");

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| update.message.as_ref() == "[1]消失了")
            .map(|update| update.target)
            .collect::<Vec<_>>(),
        vec![linked.0 as usize, caster.0 as usize]
    );
}

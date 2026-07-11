use super::*;

fn assassinate_runtime() -> (CombatRuntime, SkillId) {
    let mut builder = ExtensionRegistryBuilder::default();
    let assassinate = builder
        .register_skill(
            "core",
            "assassinate",
            BuiltinActiveSkill::Assassinate.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(21),
        )
        .expect("assassinate skill should register");
    let registry = builder.build();
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 1_000, 40)
                .with_magic(80)
                .with_magic_point(128)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(assassinate, 128)])),
            PlayerTemplate::new(2, "target", 1, 1_000_000, 3).with_def_res(0, 0),
        ],
        registry,
    ));
    (runtime, assassinate)
}

#[test]
fn plain_assassinate_first_phase_matches_single_enemy_rng_and_enters_pending() {
    let (mut runtime, _) = assassinate_runtime();
    let mut expected_rng = runtime.rng.clone();
    assert!(expected_rng.r127() < 128);
    let all_alive = vec![EntityIdx(0), EntityIdx(1)];
    for _ in 0..4 {
        assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0]), Some(1));
    }
    let _ = expected_rng.rFFFF();

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("assassinate should be selected");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Assassinate);
    assert_eq!(prepared.targets, vec![EntityIdx(1)]);
    assert_rng_state_eq(&runtime.rng, &expected_rng);

    let move_before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points;
    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(
        owner.runtime.assassinate,
        Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(1),
            break_on_damage: true,
        })
    );
    assert_eq!(owner.runtime.move_state.speed_points, move_before + owner.runtime.magic * 3);
    assert_eq!(owner.template.skills.pre_action_order(), &[0]);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0][潜行]到[1]身后"]
    );
}

#[test]
fn plain_assassinate_forced_backstab_skips_mp_gate_and_normal_dodge() {
    let (mut runtime, _) = assassinate_runtime();
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(1),
            break_on_damage: true,
        });
        owner.template.skills.ensure_pre_action_lane(0);
    }
    let rng_before = runtime.rng.clone();
    let mp_before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point;
    let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
    let prepared = runtime
        .prepare_plain_action(EntityIdx(0), false, pre_action)
        .expect("pending assassinate should force an action");
    assert_rng_state_eq(&runtime.rng, &rng_before);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point, mp_before);

    let mut expected_rng = runtime.rng.clone();
    let owner = &runtime.entities.get(EntityIdx(0)).unwrap().runtime;
    let at1 = owner.get_at(true, &mut expected_rng);
    let at2 = owner.get_at(true, &mut expected_rng);
    let at3 = owner.get_at(true, &mut expected_rng);
    let expected_damage =
        (at1.max(at2).max(at3) * 4.0 / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
    let mut updates = RunUpdates::new();
    let PreparedPlainAction::BuiltinSkill(prepared) = prepared else {
        panic!("pending assassinate should prepare a builtin skill");
    };
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(
        runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp,
        1_000_000 - expected_damage
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.assassinate, None);
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.pre_action_order().is_empty());
    assert_eq!(updates.updates.first().unwrap().message, "[0]发动[背刺]");
    assert!(updates.updates.last().unwrap().message.starts_with("[1]受到[2]点伤害"));
}

#[test]
fn plain_assassinate_forced_backstab_records_actor_boundary_target() {
    let (mut runtime, assassinate) = assassinate_runtime();
    runtime.set_skill_handler(assassinate, skill_noop);
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(1),
            break_on_damage: true,
        });
        owner.template.skills.ensure_pre_action_lane(0);
        owner.runtime.move_state.speed_points = crate::player::MOVE_POINT_THRESHOLD + 1;
    }
    runtime.scheduler.set_action_mode(ActionSchedulerMode::LegacyStep);

    let outcome = runtime.run_minimal_round_once().expect("forced backstab should produce a round");

    let action = outcome.action.expect("forced pre-action skill should record an action boundary");
    assert_eq!(action.actor, EntityIdx(0));
    assert_eq!(action.target, EntityIdx(0));
    assert_eq!(action.amount, 0);
    let frame = outcome.frame.expect("forced backstab should emit replay frames");
    assert_eq!(frame.updates.updates.first().unwrap().message, "[0]发动[背刺]");
    assert_eq!(frame.updates.updates.first().unwrap().target, 1);
}

#[test]
fn plain_assassinate_forced_backstab_keeps_frozen_target() {
    let (mut runtime, _) = assassinate_runtime();
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(1),
            break_on_damage: true,
        });
        owner.template.skills.ensure_pre_action_lane(0);
    }
    {
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
        assert!(target.runtime.active());
        assert!(!target.is_active());
    }

    let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
    let prepared = runtime
        .prepare_plain_action(EntityIdx(0), false, pre_action)
        .expect("frozen pending target should still force assassinate");
    let hp_before = runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();
    let PreparedPlainAction::BuiltinSkill(prepared) = prepared else {
        panic!("frozen pending target should prepare a builtin skill");
    };
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < hp_before);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.assassinate, None);
    assert_eq!(updates.updates.first().unwrap().message, "[0]发动[背刺]");
}

#[test]
fn plain_assassinate_forced_backstab_skips_reflect_pre_defend_rng() {
    let mut builder = ExtensionRegistryBuilder::default();
    let assassinate = builder
        .register_skill(
            "core",
            "assassinate",
            BuiltinActiveSkill::Assassinate.export_name(),
            TargetPolicy::Enemy,
            SkillPriority(21),
        )
        .expect("assassinate skill should register");
    let reflect = builder
        .register_skill_with_hooks(
            "core",
            "reflect",
            DEFAULT_CORE_REFLECT_SKILL_EXPORT,
            ProcMask::PRE_DEFEND,
            TargetPolicy::None,
            SkillPriority(1_000),
        )
        .expect("reflect skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 1_000, 40)
                .with_magic(80)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(assassinate, 128)])),
            PlayerTemplate::new(2, "reflector", 1, 1_000_000, 3)
                .with_def_res(0, 0)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 1)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(reflect, run_reflect_pre_defend_skill);
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(1),
            break_on_damage: true,
        });
        owner.template.skills.ensure_pre_action_lane(0);
    }

    let mut expected_rng = runtime.rng.clone();
    let owner = &runtime.entities.get(EntityIdx(0)).unwrap().runtime;
    owner.get_at(true, &mut expected_rng);
    owner.get_at(true, &mut expected_rng);
    owner.get_at(true, &mut expected_rng);
    let hp_before = runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_assassinate_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < hp_before);
    assert!(updates.updates.iter().all(|update| update.message.as_ref() != "[0]使用[伤害反弹]"));
    assert!(runtime.effects.is_empty());
}

#[test]
fn plain_assassinate_breaks_on_damage_unless_charge_was_active() {
    let (mut runtime, _) = assassinate_runtime();
    let mut updates = RunUpdates::new();
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.assassinate = Some(AssassinateRuntime {
        fixed_lane: 0,
        target: EntityIdx(1),
        break_on_damage: true,
    });
    runtime.entities.get_mut(EntityIdx(0)).unwrap().template.skills.ensure_pre_action_lane(0);

    runtime.run_plain_assassinate_post_damage_into(EntityIdx(0), 1, &mut updates);

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.assassinate, None);
    assert_eq!(
        updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["\n", "[0]的[潜行]被识破"]
    );

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.charge.active = true;
    let move_before = runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points;
    let mut charged_updates = RunUpdates::new();
    runtime.drain_plain_assassinate_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut charged_updates);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(
        owner.runtime.move_state.speed_points,
        move_before + owner.runtime.magic * 3 + 1600
    );
    assert_eq!(owner.runtime.assassinate.unwrap().break_on_damage, false);

    runtime.run_plain_assassinate_post_damage_into(EntityIdx(0), 10, &mut charged_updates);
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.assassinate.is_some());
    assert_eq!(charged_updates.updates.len(), 1);
}

#[test]
fn plain_assassinate_smart_poison_gate_consumes_no_rng() {
    let (mut runtime, _) = assassinate_runtime();
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::poison(
        PLAIN_POISON_STATE_KEY,
        StateId(0),
        Some(1),
        Some(0),
        10.0,
        2,
        SkillPriority(150),
    ));
    let expected_rng = runtime.rng.clone();

    assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Assassinate, 128, true,));
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn plain_assassinate_dead_target_clears_pending_and_forced_action() {
    let (mut runtime, _) = assassinate_runtime();
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane: 0,
            target: EntityIdx(1),
            break_on_damage: true,
        });
        owner.template.skills.ensure_pre_action_lane(0);
        owner.states.add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 2));
    }
    {
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 0;
        target.runtime.alive = false;
    }

    let outcome = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));

    assert!(outcome.forced_skill.is_none());
    assert!(outcome.clear_forced_action);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(owner.runtime.assassinate, None);
    assert!(owner.template.skills.pre_action_order().is_empty());
}

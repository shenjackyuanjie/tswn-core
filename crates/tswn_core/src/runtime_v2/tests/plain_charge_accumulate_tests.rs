use super::*;

#[test]
fn plain_charge_selects_self_executes_and_ticks_in_late_post_action() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            BuiltinActiveSkill::Charge.export_name(),
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(19),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let registry = builder.build();
    let loadout = SkillLoadout::from_skill_levels([(charge, 128)]);
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3).with_skill_loadout(loadout),
            PlayerTemplate::new(2, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(charge, run_charge_post_action_skill);

    let prepared = runtime
        .scan_plain_action_skill_probabilities(EntityIdx(0), false)
        .expect("charge should be selected when probability passes");
    assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Charge);
    assert_eq!(prepared.targets.as_slice(), &[EntityIdx(0)]);

    let mut updates = RunUpdates::new();
    runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(updates.updates[0].message, "[0]开始[蓄力]");
    assert_eq!(owner.runtime.magic_point, 32);
    assert!(owner.runtime.charge.active);
    assert_eq!(owner.runtime.charge.step, 2);
    assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);

    let plan = runtime.scheduler.skill_post_action_hook_plan(
        &runtime.entities,
        &runtime.registry,
        EntityIdx(0),
        SkillPostActionPhase::Late,
    );
    runtime.drain_skill_hook_plan_into(&plan, &mut updates);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.charge.active);
    assert_eq!(owner.runtime.charge.step, 1);
}

#[test]
fn plain_reraise_revives_halves_level_and_stops_kill_hooks() {
    let mut builder = ExtensionRegistryBuilder::default();
    let kill = builder
        .register_skill_with_hooks(
            "custom",
            "kill-marker",
            "custom.kill_marker",
            ProcMask::KILL,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("kill marker should register");
    let reraise = builder
        .register_skill_with_hooks(
            "core",
            "reraise",
            DEFAULT_CORE_RERAISE_SKILL_EXPORT,
            ProcMask::DIE,
            TargetPolicy::None,
            SkillPriority(10),
        )
        .expect("reraise skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "killer", 0, 100, 3).with_skills([kill]),
            PlayerTemplate::new(2, "target", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
        ],
        registry,
    ));
    runtime.set_skill_handler(kill, skill_marks_selected_target);
    runtime.set_skill_handler(reraise, run_reraise_die_skill);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;

    let mut updates = RunUpdates::new();
    runtime.drain_plain_lethal_damage_into(EntityIdx(0), EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.runtime.alive);
    assert!((1..=16).contains(&target.runtime.hp));
    assert_eq!(target.template.skills.level_at(0), Some(64));
    assert!(runtime.world.flat_alive().contains(&EntityIdx(1)));
    assert_eq!(
        updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec!["[1]被击倒了", "[0]使用[护身符]抵挡了一次死亡", "[1]回复体力[2]点"]
    );
    assert!(!updates.updates.iter().any(|update| update.message == "selected target"));
}

#[test]
fn run_skill_hooks_charge_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            "core.charge",
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([charge])],
        registry,
    ));
    runtime.set_skill_handler(charge, run_charge_post_action_skill);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().activate_charge_runtime();

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::POST_ACTION);
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert!(frame.is_none());
    assert_eq!(
        owner.runtime.charge,
        crate::runtime_v2::entity::ChargeRuntime {
            active: true,
            post_action_active: true,
            step: 1,
        }
    );
    assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);
}

#[test]
fn charge_activation_refreshes_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed(100)],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::haste_with_effective_faster(77, haste, 4, 2, 9, SkillPriority(100)));

    runtime.entities.get_mut(EntityIdx(0)).unwrap().activate_charge_runtime();

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.charge.active);
    assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);
    assert_eq!(
        owner.states.entry(77).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(owner.effective_speed(), 400);
}

#[test]
fn charge_expiry_refreshes_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed(100)],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.runtime.charge.step = 1;
        owner
            .states
            .add_entry(StateEntry::haste_with_effective_faster(77, haste, 4, 2, 9, SkillPriority(100)));
    }

    assert!(runtime.entities.get_mut(EntityIdx(0)).unwrap().tick_charge_post_action());

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(!owner.runtime.charge.active);
    assert!(!owner.runtime.charge.post_action_active);
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
    assert_eq!(
        owner.states.entry(77).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(owner.effective_speed(), 400);
}

#[test]
fn accumulate_activation_refreshes_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed(100)],
        registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::haste_with_effective_faster(77, haste, 4, 2, 9, SkillPriority(100)));

    assert!(runtime.entities.get_mut(EntityIdx(0)).unwrap().activate_accumulate_runtime());

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(owner.runtime.accumulate.active);
    assert_eq!(
        owner.states.entry(77).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(owner.effective_speed(), 400);
}

#[test]
fn accumulate_clear_refreshes_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed(100)],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_accumulate_runtime();
        owner
            .states
            .add_entry(StateEntry::haste_with_effective_faster(77, haste, 4, 2, 9, SkillPriority(100)));
    }

    assert!(runtime.entities.get_mut(EntityIdx(0)).unwrap().clear_accumulate_runtime());

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(!owner.runtime.accumulate.active);
    assert_eq!(
        owner.states.entry(77).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(owner.effective_speed(), 400);
}

#[test]
fn run_minimal_round_charge_late_post_action_clears_after_state_hooks() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            "core.charge",
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([charge]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.runtime.charge.step = 1;
        owner.states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
    }
    runtime.set_skill_handler(charge, run_charge_post_action_skill);
    runtime.set_state_handler(state, state_marks_charge_boost);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("attack and charge-observing state should emit updates");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]攻击[1]", "charge boosted"]
    );
    assert_eq!(
        owner.runtime.charge,
        crate::runtime_v2::entity::ChargeRuntime {
            active: false,
            post_action_active: false,
            step: 0,
        }
    );
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
}

#[test]
fn run_skill_hooks_accumulate_activates_runtime_and_boosts_move() {
    let mut builder = ExtensionRegistryBuilder::default();
    let accumulate = builder
        .register_skill_with_hooks(
            "core",
            "accumulate",
            "core.accumulate",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("accumulate skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(100).with_skills([accumulate])],
        registry,
    ));
    runtime.set_skill_handler(accumulate, run_accumulate_skill);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("accumulate act should emit updates");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[0]开始[聚气]", 1), ("[0]攻击力上升", 0)]
    );
    assert!(owner.runtime.accumulate.active);
    assert_eq!(owner.runtime.accumulate.charge_bonus(), 0.0);
    assert_eq!(owner.runtime.move_state.speed_points, 500);
    assert_eq!(owner.runtime.at_boost_millionths, 1_700_000);
}

#[test]
fn run_minimal_round_accumulate_uses_charge_bonus_until_late_charge_clear() {
    let mut builder = ExtensionRegistryBuilder::default();
    let accumulate = builder
        .register_skill_with_hooks(
            "core",
            "accumulate",
            "core.accumulate",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("accumulate skill should register");
    let charge = builder
        .register_skill_with_hooks_and_post_action_phase(
            "core",
            "charge",
            "core.charge",
            ProcMask::POST_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("charge skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3)
                .with_speed_points(100)
                .with_skills([accumulate, charge]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.runtime.charge.step = 1;
    }
    runtime.set_skill_handler(accumulate, run_accumulate_skill);
    runtime.set_skill_handler(charge, run_charge_post_action_skill);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("accumulate, attack, and charge tick should emit frame");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
        vec!["[0]开始[聚气]", "[0]攻击力上升", "[0]攻击[1]"]
    );
    assert!(owner.runtime.accumulate.active);
    assert_eq!(owner.runtime.accumulate.charge_bonus(), 1.0);
    assert_eq!(owner.runtime.move_state.speed_points, 1000);
    assert_eq!(owner.runtime.charge.active, false);
    assert_eq!(owner.runtime.at_boost_millionths, 2_700_000);
}

#[test]
fn run_skill_hooks_clear_positive_runtime_orders_accumulate_before_charge() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive",
            "custom.clear_positive",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.activate_accumulate_runtime();
    }
    runtime.set_skill_handler(clear, skill_clears_positive_runtime);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("clear-positive runtime should emit messages");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![(("[1]的[聚气]被打消了"), 100), (("[1]的[蓄力]被中止了"), 200)]
    );
    assert!(!owner.runtime.accumulate.active);
    assert!(!owner.runtime.charge.active);
    assert_eq!(owner.runtime.accumulate.acc(), 1.600000023841858);
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
}

#[test]
fn run_skill_hooks_clear_positive_states_removes_shield_and_orders_messages() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive-states",
            "custom.clear_positive_states",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let shield = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        store.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
        store.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    }
    runtime.set_skill_handler(clear, skill_clears_positive_states);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("clear-positive state messages should emit");
    let store = &runtime.entities.get(EntityIdx(0)).unwrap().states;

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[1]从[疾走]中解除", 300), ("[1]的[铁壁]被打消了", 400)]
    );
    assert_eq!(store.entry(74), None);
    assert_eq!(store.entry(77), None);
    assert_eq!(store.entry(79), None);
}

#[test]
fn run_skill_hooks_clear_positive_states_suppresses_dead_haste_message() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive-states",
            "custom.clear_positive_states",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.runtime.alive = false;
        owner.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        owner.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
    }
    runtime.set_skill_handler(clear, skill_clears_positive_states);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("iron clear message should emit");
    let store = &runtime.entities.get(EntityIdx(0)).unwrap().states;

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![("[1]的[铁壁]被打消了", 400)]
    );
    assert_eq!(store.entry(77), None);
    assert_eq!(store.entry(79), None);
}

#[test]
fn run_skill_hooks_clear_positive_combines_runtime_and_state_messages() {
    let mut builder = ExtensionRegistryBuilder::default();
    let clear = builder
        .register_skill_with_hooks(
            "custom",
            "clear-positive",
            "custom.clear_positive",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("clear-positive skill should register");
    let shield = builder
        .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
        .expect("shield state should register");
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron = builder
        .register_state(
            "core",
            "iron",
            "core.iron",
            ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            SkillPriority(10),
        )
        .expect("iron state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.activate_charge_runtime();
        owner.activate_accumulate_runtime();
        owner.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        owner.states.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
        owner.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
    }
    runtime.set_skill_handler(clear, skill_clears_positive);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("combined clear-positive should emit messages");
    let owner = runtime.entities.get(EntityIdx(0)).unwrap();

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![
            ("[1]的[聚气]被打消了", 100),
            ("[1]的[蓄力]被中止了", 200),
            ("[1]从[疾走]中解除", 300),
            ("[1]的[铁壁]被打消了", 400),
        ]
    );
    assert!(!owner.runtime.accumulate.active);
    assert!(!owner.runtime.charge.active);
    assert_eq!(owner.runtime.accumulate.acc(), 1.600000023841858);
    assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
    assert_eq!(owner.states.entry(74), None);
    assert_eq!(owner.states.entry(77), None);
    assert_eq!(owner.states.entry(79), None);
}

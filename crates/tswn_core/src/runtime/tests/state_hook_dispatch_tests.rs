use super::*;

#[test]
fn run_state_hooks_dispatches_registered_state_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 42,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_marks_update);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("state handler should emit update");

    assert_eq!(frame.updates.updates[0].message, "state mark");
    assert_eq!(frame.updates.updates[0].score, 42);
}

#[test]
fn run_state_hooks_exposes_controlled_rng_to_state_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state(
            "custom",
            "rng-state",
            "custom.rng_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 88,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_consumes_rng);
    let mut expected_rng = RC4::default();
    let expected_value = expected_rng.next_i32(10);
    let expected_byte = expected_rng.next_u8();

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("state rng handler should emit update");

    assert_eq!(
        frame.updates.updates[0].message,
        format!("state-rng:{expected_value}:{expected_byte}")
    );
    assert_eq!(frame.updates.updates[0].score, expected_value as u32);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
}

#[test]
fn run_state_hooks_flushes_nested_effects_and_skips_legacy_entries() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_legacy_key(11);
        store.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(state, state_pushes_nested_heal);

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("state heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[0].score, 2);
}

#[test]
fn run_state_hooks_iron_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
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
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 3, SkillPriority(10)));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(79)
            .and_then(StateEntry::iron_value),
        Some((300, 2))
    );
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        2048
    );
}

#[test]
fn run_state_hooks_haste_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
        77,
        haste_state,
        4,
        3,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(77)
            .and_then(StateEntry::haste_value),
        Some((4, 2))
    );
}

#[test]
fn run_state_hooks_haste_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
        77,
        haste_state,
        2,
        1,
        SkillPriority(100),
    ));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("haste release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(77), None);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[疾走]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_haste_post_action_clears_dead_owner_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
        77,
        haste_state,
        2,
        1,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(77), None);
}

#[test]
fn run_state_hooks_charm_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
        76,
        charm_state,
        7,
        Some(1),
        Some(2),
        Some(3),
        3,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(76)
            .and_then(StateEntry::charm_value),
        Some((7, Some(1), Some(2), Some(3), 2))
    );
}

#[test]
fn run_state_hooks_charm_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
        76,
        charm_state,
        7,
        Some(1),
        Some(2),
        Some(3),
        1,
        SkillPriority(100),
    ));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("charm release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76), None);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[魅惑]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_charm_post_action_clears_dead_owner_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
        76,
        charm_state,
        7,
        Some(1),
        Some(2),
        Some(3),
        1,
        SkillPriority(100),
    ));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76), None);
}

#[test]
fn run_state_hooks_slow_post_action_decrements_step_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let slow_state = builder
        .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("slow state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(slow_state, run_slow_post_action_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::slow(78, slow_state, 2, SkillPriority(100)));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(0))
            .unwrap()
            .states
            .entry(78)
            .and_then(StateEntry::slow_value),
        Some(1)
    );
}

#[test]
fn run_state_hooks_slow_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let slow_state = builder
        .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("slow state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
        registry,
    ));
    runtime.set_state_handler(slow_state, run_slow_post_action_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::slow(78, slow_state, 1, SkillPriority(100)));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("slow release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(78), None);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[迟缓]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_iron_post_action_clears_and_emits_release() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
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
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));

    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("iron release should emit updates");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(79), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        1920
    );
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(
        frame.updates.updates[0].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[1].message, "[1]从[铁壁]中解除");
    assert_eq!(frame.updates.updates[1].caster, 0);
    assert_eq!(frame.updates.updates[1].target, 0);
}

#[test]
fn run_state_hooks_iron_post_action_clears_expired_without_update() {
    let mut builder = ExtensionRegistryBuilder::default();
    let iron_state = builder
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
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 0, SkillPriority(10)));

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(79), None);
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        2048
    );
}

#[test]
fn run_state_hooks_iron_step_does_not_refresh_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let iron_state = builder
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
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed(100)],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.states.add_entry(StateEntry::haste_with_effective_faster(
            77,
            haste_state,
            4,
            2,
            9,
            SkillPriority(100),
        ));
        owner.states.add_entry(StateEntry::iron(79, iron_state, 300, 3, SkillPriority(10)));
    }
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);

    let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

    let owner = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(frame.is_none());
    assert_eq!(
        owner.states.entry(77).and_then(StateEntry::haste_runtime_value),
        Some((4, 2, 8))
    );
    assert_eq!(owner.states.entry(79).and_then(StateEntry::iron_value), Some((300, 2)));
    assert_eq!(owner.effective_speed(), 200);
}

#[test]
fn run_state_hooks_iron_post_action_runs_at_legacy_priority() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker_state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("marker state should register");
    let iron_state = builder
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
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 42,
        extension_state_id: Some(marker_state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(100),
        registration_order: RegistrationOrder(1),
        payload: StatePayload::None,
    });
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));
    runtime.set_state_handler(marker_state, state_marks_update);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);

    let plan = runtime.scheduler.state_hook_plan(&runtime.entities, EntityIdx(0), ProcMask::POST_ACTION);
    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("marker and iron release should emit updates");

    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| (entry.legacy_order_key, entry.priority))
            .collect::<Vec<_>>(),
        vec![(42, SkillPriority(100)), (79, SkillPriority(210))]
    );
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "state mark");
    assert_eq!(
        frame.updates.updates[1].update_type,
        crate::runtime::update::UpdateType::NextLine
    );
    assert_eq!(frame.updates.updates[2].message, "[1]从[铁壁]中解除");
}

#[test]
fn run_state_hooks_haste_charm_slow_and_iron_share_legacy_post_action_priority() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker_state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("marker state should register");
    let poison_state = builder
        .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("poison state should register");
    let haste_state = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let charm_state = builder
        .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("charm state should register");
    let slow_state = builder
        .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("slow state should register");
    let iron_state = builder
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
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(marker_state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(100),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(0),
            Some(0),
            80.0,
            2,
            SkillPriority(0),
        ));
        store.add_entry(StateEntry::haste(77, haste_state, 2, 1, SkillPriority(100)));
        store.add_entry(StateEntry::charm(
            76,
            charm_state,
            7,
            Some(1),
            Some(2),
            Some(3),
            1,
            SkillPriority(100),
        ));
        store.add_entry(StateEntry::slow(78, slow_state, 1, SkillPriority(100)));
        store.add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));
    }
    runtime.set_state_handler(marker_state, state_marks_update);
    runtime.set_state_handler(poison_state, run_poison_post_action_state);
    runtime.set_state_handler(haste_state, run_haste_post_action_state);
    runtime.set_state_handler(charm_state, run_charm_post_action_state);
    runtime.set_state_handler(slow_state, run_slow_post_action_state);
    runtime.set_state_handler(iron_state, run_iron_post_defend_state);

    let plan = runtime.scheduler.state_hook_plan(&runtime.entities, EntityIdx(0), ProcMask::POST_ACTION);
    let frame = runtime
        .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
        .expect("marker and timed release states should emit updates");

    assert_eq!(
        plan.entries
            .iter()
            .map(|entry| (entry.legacy_order_key, entry.priority))
            .collect::<Vec<_>>(),
        vec![
            (42, SkillPriority(100)),
            (75, SkillPriority(150)),
            (77, SkillPriority(210)),
            (76, SkillPriority(210)),
            (78, SkillPriority(210)),
            (79, SkillPriority(210)),
        ]
    );
    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::runtime::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>(),
        vec![
            "state mark",
            "[1][毒性发作]",
            "[1]受到[2]点伤害",
            "[1]从[疾走]中解除",
            "[1]从[魅惑]中解除",
            "[1]从[迟缓]中解除",
            "[1]从[铁壁]中解除",
        ]
    );
}

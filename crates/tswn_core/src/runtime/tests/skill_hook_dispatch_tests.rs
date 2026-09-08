use super::*;

#[test]
fn run_skill_hooks_dispatches_registered_skill_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker = builder
        .register_skill_with_hooks(
            "custom",
            "marker",
            "custom.marker",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker])],
        registry,
    ));
    runtime.set_skill_handler(marker, skill_marks_update);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("skill handler should emit update");

    assert_eq!(frame.updates.updates[0].message, "skill mark");
    assert_eq!(frame.updates.updates[0].score, marker.0);
}

#[test]
fn run_skill_hooks_exposes_controlled_rng_to_skill_handlers() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "rng-skill",
            "custom.rng_skill",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill])],
        registry,
    ));
    runtime.set_skill_handler(skill, skill_consumes_rng);
    let mut expected_rng = RC4::default();
    let expected_value = expected_rng.next_i32(10);
    let expected_byte = expected_rng.next_u8();

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("skill rng handler should emit update");

    assert_eq!(
        frame.updates.updates[0].message,
        format!("skill-rng:{expected_value}:{expected_byte}")
    );
    assert_eq!(frame.updates.updates[0].score, expected_value as u32);
    assert_eq!(runtime.rng.i, expected_rng.i);
    assert_eq!(runtime.rng.j, expected_rng.j);
    assert_eq!(runtime.rng.main_val, expected_rng.main_val);
}

#[test]
fn run_skill_hooks_flushes_nested_effects() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "damage",
            "custom.damage",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(skill, skill_pushes_nested_damage);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("nested damage should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 8);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[0].score, 2);
}

#[test]
fn run_skill_hooks_disperse_without_selected_target_noops() {
    let mut builder = ExtensionRegistryBuilder::default();
    let disperse = builder
        .register_skill_with_hooks(
            "core",
            "disperse",
            "core.disperse",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("disperse skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([disperse]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(disperse, run_disperse_skill);

    let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

    assert!(frame.is_none());
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
}

#[test]
fn run_minimal_round_dispatches_pre_action_skill_before_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let marker = builder
        .register_skill_with_hooks(
            "custom",
            "marker",
            "custom.marker",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(marker, skill_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("skill plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "skill mark");
    assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_flushes_pre_action_skill_effect_before_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "damage",
            "custom.damage",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(skill, skill_pushes_nested_damage);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("skill damage plus attack should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 5);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].score, 2);
    assert_eq!(frame.updates.updates[1].score, 3);
}

#[test]
fn run_minimal_round_dispatches_damage_skill_hooks_around_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let pre_damage = builder
        .register_skill_with_hooks(
            "custom",
            "pre-damage",
            "custom.pre_damage",
            ProcMask::PRE_DAMAGE,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("pre-damage skill should register");
    let post_damage = builder
        .register_skill_with_hooks(
            "custom",
            "post-damage",
            "custom.post_damage",
            ProcMask::POST_DAMAGE,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("post-damage skill should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([pre_damage, post_damage]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.set_skill_handler(pre_damage, skill_marks_update);
    runtime.set_skill_handler(post_damage, skill_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("damage skill hooks plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "skill mark");
    assert_eq!(frame.updates.updates[0].score, pre_damage.0);
    assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[2].message, "skill mark");
    assert_eq!(frame.updates.updates[2].score, post_damage.0);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_dispatches_post_action_skill_before_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill_with_hooks(
            "custom",
            "post-action-skill",
            "custom.post_action_skill",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("post-action skill should register");
    let state = builder
        .register_state(
            "custom",
            "post-action-state",
            "custom.post_action_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 55,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_skill_handler(skill, skill_marks_update);
    runtime.set_state_handler(state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-action skill and state plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "skill mark");
    assert_eq!(frame.updates.updates[1].score, skill.0);
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 55);
}

#[test]
fn run_minimal_round_dispatches_late_post_action_skill_after_state() {
    let mut builder = ExtensionRegistryBuilder::default();
    let early = builder
        .register_skill_with_hooks(
            "custom",
            "early-post-action",
            "custom.early_post_action",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("early post-action skill should register");
    let late = builder
        .register_skill_with_hooks_and_post_action_phase(
            "custom",
            "late-post-action",
            "custom.late_post_action",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
            SkillPostActionPhase::Late,
        )
        .expect("late post-action skill should register");
    let state = builder
        .register_state(
            "custom",
            "post-action-state",
            "custom.post_action_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([late, early]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 55,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_skill_handler(early, skill_marks_update);
    runtime.set_skill_handler(late, skill_marks_update);
    runtime.set_state_handler(state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-action hooks plus attack should emit update");

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![
            ("[0]攻击[1]", 3),
            ("skill mark", early.0),
            ("state mark", 55),
            ("skill mark", late.0),
        ]
    );
}

#[test]
fn run_minimal_round_interleaves_merge_registered_post_action_skill_between_existing_and_future_states() {
    let mut builder = ExtensionRegistryBuilder::default();
    let initial_skill = builder
        .register_skill_with_hooks(
            "custom",
            "initial-post-action",
            "custom.initial_post_action",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("initial post-action skill should register");
    let merged_skill = builder
        .register_skill_with_hooks(
            "custom",
            "merged-post-action",
            "custom.merged_post_action",
            ProcMask::POST_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("merged post-action skill should register");
    let existing_state = builder
        .register_state(
            "custom",
            "existing-post-action-state",
            "custom.existing_post_action_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("existing post-action state should register");
    let future_state = builder
        .register_state(
            "custom",
            "future-post-action-state",
            "custom.future_post_action_state",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("future post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([initial_skill, merged_skill]),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        owner.states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(existing_state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        let state_cursor = owner.states.post_action_registration_cursor();
        owner.template.skills.register_post_action_after_states(1, state_cursor);
        owner.states.add_entry(StateEntry {
            legacy_order_key: 66,
            extension_state_id: Some(future_state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_skill_handler(initial_skill, skill_marks_update);
    runtime.set_skill_handler(merged_skill, skill_marks_update);
    runtime.set_state_handler(existing_state, state_marks_update);
    runtime.set_state_handler(future_state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-action hooks plus attack should emit update");

    assert_eq!(
        frame
            .updates
            .updates
            .iter()
            .map(|update| (update.message.as_ref(), update.score))
            .collect::<Vec<_>>(),
        vec![
            ("[0]攻击[1]", 3),
            ("skill mark", initial_skill.0),
            ("state mark", 55),
            ("skill mark", merged_skill.0),
            ("state mark", 66),
        ]
    );
}

#[test]
fn run_minimal_round_dispatches_post_action_state_after_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 77,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("attack plus state hook should emit update");

    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "state mark");
    assert_eq!(frame.updates.updates[1].score, 77);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_flushes_post_action_state_effect_after_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let state = builder
        .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
        .expect("state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
    runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 88,
        extension_state_id: Some(state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    });
    runtime.set_state_handler(state, state_pushes_nested_heal);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("attack plus state heal should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    assert_eq!(frame.updates.updates.len(), 2);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[1].score, 2);
}

#[test]
fn run_minimal_round_dispatches_damage_state_hooks_around_attack() {
    let mut builder = ExtensionRegistryBuilder::default();
    let pre_damage = builder
        .register_state(
            "custom",
            "pre-damage",
            "custom.pre_damage",
            ProcMask::PRE_DAMAGE,
            SkillPriority(0),
        )
        .expect("pre-damage state should register");
    let post_damage = builder
        .register_state(
            "custom",
            "post-damage",
            "custom.post_damage",
            ProcMask::POST_DAMAGE,
            SkillPriority(0),
        )
        .expect("post-damage state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(pre_damage),
            hook_mask: ProcMask::PRE_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(post_damage),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(pre_damage, state_marks_update);
    runtime.set_state_handler(post_damage, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("damage state hooks plus attack should emit update");

    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "state mark");
    assert_eq!(frame.updates.updates[0].score, 11);
    assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 22);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
}

#[test]
fn run_minimal_round_flushes_post_damage_state_effect_before_post_action() {
    let mut builder = ExtensionRegistryBuilder::default();
    let post_damage = builder
        .register_state(
            "custom",
            "post-damage-regen",
            "custom.post_damage_regen",
            ProcMask::POST_DAMAGE,
            SkillPriority(0),
        )
        .expect("post-damage state should register");
    let post_action = builder
        .register_state(
            "custom",
            "post-action-marker",
            "custom.post_action_marker",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ],
        registry,
    ));
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
    {
        let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        store.add_entry(StateEntry {
            legacy_order_key: 33,
            extension_state_id: Some(post_damage),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        store.add_entry(StateEntry {
            legacy_order_key: 44,
            extension_state_id: Some(post_action),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
    }
    runtime.set_state_handler(post_damage, state_pushes_nested_heal);
    runtime.set_state_handler(post_action, state_marks_update);

    let outcome = runtime.run_minimal_round();
    let frame = outcome.frame.expect("post-damage effect plus post-action state should emit update");

    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
    assert_eq!(frame.updates.updates.len(), 3);
    assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
    assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
    assert_eq!(frame.updates.updates[2].message, "state mark");
    assert_eq!(frame.updates.updates[2].score, 44);
}

use super::*;

fn counter_runtime(level: u32) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let counter = builder
        .register_skill_with_hooks(
            "core",
            "counter",
            DEFAULT_CORE_COUNTER_SKILL_EXPORT,
            ProcMask::POST_DAMAGE,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("counter skill should register");
    let registry = builder.build();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "attacker", 0, 1_000, 100),
            PlayerTemplate::new(2, "counter", 1, 1_000, 100)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(counter, level)])),
        ],
        registry,
    ))
}

#[test]
fn plain_counter_update_end_skips_frozen_owner_without_mp_rng() {
    let mut runtime = counter_runtime(255);
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.active());
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().is_active());

    let mut updates = RunUpdates::new();
    {
        let counter = &mut runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.counter;
        counter.pending = true;
        counter.last_target = Some(EntityIdx(0));
        counter.last_updates_id = Some(updates.id);
    }
    updates.on_update_end.push(1);

    let rng_after_counter_schedule = runtime.rng.clone();
    runtime.drain_plain_update_end_into(&mut updates);

    assert_rng_state_eq(&runtime.rng, &rng_after_counter_schedule);
    assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.counter.pending);
    assert!(updates.on_update_end.is_empty());
    assert!(!updates.updates.iter().any(|update| update.message == "[0]发起[反击][s_counter]"));
}

#[test]
fn plain_counter_update_end_can_attack_a_frozen_living_target() {
    let mut runtime = counter_runtime(255);
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.magic_point = 100;
    let mut updates = RunUpdates::new();
    {
        let counter = &mut runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.counter;
        counter.pending = true;
        counter.last_target = Some(EntityIdx(0));
        counter.last_updates_id = Some(updates.id);
    }
    updates.on_update_end.push(1);

    runtime.drain_plain_update_end_into(&mut updates);

    assert!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
    assert!(runtime.entities.get(EntityIdx(0)).unwrap().states.is_frozen());
    assert!(updates.updates.iter().any(|update| update.message == "[0]发起[反击][s_counter]"));
}

#[test]
fn plain_counter_post_damage_requests_a_target_decision() {
    let mut runtime = counter_runtime(255);
    runtime.control.set_passive_decision_mode(true);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.magic_point = 100;
    let mut updates = RunUpdates::new();
    let rng_before = runtime.rng.clone();

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.run_plain_counter_post_damage_into(EntityIdx(1), 255, 10, EntityIdx(0), &mut updates);
    }));
    assert!(panic.is_err());
    let request = runtime.control.pending_passive().expect("counter should pause for a decision");
    assert_eq!(request.actor, 1);
    assert_eq!(request.target, Some(0));
    assert_eq!(request.initiator, Some(0));
    assert_eq!(request.skill, DEFAULT_CORE_COUNTER_SKILL_EXPORT);
    assert_eq!(request.mp_costs[DEFAULT_CORE_COUNTER_SKILL_EXPORT], rng_before.clone().r3x3());
    assert!(updates.updates.is_empty());
    assert!(updates.on_update_end.is_empty());
    assert_rng_state_eq(&runtime.rng, &rng_before);
}

#[test]
fn plain_counter_decision_success_queues_the_counter_attack() {
    let mut runtime = counter_runtime(255);
    runtime.control.set_passive_decision_mode(true);
    runtime.control.set_passive_decisions([PassiveDecision {
        actor: 1,
        skill: DEFAULT_CORE_COUNTER_SKILL_EXPORT.to_owned(),
        activate: true,
        success: true,
        target: None,
    }]);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.magic_point = 100;
    let mut updates = RunUpdates::new();

    runtime.run_plain_counter_post_damage_into(EntityIdx(1), 255, 10, EntityIdx(0), &mut updates);

    assert_eq!(updates.on_update_end.as_slice(), &[1]);
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.counter.pending);
}

#[test]
fn manual_counter_keeps_quoted_mp_cost_and_uses_counter_owner_as_initiator() {
    let mut runtime = counter_runtime(255);
    runtime.control.set_passive_decision_mode(true);
    runtime.control.set_passive_decisions([PassiveDecision {
        actor: 1,
        skill: DEFAULT_CORE_COUNTER_SKILL_EXPORT.to_owned(),
        activate: true,
        success: true,
        target: None,
    }]);
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.magic_point = 100;
    let quoted = runtime.rng.clone().r3x3() as i32;
    let mut updates = RunUpdates::new();
    runtime.run_plain_counter_post_damage_into(EntityIdx(1), 255, 10, EntityIdx(0), &mut updates);
    // 反击是延迟执行，期间其他 hook 可以消耗随机数。
    for _ in 0..5 {
        runtime.rng.next_u8();
    }
    let paused = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.run_plain_counter_update_end_into(EntityIdx(1), &mut updates);
    }));
    assert!(paused.is_err());
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, 100 - quoted);
    let request = runtime.control.pending_passive().unwrap();
    assert_eq!(request.skill, "evade");
    assert_eq!(request.actor, 0);
    assert_eq!(request.target, Some(0));
    assert_eq!(request.initiator, Some(1));
    assert!(request.mp_costs.is_empty());
}

#[test]
fn passive_mp_quotes_are_stable_and_free_choices_have_no_cost() {
    let runtime = counter_runtime(255);
    let before = runtime.rng.clone();
    for skill in ["core.skill.defend", "core.skill.reflect", "core.skill.counter"] {
        let quote = runtime.passive_mp_costs(EntityIdx(1), skill, false);
        assert_eq!(quote[skill], before.clone().r3x3());
    }
    assert!(runtime.passive_mp_costs(EntityIdx(1), "core.skill.protect", false).is_empty());
    assert!(runtime.passive_mp_costs(EntityIdx(1), "evade", false).is_empty());
    assert!(runtime.passive_mp_costs(EntityIdx(1), "core.skill.reraise", false).is_empty());
    assert_rng_state_eq(&runtime.rng, &before);
}

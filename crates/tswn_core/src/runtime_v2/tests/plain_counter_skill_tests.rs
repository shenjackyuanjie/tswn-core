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

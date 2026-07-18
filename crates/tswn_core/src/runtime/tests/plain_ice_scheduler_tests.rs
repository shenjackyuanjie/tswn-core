use super::*;

#[test]
fn ice_pre_step_zero_step_does_not_release_at_threshold() {
    let mut states = StateStore::default();
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    let generation = states.generation();

    let (step, released) = states.apply_ice_pre_step(0, crate::player::MOVE_POINT_THRESHOLD + 1);

    assert_eq!(step, 0);
    assert!(!released);
    assert!(states.is_frozen());
    assert_eq!(states.generation(), generation);
}

#[test]
fn ice_pre_step_positive_step_releases_when_crossing_threshold() {
    let mut states = StateStore::default();
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));

    let (step, released) = states.apply_ice_pre_step(1, crate::player::MOVE_POINT_THRESHOLD);

    assert_eq!(step, 0);
    assert!(released);
    assert!(!states.is_frozen());
}

#[test]
fn ice_release_refreshes_pending_haste_multiplier() {
    let mut builder = ExtensionRegistryBuilder::default();
    let haste = builder
        .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
        .expect("haste state should register");
    let mut states = StateStore::default();
    states.add_entry(StateEntry::haste_with_effective_faster(77, haste, 4, 2, 9, SkillPriority(100)));
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    assert_eq!(states.effective_speed(100), 200);

    let (step, released) = states.apply_ice_pre_step(1, crate::player::MOVE_POINT_THRESHOLD);

    assert_eq!(step, 0);
    assert!(released);
    assert!(!states.is_frozen());
    assert_eq!(states.entry(77).and_then(StateEntry::haste_runtime_value), Some((4, 4, 9)));
    assert_eq!(states.effective_speed(100), 400);
}

#[test]
fn ice_release_keeps_action_when_saved_move_points_already_crossed_threshold() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "frozen", 0, 100, 3)
            .with_speed(1)
            .with_speed_points(crate::player::MOVE_POINT_THRESHOLD + 1),
        PlayerTemplate::new(2, "target", 1, 100, 3).with_speed(1),
    ]));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    while {
        let mut probe = runtime.rng.clone();
        probe.next_u8() & 3 == 0
    } {
        runtime.rng.next_u8();
    }

    let action = runtime
        .scheduler
        .select_action(&mut runtime.world, &mut runtime.entities, &mut runtime.rng)
        .expect("ice release should keep the pending action");

    assert_eq!(action.actor, EntityIdx(0));
    assert_eq!(runtime.scheduler.take_ice_release_events().as_slice(), &[EntityIdx(0)]);
    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().states.is_frozen());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 1);
}

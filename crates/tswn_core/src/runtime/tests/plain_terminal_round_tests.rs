use super::*;

#[test]
fn terminal_charmed_self_kill_stops_before_recovery_and_batch_end() {
    for (raw, expected_rng) in [
        ("yuri@!\n\nd", (223, 80)),
        ("yuri@!\n\nconan@!", (142, 114)),
        ("mosquito@!\n\nyuri@!", (151, 2)),
    ] {
        let mut runner = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
        let terminal = (0..128).map(|_| runner.run_round()).find(|r| r.winner_team.is_some()).unwrap();
        let actor = terminal.action.unwrap().actor;
        assert_eq!(runner.runtime.entities.get(actor).unwrap().runtime.hp, 0, "{raw}");
        assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), expected_rng, "{raw}");
    }
}

#[test]
fn terminal_plain_action_skips_recover_newline_and_post_action_chain() {
    let mut builder = ExtensionRegistryBuilder::default();
    let post_action_state = builder
        .register_state(
            "custom",
            "terminal-post-action",
            "custom.terminal_post_action",
            ProcMask::POST_ACTION,
            SkillPriority(0),
        )
        .expect("post-action state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "killer", 0, 100, 100)
                .with_speed(1)
                .with_speed_points(crate::runtime::MOVE_POINT_THRESHOLD + 1)
                .with_magic_point(0)
                .with_wisdom(255),
            PlayerTemplate::new(2, "last-enemy", 1, 1, 1).with_speed(1),
        ],
        registry,
    ));
    runtime.set_state_handler(post_action_state, state_consumes_rng);
    assert!(runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
        legacy_order_key: 1,
        extension_state_id: Some(post_action_state),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(0),
        registration_order: RegistrationOrder(0),
        payload: StatePayload::None,
    }));

    let outcome = runtime.run_minimal_round();

    assert_eq!(outcome.winner_team, Some(0));
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point, 0);
    let frame = outcome.frame.expect("terminal attack should emit replay");
    let updates = &frame.updates.updates;
    let knockout = updates
        .iter()
        .position(|update| update.message == "[1]被击倒了")
        .expect("terminal attack should emit knockout");
    assert_eq!(
        knockout + 1,
        updates.len(),
        "terminal knockout must end the visible update batch"
    );
    assert!(
        updates.iter().all(|update| !update.message.starts_with("state-rng:")),
        "terminal action must not run post-action states"
    );
}

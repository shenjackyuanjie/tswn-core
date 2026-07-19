use super::*;

fn charmed_runtime() -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let charm_state = builder
        .register_state(
            "core",
            "charm-targeting",
            "core.test.charm-targeting",
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("charm state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "charmed-actor", 0, 100, 3),
            PlayerTemplate::new(2, "original-ally-a", 0, 100, 3),
            PlayerTemplate::new(3, "original-ally-b", 0, 100, 3),
            PlayerTemplate::new(4, "charmer", 1, 100, 3),
        ],
        registry,
    ));
    assert!(
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
            76,
            charm_state,
            3,
            Some(1),
            Some(1),
            Some(0),
            2,
            SkillPriority(210),
        ))
    );
    runtime
}

fn advance_until_actor_is_first_enemy_pick(runtime: &mut CombatRuntime) {
    let all_alive = runtime.world.flat_alive().to_vec();
    while {
        let mut probe = runtime.rng.clone();
        probe.pick_skip_range(&all_alive, &[3]) != Some(0)
    } {
        runtime.rng.next_u8();
    }
}

#[test]
fn charmed_actor_remains_default_enemy_candidate_by_actual_team() {
    let mut runtime = charmed_runtime();
    advance_until_actor_is_first_enemy_pick(&mut runtime);
    let all_alive = runtime.world.flat_alive().to_vec();
    let mut expected_rng = runtime.rng.clone();
    assert_eq!(expected_rng.pick_skip_range(&all_alive, &[3]), Some(0));
    expected_rng.rFFFF();

    let selected = runtime.select_plain_default_enemy_targets_with_count(EntityIdx(0), false, 1);

    assert_eq!(selected.as_slice(), &[EntityIdx(0)]);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

#[test]
fn charmed_actor_remains_berserk_enemy_candidate_by_actual_team() {
    let mut runtime = charmed_runtime();
    advance_until_actor_is_first_enemy_pick(&mut runtime);

    let selected = runtime.select_plain_berserk_targets(EntityIdx(0), false);

    assert!(
        selected.contains(&EntityIdx(0)),
        "effective team only changes the actor's ally group; candidate teams remain physical roster teams"
    );
}

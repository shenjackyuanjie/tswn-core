use super::*;

#[test]
fn plain_haste_samples_ally_alive_order_instead_of_roster_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_state(
            "core",
            "haste",
            DEFAULT_CORE_HASTE_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "slow-ally", 0, 100, 3).with_target_score_stats(10, 10, 1.0),
            PlayerTemplate::new(3, "best-ally", 0, 100, 3).with_target_score_stats(30, 30, 1.0),
            PlayerTemplate::new(4, "front-alive", 0, 100, 3).with_target_score_stats(20, 20, 1.0),
            PlayerTemplate::new(5, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    runtime.world.sync_initial_views(
        &runtime.entities,
        vec![EntityIdx(0), EntityIdx(1), EntityIdx(2), EntityIdx(3), EntityIdx(4)],
        vec![vec![EntityIdx(0), EntityIdx(1), EntityIdx(2), EntityIdx(3)], vec![EntityIdx(4)]],
        vec![vec![EntityIdx(3), EntityIdx(0), EntityIdx(1), EntityIdx(2)], vec![EntityIdx(4)]],
        vec![EntityIdx(3), EntityIdx(0), EntityIdx(1), EntityIdx(2), EntityIdx(4)],
    );
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 16;
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 40;

    let candidates = [EntityIdx(3), EntityIdx(0), EntityIdx(1), EntityIdx(2)];
    let mut expected_rng = runtime.rng.clone();
    let mut expected_selected = Vec::new();
    let mut duplicate_count = 0usize;
    let mut invalid_count = -3i32;
    while duplicate_count <= 3 && invalid_count <= 3 {
        let Some(picked) = expected_rng.pick(&candidates) else {
            break;
        };
        let target = candidates[picked];
        let valid = matches!(target, EntityIdx(2) | EntityIdx(3));
        if !valid {
            invalid_count += 1;
            continue;
        }
        if expected_selected.contains(&target) {
            duplicate_count += 1;
            continue;
        }
        expected_selected.push(target);
        if expected_selected.len() >= 3 {
            break;
        }
    }
    expected_selected.sort_by(|lhs, rhs| {
        runtime
            .score_plain_haste_target(*rhs, true)
            .partial_cmp(&runtime.score_plain_haste_target(*lhs, true))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let selected = runtime.select_plain_haste_targets(EntityIdx(0), true);

    assert_eq!(selected.as_slice(), expected_selected.as_slice());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

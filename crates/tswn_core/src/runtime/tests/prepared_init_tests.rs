use super::*;

pub(super) fn runtime_runner_from_raw<E: std::fmt::Debug>(
    raw_input: &str,
    build: impl FnOnce(&[Vec<String>]) -> Result<RuntimeRunner, E>,
) -> RuntimeRunner {
    let (raw_groups, seed) = PreparedBattleInit::split_namerena_raw(raw_input.to_owned());
    let mut runner = build(&raw_groups).expect("runtime roster builder should succeed");
    PreparedBattleInit::from_groups(&raw_groups, &seed, &runner.runtime.registry)
        .expect("runtime battle init should prepare")
        .apply(&mut runner.runtime)
        .expect("runtime battle init should apply");
    runner
}

#[test]
fn prepared_battle_init_split_matches_legacy_group_and_seed_rules() {
    for raw in [
        "left@red\nright@blue",
        "left@red\r\n\r\nseed:abc@!\r\n\r\nright@blue\r\n",
        "seed:front@!\n\nleft@red\n\nright@blue",
        "left@red\n\nseed:middle@!\n\nright@blue",
    ] {
        assert_eq!(
            PreparedBattleInit::split_namerena_raw(raw.to_owned()),
            crate::LegacyRunner::split_namerena_into_groups(raw.to_owned()),
            "raw split diverged for {raw:?}"
        );
    }
}

#[test]
fn prepared_battle_init_reports_entity_count_mismatch() {
    let raw_groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
    let template = PreparedCombatTemplate::new(vec![PlayerTemplate::new(1, "left", 0, 10, 3)]);
    let mut runtime = CombatRuntime::from_template(template);
    let error = PreparedBattleInit::from_groups(&raw_groups, &[], &runtime.registry)
        .expect("battle init should prepare")
        .apply(&mut runtime)
        .expect_err("battle init should reject a mismatched runtime");

    assert_eq!(error, RuntimeBattleInitError::EntityCountMismatch { prepared: 2, runtime: 1 });
}

#[test]
fn prepared_runtime_runner_reuses_roster_across_seeds_at_win_rate_eval_rq() {
    let raw_groups = vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]];
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let prepared = PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(
        &raw_groups,
        crate::player::eval_name::WIN_RATE_EVAL_RQ,
        config,
    )
    .expect("runtime roster should prepare");

    for seed in [
        Vec::new(),
        vec!["seed:33554432@!".to_owned()],
        vec!["seed:33554433@!".to_owned()],
        vec!["seed:prepared-runtime@!".to_owned()],
    ] {
        let mut legacy = crate::LegacyRunner::new_from_groups_with_seed_and_eval_rq_uncached(
            &raw_groups,
            &seed,
            crate::player::eval_name::WIN_RATE_EVAL_RQ,
        )
        .expect("legacy runner should construct");
        let mut runtime = prepared.new_with_seed(&seed).expect("runtime runner should instantiate");

        let expected = normalize_legacy_run(&mut legacy, 100_000);
        let actual = runtime.run_until_winner_normalized_rounds(100_000);
        assert_eq!(strict_diff_runs(&expected, &actual), Ok(()), "seed={seed:?}");
    }
}

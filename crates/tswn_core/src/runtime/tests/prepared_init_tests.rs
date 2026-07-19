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
fn prepared_battle_init_split_keeps_group_and_seed_rules() {
    assert_eq!(
        PreparedBattleInit::split_namerena_raw("left@red\nright@blue".to_owned()),
        (vec![vec!["left@red".to_owned()], vec!["right@blue".to_owned()]], Vec::new())
    );
    assert_eq!(
        PreparedBattleInit::split_namerena_raw("left@red\r\n\r\nseed:abc@!\r\n\r\nright@blue\r\n".to_owned()),
        (
            vec![
                vec!["left@red".to_owned(), "seed:abc@!".to_owned()],
                vec!["right@blue".to_owned()]
            ],
            vec!["seed:abc@!".to_owned()]
        )
    );
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
        crate::namerena::eval_name::WIN_RATE_EVAL_RQ,
        config,
    )
    .expect("runtime roster should prepare");

    for seed in [
        Vec::new(),
        vec!["seed:33554432@!".to_owned()],
        vec!["seed:33554433@!".to_owned()],
        vec!["seed:prepared-runtime@!".to_owned()],
    ] {
        let mut first = prepared.new_with_seed(&seed).expect("runtime runner should instantiate");
        let mut second = prepared.new_with_seed(&seed).expect("runtime runner should instantiate twice");
        assert_eq!(
            first.run_until_winner_normalized_rounds(100_000),
            second.run_until_winner_normalized_rounds(100_000),
            "seed={seed:?}"
        );
    }
}

use super::*;

pub(super) fn runtime_v2_runner_from_raw<E: std::fmt::Debug>(
    raw_input: &str,
    build: impl FnOnce(&[Vec<String>]) -> Result<RuntimeV2Runner, E>,
) -> RuntimeV2Runner {
    let (raw_groups, seed) = PreparedBattleInit::split_namerena_raw(raw_input.to_owned());
    let mut runner = build(&raw_groups).expect("runtime v2 roster builder should succeed");
    PreparedBattleInit::from_groups(&raw_groups, &seed, &runner.runtime.registry)
        .expect("runtime v2 battle init should prepare")
        .apply(&mut runner.runtime)
        .expect("runtime v2 battle init should apply");
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
            crate::Runner::split_namerena_into_groups(raw.to_owned()),
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

    assert_eq!(error, RuntimeV2BattleInitError::EntityCountMismatch { prepared: 2, runtime: 1 });
}

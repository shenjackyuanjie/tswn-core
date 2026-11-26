use super::*;

#[test]
fn split_namerena_raw_normalizes_crlf_and_cr_before_parsing() {
    fn roster(runner: &RuntimeRunner) -> Vec<Vec<(&str, &str)>> {
        runner
            .input_groups()
            .iter()
            .map(|team| {
                team.iter()
                    .map(|entity| {
                        let template = &runner.runtime().entities.get(*entity).unwrap().template;
                        (template.name.as_str(), template.clan_name.as_str())
                    })
                    .collect()
            })
            .collect()
    }

    fn rng_state(runner: &RuntimeRunner) -> (NormalizedRngCheckpoint, [u8; 256]) {
        let rng = &runner.runtime().rng;
        (NormalizedRngCheckpoint::from_rc4(rng), rng.main_val)
    }

    let mut runners = ["\n", "\r\n", "\r"].map(|separator| {
        let raw = ["alice@red", "bob@red", "", "seed:52@!", "", "carol@blue", ""].join(separator);
        let (groups, seed) = PreparedBattleInit::split_namerena_raw(raw.clone());
        assert_eq!(
            groups,
            [vec!["alice@red", "bob@red", "seed:52@!"], vec!["carol@blue"]],
            "换行符 {separator:?}"
        );
        assert_eq!(seed, ["seed:52@!"], "换行符 {separator:?}");
        RuntimeRunner::from_custom_mixed_namerena_raw(raw, default_custom_runtime_import_config().unwrap()).unwrap()
    });

    let [lf, crlf, cr] = runners.each_ref().map(roster);
    assert_eq!(lf, crlf, "CRLF 阵容");
    assert_eq!(lf, cr, "CR 阵容");

    let [lf, crlf, cr] = runners.each_ref().map(rng_state);
    assert_eq!(lf, crlf, "CRLF 初始 RC4");
    assert_eq!(lf, cr, "CR 初始 RC4");

    for round in 1..=1024 {
        let [lf, crlf, cr] = runners.each_mut().map(RuntimeRunner::run_round_normalized);
        assert_eq!(lf, crlf, "CRLF 第 {round} 轮");
        assert_eq!(lf, cr, "CR 第 {round} 轮");
        if lf.winner_team.is_some() {
            let [lf, crlf, cr] = runners.each_ref().map(rng_state);
            assert_eq!(lf, crlf, "CRLF 最终 RC4");
            assert_eq!(lf, cr, "CR 最终 RC4");
            return;
        }
    }
    panic!("换行符回归对局在上限内未产生胜者");
}

use super::*;
#[test]
fn runtime_runner_aligns_large_raw_initial_state_with_legacy_world() {
    let raw_input =
        "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

    let (runner, legacy) = mixed_raw_runner_for_plain_fixture(raw_input);

    assert_eq!(runner.runtime().entities.len(), 2);
    assert_eq!(legacy.world.all_plr_len(), 2);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_runtime_rng_matches_legacy(runner.runtime(), &legacy);
}

#[test]
fn runtime_runner_aligns_fight_multi_raw_initial_state_with_legacy_world() {
    let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

    let (runner, legacy) = mixed_raw_runner_for_plain_fixture(raw_input);

    assert_eq!(runner.runtime().entities.len(), 10);
    assert_eq!(legacy.world.all_plr_len(), 10);
    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_runtime_rng_matches_legacy(runner.runtime(), &legacy);
}

#[test]
fn runtime_runner_fight_multi_matches_legacy_run() {
    let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

    let mut legacy_runner =
        crate::LegacyRunner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy fight_multi should construct");
    let legacy = normalize_legacy_run(&mut legacy_runner, 256);
    let (mut runtime_runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
    let runtime = runtime_runner.run_until_winner_normalized_rounds(256);

    assert_eq!(legacy.rounds.len(), 84);
    assert_eq!(legacy.total_score, 6766);
    strict_diff_runs(&legacy, &runtime).expect("runtime fight_multi fixture should match legacy run");
}

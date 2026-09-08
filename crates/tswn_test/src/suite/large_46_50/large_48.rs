use super::*;

pub fn large_48<E: crate::EngineAdapter>() {
    const CASE: &str = r#"虚空托腮 IVHEWTNEA@TigerStar

进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇
seed:2026-03-07 22:54 #013595@!


虚空托腮发起攻击, 进口牢货.不可磨灭的回忆之殇受到114点伤害

进口牢货.不可磨灭的回忆之殇使用幻术, 召唤出幻影

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到66点伤害

进口牢货.不可磨灭的回忆之殇使用幻术, 召唤出幻影

虚空托腮使用净化, 幻影受到208点伤害

 幻影消失了

虚空托腮发起攻击, 幻影受到90点伤害

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到56点伤害

 虚空托腮发起反击, 进口牢货.不可磨灭的回忆之殇受到54点伤害

幻影发起攻击, 虚空托腮受到116点伤害

虚空托腮使用诅咒, 幻影受到25点伤害, 幻影被诅咒了

进口牢货.不可磨灭的回忆之殇使用分身, 出现一个新的进口牢货.不可磨灭的回忆之殇

幻影使用附体, 虚空托腮进入狂暴状态

 幻影消失了

进口牢货.不可磨灭的回忆之殇使用分身, 出现一个新的进口牢货.不可磨灭的回忆之殇

虚空托腮发起狂暴攻击, 进口牢货.不可磨灭的回忆之殇受到73点伤害

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到54点伤害

进口牢货.不可磨灭的回忆之殇使用幻术, 召唤出幻影

虚空托腮发起狂暴攻击, 进口牢货.不可磨灭的回忆之殇受到73点伤害

 进口牢货.不可磨灭的回忆之殇被击倒了

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到66点伤害

 虚空托腮被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-48 must contain a blank separator between input and trace",
        "sampled case-48 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1577, "large_48 score mismatch");
    assert!(guard < 20_000, "sampled case-48 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-48", &actual_lines, &expected_lines);
}

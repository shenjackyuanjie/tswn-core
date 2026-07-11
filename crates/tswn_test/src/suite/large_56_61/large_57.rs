use super::*;

pub fn large_57<E: crate::EngineAdapter>() {
    const CASE: &str = r#"涵虚不等式 PFVKEUPBU@TigerStar
Meranti FBPITYXTBPQG@nan
seed:2@!


涵虚不等式使用幻术, 召唤出幻影

Meranti潜行到幻影身后

Meranti发动背刺, 幻影受到468点伤害

 幻影消失了

涵虚不等式使用幻术, 召唤出幻影

Meranti使用幻术, 召唤出幻影

涵虚不等式发起攻击, Meranti受到0点伤害

Meranti潜行到涵虚不等式身后

涵虚不等式使用幻术, 召唤出幻影

幻影发起攻击, 幻影受到144点伤害

 幻影消失了

幻影使用附体, Meranti进入狂暴状态

 幻影消失了

Meranti发动背刺, 涵虚不等式受到334点伤害

 涵虚不等式被击倒了

 幻影消失了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-57 must contain a blank separator between input and trace",
        "sampled case-57 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1450, "large_57 score mismatch");
    assert!(guard < 20_000, "sampled case-57 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-57", &actual_lines, &expected_lines);
}

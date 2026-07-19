use super::*;

/// 用来测试 clamp
pub fn large_66<E: crate::EngineAdapter>() {
    const CASE: &str = r#"太阳帝国 #Sc2WCffta@Shabby_fish
机械革命 #nYHCsQ1ak@Shabby_fish


太阳帝国使用幻术, 召唤出幻影

机械革命潜行到幻影身后

太阳帝国发起攻击, 机械革命回避了攻击

机械革命发动背刺, 幻影受到350点伤害

 幻影消失了

太阳帝国使用分身, 出现一个新的太阳帝国

机械革命潜行到太阳帝国身后

太阳帝国开始蓄力

太阳帝国发起攻击, 机械革命受到35点伤害

 机械革命的潜行被识破

机械革命潜行到太阳帝国身后

太阳帝国使用分身, 出现一个新的太阳帝国

太阳帝国发起攻击, 机械革命受到52点伤害

 机械革命的潜行被识破

机械革命潜行到太阳帝国身后

太阳帝国发起攻击, 机械革命受到37点伤害

 机械革命的潜行被识破

太阳帝国使用分身, 出现一个新的太阳帝国

机械革命潜行到太阳帝国身后

太阳帝国开始蓄力

太阳帝国开始蓄力

太阳帝国发起攻击, 机械革命回避了攻击

太阳帝国发起攻击, 机械革命受到58点伤害

 机械革命的潜行被识破

太阳帝国发起攻击, 机械革命受到215点伤害

 机械革命被击倒了"#;

    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-66 must contain a blank separator between input and trace",
        "sampled case-66 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-66 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-66", &actual_lines, &expected_lines);
}

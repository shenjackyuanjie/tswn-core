use super::*;

pub fn large_70<E: crate::EngineAdapter>() {
    const CASE: &str = r#"Superpower #ddDROyhTJ@Shabby_fish
大化西游 #faYL5F6xL@Shabby_fish


大化西游潜行到Superpower身后

Superpower发起攻击, 大化西游回避了攻击

大化西游发动背刺, Superpower受到280点伤害

Superpower做出垂死抗争, Superpower所有属性上升

Superpower使用分身, 出现一个新的Superpower

Superpower使用魅惑, 大化西游回避了攻击

Superpower使用魅惑, 大化西游被魅惑了

大化西游发起攻击, 大化西游受到80点伤害

大化西游从魅惑中解除

Superpower使用冰冻术, 大化西游回避了攻击

Superpower使用分身, 出现一个新的Superpower

Superpower发起攻击, 大化西游受到30点伤害

大化西游发起攻击, Superpower受到57点伤害

Superpower被击倒了

Superpower使用分身, 出现一个新的Superpower

大化西游发起攻击, Superpower受到75点伤害

Superpower被击倒了

大化西游召唤亡灵, Superpower变成了丧尸

Superpower发起攻击, 大化西游受到22点伤害

丧尸发起攻击, Superpower受到63点伤害

Superpower被击倒了

大化西游发起攻击, Superpower受到84点伤害

Superpower被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-70 must contain a blank separator between input and trace",
        "sampled case-70 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-70 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-70", &actual_lines, &expected_lines);
}

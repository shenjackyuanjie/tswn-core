use super::*;

pub fn large_52<E: crate::EngineAdapter>() {
    const CASE: &str = r#"aaaa+123
bbb+324
ccc+2345


aaaa发起攻击, ccc受到100点伤害

bbb发起攻击, aaaa受到92点伤害

ccc发起攻击, bbb受到79点伤害

aaaa使用瘟疫, ccc体力减少48%

ccc发起攻击, aaaa受到60点伤害

bbb发起攻击, ccc受到53点伤害

aaaa发起攻击, bbb受到55点伤害

ccc发起攻击, aaaa受到46点伤害

aaaa开始聚气, aaaa攻击力上升

ccc发起攻击, bbb受到74点伤害

aaaa发起攻击, ccc受到65点伤害

 ccc被击倒了

bbb发起攻击, aaaa受到40点伤害

aaaa发起攻击, bbb回避了攻击

bbb发起攻击, aaaa受到56点伤害

 aaaa被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-52 must contain a blank separator between input and trace",
        "sampled case-52 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 924, "large_52 score mismatch");
    assert!(guard < 20_000, "sampled case-52 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-52", &actual_lines, &expected_lines);
}

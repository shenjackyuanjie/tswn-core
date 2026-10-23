use super::*;

/// 对编号 46 的同阵容使用 seed:3，覆盖零伤害与伤害反弹的另一条执行路径。
///
/// 期望日志逐行比较，总分固定为 705，避免只按阵容而忽略 seed 的回归差异。
pub fn large_47<E: crate::EngineAdapter>() {
    const CASE: &str = r#"Momomomo #YAORzaY@Arcadia
泠珞 itVMnXnsL@807139
seed:3@!


泠珞发起攻击, Momomomo受到63点伤害

Momomomo发起攻击, 泠珞受到115点伤害

泠珞发起攻击, Momomomo受到0点伤害

Momomomo发起攻击, 泠珞使用伤害反弹, Momomomo受到42点伤害

泠珞发起攻击, Momomomo受到46点伤害

Momomomo发起攻击, 泠珞受到96点伤害

泠珞使用冰冻术, Momomomo防御, Momomomo受到0点伤害

泠珞发起攻击, Momomomo受到86点伤害

Momomomo发起攻击, 泠珞受到64点伤害

Momomomo发起攻击, 泠珞受到62点伤害

 泠珞被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-47 must contain a blank separator between input and trace",
        "sampled case-47 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 705, "large_47 score mismatch");
    assert!(guard < 20_000, "sampled case-47 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-47", &actual_lines, &expected_lines);
}

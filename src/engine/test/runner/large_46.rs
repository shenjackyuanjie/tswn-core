use super::*;

/// 护盾 vs 净化
#[test]
fn large_46() {
    const CASE: &str = r#"Momomomo #YAORzaY@Arcadia
泠珞 itVMnXnsL@807139


Momomomo潜行到泠珞身后

泠珞使用净化, Momomomo受到54点伤害

 Momomomo的潜行被识破

泠珞发起攻击, Momomomo受到46点伤害

Momomomo发起攻击, 泠珞受到62点伤害

Momomomo发起攻击, 泠珞受到71点伤害

泠珞使用净化, Momomomo受到0点伤害

Momomomo潜行到泠珞身后

Momomomo发动背刺, 泠珞受到214点伤害

 泠珞被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-46 must contain a blank separator between input and trace",
        "sampled case-46 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-46 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-46", &actual_lines, &expected_lines);
}

/// 跟上面的一样
#[test]
fn large_47() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-47 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-47", &actual_lines, &expected_lines);
}
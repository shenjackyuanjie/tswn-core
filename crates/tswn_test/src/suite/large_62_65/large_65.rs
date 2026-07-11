use super::*;

/// diff_case 11
pub fn large_65<E: crate::EngineAdapter>() {
    const CASE: &str = r#"💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜@新纪元
失温滢霞.寂静蓝 `f.Gi\Z:R@四象柯
seed:33554432@!


💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜使用火球术, 失温滢霞.寂静蓝受到105点伤害

失温滢霞.寂静蓝发起攻击, 💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜受到53点伤害

失温滢霞.寂静蓝潜行到💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜身后

💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜使用火球术, 失温滢霞.寂静蓝受到141点伤害

 失温滢霞.寂静蓝的潜行被识破

失温滢霞.寂静蓝潜行到💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜身后

💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜使用魅惑, 失温滢霞.寂静蓝回避了攻击

失温滢霞.寂静蓝发动背刺, 💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜受到390点伤害

 💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜被击倒了


"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-65 must contain a blank separator between input and trace",
        "sampled case-65 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-65 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-65", &actual_lines, &expected_lines);
}

use super::*;

pub fn large_49<E: crate::EngineAdapter>() {
    const CASE: &str = r#"虚空托腮 IVHEWTNEA@TigerStar

跙坥咀诅阻珇伹伹怚@涵虚
seed:2026-03-07 22:53 #500299@!


虚空托腮发起攻击, 跙坥咀诅阻珇伹伹怚受到76点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 虚空托腮回避了攻击

跙坥咀诅阻珇伹伹怚发动铁壁, 跙坥咀诅阻珇伹伹怚防御力大幅上升

虚空托腮发起攻击, 跙坥咀诅阻珇伹伹怚受到0点伤害

跙坥咀诅阻珇伹伹怚潜行到虚空托腮身后

虚空托腮发起攻击, 跙坥咀诅阻珇伹伹怚受到0点伤害

跙坥咀诅阻珇伹伹怚发动背刺, 虚空托腮受到330点伤害

 跙坥咀诅阻珇伹伹怚从铁壁中解除

虚空托腮使用净化, 跙坥咀诅阻珇伹伹怚受到52点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 虚空托腮受到47点伤害

 虚空托腮被击倒了, 虚空托腮使用护身符抵挡了一次死亡, 虚空托腮回复体力4点

虚空托腮发起攻击, 跙坥咀诅阻珇伹伹怚受到55点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 虚空托腮受到48点伤害

 虚空托腮被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-49 must contain a blank separator between input and trace",
        "sampled case-49 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 910, "large_49 score mismatch");
    assert!(guard < 20_000, "sampled case-49 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-49", &actual_lines, &expected_lines);
}

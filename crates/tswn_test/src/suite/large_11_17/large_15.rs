use super::*;

pub fn large_15<E: crate::EngineAdapter>() {
    const CASE: &str = r####"QnljmCHowQ
IdUM9kx9c2
vhxSYeEzvf
qPCGw3EB8M
qze3UVC1DD


qPCGw3EB8M发起攻击, QnljmCHowQ受到126点伤害

qze3UVC1DD发起攻击, QnljmCHowQ受到63点伤害

 qze3UVC1DD连击, QnljmCHowQ受到54点伤害

IdUM9kx9c2发起攻击, QnljmCHowQ受到44点伤害

QnljmCHowQ发起攻击, IdUM9kx9c2受到53点伤害

qPCGw3EB8M发起攻击, qze3UVC1DD受到102点伤害

vhxSYeEzvf发起攻击, qPCGw3EB8M受到57点伤害

qze3UVC1DD发起攻击, qPCGw3EB8M受到63点伤害

qPCGw3EB8M使用魅惑, vhxSYeEzvf被魅惑了

IdUM9kx9c2发起攻击, vhxSYeEzvf受到93点伤害

vhxSYeEzvf发起攻击, QnljmCHowQ受到55点伤害

 vhxSYeEzvf从魅惑中解除

vhxSYeEzvf发起攻击, QnljmCHowQ受到129点伤害

 QnljmCHowQ被击倒了

 vhxSYeEzvf召唤亡灵, QnljmCHowQ变成了丧尸

qPCGw3EB8M发起攻击, vhxSYeEzvf受到50点伤害

qze3UVC1DD使用减速术, 丧尸进入迟缓状态

IdUM9kx9c2发起攻击, qze3UVC1DD受到41点伤害

qPCGw3EB8M发起攻击, vhxSYeEzvf受到103点伤害

vhxSYeEzvf潜行到qPCGw3EB8M身后

丧尸发起攻击, qze3UVC1DD受到41点伤害

vhxSYeEzvf发动背刺, qPCGw3EB8M受到324点伤害

 qPCGw3EB8M被击倒了

qze3UVC1DD发起攻击, 丧尸受到36点伤害

 qze3UVC1DD连击, vhxSYeEzvf受到76点伤害

 vhxSYeEzvf被击倒了

 丧尸消失了

IdUM9kx9c2发起攻击, qze3UVC1DD受到101点伤害

 qze3UVC1DD被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-15 must contain a blank separator between input and trace",
        "sampled case-15 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 2107, "large_15 score mismatch");

    assert!(guard < 20_000, "sampled case-15 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-15", &actual_lines, &expected_lines);
}

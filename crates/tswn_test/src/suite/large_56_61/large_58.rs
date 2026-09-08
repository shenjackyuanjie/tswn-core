use super::*;

pub fn large_58<E: crate::EngineAdapter>() {
    const CASE: &str = r#"Bascor cW1JWDuv7f@RbCl
Meltel abRC3P3Go7@RbCl

syVS:et@Hell
'Yz|AS}@Hell
seed:2@!


'Yz|AS}使用减速术, Meltel进入迟缓状态

Bascor潜行到'Yz|AS}身后

syVS:et使用幻术, 召唤出幻影

syVS:et发起攻击, Meltel受到24点伤害

'Yz|AS}使用分身, 出现一个新的'Yz|AS}

Meltel发起攻击, syVS:et受到48点伤害

Bascor发动背刺, 'Yz|AS}受到354点伤害

 'Yz|AS}被击倒了

 Bascor吞噬了'Yz|AS}, Bascor属性上升

'Yz|AS}使用减速术, Meltel回避了攻击

syVS:et使用幻术, 召唤出幻影

幻影使用附体, Bascor进入狂暴状态

 幻影消失了

syVS:et使用幻术, 召唤出幻影

Bascor潜行到syVS:et身后

'Yz|AS}使用减速术, Bascor回避了攻击

Meltel发起攻击, 幻影受到91点伤害

 Meltel从迟缓中解除

Bascor发动背刺, syVS:et受到446点伤害

 syVS:et被击倒了

 幻影消失了

 幻影消失了

 Bascor吞噬了syVS:et, Bascor属性上升

Meltel使用减速术, 'Yz|AS}进入迟缓状态

Bascor发起攻击, 'Yz|AS}受到68点伤害

'Yz|AS}使用减速术, Meltel进入迟缓状态

Meltel使用减速术, 'Yz|AS}进入迟缓状态

Bascor开始聚气, Bascor攻击力上升

Bascor发起攻击, 'Yz|AS}受到118点伤害

 'Yz|AS}被击倒了
"#;

    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-58 must contain a blank separator between input and trace",
        "sampled case-58 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 2100, "large_58 score mismatch");
    assert!(guard < 20_000, "sampled case-58 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-58", &actual_lines, &expected_lines);
}

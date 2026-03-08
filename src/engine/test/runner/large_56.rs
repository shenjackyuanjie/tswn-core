use super::*;

#[test]
fn large_56() {
    const CASE: &str = r#"渊HG bwigVwI@807139
♯ WJl5tXdd5X@807139

syVS:et@Hell
'Yz|AS}@Hell
seed:2@!


渊HG使用净化, 'Yz|AS}受到50点伤害

syVS:et使用幻术, 召唤出幻影

♯潜行到'Yz|AS}身后

'Yz|AS}发起攻击, 渊HG受到29点伤害

渊HG使用净化, 'Yz|AS}受到59点伤害

'Yz|AS}发起攻击, 渊HG回避了攻击

syVS:et使用分身, 出现一个新的syVS:et

♯发动背刺, 'Yz|AS}受到371点伤害

 'Yz|AS}被击倒了

 ♯吞噬了'Yz|AS}, ♯属性上升

syVS:et使用幻术, 召唤出幻影

渊HG使用净化, 幻影受到139点伤害

幻影发起攻击, 渊HG受到70点伤害

♯使用分身, 出现一个新的♯

syVS:et使用分身, 出现一个新的syVS:et

幻影发起攻击, ♯受到75点伤害

渊HG使用净化, 幻影受到114点伤害

 幻影消失了

♯潜行到syVS:et身后

syVS:et发起攻击, ♯受到19点伤害

♯潜行到syVS:et身后

幻影使用附体, ♯进入狂暴状态

 幻影消失了

syVS:et发起攻击, 渊HG受到63点伤害

syVS:et使用幻术, 召唤出幻影

♯发动背刺, syVS:et受到412点伤害

 syVS:et被击倒了

 幻影消失了

 ♯吞噬了syVS:et, ♯属性上升

♯发动背刺, syVS:et受到131点伤害

渊HG使用净化, syVS:et受到40点伤害

syVS:et发起攻击, ♯受到46点伤害

 ♯做出垂死抗争, ♯所有属性上升

渊HG发起攻击, syVS:et受到96点伤害

 syVS:et被击倒了

♯使用减速术, syVS:et进入迟缓状态

♯发起狂暴攻击, 渊HG受到30点伤害

渊HG发起攻击, syVS:et受到60点伤害

♯发起攻击, syVS:et受到96点伤害

 syVS:et被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-56 must contain a blank separator between input and trace",
        "sampled case-56 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-56 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-56", &actual_lines, &expected_lines);
}

#[test]
fn large_57() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-57 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-57", &actual_lines, &expected_lines);
}

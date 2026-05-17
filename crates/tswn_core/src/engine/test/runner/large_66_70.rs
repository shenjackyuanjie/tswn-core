use super::*;


/// 用来测试 clamp
#[test]
fn large_66() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-66 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-66", &actual_lines, &expected_lines);
}

/// 这个好像是用来check ts的
#[test]
fn large_67() {
    const CASE: &str = r#"Stupefy #rkISERW8@Shabby_fish
日落·日出 #Pd3J7shds@Shabby_fish


Stupefy潜行到日落·日出身后

日落·日出使用血祭, 召唤出使魔

使魔发起攻击, Stupefy受到31点伤害

 Stupefy的潜行被识破

Stupefy潜行到日落·日出身后

日落·日出使用分身, 出现一个新的日落·日出

Stupefy发动背刺, 日落·日出受到440点伤害

 日落·日出被击倒了

 使魔消失了

 Stupefy吞噬了日落·日出, Stupefy属性上升

日落·日出发起攻击, Stupefy受到50点伤害

Stupefy发起攻击, 日落·日出受到22点伤害

日落·日出发起攻击, Stupefy受到37点伤害

Stupefy发起攻击, 日落·日出受到79点伤害

日落·日出潜行到Stupefy身后

Stupefy使用血祭, 召唤出使魔

使魔发起攻击, 日落·日出防御, 日落·日出受到11点伤害

 日落·日出的潜行被识破

日落·日出发起攻击, Stupefy防御, Stupefy受到24点伤害

Stupefy发起攻击, 日落·日出受到49点伤害

使魔发起攻击, 日落·日出受到63点伤害

 日落·日出被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-67 must contain a blank separator between input and trace",
        "sampled case-67 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-67 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-67", &actual_lines, &expected_lines);
}

#[test]
fn large_69() {
    const CASE: &str = r#"洋基周刊 #40WSvjsmo@Shabby_fish
El_Hombre_que_Ríe #wnyYvAHgu@Shabby_fish

望趋疯泉之@Shabby_fish
雷达 #pYAjWilZL@Shabby_fish


洋基周刊潜行到雷达身后

望趋疯泉之潜行到El_Hombre_que_Ríe身后

El_Hombre_que_Ríe使用分身, 出现一个新的El_Hombre_que_Ríe

El_Hombre_que_Ríe发起攻击, 望趋疯泉之受到53点伤害

 望趋疯泉之的潜行被识破

洋基周刊发动背刺, 雷达受到209点伤害

望趋疯泉之潜行到洋基周刊身后

El_Hombre_que_Ríe发起攻击, 雷达防御, 雷达受到16点伤害

雷达发起攻击, El_Hombre_que_Ríe受到49点伤害

El_Hombre_que_Ríe发起攻击, 雷达回避了攻击

望趋疯泉之发动背刺, 洋基周刊受到393点伤害

 洋基周刊被击倒了

 望趋疯泉之吞噬了洋基周刊, 望趋疯泉之属性上升

望趋疯泉之潜行到El_Hombre_que_Ríe身后

El_Hombre_que_Ríe发起攻击, 雷达受到60点伤害

雷达发起攻击, El_Hombre_que_Ríe受到76点伤害

El_Hombre_que_Ríe使用减速术, 望趋疯泉之进入迟缓状态

El_Hombre_que_Ríe发起攻击, 雷达受到69点伤害

 雷达被击倒了

望趋疯泉之发动背刺, El_Hombre_que_Ríe受到302点伤害

 El_Hombre_que_Ríe被击倒了

El_Hombre_que_Ríe使用分身, 出现一个新的El_Hombre_que_Ríe

El_Hombre_que_Ríe发起攻击, 望趋疯泉之受到0点伤害

望趋疯泉之使用苏生术, 雷达复活了, 雷达回复体力82点

 望趋疯泉之从迟缓中解除

雷达使用幻术, 召唤出幻影

El_Hombre_que_Ríe使用分身, 出现一个新的El_Hombre_que_Ríe

望趋疯泉之发起攻击, El_Hombre_que_Ríe受到105点伤害

 El_Hombre_que_Ríe被击倒了, El_Hombre_que_Ríe使用护身符抵挡了一次死亡, El_Hombre_que_Ríe回复体力5点

El_Hombre_que_Ríe发起攻击, 雷达回避了攻击

望趋疯泉之使用分身, 出现一个新的望趋疯泉之

El_Hombre_que_Ríe发起攻击, 雷达受到76点伤害

El_Hombre_que_Ríe发起攻击, 望趋疯泉之受到0点伤害

雷达发起攻击, El_Hombre_que_Ríe受到53点伤害

 El_Hombre_que_Ríe被击倒了

望趋疯泉之发起攻击, El_Hombre_que_Ríe受到50点伤害

 El_Hombre_que_Ríe被击倒了

幻影发起攻击, El_Hombre_que_Ríe受到104点伤害

 El_Hombre_que_Ríe被击倒了, El_Hombre_que_Ríe使用护身符抵挡了一次死亡, El_Hombre_que_Ríe回复体力11点

El_Hombre_que_Ríe发起攻击, 雷达使用伤害反弹, El_Hombre_que_Ríe使用伤害反弹, 雷达使用伤害反弹, El_Hombre_que_Ríe受到12点伤害

 El_Hombre_que_Ríe被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-69 must contain a blank separator between input and trace",
        "sampled case-69 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-69 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-69", &actual_lines, &expected_lines);
}

#[test]
fn large_70() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-70 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-70", &actual_lines, &expected_lines);
}

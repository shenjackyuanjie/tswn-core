use super::*;

pub fn large_69<E: crate::EngineAdapter>() {
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
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-69 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-69", &actual_lines, &expected_lines);
}

use super::*;

/// 九连守护
pub fn large_50<E: crate::EngineAdapter>() {
    const CASE: &str = r#"我力 7#W2ib8D@仙蛊屋
万我 68#huMG43@仙蛊屋

Dianmu YKFMWRPXIMCQ@nan
Freddy FVNXBNVTWJEA@nan

seed:第十八届武术大赛小组赛第8组:307-3@!


我力使用幻术, 召唤出幻影

Freddy使用分身, 出现一个新的Freddy

Dianmu发起攻击, 万我受到69点伤害

万我使用减速术, Freddy进入迟缓状态

Freddy发起攻击, 万我受到39点伤害

Freddy使用分身, 出现一个新的Freddy

我力发起攻击, Freddy受到62点伤害

万我发起攻击, Freddy受到82点伤害

 Freddy被击倒了

Freddy发起攻击, 我力回避了攻击

Dianmu发起攻击, 万我受到70点伤害

万我使用减速术, Dianmu进入迟缓状态

幻影发起攻击, Freddy受到82点伤害

Freddy发起攻击, 万我受到51点伤害

我力使用幻术, 召唤出幻影

万我发起攻击, Freddy受到66点伤害

 Freddy被击倒了, Freddy使用护身符抵挡了一次死亡, Freddy回复体力10点

Dianmu使用魅惑, 我力被魅惑了

幻影发起攻击, Dianmu守护Freddy, Dianmu受到29点伤害

Freddy发起攻击, 万我防御, 万我受到21点伤害

 万我做出垂死抗争, 万我所有属性上升

 Freddy从迟缓中解除

Freddy发起攻击, 万我受到43点伤害

万我使用减速术, Dianmu进入迟缓状态

我力发起攻击, 幻影受到64点伤害

 我力从魅惑中解除

Dianmu发起攻击, 万我回避了攻击

幻影发起攻击, Dianmu受到53点伤害

Freddy发起攻击, 万我防御, 万我受到17点伤害

Freddy使用冰冻术, 幻影受到56点伤害, 幻影被冰冻了

万我使用减速术, Freddy进入迟缓状态

我力发起攻击, Dianmu守护Freddy, Dianmu受到34点伤害

Freddy发起攻击, 幻影受到62点伤害

幻影发起攻击, Freddy受到96点伤害

 Freddy被击倒了

幻影从冰冻中解除

万我发起攻击, Freddy受到65点伤害

Freddy发起攻击, 万我受到42点伤害

 万我被击倒了

我力发起攻击, Dianmu回避了攻击

Dianmu使用分身, 出现一个新的Dianmu

幻影发起攻击, Freddy受到86点伤害

Dianmu使用分身, 出现一个新的Dianmu

幻影发起攻击, Dianmu受到61点伤害

 Dianmu被击倒了

我力发起攻击, Freddy受到121点伤害

 Freddy被击倒了

我力使用分身, 出现一个新的我力

Dianmu使用魅惑, 我力回避了攻击

Dianmu使用魅惑, 幻影被魅惑了

 Dianmu从迟缓中解除

幻影使用附体, Dianmu进入狂暴状态

 幻影消失了

幻影发起攻击, 我力受到55点伤害, 我力发动隐匿

 幻影从魅惑中解除

我力使用分身, 出现一个新的我力

Dianmu发起狂暴攻击, Dianmu受到88点伤害

我力使用幻术, 召唤出幻影

我力发起攻击, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu受到14点伤害

Dianmu发起狂暴攻击, Dianmu受到43点伤害

 Dianmu被击倒了

我力发起攻击, Dianmu受到73点伤害

 Dianmu被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-50 must contain a blank separator between input and trace",
        "sampled case-50 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 3912, "large_50 score mismatch");
    assert!(guard < 20_000, "sampled case-50 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-50", &actual_lines, &expected_lines);
}

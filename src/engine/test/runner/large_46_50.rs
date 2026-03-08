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

#[test]
fn large_48() {
    const CASE: &str = r#"虚空托腮 IVHEWTNEA@TigerStar

进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇
seed:2026-03-07 22:54 #013595@!


虚空托腮发起攻击, 进口牢货.不可磨灭的回忆之殇受到114点伤害

进口牢货.不可磨灭的回忆之殇使用幻术, 召唤出幻影

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到66点伤害

进口牢货.不可磨灭的回忆之殇使用幻术, 召唤出幻影

虚空托腮使用净化, 幻影受到208点伤害

 幻影消失了

虚空托腮发起攻击, 幻影受到90点伤害

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到56点伤害

 虚空托腮发起反击, 进口牢货.不可磨灭的回忆之殇受到54点伤害

幻影发起攻击, 虚空托腮受到116点伤害

虚空托腮使用诅咒, 幻影受到25点伤害, 幻影被诅咒了

进口牢货.不可磨灭的回忆之殇使用分身, 出现一个新的进口牢货.不可磨灭的回忆之殇

幻影使用附体, 虚空托腮进入狂暴状态

 幻影消失了

进口牢货.不可磨灭的回忆之殇使用分身, 出现一个新的进口牢货.不可磨灭的回忆之殇

虚空托腮发起狂暴攻击, 进口牢货.不可磨灭的回忆之殇受到73点伤害

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到54点伤害

进口牢货.不可磨灭的回忆之殇使用幻术, 召唤出幻影

虚空托腮发起狂暴攻击, 进口牢货.不可磨灭的回忆之殇受到73点伤害

 进口牢货.不可磨灭的回忆之殇被击倒了

进口牢货.不可磨灭的回忆之殇发起攻击, 虚空托腮受到66点伤害

 虚空托腮被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-48 must contain a blank separator between input and trace",
        "sampled case-48 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-48 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-48", &actual_lines, &expected_lines);
}

#[test]
fn large_49() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-49 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-49", &actual_lines, &expected_lines);
}

/// 九连守护
#[test]
fn large_50() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-50 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-50", &actual_lines, &expected_lines);
}

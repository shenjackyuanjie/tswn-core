use super::*;

#[test]
fn large_51() {
    const CASE: &str = r#"我力 7#W2ib8D@仙蛊屋+123
万我 68#huMG43@仙蛊屋+123

Dianmu YKFMWRPXIMCQ@nan+234
Freddy FVNXBNVTWJEA@nan+234

seed:第十八届武术大赛小组赛第8组:307-3@!


我力使用幻术, 召唤出幻影

Freddy使用分身, 出现一个新的Freddy

Dianmu使用分身, 出现一个新的Dianmu

万我使用减速术, Dianmu进入迟缓状态

Dianmu使用分身, 出现一个新的Dianmu

Freddy使用生命之轮, 万我的体力值与Freddy互换

Freddy发起攻击, 万我受到60点伤害

我力使用幻术, 召唤出幻影

Dianmu发起攻击, 万我受到27点伤害

Dianmu使用魅惑, 我力被魅惑了

我力使用幻术, 召唤出幻影

 我力从魅惑中解除

万我使用减速术, Freddy进入迟缓状态

幻影发起攻击, Dianmu受到71点伤害

Dianmu发起攻击, 我力受到58点伤害

Freddy发起攻击, 万我受到67点伤害

 万我做出垂死抗争, 万我所有属性上升

万我使用减速术, Freddy进入迟缓状态

Dianmu使用魅惑, 我力被魅惑了

Freddy发起攻击, 幻影受到75点伤害

我力发起攻击, 幻影受到87点伤害

 我力从魅惑中解除

幻影发起攻击, Freddy回避了攻击

Dianmu使用魅惑, 万我被魅惑了

Dianmu使用魅惑, 我力回避了攻击

 Dianmu从迟缓中解除

Dianmu使用魅惑, 我力被魅惑了

幻影发起攻击, Dianmu受到78点伤害

 Dianmu被击倒了

万我使用减速术, 我力进入迟缓状态

 万我从魅惑中解除

Dianmu发起攻击, 幻影受到39点伤害

Freddy使用分身, 出现一个新的Freddy

幻影发起攻击, Freddy受到80点伤害

Dianmu使用分身, 出现一个新的Dianmu

幻影发起攻击, Freddy受到81点伤害

幻影发起攻击, Dianmu受到68点伤害

我力发起攻击, 幻影受到53点伤害

 幻影消失了

 我力从魅惑中解除

万我发起攻击, Freddy受到73点伤害

Freddy发起攻击, 万我受到32点伤害

 万我被击倒了

 Freddy从迟缓中解除

Dianmu开始蓄力

Dianmu使用魅惑, 我力回避了攻击

幻影发起攻击, Freddy受到131点伤害

 Freddy被击倒了, Freddy使用护身符抵挡了一次死亡, Freddy回复体力6点

幻影发起攻击, Dianmu受到64点伤害

 Dianmu被击倒了

Freddy发起攻击, 幻影受到43点伤害

Dianmu使用魅惑, 我力回避了攻击

Freddy发起攻击, 幻影受到74点伤害

Dianmu发起攻击, 我力受到43点伤害

Freddy发起攻击, 幻影受到69点伤害

 Freddy从迟缓中解除

我力发起攻击, Dianmu守护Freddy, Dianmu受到34点伤害

 我力从迟缓中解除

幻影发起攻击, Freddy受到93点伤害

 Freddy被击倒了

幻影发起攻击, Dianmu受到64点伤害

 Dianmu被击倒了

Freddy使用冰冻术, 幻影受到55点伤害

 幻影消失了

Freddy发起攻击, 幻影受到60点伤害

我力使用分身, 出现一个新的我力

Freddy发起攻击, 我力受到45点伤害, 我力发动隐匿

Dianmu使用魅惑, 幻影被魅惑了

Freddy发起攻击, 幻影受到78点伤害

 幻影消失了

Freddy发起攻击, 我力受到30点伤害

我力使用幻术, 召唤出幻影

Dianmu使用魅惑, 我力回避了攻击

Freddy发起攻击, 幻影受到68点伤害

我力使用幻术, 召唤出幻影

我力使用幻术, 召唤出幻影

Freddy使用冰冻术, 幻影受到54点伤害, 幻影被冰冻了

Freddy发起攻击, 幻影受到32点伤害

Dianmu发起攻击, 幻影受到72点伤害

 幻影消失了

我力发起攻击, Dianmu守护Freddy, Dianmu受到25点伤害

Dianmu发起攻击, 幻影受到39点伤害

我力使用分身, 出现一个新的我力

Freddy使用分身, 出现一个新的Freddy

Freddy使用分身, 出现一个新的Freddy

Freddy发起攻击, 我力受到103点伤害

 我力被击倒了

我力发起攻击, Freddy受到111点伤害

 Freddy被击倒了

幻影发起攻击, Dianmu受到86点伤害

 Dianmu被击倒了

Freddy发起攻击, 我力回避了攻击

我力发起攻击, Freddy受到34点伤害

 Freddy被击倒了

幻影发起攻击, Freddy受到70点伤害

 Freddy被击倒了, Freddy使用护身符抵挡了一次死亡, Freddy回复体力4点

我力发起攻击, Freddy受到54点伤害

 Freddy被击倒了

 我力吞噬了Freddy, 我力属性上升

我力发起攻击, Freddy回避了攻击

我力发起攻击, Freddy受到84点伤害

 Freddy被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-51 must contain a blank separator between input and trace",
        "sampled case-51 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-51 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-51", &actual_lines, &expected_lines);
}

#[test]
fn large_52() {
    const CASE: &str = r#"aaaa+123
bbb+324
ccc+2345


aaaa发起攻击, ccc受到100点伤害

bbb发起攻击, aaaa受到92点伤害

ccc发起攻击, bbb受到79点伤害

aaaa使用瘟疫, ccc体力减少48%

ccc发起攻击, aaaa受到60点伤害

bbb发起攻击, ccc受到53点伤害

aaaa发起攻击, bbb受到55点伤害

ccc发起攻击, aaaa受到46点伤害

aaaa开始聚气, aaaa攻击力上升

ccc发起攻击, bbb受到74点伤害

aaaa发起攻击, ccc受到65点伤害

 ccc被击倒了

bbb发起攻击, aaaa受到40点伤害

aaaa发起攻击, bbb回避了攻击

bbb发起攻击, aaaa受到56点伤害

 aaaa被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-52 must contain a blank separator between input and trace",
        "sampled case-52 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-52 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-52", &actual_lines, &expected_lines);
}

/// covid
#[test]
fn large_53() {
    const CASE: &str = r#"我力 7#W2ib8D@仙蛊屋+123
万我 68#huMG43@仙蛊屋+123
Dianmu YKFMWRPXIMCQ@nan+234
Freddy FVNXBNVTWJEA@nan+234

covid@!


万我使用减速术, 新冠病毒回避了攻击

我力使用幻术, 召唤出幻影

Freddy使用分身, 出现一个新的Freddy

Dianmu使用魅惑, 新冠病毒回避了攻击

Freddy使用冰冻术, 新冠病毒回避了攻击

Dianmu发起攻击, 新冠病毒回避了攻击

我力使用幻术, 召唤出幻影

新冠病毒发起攻击, Freddy受到42点伤害, Freddy感染了新冠病毒

Freddy和万我近距离接触, 万我感染了新冠病毒

万我和幻影近距离接触, 幻影感染了新冠病毒

 万我肺炎发作, 万我受到72点伤害, 新冠病毒回复体力10点

幻影和Freddy近距离接触, Freddy感染了新冠病毒

 幻影肺炎发作, 幻影受到95点伤害, 新冠病毒回复体力12点

万我和Dianmu近距离接触, Dianmu感染了新冠病毒

 万我肺炎发作, 万我受到133点伤害, 新冠病毒回复体力17点

Freddy和万我近距离接触

Dianmu和Freddy近距离接触

幻影和Dianmu近距离接触, 但Dianmu没被感染

 幻影肺炎发作, 幻影受到92点伤害

 幻影消失了, 新冠病毒回复体力12点

Freddy和我力近距离接触, 我力感染了新冠病毒

 Freddy肺炎发作, Freddy受到98点伤害, 新冠病毒回复体力13点

我力和Freddy近距离接触

万我和Freddy近距离接触

 万我肺炎发作, 万我受到94点伤害, 新冠病毒回复体力12点

Dianmu和Freddy近距离接触

 Dianmu肺炎发作, Dianmu受到127点伤害, 新冠病毒回复体力16点

我力和Freddy近距离接触

Freddy在重症监护室无法行动

 Freddy肺炎发作, Freddy受到69点伤害

 Freddy被击倒了, 新冠病毒回复体力9点

Freddy和我力近距离接触, 但我力没被感染

 Freddy肺炎发作, Freddy受到104点伤害, 新冠病毒回复体力14点

我力和Dianmu近距离接触

新冠病毒发起攻击, 万我受到83点伤害

 万我被击倒了

Dianmu和Freddy近距离接触, 但Freddy没被感染

 Dianmu肺炎发作, Dianmu受到54点伤害, 新冠病毒回复体力7点

Freddy和幻影近距离接触, 幻影感染了新冠病毒

 Freddy肺炎发作, Freddy受到87点伤害

 Freddy被击倒了, 新冠病毒回复体力11点

幻影和我力近距离接触

Dianmu在重症监护室无法行动

 Dianmu肺炎发作, Dianmu受到134点伤害, 新冠病毒回复体力17点

新冠病毒发起攻击, Dianmu回避了攻击

我力和Dianmu近距离接触

 我力肺炎发作, 我力受到122点伤害, 我力发动隐匿, 新冠病毒回复体力16点

幻影和我力近距离接触

新冠病毒发起攻击, Dianmu受到25点伤害

我力和Dianmu近距离接触

 我力肺炎发作, 我力受到112点伤害, 新冠病毒回复体力15点

Dianmu和幻影近距离接触

 Dianmu肺炎发作, Dianmu受到143点伤害

 Dianmu被击倒了, 新冠病毒回复体力4点

新冠病毒发起攻击, 幻影受到76点伤害

幻影和我力近距离接触

我力在重症监护室无法行动

 我力肺炎发作, 我力受到121点伤害, 新冠病毒回复体力16点

新冠病毒发起攻击, 我力受到31点伤害

 我力被击倒了

 幻影消失了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-53 must contain a blank separator between input and trace",
        "sampled case-53 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-53 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-53", &actual_lines, &expected_lines);
}

/// lazy
#[test]
fn large_54() {
    const CASE: &str = r#"我力 7#W2ib8D@仙蛊屋+123
万我 68#huMG43@仙蛊屋+123
Dianmu YKFMWRPXIMCQ@nan+234
Freddy FVNXBNVTWJEA@nan+234

lazy@!


万我使用减速术, 懒癌进入迟缓状态

我力使用分身, 出现一个新的我力

Dianmu使用魅惑, 懒癌回避了攻击

Freddy发起攻击, 懒癌受到32点伤害, Freddy感染了懒癌

 Freddy懒癌发作, Freddy受到87点伤害

我力使用分身, 出现一个新的我力

万我发起攻击, 懒癌受到58点伤害, 万我感染了懒癌

 万我懒癌发作, 万我受到46点伤害, 万我发动隐匿

Dianmu发起攻击, 懒癌受到42点伤害, Dianmu感染了懒癌

 Dianmu懒癌发作, Dianmu受到53点伤害

懒癌发起攻击, Freddy回避了攻击

我力使用瘟疫, 懒癌回避了攻击

我力使用生命之轮, 懒癌的体力值与我力互换, 我力感染了懒癌

 我力懒癌发作, 我力受到50点伤害

我力使用幻术, 召唤出幻影

万我使用减速术, 懒癌进入迟缓状态

 万我懒癌发作, 万我受到44点伤害

Freddy打开了文明6, 这回合什么也没做

 Freddy懒癌发作, Freddy受到67点伤害

我力使用幻术, 召唤出幻影

Dianmu打开了文明6, 这回合什么也没做

 Dianmu懒癌发作, Dianmu受到97点伤害

我力使用幻术, 召唤出幻影

我力发起攻击, 懒癌受到58点伤害, 我力感染了懒癌

 我力懒癌发作, 我力受到78点伤害

懒癌发起攻击, 我力受到57点伤害, 我力感染了懒癌, 我力发动隐匿

我力打开了朋友圈, 这回合什么也没做

 我力懒癌发作, 我力受到58点伤害

 我力被击倒了

幻影发起攻击, 懒癌受到39点伤害, 幻影感染了懒癌

 懒癌被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-54 must contain a blank separator between input and trace",
        "sampled case-54 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-54 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-54", &actual_lines, &expected_lines);
}


/// saitama
#[test]
fn large_55() {
    const CASE: &str = r#"我力 7#W2ib8D@仙蛊屋+123
万我 68#huMG43@仙蛊屋+123
Dianmu YKFMWRPXIMCQ@nan+234
Freddy FVNXBNVTWJEA@nan+234

saitama@!


Freddy使用分身, 出现一个新的Freddy

Dianmu发起攻击, 一拳超人受到0点伤害

我力使用分身, 出现一个新的我力

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用分身, 出现一个新的Dianmu

万我发起攻击, 一拳超人受到0点伤害

Freddy使用分身, 出现一个新的Freddy

我力使用幻术, 召唤出幻影

Dianmu发起攻击, 一拳超人回避了攻击

我力使用幻术, 召唤出幻影

万我发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

万我使用减速术, 一拳超人进入迟缓状态

Freddy使用冰冻术, 一拳超人受到0点伤害

我力使用幻术, 召唤出幻影

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人受到0点伤害

Freddy使用冰冻术, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人回避了攻击

我力使用幻术, 召唤出幻影

Freddy发起攻击, 一拳超人回避了攻击

万我发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

Freddy使用生命之轮, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

我力使用幻术, 召唤出幻影

Dianmu使用魅惑, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

我力使用瘟疫, 一拳超人体力减少27%

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人回避了攻击

一拳超人从魅惑中解除

我力使用幻术, 召唤出幻影

幻影发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

Dianmu使用分身, 出现一个新的Dianmu

Freddy发起攻击, 一拳超人回避了攻击

幻影使用附体, 一拳超人回避了攻击

万我使用减速术, 一拳超人回避了攻击

Freddy使用冰冻术, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

Dianmu发起攻击, 一拳超人受到0点伤害

我力使用生命之轮, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人被魅惑了

幻影发起攻击, 一拳超人受到0点伤害

一拳超人从迟缓中解除

我力发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

一拳超人从魅惑中解除

幻影使用附体, 一拳超人回避了攻击

幻影使用附体, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

我力发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人回避了攻击

幻影使用附体, 一拳超人进入狂暴状态

 幻影消失了

幻影发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

Freddy使用冰冻术, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Dianmu使用分身, 出现一个新的Dianmu

Freddy发起攻击, 一拳超人回避了攻击

一拳超人发起狂暴攻击, 幻影受到82点伤害

我力发起攻击, 一拳超人受到0点伤害

我力使用幻术, 召唤出幻影

Dianmu使用魅惑, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Dianmu受到115点伤害

 Dianmu被击倒了

我力使用生命之轮, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

Dianmu开始蓄力

Dianmu发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, 我力受到84点伤害

幻影发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人回避了攻击

幻影使用附体, 一拳超人进入狂暴状态

 幻影消失了

幻影发起攻击, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人被魅惑了

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Dianmu受到140点伤害

 Dianmu被击倒了

 一拳超人从魅惑中解除

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Freddy使用生命之轮, 一拳超人回避了攻击

一拳超人发起狂暴攻击, 幻影受到98点伤害

Freddy发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人进入狂暴状态

 幻影消失了

幻影发起攻击, 一拳超人受到0点伤害

我力使用幻术, 召唤出幻影

Dianmu使用魅惑, 一拳超人回避了攻击

幻影使用附体, 一拳超人回避了攻击

Dianmu使用苏生术, Dianmu复活了, Dianmu回复体力43点

我力发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, 幻影受到65点伤害

万我使用减速术, 一拳超人回避了攻击

幻影发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人回避了攻击

我力发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Dianmu使用瘟疫, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, 万我受到48点伤害

我力使用生命之轮, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, 幻影受到106点伤害

幻影使用附体, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

一拳超人发起狂暴攻击, 幻影受到73点伤害

幻影发起攻击, 一拳超人回避了攻击

Dianmu使用瘟疫, 一拳超人回避了攻击

Freddy使用分身, 出现一个新的Freddy

我力使用分身, 出现一个新的我力

万我使用减速术, 一拳超人回避了攻击

我力发起攻击, 一拳超人回避了攻击

幻影使用附体, 一拳超人回避了攻击

Freddy使用冰冻术, 一拳超人受到0点伤害

Freddy使用冰冻术, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, 我力受到106点伤害

Dianmu发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

万我使用减速术, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人进入狂暴状态

 幻影消失了

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用苏生术, Dianmu复活了, Dianmu回复体力43点

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, 幻影受到58点伤害

我力发起攻击, 一拳超人受到0点伤害

我力使用生命之轮, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Dianmu受到129点伤害

 Dianmu被击倒了

Freddy使用生命之轮, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人回避了攻击

Freddy使用冰冻术, 一拳超人回避了攻击

我力发起攻击, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人被魅惑了

万我使用减速术, 一拳超人进入迟缓状态

幻影使用附体, 一拳超人进入狂暴状态

 幻影消失了

Freddy发起攻击, 一拳超人回避了攻击

幻影发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

我力发起攻击, 一拳超人回避了攻击

我力发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy使用冰冻术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

幻影使用附体, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

一拳超人发起狂暴攻击, 我力受到97点伤害

 我力被击倒了

 幻影消失了

 一拳超人从魅惑中解除

万我发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

Freddy使用冰冻术, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

Freddy使用冰冻术, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

幻影发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人被魅惑了

一拳超人发起狂暴攻击, 幻影受到118点伤害

 幻影消失了

 一拳超人从迟缓中解除

 一拳超人从魅惑中解除

Freddy发起攻击, 一拳超人受到0点伤害

我力使用生命之轮, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人回避了攻击

我力发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人被魅惑了

Freddy发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

一拳超人发起狂暴攻击, 一拳超人受到0点伤害

 一拳超人从魅惑中解除

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

幻影发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

我力发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

Dianmu开始蓄力

一拳超人发起狂暴攻击, Dianmu守护Freddy, Dianmu守护Dianmu, Dianmu守护Dianmu, Dianmu受到61点伤害

Freddy使用生命之轮, 一拳超人回避了攻击

我力发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人受到0点伤害

Freddy使用生命之轮, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Freddy受到183点伤害

 Freddy被击倒了, Freddy使用护身符抵挡了一次死亡, Freddy回复体力3点

 一拳超人从魅惑中解除

Freddy使用冰冻术, 一拳超人受到0点伤害

幻影使用附体, 一拳超人进入狂暴状态

 幻影消失了

Freddy发起攻击, 一拳超人受到0点伤害

万我使用减速术, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Freddy受到118点伤害

我力发起攻击, 一拳超人受到0点伤害

Dianmu使用分身, 出现一个新的Dianmu

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人被魅惑了

我力发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用瘟疫, 一拳超人回避了攻击

一拳超人发起狂暴攻击, Freddy受到106点伤害

 Freddy被击倒了, Freddy使用护身符抵挡了一次死亡, Freddy回复体力13点

 一拳超人从魅惑中解除

万我发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

我力发起攻击, 一拳超人回避了攻击

一拳超人发起狂暴攻击, Dianmu受到119点伤害

Freddy发起攻击, 一拳超人回避了攻击

我力发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

万我发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Freddy受到152点伤害

 Freddy被击倒了

我力发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu发起攻击, 一拳超人回避了攻击

Dianmu使用魅惑, 一拳超人回避了攻击

我力发起攻击, 一拳超人回避了攻击

Dianmu发起攻击, 一拳超人受到0点伤害

我力发起攻击, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

万我发起攻击, 一拳超人回避了攻击

Freddy使用冰冻术, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

Dianmu发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Dianmu受到70点伤害

 Dianmu被击倒了

 一拳超人从魅惑中解除

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

万我发起攻击, 一拳超人受到0点伤害

我力使用幻术, 召唤出幻影

一拳超人发起狂暴攻击, Freddy受到62点伤害

 一拳超人从魅惑中解除

我力发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人被魅惑了

Dianmu使用魅惑, 一拳超人被魅惑了

Freddy发起攻击, 一拳超人受到0点伤害

一拳超人发起狂暴攻击, Dianmu守护我力, Dianmu受到33点伤害

 一拳超人从狂暴中解除

Dianmu使用魅惑, 一拳超人回避了攻击

Freddy使用生命之轮, 一拳超人的体力值与Freddy互换

万我发起攻击, 一拳超人受到0点伤害

Freddy发起攻击, 一拳超人受到0点伤害

Dianmu使用魅惑, 一拳超人回避了攻击

一拳超人觉得有点饿

 一拳超人离开了战场
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-55 must contain a blank separator between input and trace",
        "sampled case-55 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-55 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-55", &actual_lines, &expected_lines);
}
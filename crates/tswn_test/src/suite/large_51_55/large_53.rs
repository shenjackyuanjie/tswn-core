use super::*;

/// covid
pub fn large_53<E: crate::EngineAdapter>() {
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
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-53 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-53", &actual_lines, &expected_lines);
    assert_eq!(total_score, 2557, "sampled case-53 total score");
}

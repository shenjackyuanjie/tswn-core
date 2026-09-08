use super::*;

/// lazy
pub fn large_54<E: crate::EngineAdapter>() {
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
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-54 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-54", &actual_lines, &expected_lines);
    assert_eq!(total_score, 1767, "sampled case-54 total score");
}

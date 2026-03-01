use super::*;

#[test]
fn large_41() {
    const CASE: &str = r####"桃v66wy7tgu27xp@asyncTales
b64d64cfaae1f621@asyncTales

Reku_Mochizuki #494460162188@新纪元
扶灵 /eaKEZPkiLP/@新纪元

seed:第十八届武术大赛附加赛:689-1@!


Reku_Mochizuki发起攻击, b64d64cfaae1f621回避了攻击

扶灵发起攻击, b64d64cfaae1f621受到64点伤害

桃v66wy7tgu27xp使用地裂术

 Reku_Mochizuki受到55点伤害

 扶灵受到42点伤害

 扶灵发起反击, 桃v66wy7tgu27xp受到54点伤害

b64d64cfaae1f621发起攻击, 扶灵受到72点伤害

扶灵使用减速术, 桃v66wy7tgu27xp进入迟缓状态

Reku_Mochizuki使用减速术, b64d64cfaae1f621回避了攻击

b64d64cfaae1f621发起攻击, 扶灵受到91点伤害

Reku_Mochizuki使用减速术, b64d64cfaae1f621进入迟缓状态

桃v66wy7tgu27xp发动铁壁, 桃v66wy7tgu27xp防御力大幅上升

Reku_Mochizuki发起攻击, 桃v66wy7tgu27xp受到1点伤害, 桃v66wy7tgu27xp发动隐匿

扶灵使用分身, 出现一个新的扶灵

b64d64cfaae1f621使用净化, 扶灵受到36点伤害

扶灵发起攻击, b64d64cfaae1f621回避了攻击

Reku_Mochizuki使用分身, 出现一个新的Reku_Mochizuki

桃v66wy7tgu27xp使用净化, Reku_Mochizuki受到89点伤害

 桃v66wy7tgu27xp从迟缓中解除

Reku_Mochizuki发起攻击, b64d64cfaae1f621受到68点伤害

Reku_Mochizuki发起攻击, 桃v66wy7tgu27xp受到1点伤害

扶灵使用分身, 出现一个新的扶灵

桃v66wy7tgu27xp使用地裂术

 扶灵受到21点伤害

 Reku_Mochizuki受到39点伤害

 扶灵受到24点伤害

 扶灵受到32点伤害

 桃v66wy7tgu27xp从铁壁中解除

 扶灵发起反击, 桃v66wy7tgu27xp受到24点伤害

 Reku_Mochizuki发起反击, 桃v66wy7tgu27xp受到45点伤害, 桃v66wy7tgu27xp发动隐匿

扶灵发起攻击, b64d64cfaae1f621回避了攻击

扶灵发起攻击, b64d64cfaae1f621回避了攻击

Reku_Mochizuki使用减速术, b64d64cfaae1f621进入迟缓状态

b64d64cfaae1f621发起攻击, Reku_Mochizuki受到82点伤害

 Reku_Mochizuki被击倒了, Reku_Mochizuki使用护身符抵挡了一次死亡, Reku_Mochizuki回复体力10点

扶灵发起攻击, b64d64cfaae1f621受到30点伤害

桃v66wy7tgu27xp使用地裂术

 扶灵受到21点伤害

 Reku_Mochizuki受到66点伤害

 Reku_Mochizuki被击倒了

 桃v66wy7tgu27xp召唤亡灵, Reku_Mochizuki变成了丧尸

 扶灵受到42点伤害

 扶灵被击倒了

 桃v66wy7tgu27xp召唤亡灵, 扶灵变成了丧尸

 扶灵受到35点伤害

 扶灵被击倒了

 桃v66wy7tgu27xp召唤亡灵, 扶灵变成了丧尸

Reku_Mochizuki使用减速术, 丧尸进入迟缓状态

桃v66wy7tgu27xp使用地裂术

 扶灵受到17点伤害

 扶灵被击倒了, 扶灵使用护身符抵挡了一次死亡, 扶灵回复体力14点

 Reku_Mochizuki回避了攻击

b64d64cfaae1f621发起攻击, 扶灵受到103点伤害

 扶灵被击倒了

丧尸发起攻击, Reku_Mochizuki受到71点伤害

丧尸发起攻击, Reku_Mochizuki回避了攻击

Reku_Mochizuki使用减速术, 丧尸进入迟缓状态

桃v66wy7tgu27xp发起攻击, Reku_Mochizuki受到48点伤害

Reku_Mochizuki发起攻击, 丧尸受到107点伤害

丧尸发起攻击, Reku_Mochizuki回避了攻击

丧尸发起攻击, Reku_Mochizuki受到37点伤害

 Reku_Mochizuki被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-41 must contain a blank separator between input and trace",
        "sampled case-41 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-41 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-41", &actual_lines, &expected_lines);
}
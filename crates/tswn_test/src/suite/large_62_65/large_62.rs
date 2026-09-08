use super::*;

/// diff_case 04
pub fn large_62<E: crate::EngineAdapter>() {
    const CASE: &str = r#"Italian_Love #5Agn8kVYl@Shabby_fish
我会回来的 #yTneTj00J@Shabby_fish

H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
seed:33554434@!


H6PeQOTNUlx使用加速术, H6PeQOTNUlx进入疾走状态

Orbital潜行到Italian_Love身后

Italian_Love使用地裂术

 H6PeQOTNUlx受到68点伤害

 Orbital回避了攻击

H6PeQOTNUlx使用分身, 出现一个新的H6PeQOTNUlx

Orbital发动背刺, Italian_Love受到180点伤害

我会回来的使用减速术, Orbital进入迟缓状态

H6PeQOTNUlx发起攻击, Italian_Love受到46点伤害

 H6PeQOTNUlx从疾走中解除

Italian_Love发起攻击, H6PeQOTNUlx受到65点伤害

H6PeQOTNUlx使用分身, 出现一个新的H6PeQOTNUlx

H6PeQOTNUlx发起攻击, 我会回来的回避了攻击

我会回来的发起攻击, H6PeQOTNUlx回避了攻击

H6PeQOTNUlx使用加速术, H6PeQOTNUlx进入疾走状态

H6PeQOTNUlx发起攻击, 我会回来的回避了攻击

我会回来的发起攻击, H6PeQOTNUlx受到73点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力13点

Italian_Love发起攻击, H6PeQOTNUlx受到56点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力6点

H6PeQOTNUlx发起攻击, Italian_Love受到66点伤害

H6PeQOTNUlx发起攻击, 我会回来的受到75点伤害

H6PeQOTNUlx使用冰冻术, 我会回来的受到29点伤害, 我会回来的被冰冻了

Orbital发起攻击, Italian_Love回避了攻击

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

Italian_Love使用净化, H6PeQOTNUlx受到66点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力1点

我会回来的从冰冻中解除

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

H6PeQOTNUlx发起攻击, Italian_Love回避了攻击

Italian_Love发动铁壁, Italian_Love防御力大幅上升

我会回来的发起攻击, H6PeQOTNUlx受到29点伤害

 H6PeQOTNUlx被击倒了

H6PeQOTNUlx发起攻击, Italian_Love受到1点伤害

H6PeQOTNUlx发起攻击, Italian_Love受到1点伤害

 Italian_Love发起反击, H6PeQOTNUlx受到51点伤害

Italian_Love使用净化, H6PeQOTNUlx受到74点伤害

 H6PeQOTNUlx被击倒了

Orbital发起攻击, 我会回来的回避了攻击

 Orbital从迟缓中解除

我会回来的发起攻击, H6PeQOTNUlx受到88点伤害

 H6PeQOTNUlx被击倒了

Orbital发起攻击, Italian_Love受到1点伤害

我会回来的潜行到Orbital身后

Orbital发起攻击, Italian_Love受到1点伤害

Italian_Love使用净化, Orbital防御, Orbital受到0点伤害

 Italian_Love从铁壁中解除

Orbital发起攻击, Italian_Love受到8点伤害

Orbital潜行到我会回来的身后

 Orbital从疾走中解除

我会回来的发动背刺, Orbital防御, Orbital受到152点伤害

 Orbital的潜行被识破

Italian_Love发动铁壁, Italian_Love防御力大幅上升

我会回来的发起攻击, Orbital防御, Orbital受到26点伤害

Italian_Love发起攻击, Orbital防御, Orbital受到26点伤害

Orbital发起攻击, Italian_Love受到1点伤害

Italian_Love发起攻击, Orbital受到42点伤害

 Italian_Love从铁壁中解除

我会回来的发起攻击, Orbital受到62点伤害

 Orbital被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-62 must contain a blank separator between input and trace",
        "sampled case-62 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-62 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-62", &actual_lines, &expected_lines);
}

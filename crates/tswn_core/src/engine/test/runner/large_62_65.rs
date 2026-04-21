use super::*;

/// diff_case 04
#[test]
fn large_62() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-62 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-62", &actual_lines, &expected_lines);
}

/// diff_case 07
#[test]
fn large_63() {
    const CASE: &str = r#"killer YFgziuYJGUOW93Ryni2X@czr2012
dust lmHylvLqY4hn0QIMnCia@czr2012

H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
seed:33554435@!


H6PeQOTNUlx使用分身, 出现一个新的H6PeQOTNUlx

killer使用分身, 出现一个新的killer

dust使用减速术, Orbital进入迟缓状态

H6PeQOTNUlx发起攻击, dust回避了攻击

killer使用魅惑, Orbital被魅惑了

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

Orbital使用幻术, 召唤出幻影

 Orbital从魅惑中解除

H6PeQOTNUlx发起攻击, dust回避了攻击

killer使用分身, 出现一个新的killer

dust使用减速术, Orbital进入迟缓状态

killer使用魅惑, Orbital被魅惑了

H6PeQOTNUlx发起攻击, killer受到60点伤害, killer发动隐匿

killer开始蓄力

H6PeQOTNUlx使用加速术, H6PeQOTNUlx进入疾走状态

H6PeQOTNUlx发起攻击, killer受到60点伤害

 killer被击倒了

Orbital发动铁壁, Orbital防御力大幅上升

 Orbital从魅惑中解除

dust使用减速术, H6PeQOTNUlx进入迟缓状态

killer发起攻击, H6PeQOTNUlx受到65点伤害

H6PeQOTNUlx使用分身, 出现一个新的H6PeQOTNUlx

killer发起攻击, H6PeQOTNUlx受到33点伤害

Orbital潜行到killer身后

 Orbital从疾走中解除

H6PeQOTNUlx发起攻击, killer受到82点伤害

H6PeQOTNUlx发起攻击, killer受到48点伤害

 killer被击倒了

dust使用减速术, Orbital进入迟缓状态

killer使用魅惑, Orbital被魅惑了

H6PeQOTNUlx使用加速术, H6PeQOTNUlx进入疾走状态

 H6PeQOTNUlx从疾走中解除

 H6PeQOTNUlx从迟缓中解除

H6PeQOTNUlx发起攻击, dust受到63点伤害

幻影使用附体, killer进入狂暴状态

 幻影消失了

killer使用魅惑, Orbital被魅惑了

H6PeQOTNUlx发起攻击, dust受到44点伤害

Orbital发起攻击, H6PeQOTNUlx受到47点伤害

 Orbital从铁壁中解除

H6PeQOTNUlx发起攻击, dust受到43点伤害

killer使用分身, 出现一个新的killer

dust使用减速术, H6PeQOTNUlx进入迟缓状态

H6PeQOTNUlx发起攻击, killer受到72点伤害

dust发起攻击, H6PeQOTNUlx受到93点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力11点

H6PeQOTNUlx发起攻击, killer受到82点伤害

 killer被击倒了

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

H6PeQOTNUlx发起攻击, dust受到66点伤害

killer发起攻击, H6PeQOTNUlx受到71点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力13点

Orbital发起攻击, H6PeQOTNUlx受到65点伤害

 H6PeQOTNUlx被击倒了

 Orbital从魅惑中解除

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

 H6PeQOTNUlx从疾走中解除

 H6PeQOTNUlx从迟缓中解除

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

dust发起攻击, H6PeQOTNUlx受到63点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力10点

Orbital发起攻击, dust受到37点伤害

 Orbital从迟缓中解除

killer发起攻击, H6PeQOTNUlx受到52点伤害

H6PeQOTNUlx发起攻击, dust回避了攻击

dust发起攻击, H6PeQOTNUlx受到68点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力15点

H6PeQOTNUlx使用加速术, H6PeQOTNUlx进入疾走状态

Orbital发起攻击, dust受到52点伤害

dust使用苏生术, killer复活了, killer回复体力83点

Orbital发起攻击, dust回避了攻击

killer使用魅惑, Orbital被魅惑了

Orbital潜行到Orbital身后

 Orbital从魅惑中解除

H6PeQOTNUlx发起攻击, killer受到79点伤害

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

killer发起攻击, H6PeQOTNUlx受到103点伤害

 H6PeQOTNUlx被击倒了

Orbital发动背刺, Orbital受到263点伤害

dust发起攻击, H6PeQOTNUlx受到98点伤害

 H6PeQOTNUlx被击倒了

Orbital发起攻击, killer受到67点伤害

 killer被击倒了

killer发起攻击, Orbital防御, Orbital受到33点伤害

Orbital发起攻击, dust受到23点伤害

Orbital发起攻击, dust回避了攻击

 Orbital从疾走中解除

dust发起攻击, Orbital受到0点伤害

killer发起攻击, Orbital防御, Orbital受到0点伤害

dust发起攻击, Orbital受到51点伤害

 Orbital被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-63 must contain a blank separator between input and trace",
        "sampled case-63 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-63 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-63", &actual_lines, &expected_lines);
}

/// diff_case 09
#[test]
fn large_64() {
    const CASE: &str = r#"三猴一体 vFz1cu21MCaW@TigerStar
涵虚不等式 PFVKEUPBU@TigerStar

H6PeQOTNUlx@tyakasha
Orbital #sfPTzSpZz@tyakasha
seed:33554435@!


Orbital潜行到三猴一体身后

H6PeQOTNUlx发起攻击, 涵虚不等式受到83点伤害

三猴一体使用减速术, H6PeQOTNUlx进入迟缓状态

涵虚不等式使用幻术, 召唤出幻影

三猴一体发起攻击, H6PeQOTNUlx受到108点伤害

Orbital发动背刺, 三猴一体受到283点伤害

涵虚不等式使用分身, 出现一个新的涵虚不等式

三猴一体使用减速术, Orbital进入迟缓状态

H6PeQOTNUlx发起攻击, 三猴一体受到58点伤害

涵虚不等式使用幻术, 召唤出幻影

涵虚不等式使用分身, 出现一个新的涵虚不等式

三猴一体使用减速术, H6PeQOTNUlx进入迟缓状态

幻影发起攻击, Orbital防御, Orbital受到0点伤害

涵虚不等式发起攻击, H6PeQOTNUlx受到58点伤害

涵虚不等式使用幻术, 召唤出幻影

涵虚不等式发起攻击, H6PeQOTNUlx受到84点伤害

Orbital使用幻术, 召唤出幻影

三猴一体使用减速术, Orbital进入迟缓状态

幻影发起攻击, 幻影受到56点伤害

幻影发起攻击, Orbital受到0点伤害

H6PeQOTNUlx使用分身, 出现一个新的H6PeQOTNUlx

涵虚不等式发起攻击, H6PeQOTNUlx回避了攻击

涵虚不等式发起攻击, H6PeQOTNUlx受到37点伤害

 H6PeQOTNUlx做出垂死抗争, H6PeQOTNUlx所有属性上升

三猴一体发起攻击, H6PeQOTNUlx受到46点伤害

 H6PeQOTNUlx被击倒了

涵虚不等式使用治愈魔法, 三猴一体回复体力78点

幻影使用附体, Orbital回避了攻击

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

幻影使用附体, 幻影进入狂暴状态

 幻影消失了

涵虚不等式发起攻击, H6PeQOTNUlx受到65点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力10点

H6PeQOTNUlx发起攻击, 三猴一体受到0点伤害

Orbital发起攻击, 三猴一体受到37点伤害

幻影发起攻击, H6PeQOTNUlx受到51点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力13点

三猴一体发起攻击, H6PeQOTNUlx受到79点伤害

 H6PeQOTNUlx被击倒了, H6PeQOTNUlx使用护身符抵挡了一次死亡, H6PeQOTNUlx回复体力3点

 H6PeQOTNUlx发起反击, 三猴一体受到43点伤害

涵虚不等式使用幻术, 召唤出幻影

幻影发起狂暴攻击, 涵虚不等式受到104点伤害

 涵虚不等式被击倒了

 幻影消失了

 幻影消失了

H6PeQOTNUlx发起攻击, 三猴一体受到74点伤害

 三猴一体被击倒了

Orbital使用幻术, 召唤出幻影

涵虚不等式发起攻击, H6PeQOTNUlx回避了攻击

涵虚不等式发起攻击, Orbital受到0点伤害

Orbital潜行到幻影身后

 Orbital从迟缓中解除

 Orbital从疾走中解除

H6PeQOTNUlx使用加速术, 幻影进入疾走状态

幻影发起狂暴攻击, 涵虚不等式回避了攻击

涵虚不等式使用魅惑, Orbital被魅惑了

涵虚不等式发起攻击, Orbital受到61点伤害

 Orbital的潜行被识破

Orbital使用幻术, 召唤出幻影

 Orbital从魅惑中解除

H6PeQOTNUlx使用加速术, Orbital进入疾走状态

幻影发起攻击, Orbital受到49点伤害

幻影使用附体, 涵虚不等式进入狂暴状态

 幻影消失了

Orbital发起攻击, 涵虚不等式受到58点伤害

涵虚不等式发起攻击, H6PeQOTNUlx受到80点伤害

 H6PeQOTNUlx被击倒了

幻影发起攻击, Orbital防御, Orbital受到0点伤害

Orbital潜行到幻影身后

幻影发起狂暴攻击, Orbital回避了攻击

Orbital发动背刺, 幻影受到282点伤害

 幻影消失了

 Orbital从疾走中解除

涵虚不等式使用幻术, 召唤出幻影

涵虚不等式发起狂暴攻击, 涵虚不等式受到75点伤害

 涵虚不等式被击倒了

涵虚不等式发起攻击, Orbital受到0点伤害

Orbital使用幻术, 召唤出幻影

幻影发起狂暴攻击, 幻影回避了攻击

 幻影从狂暴中解除

幻影发起攻击, 幻影回避了攻击

涵虚不等式使用幻术, 召唤出幻影

幻影发起攻击, 幻影受到88点伤害

Orbital发起攻击, 幻影受到43点伤害

幻影使用附体, 涵虚不等式进入狂暴状态

 幻影消失了

幻影发起攻击, 幻影受到53点伤害

幻影发起攻击, 幻影受到44点伤害

幻影发起攻击, 涵虚不等式受到32点伤害

涵虚不等式发起狂暴攻击, 幻影受到64点伤害

Orbital使用幻术, 召唤出幻影

幻影使用附体, 幻影进入狂暴状态

 幻影消失了

幻影发起攻击, 幻影受到114点伤害

涵虚不等式发起狂暴攻击, 幻影受到67点伤害

 幻影消失了

Orbital发起攻击, 幻影受到38点伤害

幻影发起狂暴攻击, 幻影受到55点伤害

Orbital发起攻击, 幻影受到7点伤害

涵虚不等式发起狂暴攻击, 幻影受到99点伤害

 幻影消失了

幻影发起攻击, 幻影回避了攻击

涵虚不等式发起狂暴攻击, 幻影受到38点伤害

 幻影消失了

 涵虚不等式从狂暴中解除

Orbital发起攻击, 幻影受到98点伤害

 幻影消失了

涵虚不等式发起攻击, Orbital防御, Orbital受到0点伤害

Orbital发起攻击, 涵虚不等式受到81点伤害

涵虚不等式发起攻击, Orbital受到0点伤害

Orbital发起攻击, 涵虚不等式受到40点伤害

 涵虚不等式被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-64 must contain a blank separator between input and trace",
        "sampled case-64 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-64 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-64", &actual_lines, &expected_lines);
}

/// diff_case 11
#[test]
fn large_65() {
    const CASE: &str = r#"💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜@新纪元
失温滢霞.寂静蓝 `f.Gi\Z:R@四象柯
seed:33554432@!


💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜使用火球术, 失温滢霞.寂静蓝受到105点伤害

失温滢霞.寂静蓝发起攻击, 💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜受到53点伤害

失温滢霞.寂静蓝潜行到💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜身后

💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜使用火球术, 失温滢霞.寂静蓝受到141点伤害

 失温滢霞.寂静蓝的潜行被识破

失温滢霞.寂静蓝潜行到💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜身后

💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜使用魅惑, 失温滢霞.寂静蓝回避了攻击

失温滢霞.寂静蓝发动背刺, 💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜受到390点伤害

 💛💜💛🤎🩵🧡🩷🩶💛💛🤎💙💜被击倒了


"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-65 must contain a blank separator between input and trace",
        "sampled case-65 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-65 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-65", &actual_lines, &expected_lines);
}

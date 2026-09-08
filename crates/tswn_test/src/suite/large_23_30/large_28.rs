use super::*;

pub fn large_28<E: crate::EngineAdapter>() {
    const CASE: &str = r####"PTV55kiVVA
Fh7Fr248m5
EgdgwoXOXg
T9h39xrIiG
E4dzUM6s1M
apSkmaYHqx
KKGwZlzqrC
pqcmAOy1bg
lVLyWbd2M4
sk8cuwkCZx


apSkmaYHqx发起攻击, pqcmAOy1bg受到37点伤害

sk8cuwkCZx使用瘟疫, E4dzUM6s1M体力减少67%

KKGwZlzqrC使用魅惑, E4dzUM6s1M被魅惑了

T9h39xrIiG发起攻击, apSkmaYHqx受到95点伤害

lVLyWbd2M4发起攻击, EgdgwoXOXg受到39点伤害

Fh7Fr248m5使用诅咒, pqcmAOy1bg受到53点伤害, pqcmAOy1bg被诅咒了

EgdgwoXOXg发起攻击, apSkmaYHqx受到115点伤害

pqcmAOy1bg发起攻击, lVLyWbd2M4受到56点伤害

E4dzUM6s1M发起攻击, T9h39xrIiG受到65点伤害

 E4dzUM6s1M从魅惑中解除

PTV55kiVVA发起攻击, Fh7Fr248m5受到53点伤害

sk8cuwkCZx发起攻击, lVLyWbd2M4受到162点伤害

apSkmaYHqx发起攻击, T9h39xrIiG受到86点伤害

lVLyWbd2M4使用生命之轮, E4dzUM6s1M的体力值与lVLyWbd2M4互换

E4dzUM6s1M发起攻击, lVLyWbd2M4受到45点伤害

Fh7Fr248m5发起攻击, apSkmaYHqx受到96点伤害

EgdgwoXOXg发起攻击, 诅咒使伤害加倍, pqcmAOy1bg受到142点伤害

PTV55kiVVA发动铁壁, PTV55kiVVA防御力大幅上升

T9h39xrIiG使用冰冻术, EgdgwoXOXg受到40点伤害, EgdgwoXOXg被冰冻了

apSkmaYHqx发起攻击, KKGwZlzqrC受到67点伤害

pqcmAOy1bg发起攻击, sk8cuwkCZx受到47点伤害

KKGwZlzqrC发起攻击, PTV55kiVVA受到1点伤害

sk8cuwkCZx发起攻击, T9h39xrIiG受到75点伤害

lVLyWbd2M4发起攻击, E4dzUM6s1M受到91点伤害

 E4dzUM6s1M被击倒了

apSkmaYHqx发起攻击, PTV55kiVVA受到1点伤害

T9h39xrIiG发起攻击, sk8cuwkCZx受到33点伤害

PTV55kiVVA发起攻击, sk8cuwkCZx受到68点伤害

pqcmAOy1bg发起攻击, EgdgwoXOXg受到71点伤害

sk8cuwkCZx发起攻击, lVLyWbd2M4受到104点伤害

 lVLyWbd2M4被击倒了

Fh7Fr248m5发起攻击, PTV55kiVVA受到1点伤害

EgdgwoXOXg从冰冻中解除

EgdgwoXOXg发起攻击, apSkmaYHqx受到50点伤害

 apSkmaYHqx被击倒了

KKGwZlzqrC使用减速术, sk8cuwkCZx回避了攻击

sk8cuwkCZx发起攻击, PTV55kiVVA回避了攻击

PTV55kiVVA发起攻击, Fh7Fr248m5受到43点伤害

 PTV55kiVVA从铁壁中解除

pqcmAOy1bg使用火球术, PTV55kiVVA受到174点伤害

T9h39xrIiG发起攻击, KKGwZlzqrC受到39点伤害

Fh7Fr248m5潜行到KKGwZlzqrC身后

EgdgwoXOXg发起攻击, Fh7Fr248m5受到77点伤害

 Fh7Fr248m5的潜行被识破

sk8cuwkCZx发动会心一击, KKGwZlzqrC受到85点伤害

T9h39xrIiG使用冰冻术, sk8cuwkCZx受到37点伤害, sk8cuwkCZx被冰冻了

KKGwZlzqrC发起攻击, EgdgwoXOXg受到51点伤害

Fh7Fr248m5发起攻击, KKGwZlzqrC受到24点伤害

pqcmAOy1bg发起攻击, Fh7Fr248m5受到60点伤害

 Fh7Fr248m5做出垂死抗争, Fh7Fr248m5所有属性上升

PTV55kiVVA发起攻击, sk8cuwkCZx受到109点伤害

 sk8cuwkCZx被击倒了

EgdgwoXOXg发起攻击, PTV55kiVVA受到77点伤害

 PTV55kiVVA被击倒了

T9h39xrIiG发起攻击, pqcmAOy1bg受到52点伤害

 pqcmAOy1bg被击倒了

Fh7Fr248m5发起攻击, T9h39xrIiG受到57点伤害

 T9h39xrIiG被击倒了

EgdgwoXOXg发起攻击, KKGwZlzqrC受到33点伤害

KKGwZlzqrC发起攻击, Fh7Fr248m5受到35点伤害

 Fh7Fr248m5被击倒了

EgdgwoXOXg发起攻击, KKGwZlzqrC受到69点伤害

 KKGwZlzqrC被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-28 must contain a blank separator between input and trace",
        "sampled case-28 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 3824, "large_28 score mismatch");

    assert!(guard < 20_000, "sampled case-28 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-28", &actual_lines, &expected_lines);
}

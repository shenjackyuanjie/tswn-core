use super::*;

pub fn large_22<E: crate::EngineAdapter>() {
    const CASE: &str = r####"7Gk3oYoEGP
pNa6d5nbSu
Fvsbto5UXx
HNi9InX0qm
syF6lHaRkN


HNi9InX0qm发起攻击, syF6lHaRkN受到94点伤害

Fvsbto5UXx发起攻击, 7Gk3oYoEGP受到51点伤害

syF6lHaRkN发起攻击, HNi9InX0qm受到51点伤害

pNa6d5nbSu发起攻击, syF6lHaRkN受到27点伤害

Fvsbto5UXx发起攻击, pNa6d5nbSu受到92点伤害

7Gk3oYoEGP发起攻击, Fvsbto5UXx受到85点伤害

HNi9InX0qm发起攻击, 7Gk3oYoEGP受到63点伤害

pNa6d5nbSu发起攻击, syF6lHaRkN受到88点伤害

syF6lHaRkN发起攻击, 7Gk3oYoEGP受到53点伤害

7Gk3oYoEGP使用雷击术

 syF6lHaRkN受到28点伤害

 syF6lHaRkN受到23点伤害

 syF6lHaRkN受到20点伤害

HNi9InX0qm发起攻击, pNa6d5nbSu受到128点伤害

pNa6d5nbSu发起攻击, HNi9InX0qm受到60点伤害

Fvsbto5UXx发动会心一击, 7Gk3oYoEGP受到120点伤害

 7Gk3oYoEGP被击倒了

HNi9InX0qm发起攻击, pNa6d5nbSu受到76点伤害

 pNa6d5nbSu被击倒了

 HNi9InX0qm吞噬了pNa6d5nbSu, HNi9InX0qm属性上升

Fvsbto5UXx发起攻击, syF6lHaRkN受到53点伤害

HNi9InX0qm发起攻击, Fvsbto5UXx受到117点伤害

syF6lHaRkN使用生命之轮, Fvsbto5UXx的体力值与syF6lHaRkN互换

Fvsbto5UXx发起攻击, HNi9InX0qm受到104点伤害

syF6lHaRkN使用魅惑, Fvsbto5UXx被魅惑了

HNi9InX0qm发起攻击, syF6lHaRkN受到130点伤害

 syF6lHaRkN被击倒了

 HNi9InX0qm吞噬了syF6lHaRkN, HNi9InX0qm属性上升

HNi9InX0qm使用地裂术

 Fvsbto5UXx防御, Fvsbto5UXx受到57点伤害

 Fvsbto5UXx被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-22 must contain a blank separator between input and trace",
        "sampled case-22 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 2055, "large_22 score mismatch");

    assert!(guard < 20_000, "sampled case-22 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-22", &actual_lines, &expected_lines);
}

use super::*;

pub fn large_14<E: crate::EngineAdapter>() {
    const CASE: &str = r####"gKDx7bsm2Z
fIF34rkasK
LTfpktRhRR
zCqAbiIWgv
jcy0qZvM58


jcy0qZvM58发起攻击, LTfpktRhRR受到102点伤害

LTfpktRhRR发起攻击, fIF34rkasK受到48点伤害

zCqAbiIWgv使用幻术, 召唤出幻影

fIF34rkasK发起攻击, jcy0qZvM58受到67点伤害

gKDx7bsm2Z发起攻击, fIF34rkasK受到70点伤害

LTfpktRhRR发起攻击, zCqAbiIWgv受到98点伤害

jcy0qZvM58发起攻击, 幻影受到82点伤害

fIF34rkasK发起攻击, zCqAbiIWgv受到67点伤害

zCqAbiIWgv发起攻击, jcy0qZvM58受到40点伤害

gKDx7bsm2Z发起攻击, 幻影受到51点伤害

jcy0qZvM58发起攻击, LTfpktRhRR回避了攻击

LTfpktRhRR发起攻击, fIF34rkasK受到78点伤害

fIF34rkasK发起攻击, jcy0qZvM58受到152点伤害

gKDx7bsm2Z发起攻击, fIF34rkasK受到50点伤害

zCqAbiIWgv发起攻击, LTfpktRhRR受到78点伤害

jcy0qZvM58发起攻击, zCqAbiIWgv受到40点伤害

LTfpktRhRR发起攻击, jcy0qZvM58受到124点伤害

 jcy0qZvM58被击倒了

幻影发起攻击, gKDx7bsm2Z受到94点伤害

fIF34rkasK使用分身, 出现一个新的fIF34rkasK

zCqAbiIWgv发起攻击, fIF34rkasK受到19点伤害

gKDx7bsm2Z发起攻击, fIF34rkasK受到46点伤害

 fIF34rkasK被击倒了

fIF34rkasK发起攻击, gKDx7bsm2Z受到40点伤害

LTfpktRhRR使用净化, gKDx7bsm2Z受到19点伤害

gKDx7bsm2Z发起攻击, LTfpktRhRR回避了攻击

幻影使用附体, LTfpktRhRR进入狂暴状态

 幻影消失了

zCqAbiIWgv发起攻击, LTfpktRhRR受到62点伤害

fIF34rkasK发起攻击, gKDx7bsm2Z受到76点伤害

LTfpktRhRR发起狂暴攻击, fIF34rkasK受到72点伤害

 fIF34rkasK被击倒了

zCqAbiIWgv使用治愈魔法, zCqAbiIWgv回复体力63点

gKDx7bsm2Z发起攻击, zCqAbiIWgv受到57点伤害

zCqAbiIWgv使用治愈魔法, zCqAbiIWgv回复体力72点

LTfpktRhRR发起狂暴攻击, gKDx7bsm2Z受到75点伤害

 gKDx7bsm2Z被击倒了

zCqAbiIWgv使用治愈魔法, zCqAbiIWgv回复体力56点

LTfpktRhRR发起狂暴攻击, LTfpktRhRR受到63点伤害

zCqAbiIWgv发起攻击, LTfpktRhRR受到49点伤害

 LTfpktRhRR被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-14 must contain a blank separator between input and trace",
        "sampled case-14 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 2490, "large_14 score mismatch");

    assert!(guard < 20_000, "sampled case-14 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-14", &actual_lines, &expected_lines);
}

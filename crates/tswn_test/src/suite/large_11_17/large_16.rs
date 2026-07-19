use super::*;

pub fn large_16<E: crate::EngineAdapter>() {
    const CASE: &str = r####"WxDNynGfG7
BQfPHVmVNP
Qa2SeIjNn5
Ja7D2kEICH
jFpq8Wxd1S


BQfPHVmVNP使用火球术, jFpq8Wxd1S受到138点伤害

WxDNynGfG7发起攻击, Qa2SeIjNn5受到48点伤害

jFpq8Wxd1S发起攻击, Qa2SeIjNn5受到89点伤害

Qa2SeIjNn5发起攻击, WxDNynGfG7受到22点伤害

BQfPHVmVNP发起攻击, WxDNynGfG7受到78点伤害

Ja7D2kEICH潜行到BQfPHVmVNP身后

WxDNynGfG7使用冰冻术, BQfPHVmVNP受到40点伤害, BQfPHVmVNP被冰冻了

jFpq8Wxd1S使用雷击术

 Ja7D2kEICH受到42点伤害

 Ja7D2kEICH的潜行被识破

 Ja7D2kEICH受到47点伤害

 Ja7D2kEICH受到29点伤害

Qa2SeIjNn5发起攻击, Ja7D2kEICH受到95点伤害

WxDNynGfG7发起攻击, BQfPHVmVNP受到124点伤害

jFpq8Wxd1S发起吸血攻击, Qa2SeIjNn5受到98点伤害, jFpq8Wxd1S回复体力49点

Ja7D2kEICH发起攻击, jFpq8Wxd1S受到107点伤害

jFpq8Wxd1S发起攻击, WxDNynGfG7受到28点伤害

BQfPHVmVNP从冰冻中解除

Qa2SeIjNn5发起攻击, jFpq8Wxd1S受到94点伤害

WxDNynGfG7发起攻击, Qa2SeIjNn5受到115点伤害

 Qa2SeIjNn5被击倒了

BQfPHVmVNP发起攻击, WxDNynGfG7受到41点伤害

Ja7D2kEICH发起攻击, BQfPHVmVNP受到85点伤害

jFpq8Wxd1S发起攻击, Ja7D2kEICH受到116点伤害

 Ja7D2kEICH被击倒了

WxDNynGfG7发起攻击, jFpq8Wxd1S受到70点伤害

 jFpq8Wxd1S被击倒了

BQfPHVmVNP发起攻击, WxDNynGfG7受到63点伤害

WxDNynGfG7发动会心一击, BQfPHVmVNP受到120点伤害

 BQfPHVmVNP被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-16 must contain a blank separator between input and trace",
        "sampled case-16 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1984, "large_16 score mismatch");

    assert!(guard < 20_000, "sampled case-16 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-16", &actual_lines, &expected_lines);
}

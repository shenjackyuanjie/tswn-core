use super::*;

pub fn large_29<E: crate::EngineAdapter>() {
    const CASE: &str = r####"ZJRBGWwfMr
dsQ7rehhfX
ZnvjCRPklr
o5RvZTKbcJ
UWGzTd4gNj
u4jNC5MYQn
Am9wrP6S7R
p7BhCxDF8H
qjDm5UrK6p
uqeDruqHBK


p7BhCxDF8H使用冰冻术, ZnvjCRPklr受到50点伤害, ZnvjCRPklr被冰冻了

dsQ7rehhfX发起攻击, uqeDruqHBK回避了攻击

ZJRBGWwfMr开始蓄力

u4jNC5MYQn发起攻击, UWGzTd4gNj受到66点伤害

o5RvZTKbcJ发起攻击, dsQ7rehhfX受到73点伤害

qjDm5UrK6p发起攻击, ZnvjCRPklr受到65点伤害

p7BhCxDF8H发起攻击, uqeDruqHBK受到94点伤害

uqeDruqHBK使用雷击术

 UWGzTd4gNj受到31点伤害

 UWGzTd4gNj回避了攻击

dsQ7rehhfX发起攻击, p7BhCxDF8H受到80点伤害

Am9wrP6S7R发起攻击, o5RvZTKbcJ受到76点伤害

UWGzTd4gNj发起攻击, u4jNC5MYQn受到29点伤害

ZJRBGWwfMr发起攻击, Am9wrP6S7R回避了攻击

u4jNC5MYQn发起攻击, ZJRBGWwfMr受到93点伤害

p7BhCxDF8H发起攻击, Am9wrP6S7R受到36点伤害

ZnvjCRPklr从冰冻中解除

uqeDruqHBK发起攻击, u4jNC5MYQn受到60点伤害

o5RvZTKbcJ发起攻击, qjDm5UrK6p受到42点伤害

qjDm5UrK6p发起攻击, dsQ7rehhfX受到82点伤害

dsQ7rehhfX发起攻击, ZnvjCRPklr回避了攻击

ZnvjCRPklr发起攻击, Am9wrP6S7R受到59点伤害

ZJRBGWwfMr发起攻击, p7BhCxDF8H受到126点伤害

u4jNC5MYQn使用冰冻术, uqeDruqHBK受到27点伤害, uqeDruqHBK被冰冻了

Am9wrP6S7R使用狂暴术, qjDm5UrK6p受到45点伤害, qjDm5UrK6p进入狂暴状态

UWGzTd4gNj发起攻击, ZnvjCRPklr受到34点伤害

 UWGzTd4gNj连击, ZnvjCRPklr受到36点伤害

 UWGzTd4gNj连击, ZnvjCRPklr受到23点伤害

p7BhCxDF8H发起攻击, u4jNC5MYQn受到53点伤害

ZnvjCRPklr发起攻击, ZJRBGWwfMr受到39点伤害

o5RvZTKbcJ发起攻击, UWGzTd4gNj受到76点伤害

qjDm5UrK6p发起攻击, uqeDruqHBK受到81点伤害

ZJRBGWwfMr发起攻击, qjDm5UrK6p受到31点伤害

dsQ7rehhfX使用瘟疫, o5RvZTKbcJ体力减少45%

u4jNC5MYQn使用冰冻术, Am9wrP6S7R受到47点伤害, Am9wrP6S7R被冰冻了

p7BhCxDF8H发起攻击, Am9wrP6S7R受到81点伤害

uqeDruqHBK从冰冻中解除

uqeDruqHBK发起攻击, dsQ7rehhfX受到126点伤害

Am9wrP6S7R从冰冻中解除

UWGzTd4gNj发起攻击, qjDm5UrK6p受到125点伤害

 qjDm5UrK6p被击倒了

ZJRBGWwfMr发起攻击, p7BhCxDF8H回避了攻击

Am9wrP6S7R发起攻击, ZJRBGWwfMr受到28点伤害

ZnvjCRPklr发起攻击, uqeDruqHBK受到33点伤害

dsQ7rehhfX发起攻击, UWGzTd4gNj受到36点伤害

o5RvZTKbcJ发起攻击, u4jNC5MYQn受到76点伤害

uqeDruqHBK发起攻击, p7BhCxDF8H受到93点伤害

UWGzTd4gNj发起攻击, p7BhCxDF8H防御, p7BhCxDF8H受到33点伤害

 p7BhCxDF8H被击倒了

ZJRBGWwfMr发起攻击, ZnvjCRPklr受到75点伤害

u4jNC5MYQn发起攻击, o5RvZTKbcJ受到55点伤害

Am9wrP6S7R发动会心一击, dsQ7rehhfX受到70点伤害

 dsQ7rehhfX被击倒了

o5RvZTKbcJ发起攻击, ZJRBGWwfMr受到17点伤害

Am9wrP6S7R发动会心一击, o5RvZTKbcJ受到116点伤害

 o5RvZTKbcJ被击倒了

UWGzTd4gNj发起攻击, ZJRBGWwfMr受到71点伤害

uqeDruqHBK发起攻击, u4jNC5MYQn受到64点伤害

 u4jNC5MYQn被击倒了

ZnvjCRPklr投毒, Am9wrP6S7R回避了攻击

ZJRBGWwfMr发起攻击, uqeDruqHBK受到66点伤害

Am9wrP6S7R投毒, uqeDruqHBK受到90点伤害

 uqeDruqHBK被击倒了

UWGzTd4gNj发起攻击, ZnvjCRPklr受到29点伤害

ZJRBGWwfMr发起攻击, UWGzTd4gNj回避了攻击

ZnvjCRPklr发起攻击, UWGzTd4gNj回避了攻击

ZJRBGWwfMr发起攻击, UWGzTd4gNj受到91点伤害

 UWGzTd4gNj被击倒了

Am9wrP6S7R发动会心一击, ZJRBGWwfMr受到84点伤害

 ZJRBGWwfMr被击倒了

ZnvjCRPklr投毒, Am9wrP6S7R受到42点伤害

 Am9wrP6S7R被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-29 must contain a blank separator between input and trace",
        "sampled case-29 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 3972, "large_29 score mismatch");

    assert!(guard < 20_000, "sampled case-29 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-29", &actual_lines, &expected_lines);
}

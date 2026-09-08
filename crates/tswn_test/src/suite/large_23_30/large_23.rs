use super::*;

pub fn large_23<E: crate::EngineAdapter>() {
    const CASE: &str = r####"HRunkP2nIZ
wI0BhR6Rsi
CkQx0gweWL
cWZawfNbns
4j7Py4urwy
apiz1IruHh
OiEy84Cgmk
ydcaAnuoAE
cIoifRknW6
Sr2uvletLr


wI0BhR6Rsi发起攻击, 4j7Py4urwy受到55点伤害

ydcaAnuoAE发起攻击, cIoifRknW6受到46点伤害

cWZawfNbns使用诅咒, ydcaAnuoAE受到85点伤害, ydcaAnuoAE被诅咒了

OiEy84Cgmk发起攻击, 诅咒使伤害加倍, ydcaAnuoAE受到50点伤害

HRunkP2nIZ发起攻击, wI0BhR6Rsi受到78点伤害

cIoifRknW6发起攻击, HRunkP2nIZ受到61点伤害

Sr2uvletLr发起攻击, CkQx0gweWL受到56点伤害

CkQx0gweWL发起攻击, wI0BhR6Rsi受到130点伤害

ydcaAnuoAE发起攻击, HRunkP2nIZ受到89点伤害

4j7Py4urwy发起攻击, ydcaAnuoAE受到31点伤害

apiz1IruHh使用冰冻术, CkQx0gweWL受到31点伤害, CkQx0gweWL被冰冻了

HRunkP2nIZ发起攻击, OiEy84Cgmk受到35点伤害

cWZawfNbns发起攻击, 4j7Py4urwy受到91点伤害

wI0BhR6Rsi发起攻击, cIoifRknW6受到100点伤害

CkQx0gweWL从冰冻中解除

OiEy84Cgmk使用净化, CkQx0gweWL受到93点伤害

Sr2uvletLr发起攻击, 4j7Py4urwy受到29点伤害

CkQx0gweWL发起攻击, HRunkP2nIZ受到65点伤害

ydcaAnuoAE发起攻击, OiEy84Cgmk回避了攻击

4j7Py4urwy发起攻击, Sr2uvletLr受到45点伤害

 4j7Py4urwy连击, CkQx0gweWL受到78点伤害

 4j7Py4urwy连击, CkQx0gweWL回避了攻击

cWZawfNbns使用诅咒, 4j7Py4urwy受到46点伤害, 4j7Py4urwy被诅咒了

apiz1IruHh发起攻击, 诅咒使伤害加倍, 4j7Py4urwy受到88点伤害

HRunkP2nIZ发起攻击, cIoifRknW6受到85点伤害

wI0BhR6Rsi发起攻击, cWZawfNbns受到81点伤害

cIoifRknW6使用治愈魔法, cIoifRknW6回复体力154点

OiEy84Cgmk发起攻击, cWZawfNbns受到66点伤害

cWZawfNbns发起攻击, ydcaAnuoAE回避了攻击

4j7Py4urwy发起攻击, OiEy84Cgmk受到65点伤害

 4j7Py4urwy连击, apiz1IruHh受到26点伤害

 4j7Py4urwy连击, apiz1IruHh受到48点伤害

Sr2uvletLr发起攻击, cWZawfNbns受到66点伤害

CkQx0gweWL发起攻击, 诅咒使伤害加倍, ydcaAnuoAE受到46点伤害

ydcaAnuoAE使用火球术, wI0BhR6Rsi受到51点伤害

apiz1IruHh发起攻击, HRunkP2nIZ受到60点伤害

cIoifRknW6使用净化, cWZawfNbns受到31点伤害

HRunkP2nIZ发起攻击, wI0BhR6Rsi受到77点伤害

 wI0BhR6Rsi被击倒了

CkQx0gweWL发起攻击, Sr2uvletLr使用伤害反弹, CkQx0gweWL受到23点伤害

4j7Py4urwy发起攻击, Sr2uvletLr受到51点伤害

OiEy84Cgmk发起攻击, Sr2uvletLr使用伤害反弹, OiEy84Cgmk受到41点伤害

apiz1IruHh发起攻击, cWZawfNbns受到107点伤害

 cWZawfNbns被击倒了

ydcaAnuoAE发起攻击, apiz1IruHh受到79点伤害

HRunkP2nIZ发起攻击, cIoifRknW6受到96点伤害

CkQx0gweWL发起攻击, Sr2uvletLr受到77点伤害

cIoifRknW6发起攻击, 4j7Py4urwy受到67点伤害

 4j7Py4urwy被击倒了

apiz1IruHh发起攻击, Sr2uvletLr受到87点伤害

ydcaAnuoAE发动铁壁, ydcaAnuoAE防御力大幅上升

Sr2uvletLr发起攻击, OiEy84Cgmk受到64点伤害

CkQx0gweWL发起攻击, HRunkP2nIZ回避了攻击

OiEy84Cgmk发起攻击, CkQx0gweWL受到95点伤害

 CkQx0gweWL被击倒了

ydcaAnuoAE发起攻击, cIoifRknW6受到43点伤害

HRunkP2nIZ投毒, cIoifRknW6受到36点伤害, cIoifRknW6中毒

Sr2uvletLr发起攻击, OiEy84Cgmk受到41点伤害

OiEy84Cgmk发起攻击, 诅咒使伤害加倍, ydcaAnuoAE受到2点伤害

ydcaAnuoAE使用火球术, HRunkP2nIZ受到32点伤害

 HRunkP2nIZ被击倒了

 ydcaAnuoAE从铁壁中解除

cIoifRknW6发起攻击, apiz1IruHh受到20点伤害

 cIoifRknW6毒性发作, cIoifRknW6受到29点伤害

 cIoifRknW6被击倒了

apiz1IruHh发起吸血攻击, OiEy84Cgmk受到101点伤害, apiz1IruHh回复体力51点

 OiEy84Cgmk被击倒了

Sr2uvletLr发起攻击, apiz1IruHh受到31点伤害

apiz1IruHh发起攻击, 诅咒使伤害加倍, ydcaAnuoAE受到166点伤害

 ydcaAnuoAE被击倒了

Sr2uvletLr发起攻击, apiz1IruHh受到63点伤害

 apiz1IruHh做出垂死抗争, apiz1IruHh所有属性上升

apiz1IruHh发起攻击, Sr2uvletLr使用伤害反弹, apiz1IruHh受到0点伤害

apiz1IruHh发起攻击, Sr2uvletLr受到96点伤害

 Sr2uvletLr被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-23 must contain a blank separator between input and trace",
        "sampled case-23 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 4727, "large_23 score mismatch");

    assert!(guard < 20_000, "sampled case-23 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-23", &actual_lines, &expected_lines);
}

use super::*;

#[test]
fn large_23() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-23 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-23", &actual_lines, &expected_lines);
}

#[test]
fn large_24() {
    const CASE: &str = r####"JdHjkmcAQX
VJoqJLK130
EcpA1rezSh
bPB0L3QgHn
LVAZaldlZD
s0fJOqwYFq
1kAa6aNXaf
suxiWYFS7n
p1K2MgDJ6F
zp7YuG9eob


zp7YuG9eob发起攻击, 1kAa6aNXaf受到62点伤害

EcpA1rezSh使用净化, VJoqJLK130受到47点伤害

p1K2MgDJ6F使用雷击术

 s0fJOqwYFq受到8点伤害

 s0fJOqwYFq受到15点伤害

 s0fJOqwYFq受到21点伤害

 s0fJOqwYFq回避了攻击

JdHjkmcAQX发起攻击, s0fJOqwYFq受到84点伤害

1kAa6aNXaf发起攻击, zp7YuG9eob受到70点伤害

s0fJOqwYFq发起攻击, suxiWYFS7n受到28点伤害

suxiWYFS7n发起攻击, JdHjkmcAQX受到108点伤害

VJoqJLK130发起攻击, s0fJOqwYFq受到115点伤害

bPB0L3QgHn发起攻击, zp7YuG9eob受到75点伤害

LVAZaldlZD发起攻击, VJoqJLK130受到57点伤害

zp7YuG9eob发起攻击, EcpA1rezSh回避了攻击

suxiWYFS7n发起攻击, VJoqJLK130受到86点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn回避了攻击

1kAa6aNXaf发起攻击, p1K2MgDJ6F受到70点伤害

p1K2MgDJ6F发起吸血攻击, 1kAa6aNXaf受到61点伤害, p1K2MgDJ6F回复体力31点

VJoqJLK130发起攻击, LVAZaldlZD受到55点伤害

LVAZaldlZD使用治愈魔法, LVAZaldlZD回复体力55点

JdHjkmcAQX发起攻击, EcpA1rezSh受到13点伤害

s0fJOqwYFq发起攻击, EcpA1rezSh受到86点伤害

bPB0L3QgHn发起攻击, JdHjkmcAQX受到108点伤害

suxiWYFS7n使用冰冻术, LVAZaldlZD受到44点伤害, LVAZaldlZD被冰冻了

p1K2MgDJ6F发起攻击, bPB0L3QgHn受到74点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn受到79点伤害

zp7YuG9eob发起攻击, 1kAa6aNXaf受到119点伤害

1kAa6aNXaf发起攻击, p1K2MgDJ6F受到54点伤害

bPB0L3QgHn发起攻击, JdHjkmcAQX受到22点伤害

LVAZaldlZD从冰冻中解除

JdHjkmcAQX发起攻击, 1kAa6aNXaf受到66点伤害

 1kAa6aNXaf被击倒了

s0fJOqwYFq发起攻击, p1K2MgDJ6F受到49点伤害

suxiWYFS7n发起攻击, s0fJOqwYFq受到72点伤害

 s0fJOqwYFq被击倒了

VJoqJLK130发起攻击, LVAZaldlZD受到51点伤害

LVAZaldlZD发起攻击, p1K2MgDJ6F受到36点伤害

zp7YuG9eob发起攻击, p1K2MgDJ6F受到54点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn回避了攻击

p1K2MgDJ6F使用分身, 出现一个新的p1K2MgDJ6F

VJoqJLK130发起攻击, suxiWYFS7n受到29点伤害

zp7YuG9eob发起攻击, VJoqJLK130受到68点伤害

JdHjkmcAQX使用诅咒, VJoqJLK130受到132点伤害

 VJoqJLK130被击倒了

p1K2MgDJ6F发起攻击, LVAZaldlZD受到55点伤害

EcpA1rezSh发起攻击, bPB0L3QgHn受到73点伤害

bPB0L3QgHn发起攻击, suxiWYFS7n回避了攻击

LVAZaldlZD使用治愈魔法, LVAZaldlZD回复体力97点

suxiWYFS7n发起攻击, LVAZaldlZD受到42点伤害

JdHjkmcAQX发起攻击, zp7YuG9eob受到58点伤害

p1K2MgDJ6F使用雷击术

 zp7YuG9eob受到19点伤害

 zp7YuG9eob受到18点伤害

 zp7YuG9eob受到4点伤害

 zp7YuG9eob受到9点伤害

bPB0L3QgHn发起攻击, JdHjkmcAQX受到24点伤害

EcpA1rezSh发起攻击, p1K2MgDJ6F受到50点伤害

 p1K2MgDJ6F被击倒了

LVAZaldlZD使用诅咒, JdHjkmcAQX受到79点伤害

 JdHjkmcAQX被击倒了

zp7YuG9eob投毒, EcpA1rezSh受到77点伤害, EcpA1rezSh中毒

p1K2MgDJ6F发起攻击, suxiWYFS7n受到21点伤害

suxiWYFS7n发起攻击, EcpA1rezSh受到73点伤害

EcpA1rezSh使用净化, suxiWYFS7n受到37点伤害

 EcpA1rezSh毒性发作, EcpA1rezSh受到17点伤害

p1K2MgDJ6F使用火球术, zp7YuG9eob受到142点伤害

 zp7YuG9eob被击倒了

suxiWYFS7n发起攻击, LVAZaldlZD受到60点伤害

EcpA1rezSh发起攻击, LVAZaldlZD受到35点伤害

 EcpA1rezSh毒性发作, EcpA1rezSh受到14点伤害

 EcpA1rezSh被击倒了

bPB0L3QgHn发起攻击, LVAZaldlZD受到58点伤害

LVAZaldlZD发起攻击, suxiWYFS7n受到78点伤害

p1K2MgDJ6F使用雷击术

 suxiWYFS7n回避了攻击

suxiWYFS7n使用冰冻术, LVAZaldlZD受到64点伤害

 LVAZaldlZD被击倒了

bPB0L3QgHn发起攻击, suxiWYFS7n受到103点伤害

 suxiWYFS7n被击倒了

p1K2MgDJ6F发起攻击, bPB0L3QgHn受到24点伤害

 bPB0L3QgHn被击倒了, bPB0L3QgHn使用护身符抵挡了一次死亡, bPB0L3QgHn回复体力3点

bPB0L3QgHn发起攻击, p1K2MgDJ6F受到62点伤害

 p1K2MgDJ6F被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-24 must contain a blank separator between input and trace",
        "sampled case-24 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-24 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-24", &actual_lines, &expected_lines);
}

#[test]
fn large_25() {
    const CASE: &str = r####"KHw7n5yq9l
FGDv73HnFM
SYOXYHBhUe
xgSKf6cVHK
0TtHNqTVT7
4cTRHI9mby
0zCTBtyJES
d4mwxhBzUA
haas6iBSNi
QTT4bGTfLk


SYOXYHBhUe发起攻击, 4cTRHI9mby受到60点伤害

0zCTBtyJES发起攻击, 4cTRHI9mby受到60点伤害

4cTRHI9mby发起攻击, 0TtHNqTVT7受到62点伤害

haas6iBSNi使用魅惑, d4mwxhBzUA被魅惑了

KHw7n5yq9l潜行到0TtHNqTVT7身后

xgSKf6cVHK发起攻击, FGDv73HnFM受到90点伤害

0TtHNqTVT7发起攻击, 4cTRHI9mby受到78点伤害

FGDv73HnFM发动会心一击, xgSKf6cVHK受到158点伤害

QTT4bGTfLk发起攻击, KHw7n5yq9l回避了攻击

KHw7n5yq9l发动背刺, 0TtHNqTVT7受到289点伤害

 0TtHNqTVT7被击倒了

d4mwxhBzUA发起攻击, d4mwxhBzUA受到50点伤害

 d4mwxhBzUA从魅惑中解除

4cTRHI9mby发起攻击, haas6iBSNi受到83点伤害

haas6iBSNi发起攻击, QTT4bGTfLk受到61点伤害

0zCTBtyJES使用减速术, SYOXYHBhUe进入迟缓状态

FGDv73HnFM发起攻击, 0zCTBtyJES受到58点伤害

QTT4bGTfLk发起攻击, SYOXYHBhUe受到41点伤害

KHw7n5yq9l发起攻击, FGDv73HnFM受到35点伤害

xgSKf6cVHK发起攻击, SYOXYHBhUe受到45点伤害

4cTRHI9mby使用雷击术

 KHw7n5yq9l受到27点伤害

 KHw7n5yq9l受到27点伤害

 KHw7n5yq9l受到13点伤害

 KHw7n5yq9l受到21点伤害

d4mwxhBzUA发起攻击, 0zCTBtyJES受到88点伤害

QTT4bGTfLk使用净化, SYOXYHBhUe受到85点伤害

0zCTBtyJES使用生命之轮, d4mwxhBzUA回避了攻击

haas6iBSNi发起攻击, 0zCTBtyJES回避了攻击

FGDv73HnFM开始蓄力

d4mwxhBzUA发起攻击, haas6iBSNi受到64点伤害

4cTRHI9mby发起攻击, KHw7n5yq9l受到51点伤害

xgSKf6cVHK发起攻击, d4mwxhBzUA受到34点伤害

KHw7n5yq9l发起攻击, 0zCTBtyJES回避了攻击

0zCTBtyJES使用狂暴术, FGDv73HnFM受到41点伤害, FGDv73HnFM进入狂暴状态

xgSKf6cVHK发起攻击, QTT4bGTfLk受到32点伤害

SYOXYHBhUe发起攻击, QTT4bGTfLk受到47点伤害

FGDv73HnFM发起狂暴攻击, 0zCTBtyJES受到228点伤害

 0zCTBtyJES被击倒了

 FGDv73HnFM从狂暴中解除

d4mwxhBzUA发起攻击, SYOXYHBhUe受到93点伤害

KHw7n5yq9l使用减速术, d4mwxhBzUA进入迟缓状态

QTT4bGTfLk发起攻击, KHw7n5yq9l回避了攻击

4cTRHI9mby发起攻击, d4mwxhBzUA受到36点伤害

xgSKf6cVHK发起攻击, KHw7n5yq9l受到27点伤害

haas6iBSNi发起攻击, d4mwxhBzUA受到26点伤害

FGDv73HnFM发起攻击, KHw7n5yq9l回避了攻击

KHw7n5yq9l使用净化, haas6iBSNi受到22点伤害

4cTRHI9mby使用火球术, SYOXYHBhUe受到114点伤害

 SYOXYHBhUe被击倒了

QTT4bGTfLk发起攻击, d4mwxhBzUA受到35点伤害

KHw7n5yq9l发起攻击, QTT4bGTfLk受到85点伤害

xgSKf6cVHK开始聚气, xgSKf6cVHK攻击力上升

4cTRHI9mby使用魅惑, haas6iBSNi回避了攻击

haas6iBSNi发起攻击, KHw7n5yq9l受到40点伤害

d4mwxhBzUA使用幻术, 召唤出幻影

FGDv73HnFM使用地裂术

 幻影受到15点伤害

 d4mwxhBzUA受到35点伤害

 4cTRHI9mby受到38点伤害

 haas6iBSNi受到37点伤害

 xgSKf6cVHK回避了攻击

QTT4bGTfLk使用冰冻术, d4mwxhBzUA回避了攻击

KHw7n5yq9l使用减速术, 幻影进入迟缓状态

4cTRHI9mby发起攻击, 幻影受到24点伤害

xgSKf6cVHK发起攻击, 幻影受到155点伤害

 幻影消失了

haas6iBSNi发起攻击, d4mwxhBzUA受到69点伤害

FGDv73HnFM发动会心一击, 4cTRHI9mby受到102点伤害

 4cTRHI9mby被击倒了

QTT4bGTfLk发起攻击, xgSKf6cVHK受到49点伤害

d4mwxhBzUA发起攻击, FGDv73HnFM受到64点伤害

 d4mwxhBzUA从迟缓中解除

haas6iBSNi发起攻击, FGDv73HnFM受到56点伤害

 FGDv73HnFM被击倒了

KHw7n5yq9l使用减速术, QTT4bGTfLk进入迟缓状态

d4mwxhBzUA发起攻击, haas6iBSNi受到34点伤害

xgSKf6cVHK发起攻击, d4mwxhBzUA回避了攻击

KHw7n5yq9l发起攻击, xgSKf6cVHK受到106点伤害

 xgSKf6cVHK被击倒了

haas6iBSNi发起攻击, KHw7n5yq9l受到25点伤害

d4mwxhBzUA发起攻击, haas6iBSNi受到136点伤害

 haas6iBSNi被击倒了

KHw7n5yq9l发起攻击, QTT4bGTfLk受到73点伤害

 QTT4bGTfLk被击倒了

KHw7n5yq9l使用净化, d4mwxhBzUA受到28点伤害

 d4mwxhBzUA被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-25 must contain a blank separator between input and trace",
        "sampled case-25 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-25 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-25", &actual_lines, &expected_lines);
}

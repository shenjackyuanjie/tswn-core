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

#[test]
fn large_26() {
    const CASE: &str = r####"DWk4uqD7r8
tTXJKvz97a
DcW01V95mE
GKNTjhqO96
dswHgcLrgE
yem9CgYnVC
cwUzcrsQuj
NDCGJvqzmQ
uiR8YhB0SU
ShWe7Y3aZO


uiR8YhB0SU使用狂暴术, tTXJKvz97a受到43点伤害, tTXJKvz97a进入狂暴状态

ShWe7Y3aZO潜行到DcW01V95mE身后

DcW01V95mE开始聚气, DcW01V95mE攻击力上升

DWk4uqD7r8发起攻击, cwUzcrsQuj受到91点伤害

tTXJKvz97a发起狂暴攻击, NDCGJvqzmQ受到44点伤害

 tTXJKvz97a从狂暴中解除

GKNTjhqO96发起攻击, DWk4uqD7r8受到70点伤害

dswHgcLrgE发起攻击, GKNTjhqO96受到59点伤害

NDCGJvqzmQ使用火球术, ShWe7Y3aZO受到125点伤害

 ShWe7Y3aZO的潜行被识破

yem9CgYnVC发起攻击, uiR8YhB0SU回避了攻击

cwUzcrsQuj开始聚气, cwUzcrsQuj攻击力上升

DWk4uqD7r8发起攻击, uiR8YhB0SU回避了攻击

ShWe7Y3aZO发起攻击, uiR8YhB0SU受到89点伤害

DcW01V95mE发起攻击, DWk4uqD7r8受到151点伤害

cwUzcrsQuj使用分身, 出现一个新的cwUzcrsQuj

tTXJKvz97a发起攻击, uiR8YhB0SU受到52点伤害

GKNTjhqO96发起攻击, uiR8YhB0SU受到45点伤害

uiR8YhB0SU发起攻击, NDCGJvqzmQ受到44点伤害

yem9CgYnVC发起攻击, dswHgcLrgE回避了攻击

DWk4uqD7r8发起攻击, dswHgcLrgE受到120点伤害

ShWe7Y3aZO发起攻击, tTXJKvz97a受到44点伤害

dswHgcLrgE发起攻击, GKNTjhqO96受到64点伤害

NDCGJvqzmQ开始聚气, NDCGJvqzmQ攻击力上升

cwUzcrsQuj发起攻击, uiR8YhB0SU回避了攻击

tTXJKvz97a发起攻击, dswHgcLrgE受到105点伤害

 dswHgcLrgE做出垂死抗争, dswHgcLrgE所有属性上升

yem9CgYnVC发起攻击, uiR8YhB0SU受到78点伤害

dswHgcLrgE发起攻击, yem9CgYnVC使用伤害反弹, dswHgcLrgE受到32点伤害

NDCGJvqzmQ发起攻击, tTXJKvz97a回避了攻击

cwUzcrsQuj发起攻击, yem9CgYnVC受到128点伤害

ShWe7Y3aZO发起攻击, cwUzcrsQuj守护cwUzcrsQuj, cwUzcrsQuj受到8点伤害

uiR8YhB0SU发起攻击, tTXJKvz97a受到17点伤害

DWk4uqD7r8发起攻击, DcW01V95mE受到55点伤害

DcW01V95mE发起攻击, dswHgcLrgE受到58点伤害

 dswHgcLrgE被击倒了

GKNTjhqO96使用幻术, 召唤出幻影

tTXJKvz97a发起攻击, cwUzcrsQuj守护cwUzcrsQuj, cwUzcrsQuj受到35点伤害

yem9CgYnVC发起攻击, 幻影受到84点伤害

cwUzcrsQuj发起攻击, GKNTjhqO96受到25点伤害

DWk4uqD7r8发起攻击, GKNTjhqO96回避了攻击

cwUzcrsQuj发起攻击, tTXJKvz97a受到55点伤害

DcW01V95mE发起攻击, DWk4uqD7r8受到73点伤害

 DWk4uqD7r8被击倒了

NDCGJvqzmQ开始蓄力

GKNTjhqO96使用地裂术

 uiR8YhB0SU受到33点伤害

 uiR8YhB0SU被击倒了

 tTXJKvz97a受到4点伤害

 yem9CgYnVC使用伤害反弹, GKNTjhqO96受到31点伤害, GKNTjhqO96发动隐匿

 cwUzcrsQuj守护cwUzcrsQuj, cwUzcrsQuj受到16点伤害

 cwUzcrsQuj受到13点伤害

cwUzcrsQuj发起攻击, DcW01V95mE受到71点伤害

ShWe7Y3aZO潜行到yem9CgYnVC身后

tTXJKvz97a发起攻击, cwUzcrsQuj受到50点伤害

 cwUzcrsQuj被击倒了

 tTXJKvz97a吞噬了cwUzcrsQuj, tTXJKvz97a属性上升

DcW01V95mE使用冰冻术, yem9CgYnVC受到75点伤害, yem9CgYnVC被冰冻了

NDCGJvqzmQ使用冰冻术, DcW01V95mE受到387点伤害

 DcW01V95mE被击倒了

cwUzcrsQuj发起攻击, ShWe7Y3aZO受到202点伤害

 ShWe7Y3aZO的潜行被识破

 ShWe7Y3aZO被击倒了

幻影发起攻击, tTXJKvz97a回避了攻击

yem9CgYnVC从冰冻中解除

tTXJKvz97a使用治愈魔法, tTXJKvz97a回复体力163点

GKNTjhqO96发起攻击, yem9CgYnVC受到61点伤害

 yem9CgYnVC被击倒了

NDCGJvqzmQ潜行到tTXJKvz97a身后

cwUzcrsQuj发起攻击, GKNTjhqO96回避了攻击

GKNTjhqO96发起攻击, NDCGJvqzmQ受到71点伤害

 NDCGJvqzmQ的潜行被识破

tTXJKvz97a发起攻击, NDCGJvqzmQ受到43点伤害

NDCGJvqzmQ潜行到tTXJKvz97a身后

幻影使用附体, tTXJKvz97a回避了攻击

cwUzcrsQuj使用雷击术

 tTXJKvz97a受到44点伤害

 tTXJKvz97a受到25点伤害

 tTXJKvz97a受到40点伤害

 tTXJKvz97a受到13点伤害

 tTXJKvz97a受到25点伤害

 tTXJKvz97a受到40点伤害

幻影发起攻击, tTXJKvz97a受到44点伤害

GKNTjhqO96发起攻击, tTXJKvz97a受到57点伤害

NDCGJvqzmQ发动背刺, tTXJKvz97a受到667点伤害

 tTXJKvz97a被击倒了

cwUzcrsQuj发起攻击, 幻影受到128点伤害

 幻影消失了

GKNTjhqO96发起攻击, cwUzcrsQuj受到27点伤害

 cwUzcrsQuj被击倒了

NDCGJvqzmQ使用冰冻术, GKNTjhqO96受到121点伤害, GKNTjhqO96被冰冻了

NDCGJvqzmQ发起攻击, GKNTjhqO96受到146点伤害

 GKNTjhqO96被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-26 must contain a blank separator between input and trace",
        "sampled case-26 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-26 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-26", &actual_lines, &expected_lines);
}

#[test]
fn large_27() {
    const CASE: &str = r####"SsmPpVRTms
yKEK4eS1Gg
cj1nWpcqUS
gtMTFsg1rF
1Ag8PxyGcQ
ms3gtvYmk4
hcnARsrJ1e
HF8FoYknZz
EiEdB7UnSi
CN3jwvgadL


HF8FoYknZz发动会心一击, ms3gtvYmk4受到70点伤害

cj1nWpcqUS发起攻击, gtMTFsg1rF受到45点伤害

1Ag8PxyGcQ发起攻击, hcnARsrJ1e受到101点伤害

hcnARsrJ1e发起攻击, HF8FoYknZz受到29点伤害

gtMTFsg1rF使用火球术, yKEK4eS1Gg受到79点伤害

CN3jwvgadL发起攻击, 1Ag8PxyGcQ防御, 1Ag8PxyGcQ受到30点伤害

EiEdB7UnSi发起攻击, HF8FoYknZz受到56点伤害

SsmPpVRTms发起攻击, cj1nWpcqUS受到63点伤害

yKEK4eS1Gg发起攻击, EiEdB7UnSi受到95点伤害

ms3gtvYmk4发起攻击, EiEdB7UnSi受到47点伤害

hcnARsrJ1e发起攻击, ms3gtvYmk4受到26点伤害

SsmPpVRTms使用加速术, SsmPpVRTms进入疾走状态

EiEdB7UnSi发起攻击, gtMTFsg1rF受到47点伤害

CN3jwvgadL使用诅咒, hcnARsrJ1e受到94点伤害, hcnARsrJ1e被诅咒了

cj1nWpcqUS发起攻击, HF8FoYknZz受到90点伤害

1Ag8PxyGcQ发起攻击, HF8FoYknZz受到76点伤害

gtMTFsg1rF使用火球术, SsmPpVRTms回避了攻击

SsmPpVRTms发起攻击, CN3jwvgadL受到42点伤害

yKEK4eS1Gg发动会心一击, CN3jwvgadL防御, CN3jwvgadL受到37点伤害

HF8FoYknZz使用诅咒, yKEK4eS1Gg受到54点伤害, yKEK4eS1Gg被诅咒了

hcnARsrJ1e发起攻击, SsmPpVRTms受到49点伤害

cj1nWpcqUS发起攻击, 1Ag8PxyGcQ受到78点伤害

SsmPpVRTms发起攻击, gtMTFsg1rF受到71点伤害

 SsmPpVRTms从疾走中解除

ms3gtvYmk4发起攻击, 诅咒使伤害加倍, yKEK4eS1Gg受到56点伤害

EiEdB7UnSi发起攻击, gtMTFsg1rF回避了攻击

gtMTFsg1rF发起攻击, 诅咒使伤害加倍, hcnARsrJ1e受到236点伤害

 hcnARsrJ1e被击倒了

1Ag8PxyGcQ发起攻击, SsmPpVRTms受到103点伤害

SsmPpVRTms发起攻击, cj1nWpcqUS受到71点伤害

yKEK4eS1Gg发起攻击, EiEdB7UnSi回避了攻击

HF8FoYknZz发起攻击, cj1nWpcqUS受到43点伤害

cj1nWpcqUS发起攻击, SsmPpVRTms受到84点伤害

CN3jwvgadL发起攻击, gtMTFsg1rF受到27点伤害

ms3gtvYmk4发起攻击, 诅咒使伤害加倍, yKEK4eS1Gg受到76点伤害

gtMTFsg1rF发起攻击, cj1nWpcqUS受到59点伤害

EiEdB7UnSi发起攻击, SsmPpVRTms受到66点伤害

 SsmPpVRTms被击倒了

cj1nWpcqUS发起攻击, EiEdB7UnSi受到94点伤害

1Ag8PxyGcQ发起攻击, cj1nWpcqUS受到22点伤害

CN3jwvgadL使用诅咒, ms3gtvYmk4受到44点伤害, ms3gtvYmk4被诅咒了

yKEK4eS1Gg发起攻击, gtMTFsg1rF受到67点伤害

EiEdB7UnSi使用净化, 1Ag8PxyGcQ受到79点伤害

gtMTFsg1rF使用火球术, CN3jwvgadL受到95点伤害

HF8FoYknZz发动铁壁, HF8FoYknZz防御力大幅上升

yKEK4eS1Gg发起攻击, EiEdB7UnSi防御, EiEdB7UnSi受到28点伤害

CN3jwvgadL发起攻击, 诅咒使伤害加倍, yKEK4eS1Gg受到128点伤害

 yKEK4eS1Gg被击倒了

cj1nWpcqUS发起攻击, ms3gtvYmk4回避了攻击

1Ag8PxyGcQ发起攻击, gtMTFsg1rF受到54点伤害

 gtMTFsg1rF做出垂死抗争, gtMTFsg1rF所有属性上升

ms3gtvYmk4发起攻击, cj1nWpcqUS回避了攻击

HF8FoYknZz发起攻击, CN3jwvgadL受到76点伤害

gtMTFsg1rF发起攻击, 诅咒使伤害加倍, ms3gtvYmk4受到110点伤害

EiEdB7UnSi发起攻击, 诅咒使伤害加倍, ms3gtvYmk4受到112点伤害

 ms3gtvYmk4被击倒了

HF8FoYknZz发起攻击, 1Ag8PxyGcQ受到8点伤害

 HF8FoYknZz从铁壁中解除

cj1nWpcqUS发起攻击, gtMTFsg1rF受到36点伤害

 gtMTFsg1rF被击倒了

CN3jwvgadL发起攻击, HF8FoYknZz回避了攻击

1Ag8PxyGcQ发起攻击, cj1nWpcqUS受到81点伤害

 cj1nWpcqUS被击倒了

EiEdB7UnSi发起攻击, CN3jwvgadL受到72点伤害

 CN3jwvgadL被击倒了

HF8FoYknZz发起攻击, EiEdB7UnSi回避了攻击

1Ag8PxyGcQ发起攻击, EiEdB7UnSi受到93点伤害

 EiEdB7UnSi被击倒了

HF8FoYknZz开始蓄力

1Ag8PxyGcQ发起攻击, HF8FoYknZz回避了攻击

1Ag8PxyGcQ发起攻击, HF8FoYknZz受到51点伤害

 HF8FoYknZz被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-27 must contain a blank separator between input and trace",
        "sampled case-27 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-27 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-27", &actual_lines, &expected_lines);
}

#[test]
fn large_28() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-28 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-28", &actual_lines, &expected_lines);
}

#[test]
fn large_29() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-29 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-29", &actual_lines, &expected_lines);
}

#[test]
fn large_30() {
    const CASE: &str = r####"vfcqd8nfvZ
KmyRUeW1dD
U0TvbhRUZT
tWsPQmTVhW
UfjGOy7sst
mVTFneu1Vh
5BSqlS3gdA
VIDvuBJCM4
QtkPjVzRbi
8TENSACVdO


vfcqd8nfvZ发起攻击, mVTFneu1Vh受到66点伤害

QtkPjVzRbi发动会心一击, VIDvuBJCM4受到84点伤害

8TENSACVdO发起攻击, QtkPjVzRbi受到100点伤害

tWsPQmTVhW发起攻击, 8TENSACVdO受到67点伤害

VIDvuBJCM4发起攻击, KmyRUeW1dD受到41点伤害

UfjGOy7sst发起攻击, vfcqd8nfvZ受到72点伤害

U0TvbhRUZT使用血祭, 召唤出使魔

KmyRUeW1dD使用火球术, 5BSqlS3gdA受到128点伤害

5BSqlS3gdA使用幻术, 召唤出幻影

vfcqd8nfvZ发起攻击, tWsPQmTVhW受到124点伤害

QtkPjVzRbi使用诅咒, 幻影受到64点伤害, 幻影被诅咒了

5BSqlS3gdA发起攻击, 8TENSACVdO受到67点伤害

使魔发起攻击, KmyRUeW1dD使用伤害反弹, 使魔受到43点伤害, U0TvbhRUZT受到21点伤害

mVTFneu1Vh发起攻击, 5BSqlS3gdA受到58点伤害

UfjGOy7sst发起攻击, KmyRUeW1dD受到92点伤害

U0TvbhRUZT发起攻击, VIDvuBJCM4受到33点伤害

8TENSACVdO使用雷击术

 UfjGOy7sst受到33点伤害

 UfjGOy7sst受到33点伤害

 UfjGOy7sst受到11点伤害

 UfjGOy7sst受到31点伤害

 UfjGOy7sst受到29点伤害

tWsPQmTVhW开始聚气, tWsPQmTVhW攻击力上升

VIDvuBJCM4发起攻击, 5BSqlS3gdA受到104点伤害

 5BSqlS3gdA被击倒了

 幻影消失了

KmyRUeW1dD发起攻击, vfcqd8nfvZ受到68点伤害

QtkPjVzRbi发起攻击, U0TvbhRUZT受到69点伤害

UfjGOy7sst发起攻击, vfcqd8nfvZ受到62点伤害

U0TvbhRUZT发起攻击, QtkPjVzRbi受到67点伤害

vfcqd8nfvZ发起攻击, mVTFneu1Vh受到88点伤害

8TENSACVdO发起攻击, VIDvuBJCM4回避了攻击

tWsPQmTVhW发起攻击, 8TENSACVdO受到136点伤害

QtkPjVzRbi使用幻术, 召唤出幻影

使魔使用火球术, mVTFneu1Vh受到85点伤害

KmyRUeW1dD发起攻击, tWsPQmTVhW受到30点伤害

U0TvbhRUZT发起攻击, 8TENSACVdO受到16点伤害

VIDvuBJCM4发起攻击, KmyRUeW1dD受到102点伤害

vfcqd8nfvZ发起吸血攻击, UfjGOy7sst受到94点伤害, vfcqd8nfvZ回复体力47点

mVTFneu1Vh发起攻击, QtkPjVzRbi受到135点伤害

 QtkPjVzRbi被击倒了

 幻影消失了

 mVTFneu1Vh召唤亡灵, QtkPjVzRbi变成了丧尸

tWsPQmTVhW发起攻击, 使魔受到31点伤害, U0TvbhRUZT受到15点伤害

UfjGOy7sst发动铁壁, UfjGOy7sst防御力大幅上升

U0TvbhRUZT开始蓄力

8TENSACVdO发起攻击, VIDvuBJCM4受到98点伤害

KmyRUeW1dD发起攻击, U0TvbhRUZT受到37点伤害

使魔发起攻击, KmyRUeW1dD受到30点伤害

vfcqd8nfvZ发起攻击, mVTFneu1Vh受到87点伤害

 mVTFneu1Vh被击倒了

 丧尸消失了

VIDvuBJCM4发起攻击, vfcqd8nfvZ受到37点伤害

tWsPQmTVhW发起攻击, vfcqd8nfvZ受到82点伤害

 vfcqd8nfvZ被击倒了

U0TvbhRUZT使用瘟疫, UfjGOy7sst体力减少61%

8TENSACVdO发动铁壁, 8TENSACVdO防御力大幅上升

KmyRUeW1dD发起攻击, 8TENSACVdO受到1点伤害

VIDvuBJCM4发起攻击, tWsPQmTVhW受到67点伤害

使魔发起攻击, UfjGOy7sst回避了攻击

UfjGOy7sst发起攻击, tWsPQmTVhW受到74点伤害

 tWsPQmTVhW被击倒了

VIDvuBJCM4使用净化, 8TENSACVdO受到1点伤害

 8TENSACVdO的铁壁被打消了

UfjGOy7sst发起攻击, 使魔受到128点伤害, U0TvbhRUZT受到64点伤害

 使魔消失了

 UfjGOy7sst从铁壁中解除

U0TvbhRUZT发起攻击, 8TENSACVdO受到56点伤害

 8TENSACVdO被击倒了

KmyRUeW1dD发起攻击, VIDvuBJCM4受到76点伤害

U0TvbhRUZT发动铁壁, U0TvbhRUZT防御力大幅上升

VIDvuBJCM4使用净化, U0TvbhRUZT受到1点伤害

 U0TvbhRUZT的铁壁被打消了

UfjGOy7sst发起攻击, U0TvbhRUZT受到86点伤害

 U0TvbhRUZT被击倒了, U0TvbhRUZT使用护身符抵挡了一次死亡, U0TvbhRUZT回复体力9点

VIDvuBJCM4发起攻击, U0TvbhRUZT回避了攻击

KmyRUeW1dD发起攻击, VIDvuBJCM4受到51点伤害

 VIDvuBJCM4被击倒了

UfjGOy7sst发起攻击, KmyRUeW1dD受到49点伤害

 KmyRUeW1dD被击倒了

U0TvbhRUZT发起攻击, UfjGOy7sst受到38点伤害

 UfjGOy7sst被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-30 must contain a blank separator between input and trace",
        "sampled case-30 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);

    assert!(guard < 20_000, "sampled case-30 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-30", &actual_lines, &expected_lines);
}

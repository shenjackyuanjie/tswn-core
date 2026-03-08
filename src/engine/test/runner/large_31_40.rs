use super::*;

#[test]
fn large_31() {
    const CASE: &str = r####"nXOINTHn5q
emuuGEJFCU
9Qjf75FlDX
X5nHALQsd8
dLZ78AtLlO
GtZHyiydqU
PDET6mWWde
HnTM6ax71z
KzNAjCWSH7
dpCNNufW7B


dLZ78AtLlO开始蓄力

dpCNNufW7B发起攻击, emuuGEJFCU受到60点伤害

emuuGEJFCU使用冰冻术, KzNAjCWSH7受到60点伤害, KzNAjCWSH7被冰冻了

9Qjf75FlDX发起攻击, dLZ78AtLlO受到75点伤害

X5nHALQsd8发起攻击, 9Qjf75FlDX受到56点伤害

PDET6mWWde发起攻击, X5nHALQsd8受到49点伤害

nXOINTHn5q发起攻击, GtZHyiydqU受到35点伤害

HnTM6ax71z发动铁壁, HnTM6ax71z防御力大幅上升

GtZHyiydqU使用瘟疫, X5nHALQsd8回避了攻击

dpCNNufW7B发起攻击, X5nHALQsd8受到68点伤害

emuuGEJFCU发起攻击, GtZHyiydqU受到97点伤害

dLZ78AtLlO发起攻击, KzNAjCWSH7受到361点伤害

 KzNAjCWSH7被击倒了

9Qjf75FlDX发起攻击, emuuGEJFCU受到94点伤害

X5nHALQsd8发起攻击, dLZ78AtLlO受到59点伤害

HnTM6ax71z发起攻击, GtZHyiydqU受到78点伤害

GtZHyiydqU使用雷击术

 dLZ78AtLlO受到20点伤害

 dLZ78AtLlO受到40点伤害

 dLZ78AtLlO受到19点伤害

 dLZ78AtLlO受到23点伤害

nXOINTHn5q发动会心一击, X5nHALQsd8回避了攻击

dpCNNufW7B发起攻击, PDET6mWWde受到81点伤害

PDET6mWWde发起攻击, GtZHyiydqU受到35点伤害

emuuGEJFCU发起攻击, HnTM6ax71z受到1点伤害

dLZ78AtLlO发起攻击, emuuGEJFCU受到78点伤害

9Qjf75FlDX发起攻击, dpCNNufW7B受到133点伤害

X5nHALQsd8发起攻击, HnTM6ax71z受到1点伤害

dpCNNufW7B发起攻击, 9Qjf75FlDX受到33点伤害

emuuGEJFCU发起攻击, dLZ78AtLlO受到80点伤害

 dLZ78AtLlO被击倒了

HnTM6ax71z发起攻击, X5nHALQsd8受到81点伤害

 HnTM6ax71z从铁壁中解除

GtZHyiydqU发起攻击, HnTM6ax71z受到95点伤害

9Qjf75FlDX发起攻击, PDET6mWWde受到49点伤害

nXOINTHn5q发起攻击, HnTM6ax71z受到88点伤害

PDET6mWWde发起攻击, X5nHALQsd8回避了攻击

dpCNNufW7B发起攻击, X5nHALQsd8受到39点伤害

emuuGEJFCU开始聚气, emuuGEJFCU攻击力上升

9Qjf75FlDX发起攻击, X5nHALQsd8受到26点伤害

X5nHALQsd8发起攻击, nXOINTHn5q受到109点伤害

GtZHyiydqU发动会心一击, nXOINTHn5q受到83点伤害

PDET6mWWde发起攻击, 9Qjf75FlDX受到63点伤害

nXOINTHn5q发起攻击, dpCNNufW7B受到107点伤害

9Qjf75FlDX发起攻击, dpCNNufW7B受到54点伤害

 dpCNNufW7B被击倒了

HnTM6ax71z发起攻击, PDET6mWWde受到81点伤害

emuuGEJFCU发起攻击, nXOINTHn5q受到51点伤害

PDET6mWWde发起攻击, X5nHALQsd8受到71点伤害

 X5nHALQsd8被击倒了

nXOINTHn5q发起攻击, 9Qjf75FlDX回避了攻击

GtZHyiydqU发起攻击, 9Qjf75FlDX受到46点伤害

9Qjf75FlDX使用魅惑, PDET6mWWde被魅惑了

emuuGEJFCU发起攻击, HnTM6ax71z受到174点伤害

 HnTM6ax71z被击倒了

PDET6mWWde发起攻击, emuuGEJFCU受到95点伤害

 emuuGEJFCU被击倒了

 PDET6mWWde从魅惑中解除

nXOINTHn5q发起攻击, 9Qjf75FlDX受到51点伤害

9Qjf75FlDX发起攻击, GtZHyiydqU受到68点伤害

 GtZHyiydqU被击倒了

PDET6mWWde发起攻击, 9Qjf75FlDX回避了攻击

9Qjf75FlDX发起攻击, PDET6mWWde受到69点伤害

 PDET6mWWde被击倒了

nXOINTHn5q发起攻击, 9Qjf75FlDX受到60点伤害

 9Qjf75FlDX被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-31 must contain a blank separator between input and trace",
        "sampled case-31 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 3974, "large_31 score mismatch");
    assert!(guard < 20_000, "sampled case-31 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-31", &actual_lines, &expected_lines);
}

#[test]
fn large_32() {
    const CASE: &str = r####"n4UEszaJcP
caxBSldTgg
KLfaUisdMk
BjMT1lbSJH
dTV3gO4eJ2
U95xBiTGiv
nHxLGjoSmw
KSaiixrj1P
96zFtkgJRR
nXhfTAItPU


caxBSldTgg发起攻击, 96zFtkgJRR受到53点伤害

 caxBSldTgg连击, 96zFtkgJRR受到19点伤害

dTV3gO4eJ2使用冰冻术, BjMT1lbSJH受到56点伤害, BjMT1lbSJH被冰冻了

96zFtkgJRR发起攻击, dTV3gO4eJ2受到75点伤害

nXhfTAItPU发起攻击, KLfaUisdMk受到26点伤害

KSaiixrj1P发起攻击, KLfaUisdMk受到33点伤害

n4UEszaJcP发起攻击, nHxLGjoSmw回避了攻击

nHxLGjoSmw发起攻击, BjMT1lbSJH受到41点伤害

caxBSldTgg发起攻击, U95xBiTGiv受到104点伤害

KLfaUisdMk发起攻击, 96zFtkgJRR受到99点伤害

U95xBiTGiv发起攻击, dTV3gO4eJ2受到67点伤害

BjMT1lbSJH从冰冻中解除

96zFtkgJRR使用雷击术

 nHxLGjoSmw受到32点伤害

 nHxLGjoSmw受到33点伤害

 nHxLGjoSmw受到24点伤害

 nHxLGjoSmw受到22点伤害

BjMT1lbSJH发起攻击, dTV3gO4eJ2受到71点伤害

n4UEszaJcP使用血祭, 召唤出使魔

nXhfTAItPU使用加速术, nXhfTAItPU进入疾走状态

caxBSldTgg发起攻击, KSaiixrj1P受到114点伤害

dTV3gO4eJ2发起攻击, caxBSldTgg受到119点伤害

KLfaUisdMk发起攻击, nXhfTAItPU回避了攻击

nHxLGjoSmw发起攻击, n4UEszaJcP受到52点伤害

nXhfTAItPU使用加速术, nXhfTAItPU进入疾走状态

KSaiixrj1P使用减速术, BjMT1lbSJH进入迟缓状态

U95xBiTGiv发起攻击, nXhfTAItPU回避了攻击

96zFtkgJRR发起攻击, caxBSldTgg受到60点伤害

nXhfTAItPU发起攻击, U95xBiTGiv受到88点伤害

nXhfTAItPU发起攻击, nHxLGjoSmw受到29点伤害

BjMT1lbSJH发起攻击, nHxLGjoSmw受到92点伤害

n4UEszaJcP发起攻击, KSaiixrj1P受到39点伤害

使魔发起攻击, BjMT1lbSJH回避了攻击

dTV3gO4eJ2发起吸血攻击, n4UEszaJcP受到58点伤害, dTV3gO4eJ2回复体力29点

KLfaUisdMk使用瘟疫, dTV3gO4eJ2回避了攻击

U95xBiTGiv发起攻击, n4UEszaJcP受到58点伤害

nXhfTAItPU使用加速术, nXhfTAItPU进入疾走状态

nHxLGjoSmw发起攻击, n4UEszaJcP受到90点伤害

nXhfTAItPU发起攻击, KLfaUisdMk受到23点伤害

KSaiixrj1P发起攻击, nXhfTAItPU回避了攻击

caxBSldTgg发起攻击, nXhfTAItPU回避了攻击

n4UEszaJcP使用狂暴术, nHxLGjoSmw受到92点伤害

 nHxLGjoSmw被击倒了

96zFtkgJRR使用分身, 出现一个新的96zFtkgJRR

nXhfTAItPU使用加速术, nXhfTAItPU进入疾走状态

KSaiixrj1P开始聚气, KSaiixrj1P攻击力上升

U95xBiTGiv发起攻击, dTV3gO4eJ2受到70点伤害

KLfaUisdMk使用瘟疫, nXhfTAItPU体力减少42%

nXhfTAItPU使用加速术, nXhfTAItPU进入疾走状态

caxBSldTgg发起攻击, U95xBiTGiv受到42点伤害

BjMT1lbSJH发起攻击, 96zFtkgJRR受到42点伤害

 BjMT1lbSJH从迟缓中解除

使魔发起攻击, KLfaUisdMk受到64点伤害

n4UEszaJcP使用狂暴术, KLfaUisdMk受到33点伤害, KLfaUisdMk进入狂暴状态

nXhfTAItPU发起攻击, caxBSldTgg受到25点伤害

96zFtkgJRR发起攻击, KSaiixrj1P受到60点伤害

dTV3gO4eJ2发起吸血攻击, 使魔回避了攻击

96zFtkgJRR使用雷击术

 nXhfTAItPU受到0点伤害

 nXhfTAItPU受到37点伤害

 nXhfTAItPU受到17点伤害

KSaiixrj1P发起攻击, dTV3gO4eJ2受到52点伤害

 dTV3gO4eJ2被击倒了

U95xBiTGiv发起攻击, BjMT1lbSJH受到65点伤害

BjMT1lbSJH使用火球术, n4UEszaJcP受到67点伤害

 n4UEszaJcP被击倒了

 使魔消失了

KLfaUisdMk发起狂暴攻击, BjMT1lbSJH受到74点伤害

 KLfaUisdMk从狂暴中解除

nXhfTAItPU发起攻击, KLfaUisdMk防御, KLfaUisdMk受到40点伤害

96zFtkgJRR使用血祭, 召唤出使魔

caxBSldTgg发起吸血攻击, 96zFtkgJRR守护96zFtkgJRR, 96zFtkgJRR受到34点伤害, caxBSldTgg回复体力17点

使魔发起攻击, U95xBiTGiv受到100点伤害

 U95xBiTGiv被击倒了, U95xBiTGiv使用护身符抵挡了一次死亡, U95xBiTGiv回复体力13点

KSaiixrj1P发起攻击, 96zFtkgJRR受到131点伤害

 96zFtkgJRR被击倒了

 使魔消失了

U95xBiTGiv发起攻击, caxBSldTgg受到73点伤害

 caxBSldTgg被击倒了

96zFtkgJRR发起攻击, nXhfTAItPU回避了攻击

nXhfTAItPU发起攻击, 96zFtkgJRR受到83点伤害

 96zFtkgJRR被击倒了

 nXhfTAItPU从疾走中解除

KLfaUisdMk发起攻击, nXhfTAItPU受到35点伤害

BjMT1lbSJH发起攻击, U95xBiTGiv受到118点伤害

 U95xBiTGiv被击倒了, U95xBiTGiv使用护身符抵挡了一次死亡, U95xBiTGiv回复体力4点

U95xBiTGiv发起攻击, nXhfTAItPU受到34点伤害

KSaiixrj1P使用分身, 出现一个新的KSaiixrj1P

KSaiixrj1P发起攻击, KLfaUisdMk受到63点伤害

 KLfaUisdMk被击倒了

BjMT1lbSJH发起攻击, U95xBiTGiv受到87点伤害

 U95xBiTGiv被击倒了

nXhfTAItPU发起攻击, BjMT1lbSJH受到94点伤害

 BjMT1lbSJH被击倒了

KSaiixrj1P使用瘟疫, nXhfTAItPU体力减少53%

KSaiixrj1P发起攻击, nXhfTAItPU受到26点伤害

 nXhfTAItPU被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-32 must contain a blank separator between input and trace",
        "sampled case-32 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 5227, "large_32 score mismatch");
    assert!(guard < 20_000, "sampled case-32 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-32", &actual_lines, &expected_lines);
}

#[test]
fn large_33() {
    const CASE: &str = r####"5NGTLS91Xl
KyxBZuJSTh
krkcLc5u0n
q2TS3gc43x
Dbks8HcgPe
MAmaTlrqOH
29iYAaetFR
zGckKxHWMh
D0u3a5rj7B
6N8EJhGNWm


Dbks8HcgPe发起攻击, 6N8EJhGNWm受到99点伤害

6N8EJhGNWm发起攻击, KyxBZuJSTh受到80点伤害

5NGTLS91Xl发起攻击, zGckKxHWMh受到51点伤害

q2TS3gc43x发起攻击, MAmaTlrqOH受到55点伤害

zGckKxHWMh使用瘟疫, D0u3a5rj7B体力减少56%

MAmaTlrqOH发起攻击, Dbks8HcgPe受到86点伤害

D0u3a5rj7B发起攻击, q2TS3gc43x受到81点伤害

KyxBZuJSTh发起攻击, 29iYAaetFR回避了攻击

Dbks8HcgPe使用血祭, 召唤出使魔

29iYAaetFR发起攻击, Dbks8HcgPe受到94点伤害

krkcLc5u0n使用地裂术

 29iYAaetFR受到40点伤害

 q2TS3gc43x受到10点伤害

 KyxBZuJSTh回避了攻击

 使魔回避了攻击

6N8EJhGNWm发起攻击, Dbks8HcgPe受到17点伤害

5NGTLS91Xl使用地裂术

 krkcLc5u0n受到12点伤害

 zGckKxHWMh受到25点伤害

 MAmaTlrqOH受到11点伤害

 29iYAaetFR受到22点伤害

zGckKxHWMh发起攻击, MAmaTlrqOH受到61点伤害

q2TS3gc43x使用狂暴术, 5NGTLS91Xl受到62点伤害, 5NGTLS91Xl进入狂暴状态

使魔使用火球术, q2TS3gc43x受到0点伤害

D0u3a5rj7B投毒, 5NGTLS91Xl受到65点伤害, 5NGTLS91Xl中毒

krkcLc5u0n使用净化, zGckKxHWMh受到98点伤害

MAmaTlrqOH发起攻击, D0u3a5rj7B受到68点伤害

KyxBZuJSTh使用加速术, KyxBZuJSTh进入疾走状态

5NGTLS91Xl发起狂暴攻击, 29iYAaetFR受到158点伤害

 5NGTLS91Xl从狂暴中解除

 5NGTLS91Xl毒性发作, 5NGTLS91Xl受到33点伤害

Dbks8HcgPe发起攻击, krkcLc5u0n受到24点伤害

KyxBZuJSTh发起攻击, 6N8EJhGNWm受到54点伤害

29iYAaetFR发起攻击, q2TS3gc43x受到34点伤害

KyxBZuJSTh使用减速术, krkcLc5u0n进入迟缓状态

 KyxBZuJSTh从疾走中解除

q2TS3gc43x发起攻击, 6N8EJhGNWm受到48点伤害

6N8EJhGNWm发起攻击, KyxBZuJSTh受到0点伤害

使魔发起攻击, KyxBZuJSTh回避了攻击

zGckKxHWMh发起攻击, 29iYAaetFR回避了攻击

MAmaTlrqOH发起攻击, Dbks8HcgPe受到98点伤害

 Dbks8HcgPe被击倒了

 使魔消失了

5NGTLS91Xl使用幻术, 召唤出幻影

 5NGTLS91Xl毒性发作, 5NGTLS91Xl受到28点伤害

MAmaTlrqOH发起攻击, krkcLc5u0n受到113点伤害

D0u3a5rj7B发起攻击, 幻影受到62点伤害

29iYAaetFR发起吸血攻击, KyxBZuJSTh受到121点伤害, 29iYAaetFR回复体力61点

KyxBZuJSTh发起攻击, zGckKxHWMh受到70点伤害

q2TS3gc43x发起攻击, MAmaTlrqOH受到43点伤害

6N8EJhGNWm发动铁壁, 6N8EJhGNWm防御力大幅上升

zGckKxHWMh发起攻击, 6N8EJhGNWm受到1点伤害

5NGTLS91Xl使用幻术, 召唤出幻影

 5NGTLS91Xl毒性发作, 5NGTLS91Xl受到23点伤害

krkcLc5u0n发起攻击, q2TS3gc43x受到57点伤害

MAmaTlrqOH使用加速术, MAmaTlrqOH进入疾走状态

29iYAaetFR发起攻击, 幻影受到62点伤害

D0u3a5rj7B发起攻击, zGckKxHWMh受到46点伤害

KyxBZuJSTh发起攻击, 6N8EJhGNWm回避了攻击

q2TS3gc43x使用净化, KyxBZuJSTh受到36点伤害

5NGTLS91Xl发起攻击, q2TS3gc43x回避了攻击

 5NGTLS91Xl毒性发作, 5NGTLS91Xl受到19点伤害

 5NGTLS91Xl从中毒中解除

zGckKxHWMh发起攻击, krkcLc5u0n受到40点伤害

幻影发起攻击, MAmaTlrqOH受到76点伤害

MAmaTlrqOH发起攻击, 幻影回避了攻击

KyxBZuJSTh发起攻击, 29iYAaetFR受到139点伤害

 29iYAaetFR被击倒了

6N8EJhGNWm使用雷击术

 5NGTLS91Xl受到20点伤害

 5NGTLS91Xl受到14点伤害

 5NGTLS91Xl受到23点伤害

 5NGTLS91Xl被击倒了

 幻影消失了

 幻影消失了

q2TS3gc43x使用净化, krkcLc5u0n受到42点伤害

MAmaTlrqOH发起攻击, KyxBZuJSTh受到31点伤害

 KyxBZuJSTh做出垂死抗争, KyxBZuJSTh所有属性上升

 MAmaTlrqOH从疾走中解除

KyxBZuJSTh发起攻击, q2TS3gc43x受到85点伤害

zGckKxHWMh发起攻击, KyxBZuJSTh受到48点伤害

 KyxBZuJSTh被击倒了

D0u3a5rj7B发起攻击, 6N8EJhGNWm受到1点伤害

MAmaTlrqOH发起攻击, 6N8EJhGNWm受到1点伤害

6N8EJhGNWm发起攻击, MAmaTlrqOH受到29点伤害

 6N8EJhGNWm从铁壁中解除

zGckKxHWMh发起攻击, 6N8EJhGNWm受到94点伤害

 6N8EJhGNWm做出垂死抗争, 6N8EJhGNWm所有属性上升

krkcLc5u0n发起攻击, q2TS3gc43x受到47点伤害

 krkcLc5u0n从迟缓中解除

q2TS3gc43x使用狂暴术, krkcLc5u0n受到70点伤害

 krkcLc5u0n被击倒了

 q2TS3gc43x吞噬了krkcLc5u0n, q2TS3gc43x属性上升

MAmaTlrqOH发起攻击, 6N8EJhGNWm受到50点伤害

 6N8EJhGNWm被击倒了

zGckKxHWMh发起攻击, D0u3a5rj7B回避了攻击

D0u3a5rj7B发起攻击, q2TS3gc43x受到97点伤害

 q2TS3gc43x被击倒了

MAmaTlrqOH发起攻击, zGckKxHWMh受到71点伤害

 zGckKxHWMh被击倒了

D0u3a5rj7B发起攻击, MAmaTlrqOH受到96点伤害

 MAmaTlrqOH被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-33 must contain a blank separator between input and trace",
        "sampled case-33 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 5074, "large_33 score mismatch");
    assert!(guard < 20_000, "sampled case-33 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-33", &actual_lines, &expected_lines);
}

#[test]
fn large_34() {
    const CASE: &str = r####"rwoiycdN3T
ws9hX5uJwh
CNY0HLRzOx
BPbll4S27a
4l1qq0g27u
fQfZ40hRlV
ugF6VP1ErI
JgkGFCfpzK
gB4u3dlZ8g
JzJUNA3afm


BPbll4S27a发起攻击, gB4u3dlZ8g受到37点伤害

ws9hX5uJwh发起攻击, JzJUNA3afm受到67点伤害

gB4u3dlZ8g发起攻击, rwoiycdN3T受到122点伤害

JzJUNA3afm发起攻击, ws9hX5uJwh受到49点伤害

rwoiycdN3T发起攻击, JzJUNA3afm受到29点伤害

JgkGFCfpzK发动铁壁, JgkGFCfpzK防御力大幅上升

fQfZ40hRlV使用分身, 出现一个新的fQfZ40hRlV

CNY0HLRzOx发起攻击, rwoiycdN3T受到74点伤害

ws9hX5uJwh开始聚气, ws9hX5uJwh攻击力上升

gB4u3dlZ8g使用狂暴术, fQfZ40hRlV受到72点伤害, fQfZ40hRlV进入狂暴状态

ugF6VP1ErI发起攻击, 4l1qq0g27u受到97点伤害

BPbll4S27a发起攻击, JgkGFCfpzK受到1点伤害

rwoiycdN3T发起攻击, fQfZ40hRlV受到24点伤害

ws9hX5uJwh发起攻击, CNY0HLRzOx回避了攻击

4l1qq0g27u发动会心一击, fQfZ40hRlV受到103点伤害, fQfZ40hRlV发动隐匿

fQfZ40hRlV发起攻击, 4l1qq0g27u受到63点伤害

fQfZ40hRlV发起攻击, BPbll4S27a受到38点伤害

CNY0HLRzOx使用分身, 出现一个新的CNY0HLRzOx

JzJUNA3afm发起攻击, gB4u3dlZ8g受到77点伤害

fQfZ40hRlV使用生命之轮, JzJUNA3afm的体力值与fQfZ40hRlV互换

4l1qq0g27u发起攻击, JzJUNA3afm受到49点伤害

 JzJUNA3afm被击倒了

JgkGFCfpzK发起攻击, BPbll4S27a受到108点伤害

ugF6VP1ErI发起攻击, JgkGFCfpzK受到1点伤害

rwoiycdN3T发起攻击, ugF6VP1ErI受到29点伤害

ws9hX5uJwh发起攻击, JgkGFCfpzK受到1点伤害

gB4u3dlZ8g使用火球术, CNY0HLRzOx受到146点伤害

 CNY0HLRzOx被击倒了

CNY0HLRzOx发起攻击, 4l1qq0g27u受到83点伤害

BPbll4S27a发起攻击, JgkGFCfpzK受到1点伤害

JgkGFCfpzK使用净化, ws9hX5uJwh受到19点伤害

 ws9hX5uJwh的聚气被打消了

 JgkGFCfpzK从铁壁中解除

fQfZ40hRlV发起攻击, JgkGFCfpzK受到54点伤害

ugF6VP1ErI发起攻击, BPbll4S27a受到51点伤害

rwoiycdN3T发起攻击, fQfZ40hRlV受到27点伤害

4l1qq0g27u发起攻击, rwoiycdN3T受到44点伤害

ws9hX5uJwh发起攻击, fQfZ40hRlV受到38点伤害

fQfZ40hRlV使用瘟疫, CNY0HLRzOx体力减少47%

BPbll4S27a发起攻击, ws9hX5uJwh受到59点伤害

CNY0HLRzOx发起攻击, gB4u3dlZ8g受到25点伤害

fQfZ40hRlV发起攻击, ugF6VP1ErI受到49点伤害

ws9hX5uJwh发起攻击, fQfZ40hRlV受到89点伤害, fQfZ40hRlV发动隐匿

JgkGFCfpzK发起攻击, BPbll4S27a受到53点伤害

rwoiycdN3T发起攻击, JgkGFCfpzK受到61点伤害

4l1qq0g27u发起攻击, ws9hX5uJwh受到55点伤害

gB4u3dlZ8g使用火球术, fQfZ40hRlV受到123点伤害

 fQfZ40hRlV被击倒了

BPbll4S27a投毒, CNY0HLRzOx受到38点伤害, CNY0HLRzOx中毒

fQfZ40hRlV发起攻击, ugF6VP1ErI受到17点伤害

ugF6VP1ErI发起攻击, 4l1qq0g27u受到32点伤害

rwoiycdN3T发起攻击, ugF6VP1ErI受到75点伤害

CNY0HLRzOx发起攻击, BPbll4S27a回避了攻击

 CNY0HLRzOx毒性发作, CNY0HLRzOx受到42点伤害

 CNY0HLRzOx被击倒了

ws9hX5uJwh发起攻击, ugF6VP1ErI受到66点伤害

JgkGFCfpzK投毒, gB4u3dlZ8g受到62点伤害, gB4u3dlZ8g中毒

4l1qq0g27u发起攻击, ws9hX5uJwh受到130点伤害

 ws9hX5uJwh被击倒了

ugF6VP1ErI发起攻击, rwoiycdN3T受到69点伤害

 rwoiycdN3T被击倒了

BPbll4S27a发起攻击, gB4u3dlZ8g受到94点伤害

gB4u3dlZ8g发起攻击, JgkGFCfpzK受到73点伤害

 gB4u3dlZ8g毒性发作, gB4u3dlZ8g受到26点伤害

4l1qq0g27u发起攻击, BPbll4S27a受到60点伤害

fQfZ40hRlV使用分身, 出现一个新的fQfZ40hRlV

JgkGFCfpzK发起攻击, gB4u3dlZ8g受到56点伤害

 gB4u3dlZ8g被击倒了

ugF6VP1ErI发起攻击, JgkGFCfpzK受到32点伤害

BPbll4S27a发起攻击, 4l1qq0g27u受到83点伤害

 4l1qq0g27u被击倒了

fQfZ40hRlV使用苏生术, fQfZ40hRlV复活了, fQfZ40hRlV回复体力70点

ugF6VP1ErI发起攻击, fQfZ40hRlV受到71点伤害

 fQfZ40hRlV被击倒了

fQfZ40hRlV发起攻击, ugF6VP1ErI受到108点伤害

 ugF6VP1ErI被击倒了

fQfZ40hRlV发起攻击, JgkGFCfpzK受到53点伤害

 JgkGFCfpzK被击倒了

BPbll4S27a发起攻击, fQfZ40hRlV受到65点伤害

 fQfZ40hRlV被击倒了

BPbll4S27a发起攻击, fQfZ40hRlV受到72点伤害

 fQfZ40hRlV被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-34 must contain a blank separator between input and trace",
        "sampled case-34 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 5135, "large_34 score mismatch");
    assert!(guard < 20_000, "sampled case-34 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-34", &actual_lines, &expected_lines);
}

#[test]
fn large_35() {
    const CASE: &str = r####"Cf5rVGzwcr
Q0jeTYcWMo
2HLtE1PaCt
SBCarytgUT
UenBpFTxl5
Pgj5pPNlys
3YPR2KJreZ
Rr6LpD6xaZ
6ZHVUk2Mw0
SxHK3i5QZp


Rr6LpD6xaZ发起攻击, Q0jeTYcWMo受到61点伤害

UenBpFTxl5开始聚气, UenBpFTxl5攻击力上升

SBCarytgUT发起攻击, SxHK3i5QZp受到55点伤害

Cf5rVGzwcr发起攻击, Pgj5pPNlys受到86点伤害

Pgj5pPNlys发起攻击, UenBpFTxl5受到91点伤害

6ZHVUk2Mw0发起攻击, 2HLtE1PaCt受到70点伤害

Q0jeTYcWMo发起攻击, Cf5rVGzwcr受到61点伤害

2HLtE1PaCt发起攻击, Pgj5pPNlys受到19点伤害

SxHK3i5QZp使用生命之轮, Q0jeTYcWMo的体力值与SxHK3i5QZp互换

Rr6LpD6xaZ发起攻击, SxHK3i5QZp回避了攻击

UenBpFTxl5发起攻击, SxHK3i5QZp受到153点伤害

SBCarytgUT投毒, Rr6LpD6xaZ受到63点伤害, Rr6LpD6xaZ中毒

6ZHVUk2Mw0发动铁壁, 6ZHVUk2Mw0防御力大幅上升

3YPR2KJreZ使用魅惑, SBCarytgUT被魅惑了

2HLtE1PaCt发起攻击, Cf5rVGzwcr受到58点伤害

Q0jeTYcWMo开始蓄力

Cf5rVGzwcr发起攻击, SBCarytgUT受到126点伤害

SxHK3i5QZp发起攻击, 3YPR2KJreZ受到71点伤害

Pgj5pPNlys发起攻击, SBCarytgUT受到78点伤害

SBCarytgUT使用诅咒, 6ZHVUk2Mw0回避了攻击

 SBCarytgUT从魅惑中解除

UenBpFTxl5发起攻击, 2HLtE1PaCt受到115点伤害

Rr6LpD6xaZ发起攻击, Q0jeTYcWMo受到36点伤害

 Rr6LpD6xaZ毒性发作, Rr6LpD6xaZ受到30点伤害

2HLtE1PaCt发起攻击, 3YPR2KJreZ受到58点伤害

Q0jeTYcWMo发起攻击, SxHK3i5QZp受到195点伤害

 SxHK3i5QZp被击倒了

3YPR2KJreZ发起攻击, Q0jeTYcWMo受到42点伤害

6ZHVUk2Mw0发起攻击, UenBpFTxl5受到12点伤害

SBCarytgUT发起攻击, 6ZHVUk2Mw0受到1点伤害

UenBpFTxl5发起攻击, Q0jeTYcWMo受到164点伤害

 Q0jeTYcWMo被击倒了

Pgj5pPNlys发起攻击, 6ZHVUk2Mw0受到1点伤害

Cf5rVGzwcr发起攻击, Pgj5pPNlys受到71点伤害

2HLtE1PaCt发起攻击, 6ZHVUk2Mw0受到1点伤害

3YPR2KJreZ发起攻击, UenBpFTxl5受到87点伤害

Rr6LpD6xaZ使用诅咒, UenBpFTxl5受到49点伤害, UenBpFTxl5被诅咒了

 UenBpFTxl5做出垂死抗争, UenBpFTxl5所有属性上升

 Rr6LpD6xaZ毒性发作, Rr6LpD6xaZ受到25点伤害

6ZHVUk2Mw0发起攻击, Cf5rVGzwcr受到87点伤害

 6ZHVUk2Mw0从铁壁中解除

UenBpFTxl5发起攻击, 3YPR2KJreZ受到106点伤害

Cf5rVGzwcr发起攻击, Pgj5pPNlys回避了攻击

Rr6LpD6xaZ发起攻击, SBCarytgUT受到108点伤害

 SBCarytgUT被击倒了

 Rr6LpD6xaZ毒性发作, Rr6LpD6xaZ受到21点伤害

3YPR2KJreZ发起攻击, 6ZHVUk2Mw0受到75点伤害

Pgj5pPNlys发起攻击, 6ZHVUk2Mw0受到64点伤害

2HLtE1PaCt发起攻击, UenBpFTxl5回避了攻击

6ZHVUk2Mw0发起攻击, Pgj5pPNlys受到72点伤害

UenBpFTxl5发起攻击, Rr6LpD6xaZ受到166点伤害

 Rr6LpD6xaZ被击倒了

Cf5rVGzwcr发起攻击, 2HLtE1PaCt受到71点伤害

Pgj5pPNlys发起攻击, Cf5rVGzwcr受到84点伤害

3YPR2KJreZ发起攻击, 6ZHVUk2Mw0受到33点伤害

2HLtE1PaCt使用加速术, 2HLtE1PaCt进入疾走状态

6ZHVUk2Mw0发起攻击, 3YPR2KJreZ受到98点伤害

 3YPR2KJreZ被击倒了

UenBpFTxl5发起攻击, Cf5rVGzwcr受到159点伤害

 Cf5rVGzwcr被击倒了

2HLtE1PaCt发起攻击, 6ZHVUk2Mw0受到39点伤害

6ZHVUk2Mw0发起攻击, 诅咒使伤害加倍, UenBpFTxl5受到108点伤害

 UenBpFTxl5被击倒了

Pgj5pPNlys发起攻击, 2HLtE1PaCt受到32点伤害

 2HLtE1PaCt被击倒了

Pgj5pPNlys使用加速术, Pgj5pPNlys进入疾走状态

6ZHVUk2Mw0发动铁壁, 6ZHVUk2Mw0防御力大幅上升

Pgj5pPNlys发起攻击, 6ZHVUk2Mw0受到1点伤害

Pgj5pPNlys发起攻击, 6ZHVUk2Mw0回避了攻击

 Pgj5pPNlys从疾走中解除

6ZHVUk2Mw0发起攻击, Pgj5pPNlys受到0点伤害

Pgj5pPNlys发起攻击, 6ZHVUk2Mw0受到1点伤害

6ZHVUk2Mw0开始聚气, 6ZHVUk2Mw0攻击力上升

 6ZHVUk2Mw0从铁壁中解除

6ZHVUk2Mw0发起攻击, Pgj5pPNlys回避了攻击

Pgj5pPNlys发起攻击, 6ZHVUk2Mw0回避了攻击

6ZHVUk2Mw0发动铁壁, 6ZHVUk2Mw0防御力大幅上升

Pgj5pPNlys发动会心一击, 6ZHVUk2Mw0回避了攻击

6ZHVUk2Mw0发起攻击, Pgj5pPNlys受到126点伤害

 Pgj5pPNlys被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-35 must contain a blank separator between input and trace",
        "sampled case-35 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4659, "large_35 score mismatch");
    assert!(guard < 20_000, "sampled case-35 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-35", &actual_lines, &expected_lines);
}

#[test]
fn large_36() {
    const CASE: &str = r####"0_aQg8UHd6xh
1_IqwwHJnNkK
2_PPuRzmGPjg
3_uE8wZuV5Gv
4_K1h4EmBKDU
5_rtcsGEOYKz
6_4MCtUQ6DbO
7_Dai5Tf2nvv
8_4hEQ58EiT7
9_AQN4rqDpDP


4_K1h4EmBKDU使用雷击术

 3_uE8wZuV5Gv受到51点伤害

 3_uE8wZuV5Gv受到43点伤害

 3_uE8wZuV5Gv受到23点伤害

 3_uE8wZuV5Gv受到23点伤害

 3_uE8wZuV5Gv受到16点伤害

 3_uE8wZuV5Gv受到25点伤害

1_IqwwHJnNkK发起攻击, 9_AQN4rqDpDP受到53点伤害

8_4hEQ58EiT7发起攻击, 6_4MCtUQ6DbO受到149点伤害

2_PPuRzmGPjg发起攻击, 4_K1h4EmBKDU受到51点伤害

7_Dai5Tf2nvv发起攻击, 2_PPuRzmGPjg受到105点伤害

6_4MCtUQ6DbO发起攻击, 5_rtcsGEOYKz受到39点伤害

9_AQN4rqDpDP发起攻击, 7_Dai5Tf2nvv受到50点伤害

5_rtcsGEOYKz使用净化, 7_Dai5Tf2nvv受到35点伤害

3_uE8wZuV5Gv发起攻击, 9_AQN4rqDpDP回避了攻击

0_aQg8UHd6xh发起攻击, 7_Dai5Tf2nvv受到62点伤害

4_K1h4EmBKDU发起攻击, 9_AQN4rqDpDP受到72点伤害

9_AQN4rqDpDP发起攻击, 1_IqwwHJnNkK受到66点伤害

5_rtcsGEOYKz发起攻击, 4_K1h4EmBKDU受到77点伤害

2_PPuRzmGPjg发起攻击, 1_IqwwHJnNkK回避了攻击

6_4MCtUQ6DbO发起攻击, 0_aQg8UHd6xh受到40点伤害

1_IqwwHJnNkK潜行到6_4MCtUQ6DbO身后

0_aQg8UHd6xh发起攻击, 2_PPuRzmGPjg受到85点伤害

4_K1h4EmBKDU发起攻击, 3_uE8wZuV5Gv受到125点伤害

 3_uE8wZuV5Gv被击倒了

8_4hEQ58EiT7发起攻击, 9_AQN4rqDpDP回避了攻击

7_Dai5Tf2nvv发起攻击, 5_rtcsGEOYKz受到37点伤害

2_PPuRzmGPjg发起攻击, 5_rtcsGEOYKz受到37点伤害

 5_rtcsGEOYKz发起反击, 2_PPuRzmGPjg受到85点伤害

9_AQN4rqDpDP使用幻术, 召唤出幻影

5_rtcsGEOYKz使用净化, 幻影受到132点伤害

 幻影消失了

4_K1h4EmBKDU使用幻术, 召唤出幻影

6_4MCtUQ6DbO发起攻击, 8_4hEQ58EiT7受到37点伤害

0_aQg8UHd6xh发起攻击, 4_K1h4EmBKDU受到82点伤害

2_PPuRzmGPjg发起攻击, 6_4MCtUQ6DbO受到60点伤害

1_IqwwHJnNkK发动背刺, 6_4MCtUQ6DbO受到237点伤害

 6_4MCtUQ6DbO被击倒了

9_AQN4rqDpDP使用幻术, 召唤出幻影

5_rtcsGEOYKz发起攻击, 幻影受到86点伤害

4_K1h4EmBKDU使用狂暴术, 1_IqwwHJnNkK受到49点伤害, 1_IqwwHJnNkK进入狂暴状态

7_Dai5Tf2nvv发起攻击, 幻影受到41点伤害

2_PPuRzmGPjg发起攻击, 5_rtcsGEOYKz受到54点伤害

9_AQN4rqDpDP发起攻击, 7_Dai5Tf2nvv受到52点伤害

幻影发起攻击, 9_AQN4rqDpDP回避了攻击

0_aQg8UHd6xh发起攻击, 幻影受到113点伤害

 幻影消失了

8_4hEQ58EiT7发起攻击, 0_aQg8UHd6xh受到113点伤害

4_K1h4EmBKDU发起攻击, 9_AQN4rqDpDP受到104点伤害

1_IqwwHJnNkK发起狂暴攻击, 0_aQg8UHd6xh受到83点伤害

 0_aQg8UHd6xh被击倒了

 1_IqwwHJnNkK从狂暴中解除

2_PPuRzmGPjg发起攻击, 5_rtcsGEOYKz回避了攻击

5_rtcsGEOYKz发起攻击, 7_Dai5Tf2nvv受到33点伤害

9_AQN4rqDpDP发起攻击, 幻影受到70点伤害

7_Dai5Tf2nvv发起攻击, 8_4hEQ58EiT7受到112点伤害

4_K1h4EmBKDU发起攻击, 8_4hEQ58EiT7受到42点伤害

幻影使用附体, 9_AQN4rqDpDP回避了攻击

2_PPuRzmGPjg发起攻击, 4_K1h4EmBKDU受到103点伤害

 4_K1h4EmBKDU被击倒了

 幻影消失了

 2_PPuRzmGPjg吞噬了4_K1h4EmBKDU, 2_PPuRzmGPjg属性上升

8_4hEQ58EiT7发起攻击, 9_AQN4rqDpDP受到82点伤害

 9_AQN4rqDpDP被击倒了

5_rtcsGEOYKz使用净化, 8_4hEQ58EiT7受到42点伤害

2_PPuRzmGPjg发起攻击, 1_IqwwHJnNkK受到54点伤害

7_Dai5Tf2nvv发起攻击, 1_IqwwHJnNkK受到33点伤害

1_IqwwHJnNkK发起攻击, 5_rtcsGEOYKz受到46点伤害

 5_rtcsGEOYKz发起反击, 1_IqwwHJnNkK受到60点伤害

8_4hEQ58EiT7发起攻击, 7_Dai5Tf2nvv受到34点伤害

5_rtcsGEOYKz发起攻击, 2_PPuRzmGPjg受到55点伤害

 2_PPuRzmGPjg被击倒了

7_Dai5Tf2nvv发起攻击, 5_rtcsGEOYKz受到96点伤害

 5_rtcsGEOYKz被击倒了

1_IqwwHJnNkK使用净化, 7_Dai5Tf2nvv受到85点伤害

8_4hEQ58EiT7发起攻击, 7_Dai5Tf2nvv回避了攻击

1_IqwwHJnNkK发起攻击, 8_4hEQ58EiT7受到72点伤害

 8_4hEQ58EiT7被击倒了

7_Dai5Tf2nvv发起攻击, 1_IqwwHJnNkK受到53点伤害

 1_IqwwHJnNkK被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-36 must contain a blank separator between input and trace",
        "sampled case-36 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4790, "large_36 score mismatch");
    assert!(guard < 20_000, "sampled case-36 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-36", &actual_lines, &expected_lines);
}

#[test]
fn large_37() {
    const CASE: &str = r####"0_n0foAiLMcc
1_Bvpbn3b55R
2_pg6O17IbDF
3_zA5mtxMcPi
4_P0revuk8ms
5_nr70Kikkf8
6_LSUD2CsfGo
7_rRZN5tNMjb
8_7J291lHaC6
9_tZOkO2s8JU


0_n0foAiLMcc发起攻击, 1_Bvpbn3b55R受到76点伤害

3_zA5mtxMcPi投毒, 4_P0revuk8ms回避了攻击

5_nr70Kikkf8发起攻击, 8_7J291lHaC6受到88点伤害

8_7J291lHaC6发起攻击, 4_P0revuk8ms受到119点伤害

2_pg6O17IbDF发起攻击, 6_LSUD2CsfGo受到66点伤害

9_tZOkO2s8JU发起攻击, 5_nr70Kikkf8受到77点伤害

4_P0revuk8ms发起攻击, 3_zA5mtxMcPi受到90点伤害

7_rRZN5tNMjb发起攻击, 4_P0revuk8ms回避了攻击

6_LSUD2CsfGo发动铁壁, 6_LSUD2CsfGo防御力大幅上升

3_zA5mtxMcPi发起攻击, 8_7J291lHaC6受到36点伤害

5_nr70Kikkf8发起攻击, 0_n0foAiLMcc受到79点伤害

8_7J291lHaC6发起攻击, 6_LSUD2CsfGo受到1点伤害

1_Bvpbn3b55R发起攻击, 7_rRZN5tNMjb受到36点伤害

2_pg6O17IbDF发起攻击, 4_P0revuk8ms受到107点伤害

9_tZOkO2s8JU发起攻击, 6_LSUD2CsfGo防御, 6_LSUD2CsfGo受到0点伤害

4_P0revuk8ms使用生命之轮, 2_pg6O17IbDF的体力值与4_P0revuk8ms互换

7_rRZN5tNMjb发起攻击, 1_Bvpbn3b55R受到103点伤害

0_n0foAiLMcc发起攻击, 2_pg6O17IbDF受到46点伤害

6_LSUD2CsfGo发起攻击, 9_tZOkO2s8JU受到59点伤害

3_zA5mtxMcPi发起攻击, 4_P0revuk8ms受到67点伤害

5_nr70Kikkf8发起攻击, 4_P0revuk8ms回避了攻击

9_tZOkO2s8JU发起攻击, 4_P0revuk8ms受到53点伤害

7_rRZN5tNMjb投毒, 0_n0foAiLMcc回避了攻击

2_pg6O17IbDF发起攻击, 7_rRZN5tNMjb受到49点伤害

8_7J291lHaC6发起攻击, 4_P0revuk8ms受到38点伤害

 8_7J291lHaC6连击, 6_LSUD2CsfGo受到1点伤害

 8_7J291lHaC6连击, 6_LSUD2CsfGo受到1点伤害

1_Bvpbn3b55R使用雷击术

 0_n0foAiLMcc受到22点伤害

 0_n0foAiLMcc受到15点伤害

 0_n0foAiLMcc受到7点伤害

4_P0revuk8ms发起攻击, 6_LSUD2CsfGo受到1点伤害

6_LSUD2CsfGo开始聚气, 6_LSUD2CsfGo攻击力上升

 6_LSUD2CsfGo从铁壁中解除

9_tZOkO2s8JU使用魅惑, 5_nr70Kikkf8回避了攻击

0_n0foAiLMcc发起攻击, 8_7J291lHaC6回避了攻击

3_zA5mtxMcPi发动会心一击, 6_LSUD2CsfGo受到89点伤害

7_rRZN5tNMjb发起攻击, 5_nr70Kikkf8受到81点伤害

8_7J291lHaC6发起攻击, 9_tZOkO2s8JU受到42点伤害

6_LSUD2CsfGo发起攻击, 9_tZOkO2s8JU受到179点伤害

 9_tZOkO2s8JU被击倒了

4_P0revuk8ms发起攻击, 3_zA5mtxMcPi受到66点伤害

5_nr70Kikkf8发起攻击, 6_LSUD2CsfGo受到101点伤害

2_pg6O17IbDF发起攻击, 0_n0foAiLMcc受到106点伤害

0_n0foAiLMcc发起攻击, 4_P0revuk8ms受到72点伤害

3_zA5mtxMcPi发起攻击, 5_nr70Kikkf8受到66点伤害

1_Bvpbn3b55R发起攻击, 4_P0revuk8ms回避了攻击

2_pg6O17IbDF使用净化, 1_Bvpbn3b55R防御, 1_Bvpbn3b55R受到36点伤害

6_LSUD2CsfGo发起攻击, 7_rRZN5tNMjb受到84点伤害

7_rRZN5tNMjb发起攻击, 5_nr70Kikkf8受到109点伤害

 5_nr70Kikkf8被击倒了

8_7J291lHaC6发起攻击, 3_zA5mtxMcPi受到57点伤害

 3_zA5mtxMcPi被击倒了

 8_7J291lHaC6吞噬了3_zA5mtxMcPi, 8_7J291lHaC6属性上升

4_P0revuk8ms使用狂暴术, 6_LSUD2CsfGo受到73点伤害

 6_LSUD2CsfGo被击倒了

0_n0foAiLMcc发起攻击, 2_pg6O17IbDF受到22点伤害

8_7J291lHaC6发起攻击, 1_Bvpbn3b55R受到50点伤害

2_pg6O17IbDF发起攻击, 0_n0foAiLMcc受到62点伤害

 0_n0foAiLMcc被击倒了

 2_pg6O17IbDF召唤亡灵, 0_n0foAiLMcc变成了丧尸

7_rRZN5tNMjb发起攻击, 1_Bvpbn3b55R受到61点伤害

 1_Bvpbn3b55R被击倒了

8_7J291lHaC6发起攻击, 丧尸受到103点伤害

4_P0revuk8ms使用狂暴术, 7_rRZN5tNMjb受到119点伤害

 7_rRZN5tNMjb被击倒了

丧尸发起攻击, 4_P0revuk8ms受到43点伤害

8_7J291lHaC6发起攻击, 丧尸受到60点伤害

 丧尸消失了

4_P0revuk8ms发起攻击, 2_pg6O17IbDF受到69点伤害

 2_pg6O17IbDF被击倒了

8_7J291lHaC6使用净化, 4_P0revuk8ms受到53点伤害

 4_P0revuk8ms被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-37 must contain a blank separator between input and trace",
        "sampled case-37 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4371, "large_37 score mismatch");
    assert!(guard < 20_000, "sampled case-37 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-37", &actual_lines, &expected_lines);
}

#[test]
fn large_38() {
    const CASE: &str = r####"0_4RnhQrYzbq
1_MuW07xoMus
2_TLcfyIEn9Y
3_9mK8kLaw2V
4_FHUSuY76qq
5_BYAZ8S59GN
6_p7eVP0gAnh
7_qckCHq8x8z
8_Lr85xny6it
9_tfwtw3QPmO


1_MuW07xoMus发起攻击, 9_tfwtw3QPmO受到79点伤害

5_BYAZ8S59GN发起攻击, 4_FHUSuY76qq受到118点伤害

6_p7eVP0gAnh发起攻击, 7_qckCHq8x8z受到44点伤害

2_TLcfyIEn9Y使用加速术, 2_TLcfyIEn9Y进入疾走状态

0_4RnhQrYzbq使用火球术, 2_TLcfyIEn9Y受到129点伤害

7_qckCHq8x8z发起攻击, 0_4RnhQrYzbq受到137点伤害

8_Lr85xny6it发起攻击, 6_p7eVP0gAnh受到46点伤害

3_9mK8kLaw2V发起攻击, 6_p7eVP0gAnh受到133点伤害

1_MuW07xoMus发起攻击, 4_FHUSuY76qq受到78点伤害

2_TLcfyIEn9Y发起攻击, 0_4RnhQrYzbq受到75点伤害

9_tfwtw3QPmO发起攻击, 0_4RnhQrYzbq受到112点伤害

 0_4RnhQrYzbq被击倒了

4_FHUSuY76qq发起攻击, 3_9mK8kLaw2V受到40点伤害

7_qckCHq8x8z发起攻击, 1_MuW07xoMus受到54点伤害

8_Lr85xny6it发起攻击, 5_BYAZ8S59GN受到106点伤害

5_BYAZ8S59GN发起攻击, 4_FHUSuY76qq受到39点伤害

6_p7eVP0gAnh发起攻击, 9_tfwtw3QPmO受到41点伤害

2_TLcfyIEn9Y发起攻击, 3_9mK8kLaw2V受到72点伤害

 2_TLcfyIEn9Y从疾走中解除

3_9mK8kLaw2V发起攻击, 5_BYAZ8S59GN受到158点伤害

1_MuW07xoMus使用治愈魔法, 1_MuW07xoMus回复体力54点

7_qckCHq8x8z发起攻击, 4_FHUSuY76qq回避了攻击

8_Lr85xny6it发起攻击, 1_MuW07xoMus受到88点伤害

9_tfwtw3QPmO发起攻击, 8_Lr85xny6it受到45点伤害

6_p7eVP0gAnh发起攻击, 7_qckCHq8x8z受到74点伤害

5_BYAZ8S59GN发起攻击, 1_MuW07xoMus受到72点伤害

4_FHUSuY76qq发起攻击, 9_tfwtw3QPmO受到70点伤害

2_TLcfyIEn9Y发起攻击, 8_Lr85xny6it受到67点伤害

7_qckCHq8x8z发起攻击, 2_TLcfyIEn9Y回避了攻击

1_MuW07xoMus发起攻击, 3_9mK8kLaw2V受到74点伤害

3_9mK8kLaw2V发起攻击, 7_qckCHq8x8z受到57点伤害

4_FHUSuY76qq使用减速术, 7_qckCHq8x8z回避了攻击

8_Lr85xny6it发起攻击, 1_MuW07xoMus受到29点伤害

5_BYAZ8S59GN发起攻击, 7_qckCHq8x8z受到95点伤害

7_qckCHq8x8z使用净化, 9_tfwtw3QPmO受到34点伤害

6_p7eVP0gAnh发起攻击, 2_TLcfyIEn9Y受到59点伤害

4_FHUSuY76qq发起攻击, 1_MuW07xoMus受到95点伤害

9_tfwtw3QPmO发起攻击, 5_BYAZ8S59GN受到89点伤害

 5_BYAZ8S59GN被击倒了

1_MuW07xoMus发起攻击, 4_FHUSuY76qq受到57点伤害

2_TLcfyIEn9Y发起攻击, 6_p7eVP0gAnh受到152点伤害

 6_p7eVP0gAnh被击倒了

3_9mK8kLaw2V发起吸血攻击, 2_TLcfyIEn9Y受到46点伤害, 3_9mK8kLaw2V回复体力23点

7_qckCHq8x8z发起攻击, 8_Lr85xny6it受到58点伤害

8_Lr85xny6it发起攻击, 2_TLcfyIEn9Y受到41点伤害

 2_TLcfyIEn9Y被击倒了

4_FHUSuY76qq发起攻击, 3_9mK8kLaw2V受到53点伤害

1_MuW07xoMus发起攻击, 3_9mK8kLaw2V受到66点伤害

 3_9mK8kLaw2V被击倒了

9_tfwtw3QPmO发起攻击, 8_Lr85xny6it受到64点伤害

 8_Lr85xny6it被击倒了

1_MuW07xoMus发起攻击, 9_tfwtw3QPmO受到152点伤害

 9_tfwtw3QPmO被击倒了

7_qckCHq8x8z发起攻击, 4_FHUSuY76qq受到91点伤害

 4_FHUSuY76qq被击倒了

7_qckCHq8x8z发起攻击, 1_MuW07xoMus受到102点伤害

 1_MuW07xoMus被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-38 must contain a blank separator between input and trace",
        "sampled case-38 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 3961, "large_38 score mismatch");
    assert!(guard < 20_000, "sampled case-38 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-38", &actual_lines, &expected_lines);
}

#[test]
fn large_39() {
    const CASE: &str = r####"0_Q1ngYoVT97
1_TNFBreROl2
2_c0wnoTRonT
3_95ttrRGeVc
4_gR936oSZHP
5_IMzyISD9sZ
6_iCEWGiBagL
7_IxSaK45LKI
8_4mbv1tVcc2
9_jSXNz0OBTX


4_gR936oSZHP发起攻击, 3_95ttrRGeVc受到49点伤害

7_IxSaK45LKI发起攻击, 8_4mbv1tVcc2受到24点伤害

5_IMzyISD9sZ发起攻击, 7_IxSaK45LKI回避了攻击

9_jSXNz0OBTX发起攻击, 7_IxSaK45LKI受到127点伤害

6_iCEWGiBagL使用瘟疫, 1_TNFBreROl2体力减少45%

3_95ttrRGeVc发起攻击, 6_iCEWGiBagL受到45点伤害

0_Q1ngYoVT97发起攻击, 6_iCEWGiBagL受到43点伤害

8_4mbv1tVcc2发起攻击, 4_gR936oSZHP受到72点伤害

1_TNFBreROl2开始蓄力

2_c0wnoTRonT发起攻击, 8_4mbv1tVcc2受到90点伤害

7_IxSaK45LKI潜行到3_95ttrRGeVc身后

3_95ttrRGeVc发起攻击, 4_gR936oSZHP受到144点伤害

4_gR936oSZHP发起攻击, 0_Q1ngYoVT97受到66点伤害

5_IMzyISD9sZ使用幻术, 召唤出幻影

0_Q1ngYoVT97发起攻击, 3_95ttrRGeVc回避了攻击

9_jSXNz0OBTX发起攻击, 2_c0wnoTRonT受到60点伤害

6_iCEWGiBagL使用地裂术

 5_IMzyISD9sZ受到24点伤害

 9_jSXNz0OBTX受到14点伤害

 3_95ttrRGeVc受到30点伤害

 7_IxSaK45LKI受到33点伤害

 7_IxSaK45LKI的潜行被识破

 8_4mbv1tVcc2受到29点伤害

1_TNFBreROl2发起攻击, 8_4mbv1tVcc2受到191点伤害

 8_4mbv1tVcc2被击倒了

3_95ttrRGeVc发起攻击, 幻影受到35点伤害

7_IxSaK45LKI发起攻击, 5_IMzyISD9sZ受到23点伤害

4_gR936oSZHP使用治愈魔法, 4_gR936oSZHP回复体力103点

2_c0wnoTRonT发起攻击, 6_iCEWGiBagL受到52点伤害

0_Q1ngYoVT97发起攻击, 9_jSXNz0OBTX受到24点伤害

5_IMzyISD9sZ发起攻击, 9_jSXNz0OBTX回避了攻击

9_jSXNz0OBTX发起攻击, 4_gR936oSZHP受到141点伤害

1_TNFBreROl2发起攻击, 4_gR936oSZHP受到58点伤害

 4_gR936oSZHP被击倒了

6_iCEWGiBagL发起攻击, 5_IMzyISD9sZ受到76点伤害

3_95ttrRGeVc发起攻击, 9_jSXNz0OBTX受到66点伤害

0_Q1ngYoVT97发起攻击, 3_95ttrRGeVc受到103点伤害

2_c0wnoTRonT发起攻击, 9_jSXNz0OBTX受到70点伤害

5_IMzyISD9sZ发起攻击, 7_IxSaK45LKI受到75点伤害

9_jSXNz0OBTX发起攻击, 1_TNFBreROl2受到59点伤害

7_IxSaK45LKI潜行到0_Q1ngYoVT97身后

幻影发起攻击, 0_Q1ngYoVT97受到88点伤害

0_Q1ngYoVT97发起攻击, 2_c0wnoTRonT受到70点伤害

3_95ttrRGeVc发起攻击, 5_IMzyISD9sZ受到76点伤害

5_IMzyISD9sZ发起攻击, 9_jSXNz0OBTX回避了攻击

1_TNFBreROl2发起攻击, 5_IMzyISD9sZ受到21点伤害

6_iCEWGiBagL发起攻击, 2_c0wnoTRonT回避了攻击

7_IxSaK45LKI发动背刺, 0_Q1ngYoVT97受到361点伤害

 0_Q1ngYoVT97被击倒了

2_c0wnoTRonT发起攻击, 幻影受到38点伤害

幻影使用附体, 7_IxSaK45LKI进入狂暴状态

 幻影消失了

9_jSXNz0OBTX发起攻击, 3_95ttrRGeVc受到85点伤害

6_iCEWGiBagL使用减速术, 9_jSXNz0OBTX进入迟缓状态

3_95ttrRGeVc发起攻击, 9_jSXNz0OBTX受到33点伤害

5_IMzyISD9sZ发起攻击, 3_95ttrRGeVc受到56点伤害

 3_95ttrRGeVc被击倒了

1_TNFBreROl2发起攻击, 6_iCEWGiBagL受到88点伤害

5_IMzyISD9sZ发起吸血攻击, 2_c0wnoTRonT回避了攻击

2_c0wnoTRonT发起攻击, 9_jSXNz0OBTX受到87点伤害

6_iCEWGiBagL发起攻击, 1_TNFBreROl2受到44点伤害

7_IxSaK45LKI发起狂暴攻击, 6_iCEWGiBagL使用伤害反弹, 7_IxSaK45LKI受到37点伤害

1_TNFBreROl2发起攻击, 5_IMzyISD9sZ受到67点伤害

 5_IMzyISD9sZ被击倒了

7_IxSaK45LKI发起狂暴攻击, 7_IxSaK45LKI受到33点伤害

1_TNFBreROl2发起攻击, 9_jSXNz0OBTX受到120点伤害

 9_jSXNz0OBTX被击倒了

2_c0wnoTRonT发起攻击, 6_iCEWGiBagL受到61点伤害

 6_iCEWGiBagL被击倒了

2_c0wnoTRonT发起攻击, 7_IxSaK45LKI受到88点伤害

 7_IxSaK45LKI被击倒了

1_TNFBreROl2发起攻击, 2_c0wnoTRonT受到102点伤害

2_c0wnoTRonT发起攻击, 1_TNFBreROl2受到44点伤害

2_c0wnoTRonT发动铁壁, 2_c0wnoTRonT防御力大幅上升

1_TNFBreROl2发起攻击, 2_c0wnoTRonT回避了攻击

2_c0wnoTRonT使用火球术, 1_TNFBreROl2受到50点伤害

 1_TNFBreROl2被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-39 must contain a blank separator between input and trace",
        "sampled case-39 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4551, "large_39 score mismatch");
    assert!(guard < 20_000, "sampled case-39 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-39", &actual_lines, &expected_lines);
}

#[test]
fn large_40() {
    const CASE: &str = r####"0_N47q8QanNZ
1_kQLcV7rC4y
2_i20PdmDzEF
3_CVQot3gtn1
4_OoOMU4fqOA
5_EAFtoxdOB2
6_tRcahIayM4
7_T0zr6asNlZ
8_23KqaLUKKD
9_OjPjUlTNxb


2_i20PdmDzEF发起攻击, 5_EAFtoxdOB2受到86点伤害

4_OoOMU4fqOA发起攻击, 0_N47q8QanNZ受到78点伤害

8_23KqaLUKKD发起攻击, 3_CVQot3gtn1受到51点伤害

9_OjPjUlTNxb发起攻击, 7_T0zr6asNlZ回避了攻击

0_N47q8QanNZ发起攻击, 6_tRcahIayM4受到94点伤害

7_T0zr6asNlZ投毒, 2_i20PdmDzEF受到40点伤害, 2_i20PdmDzEF中毒

3_CVQot3gtn1使用净化, 1_kQLcV7rC4y受到68点伤害

5_EAFtoxdOB2发起攻击, 4_OoOMU4fqOA受到91点伤害

6_tRcahIayM4发起攻击, 1_kQLcV7rC4y受到27点伤害

 6_tRcahIayM4连击, 1_kQLcV7rC4y受到29点伤害

2_i20PdmDzEF发起攻击, 7_T0zr6asNlZ受到112点伤害

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到14点伤害

1_kQLcV7rC4y发起攻击, 8_23KqaLUKKD受到124点伤害

0_N47q8QanNZ发动铁壁, 0_N47q8QanNZ防御力大幅上升

7_T0zr6asNlZ发起攻击, 4_OoOMU4fqOA受到104点伤害

6_tRcahIayM4发起攻击, 3_CVQot3gtn1受到44点伤害

4_OoOMU4fqOA使用地裂术

 9_OjPjUlTNxb受到24点伤害

 3_CVQot3gtn1受到27点伤害

 1_kQLcV7rC4y受到26点伤害

 8_23KqaLUKKD受到40点伤害

8_23KqaLUKKD发起攻击, 4_OoOMU4fqOA受到82点伤害

 4_OoOMU4fqOA被击倒了

3_CVQot3gtn1使用净化, 7_T0zr6asNlZ受到45点伤害

1_kQLcV7rC4y发起攻击, 9_OjPjUlTNxb受到95点伤害

9_OjPjUlTNxb发起攻击, 5_EAFtoxdOB2受到39点伤害

2_i20PdmDzEF使用加速术, 2_i20PdmDzEF进入疾走状态

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到12点伤害

3_CVQot3gtn1使用净化, 7_T0zr6asNlZ受到86点伤害

8_23KqaLUKKD发起攻击, 5_EAFtoxdOB2回避了攻击

2_i20PdmDzEF发起攻击, 0_N47q8QanNZ受到1点伤害

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到10点伤害

0_N47q8QanNZ发起攻击, 2_i20PdmDzEF回避了攻击

7_T0zr6asNlZ发起攻击, 2_i20PdmDzEF受到18点伤害

2_i20PdmDzEF发起攻击, 0_N47q8QanNZ受到1点伤害

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到8点伤害

 2_i20PdmDzEF从中毒中解除

 2_i20PdmDzEF从疾走中解除

3_CVQot3gtn1使用雷击术

 0_N47q8QanNZ受到1点伤害

 0_N47q8QanNZ受到1点伤害

 0_N47q8QanNZ受到1点伤害

 0_N47q8QanNZ回避了攻击

5_EAFtoxdOB2发起攻击, 0_N47q8QanNZ受到1点伤害

6_tRcahIayM4发起攻击, 9_OjPjUlTNxb回避了攻击

8_23KqaLUKKD发起攻击, 3_CVQot3gtn1受到25点伤害

0_N47q8QanNZ发起攻击, 5_EAFtoxdOB2回避了攻击

 0_N47q8QanNZ从铁壁中解除

1_kQLcV7rC4y发起攻击, 6_tRcahIayM4受到92点伤害

2_i20PdmDzEF发动会心一击, 0_N47q8QanNZ受到84点伤害

7_T0zr6asNlZ发起攻击, 3_CVQot3gtn1受到63点伤害

9_OjPjUlTNxb发起攻击, 5_EAFtoxdOB2受到46点伤害

6_tRcahIayM4发起攻击, 3_CVQot3gtn1受到47点伤害

3_CVQot3gtn1发起攻击, 8_23KqaLUKKD受到68点伤害

 8_23KqaLUKKD被击倒了

5_EAFtoxdOB2发起攻击, 2_i20PdmDzEF受到85点伤害

1_kQLcV7rC4y投毒, 0_N47q8QanNZ受到36点伤害, 0_N47q8QanNZ中毒

9_OjPjUlTNxb发起攻击, 2_i20PdmDzEF回避了攻击

7_T0zr6asNlZ发起攻击, 3_CVQot3gtn1受到79点伤害

 3_CVQot3gtn1被击倒了

0_N47q8QanNZ使用火球术, 9_OjPjUlTNxb受到126点伤害

 0_N47q8QanNZ毒性发作, 0_N47q8QanNZ受到26点伤害

2_i20PdmDzEF发起攻击, 5_EAFtoxdOB2受到85点伤害

5_EAFtoxdOB2使用火球术, 6_tRcahIayM4受到96点伤害

6_tRcahIayM4发起攻击, 0_N47q8QanNZ受到44点伤害

1_kQLcV7rC4y发起攻击, 2_i20PdmDzEF受到47点伤害

7_T0zr6asNlZ发起攻击, 1_kQLcV7rC4y受到48点伤害

9_OjPjUlTNxb使用幻术, 召唤出幻影

6_tRcahIayM4发起攻击, 7_T0zr6asNlZ受到110点伤害

 7_T0zr6asNlZ被击倒了

0_N47q8QanNZ发起攻击, 9_OjPjUlTNxb受到116点伤害

 9_OjPjUlTNxb被击倒了

 幻影消失了

 0_N47q8QanNZ毒性发作, 0_N47q8QanNZ受到22点伤害

 0_N47q8QanNZ被击倒了

2_i20PdmDzEF发起攻击, 1_kQLcV7rC4y回避了攻击

5_EAFtoxdOB2发起攻击, 1_kQLcV7rC4y受到40点伤害

1_kQLcV7rC4y发起攻击, 5_EAFtoxdOB2防御, 5_EAFtoxdOB2受到21点伤害

6_tRcahIayM4发起攻击, 2_i20PdmDzEF受到86点伤害

 2_i20PdmDzEF被击倒了

1_kQLcV7rC4y发起攻击, 6_tRcahIayM4受到128点伤害

 6_tRcahIayM4被击倒了

5_EAFtoxdOB2使用雷击术

 1_kQLcV7rC4y受到11点伤害

 1_kQLcV7rC4y受到29点伤害

 1_kQLcV7rC4y被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-40 must contain a blank separator between input and trace",
        "sampled case-40 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4189, "large_40 score mismatch");
    assert!(guard < 20_000, "sampled case-40 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-40", &actual_lines, &expected_lines);
}

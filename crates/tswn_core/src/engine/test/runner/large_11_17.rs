use super::*;

#[test]
fn large_11() {
    const CASE: &str = r####"abc
aaaa
adwada
asdds
fdgs
dfgwat
sdc


fdgs发起攻击, sdc受到37点伤害

adwada发起攻击, fdgs受到60点伤害

aaaa发起攻击, abc受到36点伤害

asdds使用地裂术

 dfgwat受到22点伤害

 abc受到35点伤害

 sdc受到14点伤害

 adwada受到18点伤害

 fdgs受到21点伤害

abc发起攻击, dfgwat回避了攻击

dfgwat发起攻击, aaaa受到77点伤害

fdgs发起攻击, dfgwat回避了攻击

sdc发起攻击, dfgwat受到46点伤害

adwada发起攻击, aaaa受到91点伤害

abc发起攻击, adwada受到92点伤害

aaaa发起攻击, dfgwat受到58点伤害

dfgwat发起攻击, asdds受到34点伤害

sdc发起攻击, fdgs使用伤害反弹, sdc受到18点伤害

asdds发起攻击, fdgs受到40点伤害

fdgs发起攻击, sdc受到80点伤害

adwada发起攻击, sdc受到64点伤害

abc发起攻击, dfgwat受到109点伤害

dfgwat发起攻击, abc受到69点伤害

asdds使用血祭, 召唤出使魔

aaaa使用瘟疫, abc体力减少51%

使魔使用火球术, fdgs受到84点伤害

sdc发起攻击, asdds受到92点伤害

dfgwat使用魅惑, asdds被魅惑了

abc发起攻击, adwada受到102点伤害

adwada发起攻击, aaaa使用伤害反弹, adwada受到19点伤害

asdds发起攻击, abc回避了攻击

 asdds从魅惑中解除

fdgs使用火球术, asdds受到70点伤害

使魔发起攻击, abc受到35点伤害

dfgwat发起攻击, aaaa受到69点伤害

abc发起攻击, dfgwat受到39点伤害

fdgs使用净化, abc受到48点伤害

 abc被击倒了

adwada使用幻术, 召唤出幻影

asdds发起攻击, dfgwat受到88点伤害

 dfgwat被击倒了

sdc发起攻击, 使魔受到70点伤害, asdds受到35点伤害

使魔发起攻击, aaaa受到75点伤害

 aaaa被击倒了

adwada发起攻击, 使魔受到62点伤害, asdds受到31点伤害

 asdds被击倒了

 使魔消失了

fdgs发起攻击, adwada受到81点伤害

 adwada被击倒了

 幻影消失了

sdc使用狂暴术, fdgs受到44点伤害, fdgs进入狂暴状态

sdc发起攻击, fdgs受到53点伤害

 fdgs被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-11 must contain a blank separator between input and trace",
        "sampled case-11 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 3021, "large_11 score mismatch");

    assert!(guard < 20_000, "sampled case-11 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        eprintln!("Mismatch found!");
        eprintln!("Actual lines ({}):", actual_lines.len());
        for (i, line) in actual_lines.iter().enumerate() {
            eprintln!("  {}: {}", i, line);
        }
        eprintln!("Expected lines ({}):", expected_lines.len());
        for (i, line) in expected_lines.iter().enumerate() {
            eprintln!("  {}: {}", i, line);
        }
    }
    assert_trace_with_context("sampled case-11", &actual_lines, &expected_lines);
}

#[test]
fn large_12() {
    const CASE: &str = r####"WmG4iW0iZI
L6x5GQXq47
PzFvkx7lP7
m6SPYplZoz
m8iy8R0bkF


L6x5GQXq47发起攻击, WmG4iW0iZI回避了攻击

WmG4iW0iZI发起攻击, PzFvkx7lP7受到81点伤害

PzFvkx7lP7发起攻击, L6x5GQXq47受到32点伤害

m6SPYplZoz发起攻击, L6x5GQXq47受到97点伤害

m8iy8R0bkF发起攻击, L6x5GQXq47受到104点伤害

L6x5GQXq47发起攻击, WmG4iW0iZI受到83点伤害

WmG4iW0iZI发起攻击, PzFvkx7lP7受到79点伤害

PzFvkx7lP7发起攻击, m8iy8R0bkF受到61点伤害

m6SPYplZoz发起攻击, L6x5GQXq47受到62点伤害

L6x5GQXq47发起攻击, m8iy8R0bkF受到53点伤害

WmG4iW0iZI投毒, L6x5GQXq47受到42点伤害

 L6x5GQXq47被击倒了

m8iy8R0bkF发起攻击, PzFvkx7lP7受到106点伤害

PzFvkx7lP7发起攻击, m6SPYplZoz受到33点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI受到28点伤害

m8iy8R0bkF发起攻击, m6SPYplZoz受到61点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI回避了攻击

WmG4iW0iZI发起攻击, PzFvkx7lP7受到44点伤害

 PzFvkx7lP7被击倒了

WmG4iW0iZI发起攻击, m8iy8R0bkF受到0点伤害

m8iy8R0bkF发起攻击, WmG4iW0iZI受到83点伤害

m6SPYplZoz发起攻击, m8iy8R0bkF受到131点伤害

m8iy8R0bkF发起攻击, m6SPYplZoz受到72点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI受到85点伤害

WmG4iW0iZI发起攻击, m8iy8R0bkF受到61点伤害

 m8iy8R0bkF被击倒了

m6SPYplZoz发起攻击, WmG4iW0iZI受到31点伤害

WmG4iW0iZI发起攻击, m6SPYplZoz回避了攻击

m6SPYplZoz发起攻击, WmG4iW0iZI受到29点伤害

 WmG4iW0iZI被击倒了, WmG4iW0iZI使用护身符抵挡了一次死亡, WmG4iW0iZI回复体力10点

WmG4iW0iZI发起攻击, m6SPYplZoz受到70点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI受到85点伤害

 WmG4iW0iZI被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-12 must contain a blank separator between input and trace",
        "sampled case-12 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 2014, "large_12 score mismatch");

    assert!(guard < 20_000, "sampled case-12 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-12", &actual_lines, &expected_lines);
}

#[test]
fn large_13() {
    const CASE: &str = r####"qmAhJQzAVj
VuRY86K5Fy
YbhecuG73P
bFtbzLCkX3
rc6tMFMk7z


qmAhJQzAVj发起攻击, bFtbzLCkX3受到101点伤害

bFtbzLCkX3发起攻击, VuRY86K5Fy受到82点伤害

rc6tMFMk7z发起攻击, qmAhJQzAVj受到52点伤害

VuRY86K5Fy发起攻击, qmAhJQzAVj受到46点伤害

qmAhJQzAVj发起攻击, rc6tMFMk7z受到75点伤害

bFtbzLCkX3发起攻击, YbhecuG73P受到70点伤害

YbhecuG73P发起攻击, VuRY86K5Fy防御, VuRY86K5Fy受到13点伤害

VuRY86K5Fy发起攻击, bFtbzLCkX3受到120点伤害

rc6tMFMk7z发起攻击, VuRY86K5Fy受到65点伤害

YbhecuG73P使用加速术, YbhecuG73P进入疾走状态

bFtbzLCkX3发起攻击, rc6tMFMk7z受到38点伤害

YbhecuG73P发起攻击, qmAhJQzAVj受到52点伤害

qmAhJQzAVj发起攻击, VuRY86K5Fy受到31点伤害

VuRY86K5Fy发起攻击, rc6tMFMk7z受到101点伤害

YbhecuG73P发起攻击, rc6tMFMk7z受到103点伤害

 rc6tMFMk7z被击倒了

 YbhecuG73P从疾走中解除

bFtbzLCkX3使用净化, YbhecuG73P受到121点伤害

VuRY86K5Fy发起攻击, qmAhJQzAVj受到102点伤害

YbhecuG73P发起攻击, qmAhJQzAVj回避了攻击

qmAhJQzAVj发起攻击, VuRY86K5Fy受到61点伤害

bFtbzLCkX3发起攻击, qmAhJQzAVj受到66点伤害

 qmAhJQzAVj被击倒了

VuRY86K5Fy发起攻击, YbhecuG73P受到60点伤害

 YbhecuG73P被击倒了

bFtbzLCkX3发起攻击, VuRY86K5Fy受到21点伤害

VuRY86K5Fy发起攻击, bFtbzLCkX3受到113点伤害

 bFtbzLCkX3被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-13 must contain a blank separator between input and trace",
        "sampled case-13 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 1833, "large_13 score mismatch");

    assert!(guard < 20_000, "sampled case-13 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-13", &actual_lines, &expected_lines);
}

#[test]
fn large_14() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 2490, "large_14 score mismatch");

    assert!(guard < 20_000, "sampled case-14 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-14", &actual_lines, &expected_lines);
}

#[test]
fn large_15() {
    const CASE: &str = r####"QnljmCHowQ
IdUM9kx9c2
vhxSYeEzvf
qPCGw3EB8M
qze3UVC1DD


qPCGw3EB8M发起攻击, QnljmCHowQ受到126点伤害

qze3UVC1DD发起攻击, QnljmCHowQ受到63点伤害

 qze3UVC1DD连击, QnljmCHowQ受到54点伤害

IdUM9kx9c2发起攻击, QnljmCHowQ受到44点伤害

QnljmCHowQ发起攻击, IdUM9kx9c2受到53点伤害

qPCGw3EB8M发起攻击, qze3UVC1DD受到102点伤害

vhxSYeEzvf发起攻击, qPCGw3EB8M受到57点伤害

qze3UVC1DD发起攻击, qPCGw3EB8M受到63点伤害

qPCGw3EB8M使用魅惑, vhxSYeEzvf被魅惑了

IdUM9kx9c2发起攻击, vhxSYeEzvf受到93点伤害

vhxSYeEzvf发起攻击, QnljmCHowQ受到55点伤害

 vhxSYeEzvf从魅惑中解除

vhxSYeEzvf发起攻击, QnljmCHowQ受到129点伤害

 QnljmCHowQ被击倒了

 vhxSYeEzvf召唤亡灵, QnljmCHowQ变成了丧尸

qPCGw3EB8M发起攻击, vhxSYeEzvf受到50点伤害

qze3UVC1DD使用减速术, 丧尸进入迟缓状态

IdUM9kx9c2发起攻击, qze3UVC1DD受到41点伤害

qPCGw3EB8M发起攻击, vhxSYeEzvf受到103点伤害

vhxSYeEzvf潜行到qPCGw3EB8M身后

丧尸发起攻击, qze3UVC1DD受到41点伤害

vhxSYeEzvf发动背刺, qPCGw3EB8M受到324点伤害

 qPCGw3EB8M被击倒了

qze3UVC1DD发起攻击, 丧尸受到36点伤害

 qze3UVC1DD连击, vhxSYeEzvf受到76点伤害

 vhxSYeEzvf被击倒了

 丧尸消失了

IdUM9kx9c2发起攻击, qze3UVC1DD受到101点伤害

 qze3UVC1DD被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-15 must contain a blank separator between input and trace",
        "sampled case-15 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 2107, "large_15 score mismatch");

    assert!(guard < 20_000, "sampled case-15 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-15", &actual_lines, &expected_lines);
}

#[test]
fn large_16() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 1984, "large_16 score mismatch");

    assert!(guard < 20_000, "sampled case-16 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-16", &actual_lines, &expected_lines);
}

#[test]
fn case_17() {
    let expected: Vec<String> = vec![
        "aaaaa发起攻击",
        "help受到77点伤害",
        "aaaaa发起攻击",
        "help受到80点伤害",
        "help发起攻击",
        "aaaaa受到87点伤害",
        "help发起攻击",
        "aaaaa受到87点伤害",
        "aaaaa发起攻击",
        "help受到32点伤害",
        "help使用[雷击术]",
        "aaaaa受到26点伤害",
        "aaaaa受到25点伤害",
        "aaaaa受到10点伤害",
        "aaaaa受到9点伤害",
        "aaaaa受到10点伤害",
        "aaaaa受到14点伤害",
        "aaaaa发起攻击",
        "help受到43点伤害",
        "help发起攻击",
        "aaaaa受到94点伤害",
        "aaaaa被击倒了",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let mut runner = runners::Runner::new_from_namerena_raw("help\naaaaa".to_string()).unwrap();
    let (actual, guard, total_score) = collect_replay_events(&mut runner, 256, false);
    assert_eq!(total_score, 645, "case_17 score mismatch");

    assert!(guard < 256, "combat did not finish in expected rounds");
    assert_trace_with_context("case_17", &actual, &expected);

    let winner = winner_names(&runner);
    assert_eq!(winner, vec!["help".to_string()]);
}

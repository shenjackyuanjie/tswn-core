use super::*;

#[test]
fn large_18() {
    const CASE: &str = r####"xwjcqObl2L
OD2wlIdqr1
xbws21Im4P
4iJ53RqFn8
Omj8qVJppc


xwjcqObl2L使用魅惑, xbws21Im4P被魅惑了

Omj8qVJppc发起攻击, xwjcqObl2L回避了攻击

xbws21Im4P发起攻击, xbws21Im4P受到62点伤害

 xbws21Im4P从魅惑中解除

OD2wlIdqr1发起攻击, xwjcqObl2L受到70点伤害

4iJ53RqFn8使用诅咒, xbws21Im4P受到61点伤害, xbws21Im4P被诅咒了

xwjcqObl2L发起攻击, Omj8qVJppc受到124点伤害

OD2wlIdqr1发起攻击, 4iJ53RqFn8受到31点伤害

4iJ53RqFn8发起攻击, 诅咒使伤害加倍, xbws21Im4P受到120点伤害

xbws21Im4P发起攻击, 4iJ53RqFn8受到59点伤害

Omj8qVJppc发起攻击, OD2wlIdqr1受到17点伤害

 Omj8qVJppc连击, 4iJ53RqFn8回避了攻击

xwjcqObl2L发起攻击, OD2wlIdqr1受到95点伤害

OD2wlIdqr1发起攻击, xwjcqObl2L受到14点伤害

4iJ53RqFn8发动铁壁, 4iJ53RqFn8防御力大幅上升

xbws21Im4P发起攻击, xwjcqObl2L受到12点伤害

Omj8qVJppc使用冰冻术, OD2wlIdqr1受到65点伤害, OD2wlIdqr1被冰冻了

4iJ53RqFn8发起攻击, Omj8qVJppc受到30点伤害

Omj8qVJppc发起攻击, xwjcqObl2L回避了攻击

xwjcqObl2L发起攻击, 4iJ53RqFn8受到1点伤害

xbws21Im4P发起攻击, 4iJ53RqFn8受到1点伤害

4iJ53RqFn8使用分身, 出现一个新的4iJ53RqFn8

 4iJ53RqFn8从铁壁中解除

OD2wlIdqr1从冰冻中解除

Omj8qVJppc使用净化, xwjcqObl2L受到26点伤害

OD2wlIdqr1发起攻击, xwjcqObl2L回避了攻击

xwjcqObl2L发起攻击, OD2wlIdqr1受到55点伤害

OD2wlIdqr1发起攻击, Omj8qVJppc受到77点伤害

Omj8qVJppc发起攻击, xwjcqObl2L受到44点伤害

xbws21Im4P发起攻击, OD2wlIdqr1受到19点伤害

4iJ53RqFn8发动铁壁, 4iJ53RqFn8防御力大幅上升

4iJ53RqFn8发动铁壁, 4iJ53RqFn8防御力大幅上升

Omj8qVJppc发起攻击, 4iJ53RqFn8回避了攻击

xwjcqObl2L发起攻击, 4iJ53RqFn8受到1点伤害

xbws21Im4P发起攻击, 4iJ53RqFn8受到1点伤害

4iJ53RqFn8发起攻击, OD2wlIdqr1受到64点伤害

 OD2wlIdqr1被击倒了

4iJ53RqFn8使用减速术, xwjcqObl2L进入迟缓状态

Omj8qVJppc发起攻击, 4iJ53RqFn8受到1点伤害

4iJ53RqFn8发起攻击, xwjcqObl2L受到19点伤害

 4iJ53RqFn8从铁壁中解除

xbws21Im4P投毒, 4iJ53RqFn8受到24点伤害, 4iJ53RqFn8中毒

4iJ53RqFn8发起攻击, xwjcqObl2L受到38点伤害

 4iJ53RqFn8从铁壁中解除

4iJ53RqFn8发起攻击, Omj8qVJppc受到26点伤害

 4iJ53RqFn8毒性发作, 4iJ53RqFn8受到22点伤害

4iJ53RqFn8发起攻击, xwjcqObl2L受到42点伤害

Omj8qVJppc发起攻击, 4iJ53RqFn8受到89点伤害

 4iJ53RqFn8被击倒了

xwjcqObl2L发起攻击, 诅咒使伤害加倍, xbws21Im4P受到152点伤害

 xbws21Im4P被击倒了

4iJ53RqFn8发起攻击, xwjcqObl2L受到28点伤害

 4iJ53RqFn8毒性发作, 4iJ53RqFn8受到18点伤害

Omj8qVJppc使用净化, xwjcqObl2L受到42点伤害

 xwjcqObl2L被击倒了

Omj8qVJppc发起攻击, 4iJ53RqFn8受到72点伤害

 4iJ53RqFn8被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-18 must contain a blank separator between input and trace",
        "sampled case-18 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 2598, "large_18 score mismatch");

    assert!(guard < 20_000, "sampled case-18 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-18", &actual_lines, &expected_lines);
}

#[test]
fn large_19() {
    const CASE: &str = r####"l4nehpuWwK
GVZ36Iw0Q2
E42WIlaV65
2J92AWAksp
9cfhXyyfzH


2J92AWAksp发起攻击, E42WIlaV65受到93点伤害

l4nehpuWwK发起攻击, GVZ36Iw0Q2回避了攻击

GVZ36Iw0Q2发起攻击, l4nehpuWwK受到52点伤害

9cfhXyyfzH发起攻击, E42WIlaV65回避了攻击

E42WIlaV65发起攻击, GVZ36Iw0Q2受到47点伤害

GVZ36Iw0Q2发起攻击, 2J92AWAksp受到59点伤害

2J92AWAksp发起攻击, 9cfhXyyfzH受到47点伤害

9cfhXyyfzH发起攻击, E42WIlaV65受到94点伤害

l4nehpuWwK发起攻击, GVZ36Iw0Q2受到59点伤害

 GVZ36Iw0Q2发起反击, l4nehpuWwK受到47点伤害

l4nehpuWwK发起攻击, GVZ36Iw0Q2受到40点伤害

 GVZ36Iw0Q2发起反击, l4nehpuWwK受到16点伤害

E42WIlaV65发起攻击, l4nehpuWwK受到48点伤害

9cfhXyyfzH使用减速术, 2J92AWAksp进入迟缓状态

GVZ36Iw0Q2发起攻击, 2J92AWAksp受到88点伤害

2J92AWAksp发起攻击, 9cfhXyyfzH受到25点伤害

E42WIlaV65发起攻击, l4nehpuWwK受到41点伤害

l4nehpuWwK发起攻击, 9cfhXyyfzH受到25点伤害

GVZ36Iw0Q2发起攻击, 9cfhXyyfzH受到68点伤害

E42WIlaV65发起攻击, 9cfhXyyfzH受到43点伤害

9cfhXyyfzH发起攻击, E42WIlaV65受到38点伤害

2J92AWAksp使用狂暴术, E42WIlaV65受到50点伤害, E42WIlaV65进入狂暴状态

 2J92AWAksp从迟缓中解除

l4nehpuWwK发起攻击, 2J92AWAksp受到51点伤害

GVZ36Iw0Q2发起攻击, 2J92AWAksp受到29点伤害

E42WIlaV65发起狂暴攻击, l4nehpuWwK受到62点伤害

 l4nehpuWwK被击倒了

 E42WIlaV65从狂暴中解除

9cfhXyyfzH发起攻击, GVZ36Iw0Q2受到47点伤害

2J92AWAksp发起攻击, 9cfhXyyfzH受到19点伤害

GVZ36Iw0Q2使用雷击术

 9cfhXyyfzH受到23点伤害

 9cfhXyyfzH受到21点伤害

 9cfhXyyfzH被击倒了

E42WIlaV65开始蓄力

2J92AWAksp发起攻击, GVZ36Iw0Q2受到107点伤害

 GVZ36Iw0Q2被击倒了

2J92AWAksp使用地裂术

 E42WIlaV65受到76点伤害

 E42WIlaV65被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-19 must contain a blank separator between input and trace",
        "sampled case-19 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 1782, "large_19 score mismatch");

    assert!(guard < 20_000, "sampled case-19 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-19", &actual_lines, &expected_lines);
}

#[test]
fn large_20() {
    const CASE: &str = r####"k9brYO9ljp
5fh9ir4AaE
905nLuadjH
u05tVLWa49
kil5DzKTCb


k9brYO9ljp发起攻击, 905nLuadjH受到24点伤害

kil5DzKTCb发起攻击, k9brYO9ljp受到55点伤害

u05tVLWa49投毒, 905nLuadjH受到71点伤害, 905nLuadjH中毒

k9brYO9ljp发起攻击, 5fh9ir4AaE回避了攻击

905nLuadjH发起攻击, 5fh9ir4AaE受到43点伤害

 905nLuadjH毒性发作, 905nLuadjH受到26点伤害

5fh9ir4AaE发起攻击, kil5DzKTCb回避了攻击

5fh9ir4AaE发起攻击, k9brYO9ljp受到27点伤害

u05tVLWa49发起攻击, 5fh9ir4AaE受到71点伤害

kil5DzKTCb发起攻击, u05tVLWa49受到80点伤害

905nLuadjH发起攻击, kil5DzKTCb受到56点伤害

 905nLuadjH毒性发作, 905nLuadjH受到21点伤害

k9brYO9ljp发起攻击, u05tVLWa49受到33点伤害

kil5DzKTCb发起攻击, k9brYO9ljp受到95点伤害

5fh9ir4AaE发起攻击, k9brYO9ljp受到59点伤害

u05tVLWa49发起攻击, kil5DzKTCb受到80点伤害

k9brYO9ljp发起攻击, 5fh9ir4AaE受到16点伤害

5fh9ir4AaE发起攻击, 905nLuadjH受到62点伤害

kil5DzKTCb发起攻击, u05tVLWa49受到134点伤害

905nLuadjH发起攻击, 5fh9ir4AaE受到56点伤害

 905nLuadjH毒性发作, 905nLuadjH受到18点伤害

u05tVLWa49发起攻击, 5fh9ir4AaE受到28点伤害

905nLuadjH发起攻击, 5fh9ir4AaE受到47点伤害

 905nLuadjH毒性发作, 905nLuadjH受到15点伤害

 905nLuadjH从中毒中解除

k9brYO9ljp发起攻击, u05tVLWa49防御, u05tVLWa49受到23点伤害

 u05tVLWa49被击倒了

5fh9ir4AaE使用地裂术

 kil5DzKTCb受到43点伤害

 905nLuadjH受到78点伤害

 905nLuadjH被击倒了

 k9brYO9ljp受到41点伤害

 k9brYO9ljp被击倒了

kil5DzKTCb发起攻击, 5fh9ir4AaE受到42点伤害

5fh9ir4AaE发起攻击, kil5DzKTCb回避了攻击

kil5DzKTCb发起攻击, 5fh9ir4AaE受到90点伤害

 5fh9ir4AaE被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-20 must contain a blank separator between input and trace",
        "sampled case-20 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 1796, "large_20 score mismatch");

    assert!(guard < 20_000, "sampled case-20 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-20", &actual_lines, &expected_lines);
}

#[test]
fn large_21() {
    const CASE: &str = r####"YFvkJHzIuC
UeqihUcRJb
0pmsnYnvfO
RjRErZGcTZ
ur0NukExgM


ur0NukExgM发起攻击, UeqihUcRJb受到65点伤害

0pmsnYnvfO潜行到YFvkJHzIuC身后

UeqihUcRJb发起攻击, RjRErZGcTZ受到61点伤害

YFvkJHzIuC使用雷击术

 0pmsnYnvfO受到30点伤害

 0pmsnYnvfO的潜行被识破

 0pmsnYnvfO受到43点伤害

 0pmsnYnvfO受到58点伤害

 0pmsnYnvfO受到33点伤害

RjRErZGcTZ发起攻击, UeqihUcRJb回避了攻击

ur0NukExgM潜行到UeqihUcRJb身后

0pmsnYnvfO发起攻击, ur0NukExgM受到72点伤害

 ur0NukExgM的潜行被识破

RjRErZGcTZ使用狂暴术, ur0NukExgM受到63点伤害, ur0NukExgM进入狂暴状态

ur0NukExgM发起狂暴攻击, 0pmsnYnvfO受到55点伤害

 ur0NukExgM从狂暴中解除

YFvkJHzIuC发起攻击, RjRErZGcTZ受到48点伤害

UeqihUcRJb发起攻击, RjRErZGcTZ受到41点伤害

0pmsnYnvfO发起攻击, YFvkJHzIuC受到123点伤害

YFvkJHzIuC使用火球术, UeqihUcRJb受到75点伤害

RjRErZGcTZ发起攻击, ur0NukExgM受到84点伤害

ur0NukExgM发起攻击, UeqihUcRJb回避了攻击

0pmsnYnvfO发起攻击, YFvkJHzIuC受到63点伤害

UeqihUcRJb发起攻击, YFvkJHzIuC受到80点伤害

RjRErZGcTZ使用火球术, 0pmsnYnvfO受到137点伤害

 0pmsnYnvfO被击倒了

ur0NukExgM发起攻击, RjRErZGcTZ受到71点伤害

YFvkJHzIuC使用雷击术

 UeqihUcRJb受到15点伤害

 UeqihUcRJb受到11点伤害

 UeqihUcRJb受到16点伤害

RjRErZGcTZ发起攻击, UeqihUcRJb受到42点伤害

UeqihUcRJb发起攻击, RjRErZGcTZ受到56点伤害

 RjRErZGcTZ被击倒了

ur0NukExgM发起攻击, UeqihUcRJb受到43点伤害

YFvkJHzIuC发起攻击, ur0NukExgM受到72点伤害

UeqihUcRJb发起攻击, ur0NukExgM受到64点伤害

 ur0NukExgM被击倒了

YFvkJHzIuC发起攻击, UeqihUcRJb受到60点伤害

 UeqihUcRJb被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-21 must contain a blank separator between input and trace",
        "sampled case-21 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 1888, "large_21 score mismatch");

    assert!(guard < 20_000, "sampled case-21 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-21", &actual_lines, &expected_lines);
}

#[test]
fn large_22() {
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

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 2055, "large_22 score mismatch");

    assert!(guard < 20_000, "sampled case-22 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-22", &actual_lines, &expected_lines);
}

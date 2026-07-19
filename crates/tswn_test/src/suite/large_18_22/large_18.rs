use super::*;

pub fn large_18<E: crate::EngineAdapter>() {
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

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 2598, "large_18 score mismatch");

    assert!(guard < 20_000, "sampled case-18 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-18", &actual_lines, &expected_lines);
}

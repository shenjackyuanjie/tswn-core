use super::*;

pub fn large_21<E: crate::EngineAdapter>() {
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

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1888, "large_21 score mismatch");

    assert!(guard < 20_000, "sampled case-21 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-21", &actual_lines, &expected_lines);
}

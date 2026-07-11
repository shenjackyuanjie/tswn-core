use super::*;

pub fn large_19<E: crate::EngineAdapter>() {
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

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1782, "large_19 score mismatch");

    assert!(guard < 20_000, "sampled case-19 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-19", &actual_lines, &expected_lines);
}

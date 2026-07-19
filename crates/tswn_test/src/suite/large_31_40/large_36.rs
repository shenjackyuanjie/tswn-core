use super::*;

pub fn large_36<E: crate::EngineAdapter>() {
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
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 4790, "large_36 score mismatch");
    assert!(guard < 20_000, "sampled case-36 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-36", &actual_lines, &expected_lines);
}

use super::*;

/// 护盾+蓄力特性
pub fn large_43<E: crate::EngineAdapter>() {
    const CASE: &str = r####"豹山惟助 PFOQXFYL@TigerStar

泠珞 itVMnXnsL@807139


豹山惟助开始蓄力

泠珞使用净化, 豹山惟助受到65点伤害

 豹山惟助的蓄力被中止了

豹山惟助发起攻击, 泠珞回避了攻击

泠珞发起攻击, 豹山惟助受到0点伤害

豹山惟助发起攻击, 泠珞受到42点伤害

泠珞使用净化, 豹山惟助受到0点伤害

豹山惟助发起攻击, 泠珞回避了攻击

泠珞使用魅惑, 豹山惟助回避了攻击

豹山惟助开始蓄力

豹山惟助发起攻击, 泠珞受到164点伤害

泠珞使用净化, 豹山惟助受到0点伤害

泠珞使用净化, 豹山惟助受到83点伤害

 豹山惟助的蓄力被中止了

豹山惟助发起攻击, 泠珞受到66点伤害

豹山惟助发起攻击, 泠珞受到53点伤害

泠珞投毒, 豹山惟助受到66点伤害, 豹山惟助中毒

豹山惟助使用魅惑, 泠珞被魅惑了

 豹山惟助毒性发作, 豹山惟助受到21点伤害

 豹山惟助做出垂死抗争, 豹山惟助所有属性上升

泠珞发起攻击, 泠珞受到53点伤害

 泠珞被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-43 must contain a blank separator between input and trace",
        "sampled case-43 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1078, "large_43 score mismatch");
    assert!(guard < 20_000, "sampled case-43 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-43", &actual_lines, &expected_lines);
}

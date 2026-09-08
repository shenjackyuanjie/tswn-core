use super::*;

pub fn large_42<E: crate::EngineAdapter>() {
    const CASE: &str = r#"锋利Ⅴ EGZPVQMY@TigerStar
雾山惟助 BAAOVADZ@TigerStar

冥河 WyO8MUZPPtKH@Afterglow
光 jKLA6V5mirfs@Afterglow
seed:S2-week2-477-加赛-3@!


锋利Ⅴ发起攻击, 光受到48点伤害

冥河使用魅惑, 锋利Ⅴ回避了攻击

锋利Ⅴ使用瘟疫, 冥河体力减少61%

光发起攻击, 锋利Ⅴ防御, 锋利Ⅴ受到30点伤害

雾山惟助使用地裂术

 冥河受到67点伤害

 光受到59点伤害

锋利Ⅴ使用瘟疫, 冥河体力减少61%

冥河使用分身, 出现一个新的冥河

雾山惟助发起攻击, 冥河受到38点伤害

 冥河被击倒了

冥河使用苏生术, 冥河复活了, 冥河回复体力150点

光发起攻击, 锋利Ⅴ回避了攻击

冥河使用魅惑, 锋利Ⅴ被魅惑了

雾山惟助使用地裂术

 冥河受到66点伤害

 冥河被击倒了

 光受到36点伤害

 冥河受到47点伤害

冥河使用分身, 出现一个新的冥河

锋利Ⅴ发起攻击, 锋利Ⅴ受到89点伤害, 锋利Ⅴ发动隐匿

 锋利Ⅴ从魅惑中解除

雾山惟助发起攻击, 冥河受到55点伤害

 冥河被击倒了

冥河发起攻击, 雾山惟助回避了攻击

光发起攻击, 雾山惟助受到43点伤害

锋利Ⅴ发起攻击, 冥河受到111点伤害

 冥河被击倒了

 锋利Ⅴ召唤亡灵, 冥河变成了丧尸

光使用苏生术, 冥河复活了, 冥河回复体力84点

锋利Ⅴ使用瘟疫, 光体力减少64%

雾山惟助发起攻击, 光受到68点伤害

 光被击倒了, 光使用护身符抵挡了一次死亡, 光回复体力1点

丧尸发起攻击, 冥河受到66点伤害

 冥河发起反击, 丧尸受到16点伤害

锋利Ⅴ发起攻击, 光受到77点伤害

 光被击倒了, 光使用护身符抵挡了一次死亡, 光回复体力12点

冥河使用魅惑, 丧尸被魅惑了

光使用分身, 出现一个新的光

雾山惟助发起攻击, 光受到85点伤害

 光被击倒了, 光使用护身符抵挡了一次死亡, 光回复体力6点

冥河发起攻击, 丧尸受到79点伤害

光发起攻击, 锋利Ⅴ防御, 锋利Ⅴ受到42点伤害

锋利Ⅴ发起攻击, 光受到61点伤害

 光被击倒了, 光使用护身符抵挡了一次死亡, 光回复体力6点

光使用生命之轮, 雾山惟助的体力值与光互换

雾山惟助使用地裂术

 光受到46点伤害

 光被击倒了, 光使用护身符抵挡了一次死亡, 光回复体力5点

 冥河受到64点伤害

 冥河被击倒了

 光受到40点伤害

丧尸发起攻击, 锋利Ⅴ受到38点伤害

 丧尸从魅惑中解除

雾山惟助发起攻击, 光受到63点伤害

 光被击倒了

锋利Ⅴ发起攻击, 光受到141点伤害

 光被击倒了, 光使用护身符抵挡了一次死亡, 光回复体力13点

光发起攻击, 雾山惟助受到100点伤害

 雾山惟助被击倒了

 光吞噬了雾山惟助, 光属性上升

光使用地裂术

 锋利Ⅴ受到45点伤害, 锋利Ⅴ发动隐匿

 丧尸受到61点伤害

 丧尸消失了

锋利Ⅴ发起攻击, 光受到72点伤害

 光被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-42 must contain a blank separator between input and trace",
        "sampled case-42 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 5091, "large_42 score mismatch");
    assert!(guard < 20_000, "sampled case-42 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-42", &actual_lines, &expected_lines);
}

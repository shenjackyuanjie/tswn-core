use super::*;

/// 蓄力潜行不应该被识破
pub fn large_60<E: crate::EngineAdapter>() {
    const CASE: &str = r#"Appassionata #PxCllhk@Unbound
AlterGate SDBEOVSGr@candle
lcNKsTae@coat
Abby EWZCTSEXNMHL@nan
随之任之 #iWZYBGuwxX@🥒

Light_Years_Away #XgTW5RYlF@Shabby_fish
坚持 E6b10FVHvKDO@Afterglow
回忆 ct8ZxdDJEtAOUIA@Mithril
永劫回归#118486227619108@TigerStar
看到这个号说明你要豹了ZLoYMiQp@柚子不是油渍
seed:1376-5-1@!


永劫回归#118486227619108潜行到AlterGate身后

AlterGate发起攻击, Light_Years_Away受到79点伤害

随之任之发起攻击, 坚持受到37点伤害

Appassionata使用幻术, 召唤出幻影

Light_Years_Away发起攻击, 幻影受到86点伤害

坚持发动铁壁, 坚持防御力大幅上升

永劫回归#118486227619108发动背刺, AlterGate受到388点伤害

 AlterGate被击倒了

 永劫回归#118486227619108吞噬了AlterGate, 永劫回归#118486227619108属性上升

回忆使用魅惑, 随之任之被魅惑了

看到这个号说明你要豹了ZLoYMiQp使用火球术, lcNKsTae回避了攻击

Abby潜行到坚持身后

lcNKsTae使用净化, 永劫回归#118486227619108受到34点伤害

永劫回归#118486227619108发起攻击, lcNKsTae受到47点伤害

坚持发起攻击, 幻影受到127点伤害

 幻影消失了

随之任之发起攻击, lcNKsTae受到97点伤害

 随之任之从魅惑中解除

Light_Years_Away使用地裂术

 lcNKsTae受到48点伤害

 随之任之受到38点伤害

 Abby使用伤害反弹, Light_Years_Away受到19点伤害

 Appassionata受到35点伤害

看到这个号说明你要豹了ZLoYMiQp开始聚气, 看到这个号说明你要豹了ZLoYMiQp攻击力上升

lcNKsTae使用净化, Light_Years_Away受到61点伤害

回忆使用分身, 出现一个新的回忆

Appassionata发起攻击, 坚持守护永劫回归#118486227619108, 坚持受到0点伤害

回忆使用魅惑, 随之任之回避了攻击

坚持发起攻击, lcNKsTae受到79点伤害

 坚持从铁壁中解除

随之任之发动会心一击, 永劫回归#118486227619108回避了攻击

永劫回归#118486227619108使用瘟疫, 随之任之体力减少55%, 随之任之发动隐匿

lcNKsTae发起攻击, Light_Years_Away受到51点伤害

看到这个号说明你要豹了ZLoYMiQp使用火球术, lcNKsTae受到168点伤害

 lcNKsTae被击倒了

Appassionata使用诅咒, Light_Years_Away受到51点伤害, Light_Years_Away被诅咒了

Abby发动背刺, 坚持受到445点伤害

 坚持被击倒了

回忆使用魅惑, Abby被魅惑了

Light_Years_Away发起攻击, Abby受到86点伤害

Abby开始蓄力

 Abby从魅惑中解除

永劫回归#118486227619108潜行到Appassionata身后

Light_Years_Away使用地裂术

 Abby受到39点伤害

 Appassionata受到37点伤害

 随之任之受到28点伤害

看到这个号说明你要豹了ZLoYMiQp发起攻击, Abby受到146点伤害

Appassionata发起攻击, 诅咒使伤害加倍, Light_Years_Away受到64点伤害

回忆发起攻击, Abby受到40点伤害

 Abby做出垂死抗争, Abby所有属性上升, Abby发动隐匿

随之任之使用治愈魔法, Abby回复体力99点

回忆使用分身, 出现一个新的回忆

Light_Years_Away使用地裂术

 Appassionata受到28点伤害

 Abby受到37点伤害

 随之任之受到59点伤害

永劫回归#118486227619108发动背刺, Appassionata受到332点伤害

 Appassionata被击倒了

 永劫回归#118486227619108吞噬了Appassionata, 永劫回归#118486227619108属性上升

Abby潜行到永劫回归#118486227619108身后

回忆使用地裂术

 随之任之受到57点伤害

 Abby受到34点伤害, Abby发动隐匿

Abby发动背刺, 永劫回归#118486227619108受到365点伤害

 永劫回归#118486227619108被击倒了

回忆发起攻击, 随之任之受到32点伤害

 随之任之被击倒了

 回忆召唤亡灵, 随之任之变成了丧尸

Abby潜行到看到这个号说明你要豹了ZLoYMiQp身后

看到这个号说明你要豹了ZLoYMiQp使用火球术, Abby受到90点伤害

 Abby的潜行被识破

 Abby被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-60 must contain a blank separator between input and trace",
        "sampled case-60 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 5043, "large_60 score mismatch");
    assert!(guard < 20_000, "sampled case-60 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-60", &actual_lines, &expected_lines);
}

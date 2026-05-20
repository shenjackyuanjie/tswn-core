use super::*;

/// 用来测试 clamp
#[test]
fn large_66() {
    const CASE: &str = r#"太阳帝国 #Sc2WCffta@Shabby_fish
机械革命 #nYHCsQ1ak@Shabby_fish


太阳帝国使用幻术, 召唤出幻影

机械革命潜行到幻影身后

太阳帝国发起攻击, 机械革命回避了攻击

机械革命发动背刺, 幻影受到350点伤害

 幻影消失了

太阳帝国使用分身, 出现一个新的太阳帝国

机械革命潜行到太阳帝国身后

太阳帝国开始蓄力

太阳帝国发起攻击, 机械革命受到35点伤害

 机械革命的潜行被识破

机械革命潜行到太阳帝国身后

太阳帝国使用分身, 出现一个新的太阳帝国

太阳帝国发起攻击, 机械革命受到52点伤害

 机械革命的潜行被识破

机械革命潜行到太阳帝国身后

太阳帝国发起攻击, 机械革命受到37点伤害

 机械革命的潜行被识破

太阳帝国使用分身, 出现一个新的太阳帝国

机械革命潜行到太阳帝国身后

太阳帝国开始蓄力

太阳帝国开始蓄力

太阳帝国发起攻击, 机械革命回避了攻击

太阳帝国发起攻击, 机械革命受到58点伤害

 机械革命的潜行被识破

太阳帝国发起攻击, 机械革命受到215点伤害

 机械革命被击倒了"#;

    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-66 must contain a blank separator between input and trace",
        "sampled case-66 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-66 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-66", &actual_lines, &expected_lines);
}

/// 这个好像是用来check ts的
#[test]
fn large_67() {
    const CASE: &str = r#"Stupefy #rkISERW8@Shabby_fish
日落·日出 #Pd3J7shds@Shabby_fish


Stupefy潜行到日落·日出身后

日落·日出使用血祭, 召唤出使魔

使魔发起攻击, Stupefy受到31点伤害

 Stupefy的潜行被识破

Stupefy潜行到日落·日出身后

日落·日出使用分身, 出现一个新的日落·日出

Stupefy发动背刺, 日落·日出受到440点伤害

 日落·日出被击倒了

 使魔消失了

 Stupefy吞噬了日落·日出, Stupefy属性上升

日落·日出发起攻击, Stupefy受到50点伤害

Stupefy发起攻击, 日落·日出受到22点伤害

日落·日出发起攻击, Stupefy受到37点伤害

Stupefy发起攻击, 日落·日出受到79点伤害

日落·日出潜行到Stupefy身后

Stupefy使用血祭, 召唤出使魔

使魔发起攻击, 日落·日出防御, 日落·日出受到11点伤害

 日落·日出的潜行被识破

日落·日出发起攻击, Stupefy防御, Stupefy受到24点伤害

Stupefy发起攻击, 日落·日出受到49点伤害

使魔发起攻击, 日落·日出受到63点伤害

 日落·日出被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-67 must contain a blank separator between input and trace",
        "sampled case-67 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-67 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-67", &actual_lines, &expected_lines);
}

#[test]
fn large_68() {
    const CASE: &str = r#"Queen_G #LGDj0Xr29@Shabby_fish
使腿勇腋二@Shabby_fish
Light_Years_Away #XgTW5RYlF@Shabby_fish

给我看点新花样 #mucbrjtJe@Shabby_fish
Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra@Shabby_fish
清楚明白 #ltbpRp9tG@Shabby_fish

Avada_Kedavra #wJzaN9in@Shabby_fish
前进观察团 #8uGHaeqOk@Shabby_fish
Boundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish


Avada_Kedavra使用魅惑, Light_Years_Away被魅惑了

使腿勇腋二使用幻术, 召唤出幻影

清楚明白使用分身, 出现一个新的清楚明白

Boundless_Ocean,Vast_Skies使用雷击术

 Queen_G回避了攻击

Light_Years_Away使用地裂术

 Light_Years_Away受到22点伤害

 Queen_G受到27点伤害

 给我看点新花样受到29点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到27点伤害

 Light_Years_Away从魅惑中解除

给我看点新花样发起攻击, Light_Years_Away受到53点伤害

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用减速术, Boundless_Ocean,Vast_Skies进入迟缓状态

Queen_G开始聚气, Queen_G攻击力上升

使腿勇腋二发起攻击, Boundless_Ocean,Vast_Skies受到32点伤害

清楚明白发起攻击, Queen_G回避了攻击

Avada_Kedavra使用分身, 出现一个新的Avada_Kedavra

前进观察团潜行到Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra身后

清楚明白发起攻击, 使腿勇腋二受到22点伤害

Queen_G潜行到前进观察团身后

给我看点新花样使用净化, Boundless_Ocean,Vast_Skies受到40点伤害

Avada_Kedavra发起攻击, 使腿勇腋二受到20点伤害

Light_Years_Away使用地裂术

 给我看点新花样受到29点伤害

 前进观察团受到38点伤害

 前进观察团的潜行被识破

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到26点伤害

 清楚明白受到64点伤害

清楚明白发起攻击, Queen_G回避了攻击

前进观察团潜行到Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra身后

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用分身, 出现一个新的Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra

使腿勇腋二发起攻击, 清楚明白受到63点伤害

Avada_Kedavra发起攻击, Queen_G受到32点伤害

 Queen_G的潜行被识破

清楚明白发起攻击, Light_Years_Away回避了攻击

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra发起攻击, Queen_G回避了攻击

Queen_G潜行到给我看点新花样身后

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用分身, 出现一个新的Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra

Light_Years_Away发起攻击, Boundless_Ocean,Vast_Skies受到48点伤害

给我看点新花样发起攻击, Queen_G受到100点伤害

 Queen_G的潜行被识破

幻影发起攻击, 给我看点新花样受到96点伤害

Queen_G潜行到前进观察团身后

Boundless_Ocean,Vast_Skies发起攻击, 使腿勇腋二受到56点伤害

使腿勇腋二使用幻术, 召唤出幻影

前进观察团发动背刺, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到299点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了

 前进观察团吞噬了Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra, 前进观察团属性上升

Avada_Kedavra发起攻击, 使腿勇腋二受到36点伤害

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra发起攻击, 前进观察团受到72点伤害

清楚明白发起攻击, Queen_G受到19点伤害

 Queen_G的潜行被识破

幻影发起攻击, 前进观察团受到60点伤害

Avada_Kedavra使用分身, 出现一个新的Avada_Kedavra

前进观察团潜行到清楚明白身后

给我看点新花样发起攻击, Light_Years_Away受到98点伤害

清楚明白发起攻击, Boundless_Ocean,Vast_Skies受到67点伤害

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra发起攻击, Boundless_Ocean,Vast_Skies受到58点伤害

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra发起攻击, Light_Years_Away回避了攻击

Avada_Kedavra发起攻击, Light_Years_Away受到57点伤害

Queen_G潜行到给我看点新花样身后

清楚明白发起攻击, 幻影受到53点伤害

Light_Years_Away发起攻击, Avada_Kedavra受到85点伤害

给我看点新花样使用净化, Avada_Kedavra受到75点伤害

 Avada_Kedavra做出垂死抗争, Avada_Kedavra所有属性上升

清楚明白发起攻击, 幻影受到35点伤害

Avada_Kedavra发起攻击, 使腿勇腋二受到62点伤害

Avada_Kedavra使用分身, 出现一个新的Avada_Kedavra

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用魅惑, Queen_G回避了攻击

Queen_G发动背刺, 给我看点新花样受到473点伤害

 给我看点新花样被击倒了, 给我看点新花样使用护身符抵挡了一次死亡, 给我看点新花样回复体力9点

使腿勇腋二使用魅惑, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被魅惑了

幻影发起攻击, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到76点伤害

Avada_Kedavra发起攻击, Queen_G受到57点伤害

清楚明白发起攻击, 前进观察团受到84点伤害

 前进观察团的潜行被识破

Boundless_Ocean,Vast_Skies发起攻击, 使腿勇腋二受到46点伤害

 Boundless_Ocean,Vast_Skies从迟缓中解除

前进观察团使用魅惑, Light_Years_Away被魅惑了

幻影发起攻击, 给我看点新花样受到58点伤害

 给我看点新花样被击倒了

Avada_Kedavra发起攻击, 清楚明白受到59点伤害

 清楚明白做出垂死抗争, 清楚明白所有属性上升

Light_Years_Away开始聚气, Light_Years_Away攻击力上升

 Light_Years_Away从魅惑中解除

清楚明白发起攻击, Light_Years_Away受到79点伤害

Avada_Kedavra发起攻击, 幻影回避了攻击

Avada_Kedavra发起攻击, Light_Years_Away回避了攻击

使腿勇腋二发起攻击, Avada_Kedavra受到34点伤害

 Avada_Kedavra做出垂死抗争, Avada_Kedavra所有属性上升

前进观察团发起攻击, Queen_G受到93点伤害

 Queen_G做出垂死抗争, Queen_G所有属性上升

幻影发起攻击, Avada_Kedavra受到62点伤害

 Avada_Kedavra被击倒了

Avada_Kedavra发起攻击, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到77点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用护身符抵挡了一次死亡, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra回复体力14点

Queen_G发起攻击, Avada_Kedavra受到116点伤害

 Avada_Kedavra被击倒了, Avada_Kedavra使用护身符抵挡了一次死亡, Avada_Kedavra回复体力13点

清楚明白使用分身, 出现一个新的清楚明白

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用冰冻术, Boundless_Ocean,Vast_Skies受到15点伤害, Boundless_Ocean,Vast_Skies被冰冻了

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra从魅惑中解除

幻影使用附体, Avada_Kedavra进入狂暴状态

 幻影消失了

Light_Years_Away使用地裂术

 清楚明白受到31点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到84点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用护身符抵挡了一次死亡, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra回复体力7点

 Avada_Kedavra受到66点伤害

 Avada_Kedavra被击倒了

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到31点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了

 清楚明白受到50点伤害

 清楚明白被击倒了

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用苏生术, 给我看点新花样复活了, 给我看点新花样回复体力70点

给我看点新花样发起攻击, 使腿勇腋二受到58点伤害

 使腿勇腋二做出垂死抗争, 使腿勇腋二所有属性上升

使腿勇腋二发起攻击, 给我看点新花样防御, 给我看点新花样受到22点伤害

前进观察团发起攻击, Light_Years_Away受到55点伤害, Light_Years_Away发动隐匿

清楚明白使用苏生术, 清楚明白复活了, 清楚明白回复体力78点

Avada_Kedavra发起狂暴攻击, 清楚明白受到86点伤害

 清楚明白被击倒了

Boundless_Ocean,Vast_Skies从冰冻中解除

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra发起攻击, 前进观察团受到78点伤害

 前进观察团被击倒了, 前进观察团使用护身符抵挡了一次死亡, 前进观察团回复体力7点

Light_Years_Away发起攻击, 清楚明白受到88点伤害

 清楚明白被击倒了, 清楚明白使用护身符抵挡了一次死亡, 清楚明白回复体力3点

Avada_Kedavra发起攻击, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到102点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用护身符抵挡了一次死亡, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra回复体力15点

清楚明白发起攻击, Boundless_Ocean,Vast_Skies受到44点伤害

Boundless_Ocean,Vast_Skies发起攻击, 清楚明白受到54点伤害

 清楚明白被击倒了

 Boundless_Ocean,Vast_Skies召唤亡灵, 清楚明白变成了丧尸

Queen_G发起攻击, Boundless_Ocean,Vast_Skies受到86点伤害

 Boundless_Ocean,Vast_Skies被击倒了

 丧尸消失了

使腿勇腋二发起攻击, 前进观察团受到58点伤害

 前进观察团被击倒了, 前进观察团使用护身符抵挡了一次死亡, 前进观察团回复体力3点

前进观察团发起攻击, 幻影回避了攻击

Avada_Kedavra使用苏生术, Avada_Kedavra复活了, Avada_Kedavra回复体力43点

使腿勇腋二发起攻击, 给我看点新花样受到88点伤害

 给我看点新花样被击倒了

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra发起攻击, Avada_Kedavra受到79点伤害

 Avada_Kedavra被击倒了

清楚明白使用分身, 出现一个新的清楚明白

幻影使用附体, 前进观察团进入狂暴状态

 幻影消失了

Avada_Kedavra发起狂暴攻击, 清楚明白受到176点伤害

 清楚明白被击倒了, 清楚明白使用护身符抵挡了一次死亡, 清楚明白回复体力15点

Light_Years_Away发起攻击, 清楚明白受到128点伤害

 清楚明白被击倒了

Queen_G发起攻击, Avada_Kedavra受到105点伤害

 Avada_Kedavra被击倒了, Avada_Kedavra使用护身符抵挡了一次死亡, Avada_Kedavra回复体力9点

使腿勇腋二发起吸血攻击, Avada_Kedavra受到84点伤害, 使腿勇腋二回复体力42点

 Avada_Kedavra被击倒了

Avada_Kedavra使用苏生术, Avada_Kedavra复活了, Avada_Kedavra回复体力78点

清楚明白使用魅惑, Avada_Kedavra回避了攻击

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用苏生术, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra复活了, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra回复体力76点

Queen_G发起攻击, Avada_Kedavra受到72点伤害

使腿勇腋二发起攻击, Avada_Kedavra受到59点伤害

 Avada_Kedavra被击倒了

Light_Years_Away使用地裂术

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到45点伤害

 清楚明白受到59点伤害

 清楚明白被击倒了

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到31点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了

 前进观察团受到58点伤害

 前进观察团被击倒了

Avada_Kedavra发起狂暴攻击, Light_Years_Away受到65点伤害

 Light_Years_Away被击倒了

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用苏生术, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra复活了, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra回复体力35点

Queen_G发起攻击, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到177点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了

Avada_Kedavra发起狂暴攻击, Avada_Kedavra回避了攻击

 Avada_Kedavra从狂暴中解除

Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra使用魅惑, 使腿勇腋二被魅惑了

Queen_G发起攻击, Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra受到155点伤害

 Expelliarmus=----===-==---~=-==~-=-~~-~~~-Avada_Kedavra被击倒了

使腿勇腋二使用苏生术, 给我看点新花样复活了, 给我看点新花样回复体力132点

 使腿勇腋二从魅惑中解除

给我看点新花样使用净化, Queen_G受到61点伤害

 Queen_G的聚气被打消了

 Queen_G被击倒了

 给我看点新花样吞噬了Queen_G, 给我看点新花样属性上升

Avada_Kedavra发起攻击, 使腿勇腋二受到72点伤害

 使腿勇腋二被击倒了

给我看点新花样发起攻击, Avada_Kedavra回避了攻击

Avada_Kedavra发起攻击, 给我看点新花样受到98点伤害

给我看点新花样发起攻击, Avada_Kedavra受到23点伤害

 Avada_Kedavra被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-68 must contain a blank separator between input and trace",
        "sampled case-68 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-68 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-68", &actual_lines, &expected_lines);
}

#[test]
fn large_69() {
    const CASE: &str = r#"洋基周刊 #40WSvjsmo@Shabby_fish
El_Hombre_que_Ríe #wnyYvAHgu@Shabby_fish

望趋疯泉之@Shabby_fish
雷达 #pYAjWilZL@Shabby_fish


洋基周刊潜行到雷达身后

望趋疯泉之潜行到El_Hombre_que_Ríe身后

El_Hombre_que_Ríe使用分身, 出现一个新的El_Hombre_que_Ríe

El_Hombre_que_Ríe发起攻击, 望趋疯泉之受到53点伤害

 望趋疯泉之的潜行被识破

洋基周刊发动背刺, 雷达受到209点伤害

望趋疯泉之潜行到洋基周刊身后

El_Hombre_que_Ríe发起攻击, 雷达防御, 雷达受到16点伤害

雷达发起攻击, El_Hombre_que_Ríe受到49点伤害

El_Hombre_que_Ríe发起攻击, 雷达回避了攻击

望趋疯泉之发动背刺, 洋基周刊受到393点伤害

 洋基周刊被击倒了

 望趋疯泉之吞噬了洋基周刊, 望趋疯泉之属性上升

望趋疯泉之潜行到El_Hombre_que_Ríe身后

El_Hombre_que_Ríe发起攻击, 雷达受到60点伤害

雷达发起攻击, El_Hombre_que_Ríe受到76点伤害

El_Hombre_que_Ríe使用减速术, 望趋疯泉之进入迟缓状态

El_Hombre_que_Ríe发起攻击, 雷达受到69点伤害

 雷达被击倒了

望趋疯泉之发动背刺, El_Hombre_que_Ríe受到302点伤害

 El_Hombre_que_Ríe被击倒了

El_Hombre_que_Ríe使用分身, 出现一个新的El_Hombre_que_Ríe

El_Hombre_que_Ríe发起攻击, 望趋疯泉之受到0点伤害

望趋疯泉之使用苏生术, 雷达复活了, 雷达回复体力82点

 望趋疯泉之从迟缓中解除

雷达使用幻术, 召唤出幻影

El_Hombre_que_Ríe使用分身, 出现一个新的El_Hombre_que_Ríe

望趋疯泉之发起攻击, El_Hombre_que_Ríe受到105点伤害

 El_Hombre_que_Ríe被击倒了, El_Hombre_que_Ríe使用护身符抵挡了一次死亡, El_Hombre_que_Ríe回复体力5点

El_Hombre_que_Ríe发起攻击, 雷达回避了攻击

望趋疯泉之使用分身, 出现一个新的望趋疯泉之

El_Hombre_que_Ríe发起攻击, 雷达受到76点伤害

El_Hombre_que_Ríe发起攻击, 望趋疯泉之受到0点伤害

雷达发起攻击, El_Hombre_que_Ríe受到53点伤害

 El_Hombre_que_Ríe被击倒了

望趋疯泉之发起攻击, El_Hombre_que_Ríe受到50点伤害

 El_Hombre_que_Ríe被击倒了

幻影发起攻击, El_Hombre_que_Ríe受到104点伤害

 El_Hombre_que_Ríe被击倒了, El_Hombre_que_Ríe使用护身符抵挡了一次死亡, El_Hombre_que_Ríe回复体力11点

El_Hombre_que_Ríe发起攻击, 雷达使用伤害反弹, El_Hombre_que_Ríe使用伤害反弹, 雷达使用伤害反弹, El_Hombre_que_Ríe受到12点伤害

 El_Hombre_que_Ríe被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-69 must contain a blank separator between input and trace",
        "sampled case-69 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-69 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-69", &actual_lines, &expected_lines);
}

#[test]
fn large_70() {
    const CASE: &str = r#"Superpower #ddDROyhTJ@Shabby_fish
大化西游 #faYL5F6xL@Shabby_fish


大化西游潜行到Superpower身后

Superpower发起攻击, 大化西游回避了攻击

大化西游发动背刺, Superpower受到280点伤害

Superpower做出垂死抗争, Superpower所有属性上升

Superpower使用分身, 出现一个新的Superpower

Superpower使用魅惑, 大化西游回避了攻击

Superpower使用魅惑, 大化西游被魅惑了

大化西游发起攻击, 大化西游受到80点伤害

大化西游从魅惑中解除

Superpower使用冰冻术, 大化西游回避了攻击

Superpower使用分身, 出现一个新的Superpower

Superpower发起攻击, 大化西游受到30点伤害

大化西游发起攻击, Superpower受到57点伤害

Superpower被击倒了

Superpower使用分身, 出现一个新的Superpower

大化西游发起攻击, Superpower受到75点伤害

Superpower被击倒了

大化西游召唤亡灵, Superpower变成了丧尸

Superpower发起攻击, 大化西游受到22点伤害

丧尸发起攻击, Superpower受到63点伤害

Superpower被击倒了

大化西游发起攻击, Superpower受到84点伤害

Superpower被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-70 must contain a blank separator between input and trace",
        "sampled case-70 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-70 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-70", &actual_lines, &expected_lines);
}

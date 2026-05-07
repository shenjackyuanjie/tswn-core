use super::*;

#[test]
fn large_66() {
    const CASE: &str = r#"左慈发动化身，获得了降祸连诛去疾业炎荐杰@Shabby_fish
Fly_Away #pBLQfffn9@Shabby_fish
飘雌喇拴供@Shabby_fish
Stupefy #mlMUWL12@Shabby_fish


Fly_Away开始蓄力

Stupefy发起攻击, Fly_Away受到31点伤害

左慈发动化身，获得了降祸连诛去疾业炎荐杰使用幻术, 召唤出幻影

飘雌喇拴供使用分身, 出现一个新的飘雌喇拴供

Stupefy发起攻击, 左慈发动化身，获得了降祸连诛去疾业炎荐杰受到36点伤害

飘雌喇拴供发起攻击, 左慈发动化身，获得了降祸连诛去疾业炎荐杰受到49点伤害

左慈发动化身，获得了降祸连诛去疾业炎荐杰发起攻击, 飘雌喇拴供受到78点伤害

Fly_Away潜行到幻影身后

Fly_Away发动背刺, 幻影受到534点伤害

 幻影消失了

 Fly_Away吞噬了幻影, Fly_Away属性上升

飘雌喇拴供发起攻击, Stupefy受到78点伤害

Stupefy发起攻击, Fly_Away受到40点伤害

飘雌喇拴供发起攻击, 左慈发动化身，获得了降祸连诛去疾业炎荐杰受到89点伤害

左慈发动化身，获得了降祸连诛去疾业炎荐杰使用幻术, 召唤出幻影

Fly_Away潜行到幻影身后

Stupefy发起攻击, 左慈发动化身，获得了降祸连诛去疾业炎荐杰受到21点伤害

飘雌喇拴供发起攻击, 幻影受到111点伤害

左慈发动化身，获得了降祸连诛去疾业炎荐杰发起攻击, Fly_Away受到65点伤害

 Fly_Away的潜行被识破

飘雌喇拴供开始蓄力

Fly_Away使用火球术, 飘雌喇拴供受到172点伤害

 飘雌喇拴供被击倒了

左慈发动化身，获得了降祸连诛去疾业炎荐杰发起攻击, Fly_Away受到85点伤害

幻影发起攻击, Fly_Away受到80点伤害

Stupefy使用分身, 出现一个新的Stupefy

飘雌喇拴供发起攻击, 左慈发动化身，获得了降祸连诛去疾业炎荐杰受到81点伤害

Fly_Away使用火球术, Stupefy受到113点伤害

 Stupefy被击倒了

Stupefy使用苏生术, Stupefy复活了, Stupefy回复体力63点

幻影使用附体, 飘雌喇拴供进入狂暴状态

 幻影消失了

Stupefy发起攻击, 飘雌喇拴供受到65点伤害

左慈发动化身，获得了降祸连诛去疾业炎荐杰使用魅惑, Stupefy被魅惑了

Stupefy潜行到Fly_Away身后

Fly_Away潜行到Stupefy身后

左慈发动化身，获得了降祸连诛去疾业炎荐杰使用魅惑, Stupefy被魅惑了

Fly_Away发动背刺, Stupefy受到386点伤害

 Stupefy的潜行被识破

 Stupefy被击倒了, Stupefy使用护身符抵挡了一次死亡, Stupefy回复体力1点

飘雌喇拴供发起狂暴攻击, Stupefy回避了攻击

Stupefy开始蓄力

 Stupefy从魅惑中解除

Stupefy使用分身, 出现一个新的Stupefy

 Stupefy从魅惑中解除

Fly_Away开始蓄力

Stupefy发起攻击, 左慈发动化身，获得了降祸连诛去疾业炎荐杰受到51点伤害

 左慈发动化身，获得了降祸连诛去疾业炎荐杰被击倒了

飘雌喇拴供发起狂暴攻击, Stupefy受到104点伤害

 Stupefy被击倒了

Fly_Away使用火球术, Stupefy受到432点伤害

 Stupefy被击倒了

Stupefy潜行到Fly_Away身后

Stupefy发动背刺, Fly_Away受到205点伤害

 Fly_Away被击倒了

飘雌喇拴供发起狂暴攻击, Stupefy受到0点伤害

飘雌喇拴供发起狂暴攻击, Stupefy受到0点伤害

 飘雌喇拴供从狂暴中解除

Stupefy发起攻击, 飘雌喇拴供受到41点伤害

 飘雌喇拴供被击倒了
"#;
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

#[test]
fn large_67() {
    const CASE: &str = r#"Superpower #yWBFbBGZl@Shabby_fish
Accio #wFS52SNn@Shabby_fish
Mythic Jf)qvWCQ@Shabby_fish
Long_Distance #vKarTtBnw@Shabby_fish


Superpower发起攻击, Long_Distance受到53点伤害

Mythic潜行到Accio身后

Accio潜行到Mythic身后

Long_Distance发起攻击, Accio回避了攻击

Accio发动背刺, Mythic受到363点伤害

 Mythic的潜行被识破

 Mythic被击倒了

 Accio吞噬了Mythic, Accio属性上升

Superpower潜行到Accio身后

Long_Distance发起攻击, Accio受到38点伤害

Accio发起攻击, Superpower回避了攻击

Superpower发动背刺, Accio受到402点伤害

 Accio被击倒了

Long_Distance潜行到Superpower身后

Superpower发起攻击, Long_Distance受到76点伤害

 Long_Distance的潜行被识破

Long_Distance发起攻击, Superpower受到11点伤害

Superpower使用幻术, 召唤出幻影

Long_Distance使用分身, 出现一个新的Long_Distance

Superpower发起攻击, Long_Distance受到66点伤害

Superpower使用冰冻术, Long_Distance受到76点伤害

 Long_Distance被击倒了

Long_Distance使用魅惑, Superpower被魅惑了

幻影使用附体, Long_Distance进入狂暴状态

 幻影消失了

Long_Distance发起狂暴攻击, Superpower受到67点伤害

Superpower使用幻术, 召唤出幻影

 Superpower从魅惑中解除

Long_Distance发起狂暴攻击, 幻影受到79点伤害

Superpower发起攻击, Long_Distance受到87点伤害

Long_Distance发起狂暴攻击, Superpower回避了攻击

幻影使用附体, Long_Distance进入狂暴状态

 幻影消失了

Superpower发起攻击, Long_Distance受到93点伤害

 Long_Distance被击倒了
"#;
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
    const CASE: &str = r#"望趋疯泉之@Shabby_fish
雷达 #pYAjWilZL@Shabby_fish

酱夯妹着哄@Shabby_fish
防空 #CCHmn6NjU@Shabby_fish


酱夯妹着哄潜行到雷达身后

防空使用分身, 出现一个新的防空

望趋疯泉之潜行到酱夯妹着哄身后

雷达使用幻术, 召唤出幻影

酱夯妹着哄发动背刺, 雷达受到238点伤害

防空发起攻击, 幻影受到36点伤害

望趋疯泉之发动背刺, 酱夯妹着哄受到262点伤害

防空发起攻击, 幻影受到93点伤害

酱夯妹着哄发起攻击, 幻影受到63点伤害

 幻影消失了

雷达使用幻术, 召唤出幻影

望趋疯泉之发起攻击, 酱夯妹着哄受到37点伤害

 酱夯妹着哄被击倒了

防空发起攻击, 幻影受到36点伤害

防空发起攻击, 望趋疯泉之受到0点伤害

雷达发起攻击, 防空受到40点伤害

望趋疯泉之潜行到防空身后

防空潜行到望趋疯泉之身后

防空发起攻击, 幻影受到49点伤害

幻影发起攻击, 防空受到56点伤害

 防空的潜行被识破

望趋疯泉之发动背刺, 防空受到318点伤害

 防空被击倒了

防空使用苏生术, 酱夯妹着哄复活了, 酱夯妹着哄回复体力113点

雷达发起攻击, 酱夯妹着哄受到24点伤害

望趋疯泉之发起攻击, 酱夯妹着哄受到80点伤害

 酱夯妹着哄做出垂死抗争, 酱夯妹着哄所有属性上升

幻影使用附体, 防空进入狂暴状态

 幻影消失了

酱夯妹着哄使用净化, 望趋疯泉之受到0点伤害

雷达发起攻击, 酱夯妹着哄受到45点伤害

 酱夯妹着哄被击倒了

望趋疯泉之发起攻击, 防空受到68点伤害

防空发起狂暴攻击, 雷达受到86点伤害

 雷达被击倒了

望趋疯泉之发起攻击, 防空受到51点伤害

防空发起狂暴攻击, 防空受到122点伤害

 防空被击倒了
"#;
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
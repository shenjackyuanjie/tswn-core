use super::*;

#[test]
fn large_56() {
    const CASE: &str = r#"渊HG bwigVwI@807139
♯ WJl5tXdd5X@807139

syVS:et@Hell
'Yz|AS}@Hell
seed:2@!


渊HG使用净化, 'Yz|AS}受到50点伤害

syVS:et使用幻术, 召唤出幻影

♯潜行到'Yz|AS}身后

'Yz|AS}发起攻击, 渊HG受到29点伤害

渊HG使用净化, 'Yz|AS}受到59点伤害

'Yz|AS}发起攻击, 渊HG回避了攻击

syVS:et使用分身, 出现一个新的syVS:et

♯发动背刺, 'Yz|AS}受到371点伤害

 'Yz|AS}被击倒了

 ♯吞噬了'Yz|AS}, ♯属性上升

syVS:et使用幻术, 召唤出幻影

渊HG使用净化, 幻影受到139点伤害

幻影发起攻击, 渊HG受到70点伤害

♯使用分身, 出现一个新的♯

syVS:et使用分身, 出现一个新的syVS:et

幻影发起攻击, ♯受到75点伤害

渊HG使用净化, 幻影受到114点伤害

 幻影消失了

♯潜行到syVS:et身后

syVS:et发起攻击, ♯受到19点伤害

♯潜行到syVS:et身后

幻影使用附体, ♯进入狂暴状态

 幻影消失了

syVS:et发起攻击, 渊HG受到63点伤害

syVS:et使用幻术, 召唤出幻影

♯发动背刺, syVS:et受到412点伤害

 syVS:et被击倒了

 幻影消失了

 ♯吞噬了syVS:et, ♯属性上升

♯发动背刺, syVS:et受到131点伤害

渊HG使用净化, syVS:et受到40点伤害

syVS:et发起攻击, ♯受到46点伤害

 ♯做出垂死抗争, ♯所有属性上升

渊HG发起攻击, syVS:et受到96点伤害

 syVS:et被击倒了

♯使用减速术, syVS:et进入迟缓状态

♯发起狂暴攻击, 渊HG受到30点伤害

渊HG发起攻击, syVS:et受到60点伤害

♯发起攻击, syVS:et受到96点伤害

 syVS:et被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-56 must contain a blank separator between input and trace",
        "sampled case-56 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-56 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-56", &actual_lines, &expected_lines);
}

#[test]
fn large_57() {
    const CASE: &str = r#"涵虚不等式 PFVKEUPBU@TigerStar
Meranti FBPITYXTBPQG@nan
seed:2@!


涵虚不等式使用幻术, 召唤出幻影

Meranti潜行到幻影身后

Meranti发动背刺, 幻影受到468点伤害

 幻影消失了

涵虚不等式使用幻术, 召唤出幻影

Meranti使用幻术, 召唤出幻影

涵虚不等式发起攻击, Meranti受到0点伤害

Meranti潜行到涵虚不等式身后

涵虚不等式使用幻术, 召唤出幻影

幻影发起攻击, 幻影受到144点伤害

 幻影消失了

幻影使用附体, Meranti进入狂暴状态

 幻影消失了

Meranti发动背刺, 涵虚不等式受到334点伤害

 涵虚不等式被击倒了

 幻影消失了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-57 must contain a blank separator between input and trace",
        "sampled case-57 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-57 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-57", &actual_lines, &expected_lines);
}

#[test]
fn large_58() {
    const CASE: &str = r#"Bascor cW1JWDuv7f@RbCl
Meltel abRC3P3Go7@RbCl

syVS:et@Hell
'Yz|AS}@Hell
seed:2@!


'Yz|AS}使用减速术, Meltel进入迟缓状态

Bascor潜行到'Yz|AS}身后

syVS:et使用幻术, 召唤出幻影

syVS:et发起攻击, Meltel受到24点伤害

'Yz|AS}使用分身, 出现一个新的'Yz|AS}

Meltel发起攻击, syVS:et受到48点伤害

Bascor发动背刺, 'Yz|AS}受到354点伤害

 'Yz|AS}被击倒了

 Bascor吞噬了'Yz|AS}, Bascor属性上升

'Yz|AS}使用减速术, Meltel回避了攻击

syVS:et使用幻术, 召唤出幻影

幻影使用附体, Bascor进入狂暴状态

 幻影消失了

syVS:et使用幻术, 召唤出幻影

Bascor潜行到syVS:et身后

'Yz|AS}使用减速术, Bascor回避了攻击

Meltel发起攻击, 幻影受到91点伤害

 Meltel从迟缓中解除

Bascor发动背刺, syVS:et受到446点伤害

 syVS:et被击倒了

 幻影消失了

 幻影消失了

 Bascor吞噬了syVS:et, Bascor属性上升

Meltel使用减速术, 'Yz|AS}进入迟缓状态

Bascor发起攻击, 'Yz|AS}受到68点伤害

'Yz|AS}使用减速术, Meltel进入迟缓状态

Meltel使用减速术, 'Yz|AS}进入迟缓状态

Bascor开始聚气, Bascor攻击力上升

Bascor发起攻击, 'Yz|AS}受到118点伤害

 'Yz|AS}被击倒了
"#;

    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-58 must contain a blank separator between input and trace",
        "sampled case-58 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-58 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-58", &actual_lines, &expected_lines);
}

#[test]
fn large_59() {
    const CASE: &str = r#"4SSRr!r%Z2H9COtk`n~x@czr2012
小野一会 kIb3TA5XWJet@fx
鑺掕悂 R9feo6cEJ4Aj@芒萁
若血橇 lK#dmJ;12@四象柯
江南小子 #LLVKEPMU@暗黑突击

真夜霞 #FBNWDPBPW@无惨
发发开行进开力开进瓜进进行瓜进发开开@Squall
Fengshen ONVWTGMPNCKV@nan
Tan965596700284@酸橙
高潮迭起 #YFgJfQFyN@Shabby_fish
seed:1376-5-16@!


鑺掕悂使用血祭, 召唤出使魔

江南小子使用净化, 高潮迭起受到99点伤害

4SSRr!r%Z2H9COtk`n~x发起攻击, Fengshen受到28点伤害

Tan965596700284潜行到江南小子身后

真夜霞发起攻击, 若血橇受到35点伤害

发发开行进开力开进瓜进进行瓜进发开开发动铁壁, 发发开行进开力开进瓜进进行瓜进发开开防御力大幅上升

若血橇使用魅惑, 发发开行进开力开进瓜进进行瓜进发开开被魅惑了

小野一会发动铁壁, 小野一会防御力大幅上升

使魔使用火球术, 高潮迭起受到62点伤害

Fengshen使用减速术, 小野一会进入迟缓状态

鑺掕悂发起攻击, 高潮迭起回避了攻击

江南小子使用火球术, Fengshen受到65点伤害

Tan965596700284发动背刺, 江南小子受到298点伤害

若血橇发起攻击, 高潮迭起受到53点伤害

高潮迭起发起攻击, 小野一会受到0点伤害

Fengshen使用分身, 出现一个新的Fengshen

真夜霞使用治愈魔法, 高潮迭起回复体力97点

发发开行进开力开进瓜进进行瓜进发开开发起攻击, Fengshen受到61点伤害

 发发开行进开力开进瓜进进行瓜进发开开从魅惑中解除

4SSRr!r%Z2H9COtk`n~x使用幻术, 召唤出幻影

江南小子使用净化, 高潮迭起受到60点伤害

使魔发起攻击, 高潮迭起回避了攻击

Fengshen发起攻击, 幻影受到68点伤害

Tan965596700284潜行到鑺掕悂身后

4SSRr!r%Z2H9COtk`n~x发起攻击, 发发开行进开力开进瓜进进行瓜进发开开受到0点伤害

高潮迭起发起攻击, 小野一会受到1点伤害

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 小野一会受到1点伤害

 发发开行进开力开进瓜进进行瓜进发开开从铁壁中解除

小野一会发起攻击, Fengshen受到56点伤害

鑺掕悂使用净化, Fengshen受到110点伤害

 Fengshen被击倒了

江南小子使用火球术, Fengshen受到131点伤害

 Fengshen被击倒了, Fengshen使用护身符抵挡了一次死亡, Fengshen回复体力13点

使魔使用火球术, Fengshen受到131点伤害

 Fengshen被击倒了, Fengshen使用护身符抵挡了一次死亡, Fengshen回复体力9点

真夜霞使用幻术, 召唤出幻影

高潮迭起发起攻击, 4SSRr!r%Z2H9COtk`n~x受到54点伤害

若血橇开始蓄力

Fengshen使用减速术, 若血橇进入迟缓状态

鑺掕悂使用净化, Fengshen受到55点伤害

 Fengshen被击倒了

幻影发起攻击, 发发开行进开力开进瓜进进行瓜进发开开受到67点伤害

发发开行进开力开进瓜进进行瓜进发开开发动铁壁, 发发开行进开力开进瓜进进行瓜进发开开防御力大幅上升

Tan965596700284发动背刺, 鑺掕悂受到363点伤害

 鑺掕悂被击倒了

 使魔消失了

 Tan965596700284吞噬了鑺掕悂, Tan965596700284属性上升

高潮迭起发起攻击, 幻影受到66点伤害

江南小子使用火球术, 高潮迭起受到118点伤害

4SSRr!r%Z2H9COtk`n~x发起攻击, 高潮迭起回避了攻击

Tan965596700284潜行到小野一会身后

真夜霞使用血祭, 召唤出使魔

4SSRr!r%Z2H9COtk`n~x发起攻击, 高潮迭起受到87点伤害

 高潮迭起被击倒了

幻影使用附体, Tan965596700284进入狂暴状态

 幻影消失了

小野一会发起攻击, 幻影受到51点伤害

 小野一会从铁壁中解除

 小野一会从迟缓中解除

使魔发起攻击, 江南小子回避了攻击

4SSRr!r%Z2H9COtk`n~x使用分身, 出现一个新的4SSRr!r%Z2H9COtk`n~x

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 4SSRr!r%Z2H9COtk`n~x受到62点伤害

Tan965596700284发动背刺, 小野一会受到203点伤害

若血橇使用魅惑, 真夜霞被魅惑了

真夜霞使用幻术, 召唤出幻影

江南小子使用火球术, 幻影受到56点伤害

Tan965596700284发起狂暴攻击, 4SSRr!r%Z2H9COtk`n~x受到100点伤害

幻影发起攻击, 江南小子回避了攻击

使魔发起攻击, 若血橇受到17点伤害

真夜霞使用幻术, 召唤出幻影

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 小野一会受到103点伤害

 发发开行进开力开进瓜进进行瓜进发开开从铁壁中解除

4SSRr!r%Z2H9COtk`n~x发起攻击, 发发开行进开力开进瓜进进行瓜进发开开受到35点伤害

小野一会发起攻击, 幻影受到68点伤害

江南小子使用火球术, 幻影受到92点伤害

Tan965596700284发起狂暴攻击, 发发开行进开力开进瓜进进行瓜进发开开受到111点伤害

发发开行进开力开进瓜进进行瓜进发开开发动铁壁, 发发开行进开力开进瓜进进行瓜进发开开防御力大幅上升

若血橇潜行到Tan965596700284身后

 若血橇从迟缓中解除

4SSRr!r%Z2H9COtk`n~x发起攻击, 幻影受到62点伤害

真夜霞使用幻术, 召唤出幻影

小野一会发动铁壁, 小野一会防御力大幅上升

江南小子发起攻击, 幻影受到37点伤害

 幻影消失了

幻影使用附体, 小野一会回避了攻击

使魔发起攻击, 小野一会回避了攻击

若血橇发动背刺, Tan965596700284受到326点伤害

4SSRr!r%Z2H9COtk`n~x发起攻击, 幻影受到37点伤害

4SSRr!r%Z2H9COtk`n~x发起攻击, Tan965596700284受到85点伤害

 Tan965596700284被击倒了

幻影发起攻击, 小野一会受到0点伤害

真夜霞使用减速术, 幻影进入迟缓状态

 真夜霞从魅惑中解除

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 小野一会受到0点伤害

若血橇发起攻击, 发发开行进开力开进瓜进进行瓜进发开开受到0点伤害

小野一会发起攻击, 幻影受到46点伤害

 幻影消失了

使魔使用火球术, 江南小子受到54点伤害

 江南小子被击倒了

4SSRr!r%Z2H9COtk`n~x发起攻击, 幻影受到53点伤害

4SSRr!r%Z2H9COtk`n~x发起攻击, 发发开行进开力开进瓜进进行瓜进发开开受到0点伤害

幻影发起攻击, 4SSRr!r%Z2H9COtk`n~x受到67点伤害

 4SSRr!r%Z2H9COtk`n~x被击倒了

真夜霞发起攻击, 小野一会受到0点伤害

若血橇发起攻击, 发发开行进开力开进瓜进进行瓜进发开开受到0点伤害

小野一会发起攻击, 幻影受到82点伤害

 小野一会从铁壁中解除

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 小野一会受到0点伤害

 发发开行进开力开进瓜进进行瓜进发开开从铁壁中解除

4SSRr!r%Z2H9COtk`n~x使用幻术, 召唤出幻影

使魔使用自爆, 4SSRr!r%Z2H9COtk`n~x受到172点伤害

 4SSRr!r%Z2H9COtk`n~x被击倒了

 幻影消失了

 使魔消失了

幻影发起攻击, 小野一会回避了攻击

真夜霞发起攻击, 小野一会回避了攻击

小野一会发起攻击, 真夜霞受到67点伤害

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 小野一会回避了攻击

真夜霞使用治愈魔法, 发发开行进开力开进瓜进进行瓜进发开开回复体力84点

幻影发起攻击, 小野一会受到0点伤害

 幻影从迟缓中解除

幻影发起攻击, 小野一会受到39点伤害

 小野一会被击倒了

若血橇开始蓄力

发发开行进开力开进瓜进进行瓜进发开开发起攻击, 若血橇受到53点伤害

幻影使用附体, 若血橇进入狂暴状态

 幻影消失了

真夜霞使用减速术, 若血橇进入迟缓状态

真夜霞使用血祭, 召唤出使魔

幻影发起攻击, 若血橇受到52点伤害

真夜霞发起攻击, 若血橇受到31点伤害

发发开行进开力开进瓜进进行瓜进发开开发动铁壁, 发发开行进开力开进瓜进进行瓜进发开开防御力大幅上升

使魔使用火球术, 若血橇受到91点伤害

若血橇潜行到真夜霞身后

幻影发起攻击, 若血橇受到55点伤害

 若血橇被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-59 must contain a blank separator between input and trace",
        "sampled case-59 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-59 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-59", &actual_lines, &expected_lines);
}

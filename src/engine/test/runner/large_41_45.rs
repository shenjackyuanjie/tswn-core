use super::*;

/// 滚木
#[test]
fn large_41() {
    const CASE: &str = r####"桃v66wy7tgu27xp@asyncTales
b64d64cfaae1f621@asyncTales

Reku_Mochizuki #494460162188@新纪元
扶灵 /eaKEZPkiLP/@新纪元

seed:第十八届武术大赛附加赛:689-1@!


Reku_Mochizuki发起攻击, b64d64cfaae1f621回避了攻击

扶灵发起攻击, b64d64cfaae1f621受到64点伤害

桃v66wy7tgu27xp使用地裂术

 Reku_Mochizuki受到55点伤害

 扶灵受到42点伤害

 扶灵发起反击, 桃v66wy7tgu27xp受到54点伤害

b64d64cfaae1f621发起攻击, 扶灵受到72点伤害

扶灵使用减速术, 桃v66wy7tgu27xp进入迟缓状态

Reku_Mochizuki使用减速术, b64d64cfaae1f621回避了攻击

b64d64cfaae1f621发起攻击, 扶灵受到91点伤害

Reku_Mochizuki使用减速术, b64d64cfaae1f621进入迟缓状态

桃v66wy7tgu27xp发动铁壁, 桃v66wy7tgu27xp防御力大幅上升

Reku_Mochizuki发起攻击, 桃v66wy7tgu27xp受到1点伤害, 桃v66wy7tgu27xp发动隐匿

扶灵使用分身, 出现一个新的扶灵

b64d64cfaae1f621使用净化, 扶灵受到36点伤害

扶灵发起攻击, b64d64cfaae1f621回避了攻击

Reku_Mochizuki使用分身, 出现一个新的Reku_Mochizuki

桃v66wy7tgu27xp使用净化, Reku_Mochizuki受到89点伤害

 桃v66wy7tgu27xp从迟缓中解除

Reku_Mochizuki发起攻击, b64d64cfaae1f621受到68点伤害

Reku_Mochizuki发起攻击, 桃v66wy7tgu27xp受到1点伤害

扶灵使用分身, 出现一个新的扶灵

桃v66wy7tgu27xp使用地裂术

 扶灵受到21点伤害

 Reku_Mochizuki受到39点伤害

 扶灵受到24点伤害

 扶灵受到32点伤害

 桃v66wy7tgu27xp从铁壁中解除

 扶灵发起反击, 桃v66wy7tgu27xp受到24点伤害

 Reku_Mochizuki发起反击, 桃v66wy7tgu27xp受到45点伤害, 桃v66wy7tgu27xp发动隐匿

扶灵发起攻击, b64d64cfaae1f621回避了攻击

扶灵发起攻击, b64d64cfaae1f621回避了攻击

Reku_Mochizuki使用减速术, b64d64cfaae1f621进入迟缓状态

b64d64cfaae1f621发起攻击, Reku_Mochizuki受到82点伤害

 Reku_Mochizuki被击倒了, Reku_Mochizuki使用护身符抵挡了一次死亡, Reku_Mochizuki回复体力10点

扶灵发起攻击, b64d64cfaae1f621受到30点伤害

桃v66wy7tgu27xp使用地裂术

 扶灵受到21点伤害

 Reku_Mochizuki受到66点伤害

 Reku_Mochizuki被击倒了

 桃v66wy7tgu27xp召唤亡灵, Reku_Mochizuki变成了丧尸

 扶灵受到42点伤害

 扶灵被击倒了

 桃v66wy7tgu27xp召唤亡灵, 扶灵变成了丧尸

 扶灵受到35点伤害

 扶灵被击倒了

 桃v66wy7tgu27xp召唤亡灵, 扶灵变成了丧尸

Reku_Mochizuki使用减速术, 丧尸进入迟缓状态

桃v66wy7tgu27xp使用地裂术

 扶灵受到17点伤害

 扶灵被击倒了, 扶灵使用护身符抵挡了一次死亡, 扶灵回复体力14点

 Reku_Mochizuki回避了攻击

b64d64cfaae1f621发起攻击, 扶灵受到103点伤害

 扶灵被击倒了

丧尸发起攻击, Reku_Mochizuki受到71点伤害

丧尸发起攻击, Reku_Mochizuki回避了攻击

Reku_Mochizuki使用减速术, 丧尸进入迟缓状态

桃v66wy7tgu27xp发起攻击, Reku_Mochizuki受到48点伤害

Reku_Mochizuki发起攻击, 丧尸受到107点伤害

丧尸发起攻击, Reku_Mochizuki回避了攻击

丧尸发起攻击, Reku_Mochizuki受到37点伤害

 Reku_Mochizuki被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-41 must contain a blank separator between input and trace",
        "sampled case-41 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-41 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-41", &actual_lines, &expected_lines);
}

/// 滚木特性
#[test]
fn large_42() {
    const CASE: &str = r####"测707640862046T，烦恼立刻消失@爱
坚持 E6b10FVHvKDO@Afterglow
Influence #MEZC2wa@Unbound
耀眼之星 /JxrJYwouGw/@新纪元
随之任之 #iWZYBGuwxX@🥒

真夜霞 #FBNWDPBPW@无惨
虚空托腮 UMOXFIARH@TigerStar
Fengshen ONVWTGMPNCKV@nan
Boundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish
Spearmaster ZbblyZQQwr@RainWorld_XIV
seed:1376-2-15@!


虚空托腮发起攻击, 耀眼之星受到84点伤害

真夜霞使用血祭, 召唤出使魔

耀眼之星使用瘟疫, Fengshen回避了攻击

Influence使用加速术, Influence进入疾走状态

Spearmaster发动会心一击, 耀眼之星受到98点伤害

坚持发起攻击, Spearmaster受到41点伤害

测707640862046T，烦恼立刻消失发起攻击, Spearmaster受到76点伤害

Fengshen使用减速术, Influence进入迟缓状态

Influence使用加速术, 坚持进入疾走状态

虚空托腮潜行到Influence身后

Boundless_Ocean,Vast_Skies潜行到Influence身后

耀眼之星使用瘟疫, 虚空托腮体力减少67%

 虚空托腮的潜行被识破

Spearmaster发起攻击, 耀眼之星受到112点伤害

真夜霞发起攻击, 耀眼之星受到121点伤害

 耀眼之星被击倒了

坚持发动铁壁, 坚持防御力大幅上升

使魔使用自爆, 测707640862046T，烦恼立刻消失防御, 测707640862046T，烦恼立刻消失受到122点伤害

 使魔消失了

随之任之开始聚气, 随之任之攻击力上升

Fengshen使用减速术, 坚持进入迟缓状态

测707640862046T，烦恼立刻消失使用地裂术

 虚空托腮受到23点伤害

 Spearmaster受到48点伤害

 Fengshen受到32点伤害

 真夜霞受到43点伤害

虚空托腮发动铁壁, 虚空托腮防御力大幅上升

随之任之发起攻击, 虚空托腮受到1点伤害

Influence发起攻击, 虚空托腮受到1点伤害

 Influence从疾走中解除

 Influence从迟缓中解除

Spearmaster发动会心一击, 坚持受到0点伤害

坚持发起攻击, Spearmaster受到35点伤害, Spearmaster发动隐匿

Fengshen使用分身, 出现一个新的Fengshen

真夜霞发起攻击, 测707640862046T，烦恼立刻消失回避了攻击

随之任之发起攻击

 虚空托腮的铁壁被打消了, 虚空托腮受到1点伤害

虚空托腮发动铁壁, 虚空托腮防御力大幅上升

测707640862046T，烦恼立刻消失发起攻击, Fengshen受到103点伤害

Boundless_Ocean,Vast_Skies发动背刺, Influence受到262点伤害

Influence使用加速术, 测707640862046T，烦恼立刻消失进入疾走状态

Spearmaster发动会心一击, Influence受到99点伤害

Fengshen使用减速术, Influence回避了攻击

测707640862046T，烦恼立刻消失发起攻击, 虚空托腮受到1点伤害

坚持发起攻击, Spearmaster受到56点伤害

 坚持从疾走中解除

 坚持从铁壁中解除

 坚持从迟缓中解除

Fengshen使用减速术, 坚持进入迟缓状态

Boundless_Ocean,Vast_Skies使用雷击术

 Influence受到17点伤害

 Influence受到30点伤害

 Influence被击倒了

 Boundless_Ocean,Vast_Skies召唤亡灵, Influence变成了丧尸

真夜霞使用幻术, 召唤出幻影

随之任之发起攻击

 虚空托腮的铁壁被打消了, 虚空托腮受到20点伤害

Spearmaster发动会心一击, 测707640862046T，烦恼立刻消失防御, 测707640862046T，烦恼立刻消失受到46点伤害

丧尸发起攻击, 坚持受到72点伤害

随之任之发动会心一击, Fengshen受到212点伤害

 Fengshen被击倒了, Fengshen使用护身符抵挡了一次死亡, Fengshen回复体力8点

Fengshen发起攻击, 测707640862046T，烦恼立刻消失受到54点伤害

Boundless_Ocean,Vast_Skies发起攻击, 测707640862046T，烦恼立刻消失受到49点伤害

Fengshen使用减速术, 测707640862046T，烦恼立刻消失进入迟缓状态

虚空托腮发起攻击, 坚持受到111点伤害

真夜霞使用幻术, 召唤出幻影

测707640862046T，烦恼立刻消失使用地裂术

 Fengshen受到19点伤害

 Fengshen被击倒了

 测707640862046T，烦恼立刻消失吞噬了Fengshen, 测707640862046T，烦恼立刻消失属性上升

 虚空托腮回避了攻击

 Spearmaster受到34点伤害, Spearmaster发动隐匿

 幻影受到22点伤害

 丧尸受到31点伤害

测707640862046T，烦恼立刻消失使用地裂术

 虚空托腮受到33点伤害

 虚空托腮做出垂死抗争, 虚空托腮所有属性上升

 丧尸受到54点伤害

 幻影受到16点伤害

 真夜霞受到44点伤害

 Spearmaster受到45点伤害

 Spearmaster被击倒了

 测707640862046T，烦恼立刻消失从疾走中解除

 测707640862046T，烦恼立刻消失从迟缓中解除

虚空托腮发动铁壁, 虚空托腮防御力大幅上升

丧尸发起攻击, 随之任之受到37点伤害

真夜霞使用幻术, 召唤出幻影

坚持发动铁壁, 坚持防御力大幅上升

Fengshen使用减速术, 随之任之进入迟缓状态

幻影发起攻击, 测707640862046T，烦恼立刻消失受到21点伤害

Boundless_Ocean,Vast_Skies发起攻击, 随之任之受到15点伤害

 随之任之发起反击, Boundless_Ocean,Vast_Skies受到152点伤害

虚空托腮使用治愈魔法, 虚空托腮回复体力150点

丧尸发起攻击

随之任之发动会心一击, Fengshen受到150点伤害

 Fengshen被击倒了, Fengshen使用护身符抵挡了一次死亡, Fengshen回复体力9点

测707640862046T，烦恼立刻消失使用地裂术

 Fengshen受到26点伤害

 Fengshen被击倒了, Fengshen使用护身符抵挡了一次死亡, Fengshen回复体力4点

 虚空托腮回避了攻击

 丧尸受到39点伤害

 幻影受到37点伤害

 幻影受到14点伤害

真夜霞使用血祭, 召唤出使魔

Fengshen使用减速术, 测707640862046T，烦恼立刻消失进入迟缓状态

幻影使用附体, 坚持进入狂暴状态

 幻影消失了

Boundless_Ocean,Vast_Skies潜行到坚持身后

虚空托腮使用治愈魔法, Fengshen回复体力160点

 虚空托腮从铁壁中解除

丧尸发起攻击, 坚持受到0点伤害

幻影发起攻击, 坚持受到0点伤害

幻影发起攻击, 坚持回避了攻击

丧尸发起攻击, 随之任之回避了攻击

使魔发起攻击, 坚持受到0点伤害

幻影发起攻击, 随之任之受到78点伤害, 随之任之发动隐匿

真夜霞发起攻击, 测707640862046T，烦恼立刻消失受到76点伤害

 测707640862046T，烦恼立刻消失被击倒了

随之任之发起攻击, 虚空托腮回避了攻击

 随之任之从迟缓中解除

坚持发起狂暴攻击, Fengshen受到44点伤害

 坚持从迟缓中解除

真夜霞发起攻击, 坚持受到0点伤害

幻影发起攻击, 随之任之受到47点伤害

虚空托腮发动铁壁, 虚空托腮防御力大幅上升

Fengshen发起攻击, 坚持受到0点伤害

Boundless_Ocean,Vast_Skies发动背刺

 坚持的铁壁被打消了, 坚持受到244点伤害

 坚持被击倒了

 Boundless_Ocean,Vast_Skies召唤亡灵, 坚持变成了丧尸

幻影使用附体, 随之任之进入狂暴状态

 幻影消失了

使魔发起攻击, 随之任之受到45点伤害

丧尸发起攻击, 随之任之受到56点伤害

丧尸发起攻击, 随之任之受到49点伤害

幻影发起攻击, 随之任之受到81点伤害

 随之任之被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-42 must contain a blank separator between input and trace",
        "sampled case-42 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-42 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-42", &actual_lines, &expected_lines);
}

/// 护盾+蓄力特性
#[test]
fn large_43() {
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
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-43 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-43", &actual_lines, &expected_lines);
}

/// 护盾
#[test]
fn large_44() {
    const CASE: &str = r####"虚蚓嬉申杆@Shabby_fish
曾搪激归汗@Hell
seed:1129 R1-#9-3@!


曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

虚蚓嬉申杆发动铁壁, 虚蚓嬉申杆防御力大幅上升

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗使用减速术, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆使用分身, 出现一个新的虚蚓嬉申杆

虚蚓嬉申杆发起攻击, 曾搪激归汗受到84点伤害

 曾搪激归汗的潜行被识破

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

虚蚓嬉申杆开始蓄力

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用瘟疫, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆发起攻击, 曾搪激归汗受到77点伤害

 曾搪激归汗被击倒了

 虚蚓嬉申杆从铁壁中解除

曾搪激归汗使用苏生术, 曾搪激归汗复活了, 曾搪激归汗回复体力82点

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到56点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆发动铁壁, 虚蚓嬉申杆防御力大幅上升

 虚蚓嬉申杆从迟缓中解除

曾搪激归汗发起攻击, 虚蚓嬉申杆受到90点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

虚蚓嬉申杆发动铁壁, 虚蚓嬉申杆防御力大幅上升

 虚蚓嬉申杆从迟缓中解除

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

虚蚓嬉申杆发起攻击, 曾搪激归汗回避了攻击

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

 虚蚓嬉申杆从铁壁中解除

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗潜行到虚蚓嬉申杆身后

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆防御, 虚蚓嬉申杆受到38点伤害

 虚蚓嬉申杆被击倒了

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

虚蚓嬉申杆发起攻击, 曾搪激归汗回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用瘟疫, 虚蚓嬉申杆体力减少52%

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆防御, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆使用分身, 出现一个新的虚蚓嬉申杆

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆开始蓄力

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗使用减速术, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆发动铁壁, 虚蚓嬉申杆防御力大幅上升

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

 虚蚓嬉申杆从铁壁中解除

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆开始蓄力

曾搪激归汗发起攻击, 虚蚓嬉申杆受到85点伤害

 虚蚓嬉申杆被击倒了

曾搪激归汗使用瘟疫, 虚蚓嬉申杆体力减少51%

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗使用减速术, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆发起攻击, 曾搪激归汗受到173点伤害

 曾搪激归汗被击倒了

 虚蚓嬉申杆连击, 曾搪激归汗受到215点伤害

 曾搪激归汗被击倒了

曾搪激归汗使用瘟疫, 虚蚓嬉申杆体力减少54%

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用苏生术, 曾搪激归汗复活了, 曾搪激归汗回复体力21点

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用苏生术, 曾搪激归汗复活了, 曾搪激归汗回复体力21点

曾搪激归汗使用减速术, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆防御, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用分身, 出现一个新的曾搪激归汗

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发动背刺, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗使用减速术, 虚蚓嬉申杆进入迟缓状态

曾搪激归汗潜行到虚蚓嬉申杆身后

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆回避了攻击

虚蚓嬉申杆发起攻击, 曾搪激归汗受到0点伤害

 虚蚓嬉申杆从铁壁中解除

曾搪激归汗发起攻击, 虚蚓嬉申杆受到0点伤害

曾搪激归汗发起攻击, 虚蚓嬉申杆受到54点伤害

 虚蚓嬉申杆被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-44 must contain a blank separator between input and trace",
        "sampled case-44 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-44 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-44", &actual_lines, &expected_lines);
}

/// 滚木?
#[test]
fn large_45() {
    const CASE: &str = r####"桃v66wy7tgu27xp@asyncTales
b64d64cfaae1f621@asyncTales

Reku_Mochizuki #494460162188@新纪元
扶灵 /eaKEZPkiLP/@新纪元

seed:第十八届武术大赛附加赛:689-1@!


Reku_Mochizuki发起攻击, b64d64cfaae1f621回避了攻击

扶灵发起攻击, b64d64cfaae1f621受到64点伤害

桃v66wy7tgu27xp使用地裂术

 Reku_Mochizuki受到55点伤害

 扶灵受到42点伤害

 扶灵发起反击, 桃v66wy7tgu27xp受到54点伤害

b64d64cfaae1f621发起攻击, 扶灵受到72点伤害

扶灵使用减速术, 桃v66wy7tgu27xp进入迟缓状态

Reku_Mochizuki使用减速术, b64d64cfaae1f621回避了攻击

b64d64cfaae1f621发起攻击, 扶灵受到91点伤害

Reku_Mochizuki使用减速术, b64d64cfaae1f621进入迟缓状态

桃v66wy7tgu27xp发动铁壁, 桃v66wy7tgu27xp防御力大幅上升

Reku_Mochizuki发起攻击, 桃v66wy7tgu27xp受到1点伤害, 桃v66wy7tgu27xp发动隐匿

扶灵使用分身, 出现一个新的扶灵

b64d64cfaae1f621使用净化, 扶灵受到36点伤害

扶灵发起攻击, b64d64cfaae1f621回避了攻击

Reku_Mochizuki使用分身, 出现一个新的Reku_Mochizuki

桃v66wy7tgu27xp使用净化, Reku_Mochizuki受到89点伤害

 桃v66wy7tgu27xp从迟缓中解除

Reku_Mochizuki发起攻击, b64d64cfaae1f621受到68点伤害

Reku_Mochizuki发起攻击, 桃v66wy7tgu27xp受到1点伤害

扶灵使用分身, 出现一个新的扶灵

桃v66wy7tgu27xp使用地裂术

 扶灵受到21点伤害

 Reku_Mochizuki受到39点伤害

 扶灵受到24点伤害

 扶灵受到32点伤害

 桃v66wy7tgu27xp从铁壁中解除

 扶灵发起反击, 桃v66wy7tgu27xp受到24点伤害

 Reku_Mochizuki发起反击, 桃v66wy7tgu27xp受到45点伤害, 桃v66wy7tgu27xp发动隐匿

扶灵发起攻击, b64d64cfaae1f621回避了攻击

扶灵发起攻击, b64d64cfaae1f621回避了攻击

Reku_Mochizuki使用减速术, b64d64cfaae1f621进入迟缓状态

b64d64cfaae1f621发起攻击, Reku_Mochizuki受到82点伤害

 Reku_Mochizuki被击倒了, Reku_Mochizuki使用护身符抵挡了一次死亡, Reku_Mochizuki回复体力10点

扶灵发起攻击, b64d64cfaae1f621受到30点伤害

桃v66wy7tgu27xp使用地裂术

 扶灵受到21点伤害

 Reku_Mochizuki受到66点伤害

 Reku_Mochizuki被击倒了

 桃v66wy7tgu27xp召唤亡灵, Reku_Mochizuki变成了丧尸

 扶灵受到42点伤害

 扶灵被击倒了

 桃v66wy7tgu27xp召唤亡灵, 扶灵变成了丧尸

 扶灵受到35点伤害

 扶灵被击倒了

 桃v66wy7tgu27xp召唤亡灵, 扶灵变成了丧尸

Reku_Mochizuki使用减速术, 丧尸进入迟缓状态

桃v66wy7tgu27xp使用地裂术

 扶灵受到17点伤害

 扶灵被击倒了, 扶灵使用护身符抵挡了一次死亡, 扶灵回复体力14点

 Reku_Mochizuki回避了攻击

b64d64cfaae1f621发起攻击, 扶灵受到103点伤害

 扶灵被击倒了

丧尸发起攻击, Reku_Mochizuki受到71点伤害

丧尸发起攻击, Reku_Mochizuki回避了攻击

Reku_Mochizuki使用减速术, 丧尸进入迟缓状态

桃v66wy7tgu27xp发起攻击, Reku_Mochizuki受到48点伤害

Reku_Mochizuki发起攻击, 丧尸受到107点伤害

丧尸发起攻击, Reku_Mochizuki回避了攻击

丧尸发起攻击, Reku_Mochizuki受到37点伤害

 Reku_Mochizuki被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-45 must contain a blank separator between input and trace",
        "sampled case-45 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-45 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-45", &actual_lines, &expected_lines);
}

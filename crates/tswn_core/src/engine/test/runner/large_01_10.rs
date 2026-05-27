//! 大型回放测试分片 01-10。
//!
//! 保存由真实/采样输入生成的长回放 fixture，按编号拆分以降低单文件体积并方便定位失败 case。

use super::*;

#[test]
fn large_01() {
    const CASE: &str = r####"「OS」#c1#bFc71OCDuO35@mwh_425
血谣染硫决@Mithril425
锋利ⅤEGZPVQMY@TigerStar425
(S("p{GE2up',7%^UGrP@czr2012425
针刀霜|U/T)h8J"@四象柯425
东乡幻翎#BCBNRCXFX@无惨425
无惨不等式#YMGTFCOPE@星球结晶425
愞㢯老海@昀澤425
Imperio#4B4UZThv@Shabby_fish425
末otW7sfqOze@807139425


血谣染硫决发起攻击, 末otW7sfqOze受到69点伤害

无惨不等式#YMGTFCOPE发起攻击, 东乡幻翎#BCBNRCXFX受到54点伤害

Imperio#4B4UZThv使用魅惑, 锋利ⅤEGZPVQMY被魅惑了

(S("p{GE2up',7%^UGrP发起攻击, 锋利ⅤEGZPVQMY受到54点伤害

针刀霜|U/T)h8J"发起攻击, Imperio#4B4UZThv受到38点伤害

「OS」#c1#bFc71OCDuO35使用魅惑, (S("p{GE2up',7%^UGrP被魅惑了

末otW7sfqOze发起攻击, 东乡幻翎#BCBNRCXFX受到51点伤害

愞㢯老海发起攻击, 针刀霜|U/T)h8J"受到75点伤害

锋利ⅤEGZPVQMY发起攻击, 东乡幻翎#BCBNRCXFX受到107点伤害

 锋利ⅤEGZPVQMY从魅惑中解除

东乡幻翎#BCBNRCXFX发起攻击, (S("p{GE2up',7%^UGrP受到60点伤害

血谣染硫决使用幻术, 召唤出幻影

东乡幻翎#BCBNRCXFX发起攻击, 锋利ⅤEGZPVQMY回避了攻击

末otW7sfqOze使用诅咒, 「OS」#c1#bFc71OCDuO35受到124点伤害, 「OS」#c1#bFc71OCDuO35被诅咒了

无惨不等式#YMGTFCOPE发起攻击, 东乡幻翎#BCBNRCXFX回避了攻击

Imperio#4B4UZThv发起攻击, 无惨不等式#YMGTFCOPE受到38点伤害

愞㢯老海发起攻击, (S("p{GE2up',7%^UGrP受到73点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 血谣染硫决受到87点伤害

锋利ⅤEGZPVQMY发起攻击, 东乡幻翎#BCBNRCXFX受到82点伤害

血谣染硫决使用血祭, 召唤出使魔

针刀霜|U/T)h8J"发起攻击, 末otW7sfqOze受到38点伤害

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE回避了攻击

无惨不等式#YMGTFCOPE发动会心一击, 幻影受到154点伤害

 幻影消失了

东乡幻翎#BCBNRCXFX发起攻击, 针刀霜|U/T)h8J"受到77点伤害

(S("p{GE2up',7%^UGrP发起攻击, 东乡幻翎#BCBNRCXFX受到36点伤害

 东乡幻翎#BCBNRCXFX被击倒了

 (S("p{GE2up',7%^UGrP从魅惑中解除

使魔发起攻击, 针刀霜|U/T)h8J"受到43点伤害

Imperio#4B4UZThv发起攻击, 诅咒使伤害加倍, 「OS」#c1#bFc71OCDuO35受到130点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

愞㢯老海发起攻击, 血谣染硫决受到41点伤害

 血谣染硫决发起反击, 愞㢯老海受到55点伤害

锋利ⅤEGZPVQMY发起攻击, (S("p{GE2up',7%^UGrP受到107点伤害

血谣染硫决发起攻击, (S("p{GE2up',7%^UGrP受到107点伤害

 (S("p{GE2up',7%^UGrP被击倒了

使魔发起攻击, 无惨不等式#YMGTFCOPE受到25点伤害

Imperio#4B4UZThv使用分身, 出现一个新的Imperio#4B4UZThv

无惨不等式#YMGTFCOPE发起攻击, Imperio#4B4UZThv受到88点伤害

末otW7sfqOze发起攻击, 针刀霜|U/T)h8J"受到101点伤害

针刀霜|U/T)h8J"发起攻击, 锋利ⅤEGZPVQMY受到83点伤害

使魔发起攻击, Imperio#4B4UZThv受到76点伤害

愞㢯老海发起攻击, 锋利ⅤEGZPVQMY回避了攻击

Imperio#4B4UZThv发起攻击, 愞㢯老海受到72点伤害

锋利ⅤEGZPVQMY发起攻击, 末otW7sfqOze受到51点伤害

血谣染硫决发起攻击, 锋利ⅤEGZPVQMY受到77点伤害

末otW7sfqOze发起攻击, 锋利ⅤEGZPVQMY受到50点伤害

Imperio#4B4UZThv发起攻击, 愞㢯老海受到61点伤害

 Imperio#4B4UZThv连击, 针刀霜|U/T)h8J"防御, 针刀霜|U/T)h8J"受到40点伤害

 针刀霜|U/T)h8J"被击倒了

愞㢯老海发起攻击, 血谣染硫决受到32点伤害

无惨不等式#YMGTFCOPE发动会心一击, 使魔受到126点伤害, 血谣染硫决受到63点伤害

 血谣染硫决被击倒了

 使魔消失了

Imperio#4B4UZThv使用魅惑, 无惨不等式#YMGTFCOPE被魅惑了

锋利ⅤEGZPVQMY发起攻击, Imperio#4B4UZThv受到74点伤害

 Imperio#4B4UZThv被击倒了

无惨不等式#YMGTFCOPE发动会心一击, 末otW7sfqOze受到99点伤害

 无惨不等式#YMGTFCOPE从魅惑中解除

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE受到35点伤害

Imperio#4B4UZThv发起攻击, 愞㢯老海受到105点伤害

 愞㢯老海被击倒了

锋利ⅤEGZPVQMY发起攻击, 无惨不等式#YMGTFCOPE防御, 无惨不等式#YMGTFCOPE受到23点伤害

无惨不等式#YMGTFCOPE发起攻击, Imperio#4B4UZThv受到51点伤害

 Imperio#4B4UZThv被击倒了

末otW7sfqOze发起攻击, 无惨不等式#YMGTFCOPE受到47点伤害

无惨不等式#YMGTFCOPE发起攻击, 末otW7sfqOze使用伤害反弹, 无惨不等式#YMGTFCOPE受到44点伤害

锋利ⅤEGZPVQMY发起攻击, 末otW7sfqOze受到103点伤害

 末otW7sfqOze被击倒了

无惨不等式#YMGTFCOPE使用冰冻术, 锋利ⅤEGZPVQMY受到40点伤害, 锋利ⅤEGZPVQMY被冰冻了

无惨不等式#YMGTFCOPE发起攻击, 锋利ⅤEGZPVQMY受到134点伤害

 锋利ⅤEGZPVQMY被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-01 must contain a blank separator between input and trace",
        "sampled case-01 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4830, "large_01 score mismatch");

    assert!(guard < 20_000, "sampled case-01 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-01", &actual_lines, &expected_lines);
}

#[test]
fn large_02() {
    const CASE: &str = r####"「OS」#H1#YoRmfG4zW9@mwh_425
「OS」#c1#E7WGTekQTugF@mwh_425
RedOT<{f2=v}67w@流浪冒险者425
mVf4YCPDlRm@tyakasha425
十六夜咲夜zgJ6eH3TkLFp@芒萁425
Sayakagh8yaICYo@candle425
我力7#W2ib8D@仙蛊屋425
SDPC#AZLZJQUPN@星球结晶425
稗田阿求OQL68NN8@Squall425
跙坥咀诅阻珇伹伹怚@涵虚425


RedOT<{f2=v}67w发起攻击, 「OS」#H1#YoRmfG4zW9回避了攻击

稗田阿求OQL68NN8发起攻击, Sayakagh8yaICYo受到27点伤害

Sayakagh8yaICYo使用幻术, 召唤出幻影

我力7#W2ib8D发动会心一击, RedOT<{f2=v}67w受到154点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, SDPC#AZLZJQUPN回避了攻击

跙坥咀诅阻珇伹伹怚发起攻击, 稗田阿求OQL68NN8受到44点伤害

mVf4YCPDlRm发起攻击, SDPC#AZLZJQUPN受到88点伤害

SDPC#AZLZJQUPN发起攻击, Sayakagh8yaICYo受到47点伤害

「OS」#c1#E7WGTekQTugF发起攻击, Sayakagh8yaICYo受到79点伤害

RedOT<{f2=v}67w发起攻击, 「OS」#c1#E7WGTekQTugF回避了攻击

稗田阿求OQL68NN8发起攻击, 「OS」#c1#E7WGTekQTugF受到76点伤害

「OS」#H1#YoRmfG4zW9发起攻击, 「OS」#c1#E7WGTekQTugF受到80点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, 稗田阿求OQL68NN8受到56点伤害

Sayakagh8yaICYo发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到52点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 我力7#W2ib8D受到65点伤害

mVf4YCPDlRm发起攻击, 幻影受到73点伤害

SDPC#AZLZJQUPN发起攻击, 稗田阿求OQL68NN8受到138点伤害

稗田阿求OQL68NN8发起攻击, Sayakagh8yaICYo受到74点伤害

我力7#W2ib8D发起攻击, Sayakagh8yaICYo受到71点伤害

 Sayakagh8yaICYo被击倒了

 幻影消失了

 我力7#W2ib8D吞噬了Sayakagh8yaICYo, 我力7#W2ib8D属性上升

我力7#W2ib8D投毒, mVf4YCPDlRm受到77点伤害, mVf4YCPDlRm中毒

RedOT<{f2=v}67w使用净化, 「OS」#c1#E7WGTekQTugF受到47点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 「OS」#H1#YoRmfG4zW9受到46点伤害

十六夜咲夜zgJ6eH3TkLFp发起攻击, RedOT<{f2=v}67w受到32点伤害

「OS」#H1#YoRmfG4zW9发起攻击, RedOT<{f2=v}67w受到134点伤害

 RedOT<{f2=v}67w被击倒了

跙坥咀诅阻珇伹伹怚使用魅惑, 十六夜咲夜zgJ6eH3TkLFp被魅惑了

稗田阿求OQL68NN8发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到103点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 我力7#W2ib8D受到24点伤害

我力7#W2ib8D发动会心一击, 跙坥咀诅阻珇伹伹怚受到166点伤害

SDPC#AZLZJQUPN发起攻击, 跙坥咀诅阻珇伹伹怚受到52点伤害

mVf4YCPDlRm发起攻击, SDPC#AZLZJQUPN受到37点伤害

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到36点伤害

「OS」#c1#E7WGTekQTugF发起吸血攻击, SDPC#AZLZJQUPN受到132点伤害, 「OS」#c1#E7WGTekQTugF回复体力66点

稗田阿求OQL68NN8使用幻术, 召唤出幻影

十六夜咲夜zgJ6eH3TkLFp发起攻击, 「OS」#H1#YoRmfG4zW9受到34点伤害

 十六夜咲夜zgJ6eH3TkLFp从魅惑中解除

SDPC#AZLZJQUPN发起攻击, 我力7#W2ib8D受到54点伤害

跙坥咀诅阻珇伹伹怚发起吸血攻击, 「OS」#H1#YoRmfG4zW9受到128点伤害, 跙坥咀诅阻珇伹伹怚回复体力64点

我力7#W2ib8D发动会心一击, 跙坥咀诅阻珇伹伹怚受到128点伤害

「OS」#H1#YoRmfG4zW9使用减速术, 幻影进入迟缓状态

SDPC#AZLZJQUPN潜行到mVf4YCPDlRm身后

跙坥咀诅阻珇伹伹怚发起攻击, 幻影受到43点伤害

mVf4YCPDlRm发起攻击, 十六夜咲夜zgJ6eH3TkLFp防御, 十六夜咲夜zgJ6eH3TkLFp受到66点伤害

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到30点伤害

稗田阿求OQL68NN8发起攻击, 我力7#W2ib8D受到36点伤害

「OS」#c1#E7WGTekQTugF使用冰冻术, 我力7#W2ib8D受到17点伤害, 我力7#W2ib8D被冰冻了

十六夜咲夜zgJ6eH3TkLFp发起攻击, SDPC#AZLZJQUPN受到28点伤害

 SDPC#AZLZJQUPN的潜行被识破

SDPC#AZLZJQUPN发起攻击, 幻影受到51点伤害

 SDPC#AZLZJQUPN连击, 幻影受到29点伤害

 SDPC#AZLZJQUPN连击, 幻影受到29点伤害

「OS」#H1#YoRmfG4zW9发起攻击, 幻影受到81点伤害

 幻影消失了

稗田阿求OQL68NN8发起攻击, 「OS」#c1#E7WGTekQTugF受到110点伤害

 「OS」#c1#E7WGTekQTugF做出垂死抗争, 「OS」#c1#E7WGTekQTugF所有属性上升

我力7#W2ib8D从冰冻中解除

跙坥咀诅阻珇伹伹怚使用魅惑, SDPC#AZLZJQUPN被魅惑了

「OS」#c1#E7WGTekQTugF发起攻击, mVf4YCPDlRm受到54点伤害

SDPC#AZLZJQUPN发起攻击, 「OS」#c1#E7WGTekQTugF受到29点伤害

 「OS」#c1#E7WGTekQTugF被击倒了

 SDPC#AZLZJQUPN连击, 「OS」#H1#YoRmfG4zW9受到37点伤害

 SDPC#AZLZJQUPN从魅惑中解除

我力7#W2ib8D发起攻击, 稗田阿求OQL68NN8受到78点伤害

 稗田阿求OQL68NN8被击倒了

mVf4YCPDlRm发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到96点伤害

 十六夜咲夜zgJ6eH3TkLFp被击倒了

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到25点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 我力7#W2ib8D受到53点伤害

 我力7#W2ib8D被击倒了

SDPC#AZLZJQUPN发起攻击, 「OS」#H1#YoRmfG4zW9受到66点伤害

 「OS」#H1#YoRmfG4zW9被击倒了

mVf4YCPDlRm发起攻击, SDPC#AZLZJQUPN受到38点伤害

 SDPC#AZLZJQUPN被击倒了

 mVf4YCPDlRm毒性发作, mVf4YCPDlRm受到20点伤害

 mVf4YCPDlRm被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-02 must contain a blank separator between input and trace",
        "sampled case-02 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 10_000, true);
    assert_eq!(total_score, 4824, "large_02 score mismatch");

    assert!(guard < 20_000, "sampled case-02 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-02", &actual_lines, &expected_lines);
}

#[test]
fn large_03() {
    const CASE: &str = r####"#念-GP8LKM21D4JZ@柚子不是油渍425
Wakaba_mutsumi#pjFhEhSbjy@🥒425
Tachibana_akira#BydbIMidbs@🥒425
十六夜咲夜zgJ6eH3TkLFp@芒萁425
MeltelabRC3P3Go7@RbCl425
三田一重TxtrdTN4l8nT@fx425
SDPC#AZLZJQUPN@星球结晶425
七七#EUEMIGPI@暗黑突击425
Hypochondriac#TtwN3jZ@Unbound425
跙坥咀诅阻珇伹伹怚@涵虚425


三田一重TxtrdTN4l8nT发起攻击, SDPC#AZLZJQUPN受到93点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 七七#EUEMIGPI回避了攻击

跙坥咀诅阻珇伹伹怚发起攻击, 七七#EUEMIGPI受到114点伤害

SDPC#AZLZJQUPN发起攻击, #念-GP8LKM21D4JZ受到86点伤害

 SDPC#AZLZJQUPN连击, Wakaba_mutsumi#pjFhEhSbjy受到40点伤害

七七#EUEMIGPI发起攻击, #念-GP8LKM21D4JZ受到70点伤害

Tachibana_akira#BydbIMidbs发起攻击, 跙坥咀诅阻珇伹伹怚受到109点伤害

#念-GP8LKM21D4JZ发起攻击, 十六夜咲夜zgJ6eH3TkLFp回避了攻击

MeltelabRC3P3Go7发起攻击, Tachibana_akira#BydbIMidbs受到110点伤害

Hypochondriac#TtwN3jZ使用魅惑, MeltelabRC3P3Go7被魅惑了

十六夜咲夜zgJ6eH3TkLFp发起攻击, MeltelabRC3P3Go7受到81点伤害

Tachibana_akira#BydbIMidbs发起攻击, SDPC#AZLZJQUPN受到58点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 三田一重TxtrdTN4l8nT受到115点伤害

七七#EUEMIGPI使用净化, MeltelabRC3P3Go7受到105点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, 七七#EUEMIGPI受到100点伤害

#念-GP8LKM21D4JZ发起攻击, MeltelabRC3P3Go7受到67点伤害

MeltelabRC3P3Go7发起攻击, 三田一重TxtrdTN4l8nT受到152点伤害

 MeltelabRC3P3Go7从魅惑中解除

三田一重TxtrdTN4l8nT发起攻击, Hypochondriac#TtwN3jZ回避了攻击

Hypochondriac#TtwN3jZ发起攻击, 跙坥咀诅阻珇伹伹怚受到42点伤害

SDPC#AZLZJQUPN发起攻击, 七七#EUEMIGPI受到86点伤害

十六夜咲夜zgJ6eH3TkLFp使用地裂术

 Wakaba_mutsumi#pjFhEhSbjy受到42点伤害

 三田一重TxtrdTN4l8nT受到15点伤害

 MeltelabRC3P3Go7受到20点伤害

 跙坥咀诅阻珇伹伹怚受到37点伤害

 SDPC#AZLZJQUPN受到30点伤害

Tachibana_akira#BydbIMidbs使用狂暴术, 十六夜咲夜zgJ6eH3TkLFp受到43点伤害, 十六夜咲夜zgJ6eH3TkLFp进入狂暴状态

MeltelabRC3P3Go7发起攻击, 三田一重TxtrdTN4l8nT受到119点伤害

 三田一重TxtrdTN4l8nT被击倒了

Hypochondriac#TtwN3jZ发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到131点伤害

#念-GP8LKM21D4JZ发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到61点伤害

跙坥咀诅阻珇伹伹怚发起攻击, #念-GP8LKM21D4JZ受到106点伤害

MeltelabRC3P3Go7发起攻击, SDPC#AZLZJQUPN受到32点伤害

SDPC#AZLZJQUPN发起攻击, 十六夜咲夜zgJ6eH3TkLFp防御, 十六夜咲夜zgJ6eH3TkLFp受到85点伤害

七七#EUEMIGPI发起攻击, 跙坥咀诅阻珇伹伹怚受到64点伤害

Wakaba_mutsumi#pjFhEhSbjy发起攻击, Hypochondriac#TtwN3jZ受到119点伤害

Hypochondriac#TtwN3jZ发动铁壁, Hypochondriac#TtwN3jZ防御力大幅上升

跙坥咀诅阻珇伹伹怚使用魅惑, 七七#EUEMIGPI回避了攻击

十六夜咲夜zgJ6eH3TkLFp发起狂暴攻击, #念-GP8LKM21D4JZ受到73点伤害

 #念-GP8LKM21D4JZ被击倒了

 十六夜咲夜zgJ6eH3TkLFp从狂暴中解除

七七#EUEMIGPI使用狂暴术, Hypochondriac#TtwN3jZ受到1点伤害, Hypochondriac#TtwN3jZ进入狂暴状态

MeltelabRC3P3Go7发起攻击, 十六夜咲夜zgJ6eH3TkLFp受到117点伤害

 十六夜咲夜zgJ6eH3TkLFp被击倒了

Tachibana_akira#BydbIMidbs发起攻击, Hypochondriac#TtwN3jZ受到1点伤害

SDPC#AZLZJQUPN发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到88点伤害

Hypochondriac#TtwN3jZ发起狂暴攻击, Tachibana_akira#BydbIMidbs受到97点伤害

 Hypochondriac#TtwN3jZ从狂暴中解除

七七#EUEMIGPI发起攻击, Hypochondriac#TtwN3jZ受到1点伤害

MeltelabRC3P3Go7潜行到Hypochondriac#TtwN3jZ身后

跙坥咀诅阻珇伹伹怚发起吸血攻击, Hypochondriac#TtwN3jZ回避了攻击

Wakaba_mutsumi#pjFhEhSbjy使用诅咒, Hypochondriac#TtwN3jZ受到1点伤害, Hypochondriac#TtwN3jZ被诅咒了

Tachibana_akira#BydbIMidbs发起攻击, 跙坥咀诅阻珇伹伹怚受到92点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

SDPC#AZLZJQUPN发起攻击, Wakaba_mutsumi#pjFhEhSbjy受到92点伤害

 Wakaba_mutsumi#pjFhEhSbjy被击倒了

Hypochondriac#TtwN3jZ发起攻击, SDPC#AZLZJQUPN受到79点伤害

 Hypochondriac#TtwN3jZ从铁壁中解除

MeltelabRC3P3Go7发动背刺, Hypochondriac#TtwN3jZ受到325点伤害

 Hypochondriac#TtwN3jZ被击倒了

七七#EUEMIGPI发起攻击, Tachibana_akira#BydbIMidbs受到82点伤害

 Tachibana_akira#BydbIMidbs被击倒了

SDPC#AZLZJQUPN发起攻击, MeltelabRC3P3Go7受到37点伤害

 MeltelabRC3P3Go7被击倒了

 SDPC#AZLZJQUPN连击, 七七#EUEMIGPI受到53点伤害

 七七#EUEMIGPI被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-03 must contain a blank separator between input and trace",
        "sampled case-03 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4452, "large_03 score mismatch");

    assert!(guard < 20_000, "sampled case-03 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-03", &actual_lines, &expected_lines);
}

#[test]
fn large_04() {
    const CASE: &str = r####"沉睡在悲伤的海洋中#056ARx3e@爱425
「OS」#c1#E7WGTekQTugF@mwh_425
RedOT<{f2=v}67w@流浪冒险者425
血谣染硫决@Mithril425
MeltelabRC3P3Go7@RbCl425
10l-DYWg@Hell425
(S("p{GE2up',7%^UGrP@czr2012425
东乡幻翎#BCBNRCXFX@无惨425
Hypochondriac#TtwN3jZ@Unbound425
seed:第十八届武术大赛抽签:425-0@!


Hypochondriac#TtwN3jZ发起攻击, 沉睡在悲伤的海洋中#056ARx3e受到50点伤害

东乡幻翎#BCBNRCXFX使用火球术, 血谣染硫决受到66点伤害

RedOT<{f2=v}67w发起攻击, 东乡幻翎#BCBNRCXFX受到69点伤害

血谣染硫决使用血祭, 召唤出使魔

(S("p{GE2up',7%^UGrP发起攻击, 使魔回避了攻击

10l-DYWg发起攻击, Hypochondriac#TtwN3jZ受到123点伤害

MeltelabRC3P3Go7潜行到沉睡在悲伤的海洋中#056ARx3e身后

「OS」#c1#E7WGTekQTugF使用幻术, 召唤出幻影

Hypochondriac#TtwN3jZ发起攻击, RedOT<{f2=v}67w受到55点伤害

沉睡在悲伤的海洋中#056ARx3e发起攻击, Hypochondriac#TtwN3jZ受到54点伤害

使魔发起攻击, Hypochondriac#TtwN3jZ受到67点伤害

10l-DYWg发起攻击, 「OS」#c1#E7WGTekQTugF受到108点伤害

(S("p{GE2up',7%^UGrP发动会心一击, 血谣染硫决受到104点伤害

东乡幻翎#BCBNRCXFX发起攻击, 10l-DYWg受到86点伤害

RedOT<{f2=v}67w使用魅惑, Hypochondriac#TtwN3jZ回避了攻击

「OS」#c1#E7WGTekQTugF使用治愈魔法, 「OS」#c1#E7WGTekQTugF回复体力108点

血谣染硫决发起攻击, (S("p{GE2up',7%^UGrP受到40点伤害

使魔发起攻击, 东乡幻翎#BCBNRCXFX受到47点伤害

MeltelabRC3P3Go7发动背刺, 沉睡在悲伤的海洋中#056ARx3e受到346点伤害

 沉睡在悲伤的海洋中#056ARx3e被击倒了

10l-DYWg发起攻击, (S("p{GE2up',7%^UGrP受到72点伤害

Hypochondriac#TtwN3jZ发起攻击, MeltelabRC3P3Go7受到60点伤害

使魔发起攻击, 幻影受到79点伤害

血谣染硫决发动铁壁, 血谣染硫决防御力大幅上升

RedOT<{f2=v}67w发起攻击, MeltelabRC3P3Go7受到87点伤害

东乡幻翎#BCBNRCXFX发起攻击, (S("p{GE2up',7%^UGrP受到73点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 10l-DYWg受到55点伤害

使魔发起攻击, (S("p{GE2up',7%^UGrP受到80点伤害

幻影发起攻击, MeltelabRC3P3Go7受到101点伤害

MeltelabRC3P3Go7发起攻击, 「OS」#c1#E7WGTekQTugF受到38点伤害

(S("p{GE2up',7%^UGrP发起攻击, 血谣染硫决受到1点伤害

10l-DYWg发起攻击, 东乡幻翎#BCBNRCXFX受到63点伤害

Hypochondriac#TtwN3jZ发起攻击, (S("p{GE2up',7%^UGrP受到58点伤害

 (S("p{GE2up',7%^UGrP被击倒了

血谣染硫决发起攻击, RedOT<{f2=v}67w受到52点伤害

东乡幻翎#BCBNRCXFX发起攻击, MeltelabRC3P3Go7受到87点伤害

 MeltelabRC3P3Go7被击倒了

RedOT<{f2=v}67w发起攻击, 血谣染硫决受到1点伤害

 血谣染硫决发起反击, RedOT<{f2=v}67w受到128点伤害

使魔发起攻击, 「OS」#c1#E7WGTekQTugF回避了攻击

幻影发起攻击, RedOT<{f2=v}67w受到29点伤害

 RedOT<{f2=v}67w被击倒了

「OS」#c1#E7WGTekQTugF使用冰冻术, 10l-DYWg回避了攻击

10l-DYWg发起攻击, 幻影受到88点伤害

 幻影消失了

Hypochondriac#TtwN3jZ使用魅惑, 使魔被魅惑了

东乡幻翎#BCBNRCXFX发起攻击, 使魔受到111点伤害, 血谣染硫决受到55点伤害

 血谣染硫决被击倒了

 使魔消失了

10l-DYWg发起攻击, 「OS」#c1#E7WGTekQTugF受到70点伤害

Hypochondriac#TtwN3jZ发起攻击, 10l-DYWg受到100点伤害

「OS」#c1#E7WGTekQTugF使用冰冻术, 东乡幻翎#BCBNRCXFX回避了攻击

东乡幻翎#BCBNRCXFX发起攻击, 「OS」#c1#E7WGTekQTugF受到82点伤害

Hypochondriac#TtwN3jZ发起攻击, 东乡幻翎#BCBNRCXFX受到58点伤害

10l-DYWg发起攻击, 东乡幻翎#BCBNRCXFX受到69点伤害

「OS」#c1#E7WGTekQTugF发起攻击, 10l-DYWg受到88点伤害

 10l-DYWg被击倒了

Hypochondriac#TtwN3jZ发起攻击, 东乡幻翎#BCBNRCXFX受到122点伤害

 东乡幻翎#BCBNRCXFX被击倒了

「OS」#c1#E7WGTekQTugF使用净化, Hypochondriac#TtwN3jZ受到62点伤害

 Hypochondriac#TtwN3jZ被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-04 must contain a blank separator between input and trace",
        "sampled case-04 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4221, "large_04 score mismatch");

    assert!(guard < 20_000, "sampled case-04 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-04", &actual_lines, &expected_lines);
}

#[test]
fn large_05() {
    const CASE: &str = r####"「OS」#c1#bFc71OCDuO35@mwh_425
GordonALYJDXORPTER@nan425
"铁胆"哈拉文领主-ksbGnquBbq-@新纪元425
冥河WyO8MUZPPtKH@Afterglow425
tCtrVweRgshV@Afterglow425
湖心SHVPEMAPV@TigerStar425
地气14#emOKVY@仙蛊屋425
SDPC#AZLZJQUPN@星球结晶425
缇亚卡#WOVLHAESD@星球结晶425
直接命中#Dfdt3d2uT@Shabby_fish425


SDPC#AZLZJQUPN发起攻击, 「OS」#c1#bFc71OCDuO35受到67点伤害

 SDPC#AZLZJQUPN连击, 「OS」#c1#bFc71OCDuO35受到31点伤害

 「OS」#c1#bFc71OCDuO35发起反击, SDPC#AZLZJQUPN受到74点伤害

GordonALYJDXORPTER发起攻击, 冥河WyO8MUZPPtKH受到16点伤害

湖心SHVPEMAPV发起攻击, 地气14#emOKVY回避了攻击

直接命中#Dfdt3d2uT发起攻击, 「OS」#c1#bFc71OCDuO35受到67点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 缇亚卡#WOVLHAESD受到97点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 地气14#emOKVY受到144点伤害

tCtrVweRgshV发起攻击, GordonALYJDXORPTER受到62点伤害

缇亚卡#WOVLHAESD发起攻击, 直接命中#Dfdt3d2uT受到114点伤害

冥河WyO8MUZPPtKH发起攻击, 直接命中#Dfdt3d2uT受到37点伤害

地气14#emOKVY发起攻击, 冥河WyO8MUZPPtKH受到60点伤害

湖心SHVPEMAPV发起攻击, 缇亚卡#WOVLHAESD受到54点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 地气14#emOKVY受到30点伤害

GordonALYJDXORPTER发起攻击, tCtrVweRgshV受到17点伤害

tCtrVweRgshV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到113点伤害

SDPC#AZLZJQUPN发起攻击, 地气14#emOKVY受到116点伤害

 地气14#emOKVY被击倒了

「OS」#c1#bFc71OCDuO35发起攻击, GordonALYJDXORPTER受到88点伤害

缇亚卡#WOVLHAESD发起攻击, 湖心SHVPEMAPV受到117点伤害

直接命中#Dfdt3d2uT发起攻击, 冥河WyO8MUZPPtKH受到42点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 缇亚卡#WOVLHAESD受到24点伤害

SDPC#AZLZJQUPN发起攻击, 冥河WyO8MUZPPtKH受到77点伤害

冥河WyO8MUZPPtKH发起攻击, 湖心SHVPEMAPV回避了攻击

湖心SHVPEMAPV发起攻击, SDPC#AZLZJQUPN回避了攻击

GordonALYJDXORPTER发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到51点伤害

「OS」#c1#bFc71OCDuO35发起攻击, SDPC#AZLZJQUPN受到73点伤害

缇亚卡#WOVLHAESD使用减速术, tCtrVweRgshV回避了攻击

SDPC#AZLZJQUPN发起攻击, 湖心SHVPEMAPV受到65点伤害

tCtrVweRgshV发起攻击, 直接命中#Dfdt3d2uT受到86点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, 直接命中#Dfdt3d2uT受到106点伤害

 直接命中#Dfdt3d2uT被击倒了

冥河WyO8MUZPPtKH开始蓄力

缇亚卡#WOVLHAESD发起攻击, SDPC#AZLZJQUPN受到57点伤害

tCtrVweRgshV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到94点伤害

GordonALYJDXORPTER发起攻击, 湖心SHVPEMAPV受到62点伤害

「OS」#c1#bFc71OCDuO35发起攻击, GordonALYJDXORPTER受到100点伤害

湖心SHVPEMAPV发起攻击, 冥河WyO8MUZPPtKH受到94点伤害

SDPC#AZLZJQUPN开始聚气, SDPC#AZLZJQUPN攻击力上升

冥河WyO8MUZPPtKH发起攻击, 「OS」#c1#bFc71OCDuO35受到305点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

缇亚卡#WOVLHAESD使用冰冻术, 湖心SHVPEMAPV受到32点伤害

 湖心SHVPEMAPV被击倒了

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, GordonALYJDXORPTER受到44点伤害

 GordonALYJDXORPTER被击倒了

tCtrVweRgshV使用净化, 缇亚卡#WOVLHAESD受到92点伤害

SDPC#AZLZJQUPN发起攻击, 缇亚卡#WOVLHAESD受到124点伤害

 缇亚卡#WOVLHAESD被击倒了

冥河WyO8MUZPPtKH发起攻击, tCtrVweRgshV受到74点伤害

"铁胆"哈拉文领主-ksbGnquBbq-发起攻击, tCtrVweRgshV受到43点伤害

 "铁胆"哈拉文领主-ksbGnquBbq-连击, tCtrVweRgshV受到42点伤害

冥河WyO8MUZPPtKH使用魅惑, tCtrVweRgshV被魅惑了

tCtrVweRgshV发起攻击, "铁胆"哈拉文领主-ksbGnquBbq-受到90点伤害

 "铁胆"哈拉文领主-ksbGnquBbq-被击倒了

 tCtrVweRgshV从魅惑中解除

SDPC#AZLZJQUPN发起攻击, tCtrVweRgshV受到233点伤害

 tCtrVweRgshV被击倒了

SDPC#AZLZJQUPN发起攻击, 冥河WyO8MUZPPtKH受到108点伤害

 冥河WyO8MUZPPtKH被击倒了, 冥河WyO8MUZPPtKH使用护身符抵挡了一次死亡, 冥河WyO8MUZPPtKH回复体力8点

冥河WyO8MUZPPtKH使用魅惑, SDPC#AZLZJQUPN回避了攻击

SDPC#AZLZJQUPN发起攻击, 冥河WyO8MUZPPtKH受到68点伤害

 冥河WyO8MUZPPtKH被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-05 must contain a blank separator between input and trace",
        "sampled case-05 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4319, "large_05 score mismatch");

    assert!(guard < 20_000, "sampled case-05 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-05", &actual_lines, &expected_lines);
}

#[test]
fn large_06() {
    const CASE: &str = r####"都江堰00217109183087@abruce425
血谣染硫决@Mithril425
Straight_into_the_lights#VpdbCrcFJV@🥒425
仇决clFJZCMHS@candle425
1^GNC.%F@Hell425
前尘如梦UYGMHRNX@LuoTianyi425
我力7#W2ib8D@仙蛊屋425
东乡幻翎#BCBNRCXFX@无惨425
咲夜bJjbFYez@Squall425
Tik_Tok#IBxWzGZtr@Shabby_fish425


Straight_into_the_lights#VpdbCrcFJV发起攻击, 都江堰00217109183087受到77点伤害

咲夜bJjbFYez使用血祭, 召唤出使魔

1^GNC.%F发起攻击, 东乡幻翎#BCBNRCXFX受到81点伤害

都江堰00217109183087发动会心一击, 咲夜bJjbFYez受到131点伤害

前尘如梦UYGMHRNX发起攻击, 东乡幻翎#BCBNRCXFX受到43点伤害

Tik_Tok#IBxWzGZtr发起攻击, 我力7#W2ib8D受到81点伤害

东乡幻翎#BCBNRCXFX发起攻击, 前尘如梦UYGMHRNX受到110点伤害

血谣染硫决发起攻击, Tik_Tok#IBxWzGZtr受到114点伤害

我力7#W2ib8D发起攻击, Straight_into_the_lights#VpdbCrcFJV回避了攻击

使魔发起攻击, Straight_into_the_lights#VpdbCrcFJV受到21点伤害

Tik_Tok#IBxWzGZtr发起攻击, 前尘如梦UYGMHRNX受到73点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, Tik_Tok#IBxWzGZtr受到67点伤害

咲夜bJjbFYez发起攻击, 1^GNC.%F受到85点伤害

仇决clFJZCMHS发起攻击, 1^GNC.%F受到62点伤害

血谣染硫决发起攻击, 仇决clFJZCMHS受到116点伤害

前尘如梦UYGMHRNX发起攻击, 都江堰00217109183087受到69点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 仇决clFJZCMHS受到88点伤害

1^GNC.%F发起攻击, Tik_Tok#IBxWzGZtr受到35点伤害

都江堰00217109183087发起攻击, 血谣染硫决受到96点伤害

Tik_Tok#IBxWzGZtr发起攻击, 1^GNC.%F受到73点伤害

我力7#W2ib8D发起攻击, 东乡幻翎#BCBNRCXFX受到69点伤害

东乡幻翎#BCBNRCXFX使用冰冻术, 前尘如梦UYGMHRNX受到50点伤害, 前尘如梦UYGMHRNX被冰冻了

咲夜bJjbFYez使用净化, Straight_into_the_lights#VpdbCrcFJV受到25点伤害

使魔发起攻击, 都江堰00217109183087受到63点伤害

仇决clFJZCMHS发起攻击, 都江堰00217109183087回避了攻击

我力7#W2ib8D发起攻击, Straight_into_the_lights#VpdbCrcFJV受到53点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 1^GNC.%F受到168点伤害

 1^GNC.%F被击倒了

血谣染硫决发动铁壁, 血谣染硫决防御力大幅上升

使魔使用自爆, 我力7#W2ib8D受到119点伤害

 使魔消失了

咲夜bJjbFYez使用治愈魔法, 咲夜bJjbFYez回复体力85点

仇决clFJZCMHS发起攻击, Tik_Tok#IBxWzGZtr受到40点伤害

Tik_Tok#IBxWzGZtr发起攻击, 血谣染硫决受到1点伤害

都江堰00217109183087发起攻击, 仇决clFJZCMHS回避了攻击

咲夜bJjbFYez发起攻击, Straight_into_the_lights#VpdbCrcFJV受到40点伤害

 咲夜bJjbFYez连击, 都江堰00217109183087受到48点伤害

 咲夜bJjbFYez连击, Straight_into_the_lights#VpdbCrcFJV受到36点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 我力7#W2ib8D受到65点伤害

 我力7#W2ib8D被击倒了

前尘如梦UYGMHRNX从冰冻中解除

东乡幻翎#BCBNRCXFX使用冰冻术, Straight_into_the_lights#VpdbCrcFJV回避了攻击

前尘如梦UYGMHRNX发起攻击, 咲夜bJjbFYez受到31点伤害

Tik_Tok#IBxWzGZtr发起攻击, 咲夜bJjbFYez受到69点伤害

咲夜bJjbFYez发起攻击, 都江堰00217109183087回避了攻击

仇决clFJZCMHS发起攻击, 血谣染硫决受到1点伤害

Straight_into_the_lights#VpdbCrcFJV发起攻击, 咲夜bJjbFYez受到64点伤害

血谣染硫决发起攻击, 仇决clFJZCMHS回避了攻击

东乡幻翎#BCBNRCXFX发起攻击, Straight_into_the_lights#VpdbCrcFJV受到36点伤害

Tik_Tok#IBxWzGZtr发起攻击, 血谣染硫决受到1点伤害

 血谣染硫决发起反击, Tik_Tok#IBxWzGZtr防御, Tik_Tok#IBxWzGZtr受到29点伤害

前尘如梦UYGMHRNX使用生命之轮, 东乡幻翎#BCBNRCXFX的体力值与前尘如梦UYGMHRNX互换

咲夜bJjbFYez发起攻击, 前尘如梦UYGMHRNX受到80点伤害

都江堰00217109183087发起攻击, Straight_into_the_lights#VpdbCrcFJV受到99点伤害

 Straight_into_the_lights#VpdbCrcFJV被击倒了

仇决clFJZCMHS发起攻击, 血谣染硫决受到1点伤害

血谣染硫决发起攻击, 都江堰00217109183087受到18点伤害

 血谣染硫决从铁壁中解除

Tik_Tok#IBxWzGZtr发起攻击, 血谣染硫决防御, 血谣染硫决受到71点伤害

东乡幻翎#BCBNRCXFX发起攻击, 血谣染硫决受到70点伤害

 血谣染硫决被击倒了

咲夜bJjbFYez发起攻击, 仇决clFJZCMHS受到61点伤害

都江堰00217109183087发起攻击, 东乡幻翎#BCBNRCXFX受到30点伤害

东乡幻翎#BCBNRCXFX发起攻击, 咲夜bJjbFYez受到80点伤害

 咲夜bJjbFYez被击倒了

前尘如梦UYGMHRNX发起攻击, 东乡幻翎#BCBNRCXFX受到99点伤害

 东乡幻翎#BCBNRCXFX被击倒了

仇决clFJZCMHS发起攻击, Tik_Tok#IBxWzGZtr受到60点伤害

 Tik_Tok#IBxWzGZtr被击倒了

都江堰00217109183087使用火球术, 仇决clFJZCMHS受到156点伤害

 仇决clFJZCMHS被击倒了

前尘如梦UYGMHRNX发起攻击, 都江堰00217109183087受到44点伤害

都江堰00217109183087使用地裂术

 前尘如梦UYGMHRNX受到103点伤害

 前尘如梦UYGMHRNX被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-06 must contain a blank separator between input and trace",
        "sampled case-06 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4456, "large_06 score mismatch");

    assert!(guard < 20_000, "sampled case-06 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-06", &actual_lines, &expected_lines);
}

#[test]
fn large_07() {
    const CASE: &str = r####"mVf4YCPDlRm@tyakasha425
➐M1jC95o@新纪元425
锋利ⅤEGZPVQMY@TigerStar425
BZoPIow@酸橙425
10l-DYWg@Hell425
冷霞洞.鸣湘榔狞@四象柯425
樱井光#CQMQFHIEV@无惨425
运松翁nkJspy1Oh54A@橙红耀阳425
Hypochondriac#TtwN3jZ@Unbound425
跙坥咀诅阻珇伹伹怚@涵虚425


冷霞洞.鸣湘榔狞使用血祭, 召唤出使魔

BZoPIow发起攻击, mVf4YCPDlRm使用伤害反弹, BZoPIow受到18点伤害

运松翁nkJspy1Oh54A发起攻击, 跙坥咀诅阻珇伹伹怚受到45点伤害

樱井光#CQMQFHIEV发起攻击, 运松翁nkJspy1Oh54A受到53点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 锋利ⅤEGZPVQMY受到19点伤害

10l-DYWg发起攻击, 冷霞洞.鸣湘榔狞受到65点伤害

Hypochondriac#TtwN3jZ使用地裂术

 锋利ⅤEGZPVQMY受到40点伤害

 冷霞洞.鸣湘榔狞受到40点伤害

 ➐M1jC95o受到22点伤害

 BZoPIow受到21点伤害

 mVf4YCPDlRm受到13点伤害

mVf4YCPDlRm发起攻击, 冷霞洞.鸣湘榔狞受到52点伤害

使魔发起攻击, mVf4YCPDlRm受到76点伤害

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ受到57点伤害

➐M1jC95o发起攻击, 锋利ⅤEGZPVQMY受到65点伤害

BZoPIow发起攻击, 樱井光#CQMQFHIEV受到50点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 10l-DYWg受到111点伤害

樱井光#CQMQFHIEV发起攻击, 运松翁nkJspy1Oh54A回避了攻击

Hypochondriac#TtwN3jZ发起攻击, 运松翁nkJspy1Oh54A回避了攻击

10l-DYWg发起攻击, 运松翁nkJspy1Oh54A受到81点伤害

运松翁nkJspy1Oh54A潜行到使魔身后

使魔发起攻击, ➐M1jC95o回避了攻击

冷霞洞.鸣湘榔狞使用魅惑, BZoPIow被魅惑了

➐M1jC95o发起攻击, BZoPIow回避了攻击

BZoPIow发起攻击, 10l-DYWg受到73点伤害

 BZoPIow从魅惑中解除

跙坥咀诅阻珇伹伹怚发起攻击, 樱井光#CQMQFHIEV受到83点伤害

Hypochondriac#TtwN3jZ发起攻击, 冷霞洞.鸣湘榔狞受到33点伤害

mVf4YCPDlRm发起攻击, BZoPIow受到61点伤害

锋利ⅤEGZPVQMY使用减速术, mVf4YCPDlRm进入迟缓状态

运松翁nkJspy1Oh54A发动背刺, 使魔受到374点伤害, 冷霞洞.鸣湘榔狞受到187点伤害

 冷霞洞.鸣湘榔狞被击倒了

 使魔消失了

10l-DYWg发起攻击, ➐M1jC95o受到83点伤害

樱井光#CQMQFHIEV发起攻击, 跙坥咀诅阻珇伹伹怚受到55点伤害

跙坥咀诅阻珇伹伹怚发起攻击, Hypochondriac#TtwN3jZ受到68点伤害

运松翁nkJspy1Oh54A使用瘟疫, BZoPIow体力减少56%

➐M1jC95o使用加速术, ➐M1jC95o进入疾走状态

Hypochondriac#TtwN3jZ发起攻击, 樱井光#CQMQFHIEV受到72点伤害

BZoPIow发起攻击, Hypochondriac#TtwN3jZ受到90点伤害

10l-DYWg发起攻击, 跙坥咀诅阻珇伹伹怚受到123点伤害

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ回避了攻击

➐M1jC95o发起攻击, 锋利ⅤEGZPVQMY受到52点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 10l-DYWg受到114点伤害

 10l-DYWg被击倒了

 跙坥咀诅阻珇伹伹怚召唤亡灵, 10l-DYWg变成了丧尸

运松翁nkJspy1Oh54A使用分身, 出现一个新的运松翁nkJspy1Oh54A

樱井光#CQMQFHIEV发起攻击, mVf4YCPDlRm受到101点伤害

➐M1jC95o发起攻击, 跙坥咀诅阻珇伹伹怚受到54点伤害

 ➐M1jC95o从疾走中解除

mVf4YCPDlRm发起攻击, 锋利ⅤEGZPVQMY使用伤害反弹, mVf4YCPDlRm回避了攻击

BZoPIow发起攻击, 运松翁nkJspy1Oh54A受到42点伤害

➐M1jC95o发起攻击, BZoPIow受到65点伤害

 BZoPIow做出垂死抗争, BZoPIow所有属性上升

丧尸发起攻击, 运松翁nkJspy1Oh54A受到48点伤害

 运松翁nkJspy1Oh54A被击倒了

跙坥咀诅阻珇伹伹怚发起攻击, BZoPIow受到68点伤害

 BZoPIow被击倒了

Hypochondriac#TtwN3jZ发起攻击, ➐M1jC95o受到65点伤害

锋利ⅤEGZPVQMY发起攻击, Hypochondriac#TtwN3jZ受到50点伤害

运松翁nkJspy1Oh54A发起攻击, 锋利ⅤEGZPVQMY受到21点伤害

樱井光#CQMQFHIEV使用幻术, 召唤出幻影

跙坥咀诅阻珇伹伹怚发起攻击, 幻影受到82点伤害

Hypochondriac#TtwN3jZ使用地裂术

 丧尸受到17点伤害

 樱井光#CQMQFHIEV受到36点伤害

 幻影受到38点伤害

 运松翁nkJspy1Oh54A受到0点伤害

 ➐M1jC95o受到24点伤害

➐M1jC95o发起攻击, 樱井光#CQMQFHIEV受到103点伤害

 樱井光#CQMQFHIEV被击倒了

 幻影消失了

丧尸发起攻击, ➐M1jC95o受到11点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 运松翁nkJspy1Oh54A受到109点伤害

 运松翁nkJspy1Oh54A被击倒了

mVf4YCPDlRm发起攻击, 锋利ⅤEGZPVQMY受到69点伤害

 mVf4YCPDlRm从迟缓中解除

锋利ⅤEGZPVQMY发起攻击, 丧尸受到74点伤害

Hypochondriac#TtwN3jZ发起攻击, 丧尸受到47点伤害

丧尸发起攻击, 锋利ⅤEGZPVQMY受到49点伤害

 锋利ⅤEGZPVQMY被击倒了

➐M1jC95o使用分身, 出现一个新的➐M1jC95o

Hypochondriac#TtwN3jZ发起攻击, ➐M1jC95o受到99点伤害

 ➐M1jC95o被击倒了

➐M1jC95o发起攻击, 跙坥咀诅阻珇伹伹怚受到41点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

 丧尸消失了

mVf4YCPDlRm发起攻击, ➐M1jC95o受到152点伤害

 ➐M1jC95o被击倒了

Hypochondriac#TtwN3jZ发起攻击, mVf4YCPDlRm受到72点伤害

 mVf4YCPDlRm被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-07 must contain a blank separator between input and trace",
        "sampled case-07 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 5439, "large_07 score mismatch");

    assert!(guard < 20_000, "sampled case-07 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-07", &actual_lines, &expected_lines);
}

#[test]
fn large_08() {
    const CASE: &str = r####"「OS」#c1#bFc71OCDuO35@mwh_425
#念-GP8LKM21D4JZ@柚子不是油渍425
RedOT<{f2=v}67w@流浪冒险者425
➐M1jC95o@新纪元425
Reku_Mochizuki#494460162188@新纪元425
Sayakagh8yaICYo@candle425
Eaquirasd2D5HoYES@RbCl425
tCtrVweRgshV@Afterglow425
神谷紫苑#EUKSOXAA@暗黑突击425
Obsession#EYNIZRX@Unbound425


Sayakagh8yaICYo发起攻击, 神谷紫苑#EUKSOXAA回避了攻击

➐M1jC95o发起攻击, Reku_Mochizuki#494460162188受到80点伤害

Reku_Mochizuki#494460162188发起攻击, 神谷紫苑#EUKSOXAA受到61点伤害

#念-GP8LKM21D4JZ发起攻击, Eaquirasd2D5HoYES回避了攻击

Eaquirasd2D5HoYES使用地裂术

 Obsession#EYNIZRX受到21点伤害

 Sayakagh8yaICYo受到8点伤害

 Reku_Mochizuki#494460162188回避了攻击

 #念-GP8LKM21D4JZ受到39点伤害

 tCtrVweRgshV回避了攻击

tCtrVweRgshV发起攻击, #念-GP8LKM21D4JZ受到123点伤害

RedOT<{f2=v}67w发起攻击, Reku_Mochizuki#494460162188受到62点伤害

Obsession#EYNIZRX发起攻击, Eaquirasd2D5HoYES回避了攻击

「OS」#c1#bFc71OCDuO35开始聚气, 「OS」#c1#bFc71OCDuO35攻击力上升

Reku_Mochizuki#494460162188发起攻击, RedOT<{f2=v}67w受到94点伤害

➐M1jC95o发起攻击, Obsession#EYNIZRX受到89点伤害

神谷紫苑#EUKSOXAA发起攻击, Obsession#EYNIZRX受到64点伤害

tCtrVweRgshV使用加速术, tCtrVweRgshV进入疾走状态

#念-GP8LKM21D4JZ发起攻击, Obsession#EYNIZRX受到75点伤害

RedOT<{f2=v}67w发起攻击, tCtrVweRgshV回避了攻击

Obsession#EYNIZRX发起攻击, tCtrVweRgshV受到52点伤害

tCtrVweRgshV发起攻击, RedOT<{f2=v}67w受到115点伤害

Sayakagh8yaICYo发起攻击, 「OS」#c1#bFc71OCDuO35受到39点伤害

Eaquirasd2D5HoYES发起攻击, 神谷紫苑#EUKSOXAA受到115点伤害

「OS」#c1#bFc71OCDuO35发起攻击, tCtrVweRgshV受到127点伤害

Reku_Mochizuki#494460162188使用雷击术

 Obsession#EYNIZRX受到25点伤害

 Obsession#EYNIZRX受到44点伤害

 Obsession#EYNIZRX被击倒了

tCtrVweRgshV发起攻击, ➐M1jC95o受到28点伤害

 tCtrVweRgshV从疾走中解除

➐M1jC95o发起攻击, Eaquirasd2D5HoYES受到63点伤害

神谷紫苑#EUKSOXAA发起攻击, #念-GP8LKM21D4JZ受到63点伤害

Sayakagh8yaICYo发起攻击, RedOT<{f2=v}67w受到47点伤害

 RedOT<{f2=v}67w被击倒了

#念-GP8LKM21D4JZ发起攻击, Eaquirasd2D5HoYES受到75点伤害

神谷紫苑#EUKSOXAA发起攻击, Sayakagh8yaICYo受到72点伤害

Eaquirasd2D5HoYES发起攻击, Sayakagh8yaICYo受到82点伤害

➐M1jC95o使用分身, 出现一个新的➐M1jC95o

「OS」#c1#bFc71OCDuO35发起攻击, ➐M1jC95o受到160点伤害

 ➐M1jC95o被击倒了

Reku_Mochizuki#494460162188发起攻击, Eaquirasd2D5HoYES受到75点伤害

tCtrVweRgshV发起攻击, 神谷紫苑#EUKSOXAA受到77点伤害

Sayakagh8yaICYo发起攻击, ➐M1jC95o受到86点伤害

#念-GP8LKM21D4JZ使用地裂术

 Sayakagh8yaICYo受到10点伤害

 ➐M1jC95o受到44点伤害

 ➐M1jC95o被击倒了

 #念-GP8LKM21D4JZ吞噬了➐M1jC95o, #念-GP8LKM21D4JZ属性上升

 Reku_Mochizuki#494460162188受到24点伤害

 神谷紫苑#EUKSOXAA受到29点伤害

 「OS」#c1#bFc71OCDuO35受到29点伤害

Eaquirasd2D5HoYES发起攻击, Sayakagh8yaICYo受到86点伤害

 Sayakagh8yaICYo被击倒了

#念-GP8LKM21D4JZ使用瘟疫, Reku_Mochizuki#494460162188体力减少64%

神谷紫苑#EUKSOXAA使用生命之轮, #念-GP8LKM21D4JZ的体力值与神谷紫苑#EUKSOXAA互换

tCtrVweRgshV使用加速术, tCtrVweRgshV进入疾走状态

神谷紫苑#EUKSOXAA发起攻击, 「OS」#c1#bFc71OCDuO35受到41点伤害

「OS」#c1#bFc71OCDuO35发起攻击, 神谷紫苑#EUKSOXAA受到158点伤害

 神谷紫苑#EUKSOXAA被击倒了

Reku_Mochizuki#494460162188发起攻击, #念-GP8LKM21D4JZ受到71点伤害

 #念-GP8LKM21D4JZ被击倒了

tCtrVweRgshV发起攻击, 「OS」#c1#bFc71OCDuO35受到126点伤害

 「OS」#c1#bFc71OCDuO35被击倒了

Reku_Mochizuki#494460162188发起攻击, Eaquirasd2D5HoYES受到22点伤害

tCtrVweRgshV发起攻击, Eaquirasd2D5HoYES回避了攻击

 tCtrVweRgshV从疾走中解除

Eaquirasd2D5HoYES发起攻击, tCtrVweRgshV受到53点伤害

tCtrVweRgshV发起攻击, Eaquirasd2D5HoYES受到31点伤害

Reku_Mochizuki#494460162188发起攻击, Eaquirasd2D5HoYES受到68点伤害

 Eaquirasd2D5HoYES被击倒了

tCtrVweRgshV发起攻击, Reku_Mochizuki#494460162188受到93点伤害

 Reku_Mochizuki#494460162188被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-08 must contain a blank separator between input and trace",
        "sampled case-08 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4090, "large_08 score mismatch");

    assert!(guard < 20_000, "sampled case-08 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-08", &actual_lines, &expected_lines);
}

#[test]
fn large_09() {
    const CASE: &str = r####"#念-GP8LKM21D4JZ@柚子不是油渍425
血谣染硫决@Mithril425
仇决clFJZCMHS@candle425
态度jX2HoULfsFU9@Afterglow425
wangif9nWzNbxCJ7wXi8E@WDGod425
Imperio#4B4UZThv@Shabby_fish425
Obsession#EYNIZRX@Unbound425
末otW7sfqOze@807139425
<ζε>-fhepgq2n@ReturnVoid425
seed:第十八届武术大赛抽签:425-0@!


末otW7sfqOze使用诅咒, #念-GP8LKM21D4JZ回避了攻击

Imperio#4B4UZThv使用分身, 出现一个新的Imperio#4B4UZThv

Obsession#EYNIZRX发起攻击, #念-GP8LKM21D4JZ受到57点伤害

wangif9nWzNbxCJ7wXi8E使用火球术, 态度jX2HoULfsFU9受到64点伤害

<ζε>-fhepgq2n发起攻击, Imperio#4B4UZThv受到45点伤害

血谣染硫决发起攻击, 末otW7sfqOze受到79点伤害

末otW7sfqOze发起攻击, wangif9nWzNbxCJ7wXi8E受到72点伤害

态度jX2HoULfsFU9使用地裂术

 #念-GP8LKM21D4JZ受到40点伤害

 Imperio#4B4UZThv受到35点伤害

 Obsession#EYNIZRX受到58点伤害

 血谣染硫决受到44点伤害

仇决clFJZCMHS发起攻击, wangif9nWzNbxCJ7wXi8E受到80点伤害

#念-GP8LKM21D4JZ发起攻击, 态度jX2HoULfsFU9受到122点伤害

Imperio#4B4UZThv发起攻击, Obsession#EYNIZRX受到63点伤害

Obsession#EYNIZRX发起攻击, Imperio#4B4UZThv受到50点伤害

Imperio#4B4UZThv使用生命之轮, <ζε>-fhepgq2n的体力值与Imperio#4B4UZThv互换

<ζε>-fhepgq2n发起攻击, 血谣染硫决受到142点伤害

wangif9nWzNbxCJ7wXi8E发起攻击, 末otW7sfqOze回避了攻击

Obsession#EYNIZRX发起攻击, 末otW7sfqOze回避了攻击

末otW7sfqOze发起攻击, wangif9nWzNbxCJ7wXi8E受到73点伤害

态度jX2HoULfsFU9发起攻击, 末otW7sfqOze受到127点伤害

仇决clFJZCMHS发起攻击, 态度jX2HoULfsFU9受到90点伤害

#念-GP8LKM21D4JZ发起攻击, 末otW7sfqOze受到17点伤害

Imperio#4B4UZThv发起攻击, wangif9nWzNbxCJ7wXi8E受到114点伤害

 wangif9nWzNbxCJ7wXi8E被击倒了

血谣染硫决发起攻击, Obsession#EYNIZRX受到93点伤害

末otW7sfqOze发起攻击, #念-GP8LKM21D4JZ受到44点伤害

 #念-GP8LKM21D4JZ发起反击, 末otW7sfqOze受到35点伤害

Imperio#4B4UZThv发起攻击, #念-GP8LKM21D4JZ受到81点伤害

#念-GP8LKM21D4JZ使用地裂术

 仇决clFJZCMHS受到40点伤害

 末otW7sfqOze回避了攻击

 血谣染硫决回避了攻击

 Obsession#EYNIZRX受到58点伤害

 Imperio#4B4UZThv回避了攻击

Imperio#4B4UZThv发起攻击, <ζε>-fhepgq2n受到47点伤害

<ζε>-fhepgq2n发起攻击, 态度jX2HoULfsFU9受到35点伤害

 态度jX2HoULfsFU9被击倒了

血谣染硫决发起攻击, 末otW7sfqOze受到81点伤害

 末otW7sfqOze被击倒了

Obsession#EYNIZRX发动铁壁, Obsession#EYNIZRX防御力大幅上升

仇决clFJZCMHS使用血祭, 召唤出使魔

Imperio#4B4UZThv使用魅惑, 使魔被魅惑了

<ζε>-fhepgq2n发起攻击, #念-GP8LKM21D4JZ受到47点伤害

Imperio#4B4UZThv发起攻击, <ζε>-fhepgq2n受到36点伤害

 <ζε>-fhepgq2n被击倒了

仇决clFJZCMHS发起攻击, Obsession#EYNIZRX受到1点伤害

Obsession#EYNIZRX发起攻击, 血谣染硫决防御, 血谣染硫决受到36点伤害

 血谣染硫决被击倒了

 Obsession#EYNIZRX召唤亡灵, 血谣染硫决变成了丧尸

#念-GP8LKM21D4JZ发起攻击, Imperio#4B4UZThv回避了攻击

Imperio#4B4UZThv使用魅惑, 仇决clFJZCMHS被魅惑了

使魔发起攻击, 丧尸受到58点伤害

 使魔从魅惑中解除

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到48点伤害

使魔发起攻击, Imperio#4B4UZThv受到52点伤害

Obsession#EYNIZRX发起攻击, 使魔受到113点伤害, 仇决clFJZCMHS受到56点伤害

 使魔消失了

 Obsession#EYNIZRX从铁壁中解除

仇决clFJZCMHS发起攻击, 丧尸受到118点伤害

 丧尸消失了

 仇决clFJZCMHS从魅惑中解除

#念-GP8LKM21D4JZ发起攻击, Obsession#EYNIZRX受到81点伤害

 Obsession#EYNIZRX被击倒了

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到62点伤害

仇决clFJZCMHS发起攻击, Imperio#4B4UZThv回避了攻击

Imperio#4B4UZThv使用魅惑, #念-GP8LKM21D4JZ被魅惑了

#念-GP8LKM21D4JZ使用治愈魔法, Imperio#4B4UZThv回复体力102点

 #念-GP8LKM21D4JZ从魅惑中解除

仇决clFJZCMHS发起攻击, Imperio#4B4UZThv受到31点伤害

Imperio#4B4UZThv发起攻击, #念-GP8LKM21D4JZ受到112点伤害

 #念-GP8LKM21D4JZ被击倒了

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到51点伤害

仇决clFJZCMHS发起攻击, Imperio#4B4UZThv受到19点伤害

Imperio#4B4UZThv使用魅惑, 仇决clFJZCMHS被魅惑了

Imperio#4B4UZThv发起攻击, 仇决clFJZCMHS受到57点伤害

 仇决clFJZCMHS被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-09 must contain a blank separator between input and trace",
        "sampled case-09 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4878, "large_09 score mismatch");

    assert!(guard < 20_000, "sampled case-09 combat did not finish in expected rounds");
    assert_trace_with_name_noise_ignored("sampled case-09", &actual_lines, &expected_lines);
}

#[test]
fn large_10() {
    const CASE: &str = r####"子子油渍柚不子油不是子柚渍不不渍柚油柚子@柚子不是油渍425
#念-GP8LKM21D4JZ@柚子不是油渍425
权计WN13vmJnn@candle425
ImmutableZYsdlabOOz@RbCl425
MeltelabRC3P3Go7@RbCl425
Eaquirasd2D5HoYES@RbCl425
氯化钠8UJMGcZ@fx425
[oWmjI_$'4Z#[GK,,BX2@czr2012425
wangifc5NuJx52y1cMSaD@WDGod425
跙坥咀诅阻珇伹伹怚@涵虚425


氯化钠8UJMGcZ发起攻击, 跙坥咀诅阻珇伹伹怚受到85点伤害

MeltelabRC3P3Go7发起攻击, wangifc5NuJx52y1cMSaD回避了攻击

#念-GP8LKM21D4JZ发起攻击, 氯化钠8UJMGcZ受到57点伤害

跙坥咀诅阻珇伹伹怚发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到56点伤害

权计WN13vmJnn发起攻击, ImmutableZYsdlabOOz受到42点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, ImmutableZYsdlabOOz回避了攻击

wangifc5NuJx52y1cMSaD发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到70点伤害

ImmutableZYsdlabOOz发起攻击, 跙坥咀诅阻珇伹伹怚受到77点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子使用幻术, 召唤出幻影

氯化钠8UJMGcZ发起攻击, #念-GP8LKM21D4JZ受到77点伤害

 #念-GP8LKM21D4JZ发起反击, 氯化钠8UJMGcZ回避了攻击

MeltelabRC3P3Go7发起攻击, #念-GP8LKM21D4JZ受到42点伤害

Eaquirasd2D5HoYES发起攻击, 权计WN13vmJnn受到86点伤害

权计WN13vmJnn发起攻击, ImmutableZYsdlabOOz受到63点伤害

#念-GP8LKM21D4JZ使用净化, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到97点伤害

ImmutableZYsdlabOOz使用净化, wangifc5NuJx52y1cMSaD受到31点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 权计WN13vmJnn受到60点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, wangifc5NuJx52y1cMSaD受到58点伤害

wangifc5NuJx52y1cMSaD发起攻击, [oWmjI_$'4Z#[GK,,BX2受到87点伤害

跙坥咀诅阻珇伹伹怚发起攻击, MeltelabRC3P3Go7受到147点伤害

Eaquirasd2D5HoYES使用地裂术

 幻影受到16点伤害

 wangifc5NuJx52y1cMSaD受到43点伤害

 权计WN13vmJnn受到10点伤害

 MeltelabRC3P3Go7受到35点伤害

氯化钠8UJMGcZ使用净化, ImmutableZYsdlabOOz受到118点伤害

MeltelabRC3P3Go7使用分身, 出现一个新的MeltelabRC3P3Go7

权计WN13vmJnn发起攻击, [oWmjI_$'4Z#[GK,,BX2受到91点伤害

ImmutableZYsdlabOOz发起攻击, 幻影受到87点伤害

#念-GP8LKM21D4JZ使用净化, Eaquirasd2D5HoYES受到86点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, 权计WN13vmJnn回避了攻击

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 跙坥咀诅阻珇伹伹怚受到94点伤害

wangifc5NuJx52y1cMSaD发起攻击, 跙坥咀诅阻珇伹伹怚受到113点伤害

 跙坥咀诅阻珇伹伹怚被击倒了

幻影发起攻击, #念-GP8LKM21D4JZ受到36点伤害

MeltelabRC3P3Go7潜行到Eaquirasd2D5HoYES身后

氯化钠8UJMGcZ发起攻击, [oWmjI_$'4Z#[GK,,BX2受到93点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, #念-GP8LKM21D4JZ受到140点伤害

[oWmjI_$'4Z#[GK,,BX2发起攻击, MeltelabRC3P3Go7受到75点伤害

 MeltelabRC3P3Go7的潜行被识破

 MeltelabRC3P3Go7被击倒了

MeltelabRC3P3Go7发起攻击, 权计WN13vmJnn受到27点伤害

Eaquirasd2D5HoYES发起攻击, wangifc5NuJx52y1cMSaD受到62点伤害

ImmutableZYsdlabOOz发起攻击, Eaquirasd2D5HoYES受到62点伤害

幻影发起攻击, 氯化钠8UJMGcZ受到47点伤害

权计WN13vmJnn使用诅咒, [oWmjI_$'4Z#[GK,,BX2受到38点伤害

 [oWmjI_$'4Z#[GK,,BX2被击倒了

wangifc5NuJx52y1cMSaD发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到83点伤害

#念-GP8LKM21D4JZ发起攻击, 氯化钠8UJMGcZ回避了攻击

氯化钠8UJMGcZ投毒, Eaquirasd2D5HoYES受到69点伤害, Eaquirasd2D5HoYES中毒

子子油渍柚不子油不是子柚渍不不渍柚油柚子发动铁壁, 子子油渍柚不子油不是子柚渍不不渍柚油柚子防御力大幅上升

ImmutableZYsdlabOOz发起攻击, wangifc5NuJx52y1cMSaD受到54点伤害

幻影发起攻击, ImmutableZYsdlabOOz受到45点伤害

MeltelabRC3P3Go7使用生命之轮, Eaquirasd2D5HoYES的体力值与MeltelabRC3P3Go7互换

氯化钠8UJMGcZ发起攻击, 权计WN13vmJnn受到77点伤害

 权计WN13vmJnn被击倒了

Eaquirasd2D5HoYES发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

 Eaquirasd2D5HoYES毒性发作, Eaquirasd2D5HoYES受到35点伤害

wangifc5NuJx52y1cMSaD发起攻击, 幻影受到67点伤害

 幻影消失了

 wangifc5NuJx52y1cMSaD吞噬了幻影, wangifc5NuJx52y1cMSaD属性上升

wangifc5NuJx52y1cMSaD使用净化, 氯化钠8UJMGcZ受到68点伤害

#念-GP8LKM21D4JZ发起攻击, Eaquirasd2D5HoYES受到45点伤害

 Eaquirasd2D5HoYES被击倒了

ImmutableZYsdlabOOz使用净化, MeltelabRC3P3Go7受到98点伤害

 MeltelabRC3P3Go7被击倒了

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, wangifc5NuJx52y1cMSaD受到50点伤害

氯化钠8UJMGcZ发起攻击, #念-GP8LKM21D4JZ受到87点伤害

 #念-GP8LKM21D4JZ被击倒了

ImmutableZYsdlabOOz发起攻击, wangifc5NuJx52y1cMSaD受到42点伤害

 wangifc5NuJx52y1cMSaD被击倒了

ImmutableZYsdlabOOz发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到0点伤害

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, 氯化钠8UJMGcZ受到63点伤害

 氯化钠8UJMGcZ被击倒了

 子子油渍柚不子油不是子柚渍不不渍柚油柚子从铁壁中解除

子子油渍柚不子油不是子柚渍不不渍柚油柚子发起攻击, ImmutableZYsdlabOOz回避了攻击

ImmutableZYsdlabOOz发起攻击, 子子油渍柚不子油不是子柚渍不不渍柚油柚子受到56点伤害

 子子油渍柚不子油不是子柚渍不不渍柚油柚子被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-10 must contain a blank separator between input and trace",
        "sampled case-10 trace is empty",
    );

    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert_eq!(total_score, 4537, "large_10 score mismatch");

    assert!(guard < 20_000, "sampled case-10 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-10", &actual_lines, &expected_lines);
}

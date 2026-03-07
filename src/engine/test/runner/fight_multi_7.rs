use super::*;

#[test]
fn fight_multi_7() {
    const FIGHT_CASE: &str = r###"0_0_QcQymPrUFh
0_1_U13KKqyRie
0_2_wUxi88cIoS
0_3_kM8rP6G5Ta
0_4_7aWRHl1hzq

1_0_NCm3rTQ6Xi
1_1_ttAYKGCEUu
1_2_q7AVVjjuyZ
1_3_HUGBHL6r0W
1_4_Nm8iFY9s8u

2_0_5uVmafjdIo
2_1_ZF0iBbPJva
2_2_3Y2CHuS2Tl
2_3_fBubINRSbN
2_4_HRdDGpmxdi

3_0_OIVQH30pU1
3_1_BQalD7BQD9
3_2_7NYUaThfG9
3_3_sMlFjQuDO2
3_4_H58PY9DUd0

4_0_6LlMu64Hov
4_1_bedE7ksqcc
4_2_oL5lbVS1z9
4_3_ByRYMmIQng
4_4_vpuJDEmsAb

5_0_8r19XDOwrw
5_1_VRSqeIA6wB
5_2_L6XiOdOfiz
5_3_fq3I5eI0Pt
5_4_lBgDQHEh5b

6_0_douuJVTCHS
6_1_SJeS4VtieG
6_2_kXNOnc6Gc1
6_3_Alc7EY3OZz
6_4_wXHMPGGJuX

7_0_Nfp7EdR79U
7_1_EoFaPIxreV
7_2_iCxkh7njv5
7_3_zC5YeIA0H2
7_4_fFzNsWUg4M

8_0_Hpj3ECqmur
8_1_BNOGlrVzHs
8_2_hlkU3Dm0ea
8_3_kn03spEYH3
8_4_8o2TAT6AzP

9_0_ubNPNYhYfD
9_1_8My1bOMVCr
9_2_MlnfYjHeHi
9_3_gOu3Qc3TFf
9_4_OKpol4CPvV


3_1_BQalD7BQD9使用血祭, 召唤出使魔

8_4_8o2TAT6AzP发起攻击, 3_3_sMlFjQuDO2受到79点伤害

4_1_bedE7ksqcc发起攻击, 3_3_sMlFjQuDO2受到92点伤害

3_3_sMlFjQuDO2发起攻击, 4_3_ByRYMmIQng受到150点伤害

3_4_H58PY9DUd0发起攻击, 9_4_OKpol4CPvV受到45点伤害

6_0_douuJVTCHS发起攻击, 7_4_fFzNsWUg4M受到71点伤害

8_2_hlkU3Dm0ea使用地裂术

 4_0_6LlMu64Hov受到40点伤害

 5_2_L6XiOdOfiz受到22点伤害

 4_1_bedE7ksqcc回避了攻击

 6_1_SJeS4VtieG受到33点伤害

3_2_7NYUaThfG9发动铁壁, 3_2_7NYUaThfG9防御力大幅上升

6_2_kXNOnc6Gc1发起攻击, 4_2_oL5lbVS1z9受到69点伤害

5_4_lBgDQHEh5b使用幻术, 召唤出幻影

9_1_8My1bOMVCr发起攻击, 7_1_EoFaPIxreV受到74点伤害

3_0_OIVQH30pU1发起攻击, 6_0_douuJVTCHS受到110点伤害

9_4_OKpol4CPvV发起攻击, 5_2_L6XiOdOfiz受到116点伤害

0_2_wUxi88cIoS发起攻击, 5_0_8r19XDOwrw受到23点伤害

2_0_5uVmafjdIo发起攻击, 使魔受到68点伤害, 3_1_BQalD7BQD9受到34点伤害

2_1_ZF0iBbPJva发动会心一击, 5_3_fq3I5eI0Pt受到143点伤害

2_4_HRdDGpmxdi发起攻击, 6_3_Alc7EY3OZz回避了攻击

4_0_6LlMu64Hov发起攻击, 9_2_MlnfYjHeHi受到66点伤害

1_1_ttAYKGCEUu发起攻击, 6_0_douuJVTCHS受到57点伤害

2_2_3Y2CHuS2Tl潜行到幻影身后

4_2_oL5lbVS1z9发起吸血攻击, 2_3_fBubINRSbN受到60点伤害, 4_2_oL5lbVS1z9回复体力30点

6_3_Alc7EY3OZz发起攻击, 9_3_gOu3Qc3TFf受到75点伤害

5_3_fq3I5eI0Pt发起攻击, 3_0_OIVQH30pU1受到115点伤害

7_2_iCxkh7njv5使用魅惑, 6_2_kXNOnc6Gc1被魅惑了

1_3_HUGBHL6r0W发起攻击, 4_0_6LlMu64Hov受到87点伤害

0_0_QcQymPrUFh发起攻击, 2_0_5uVmafjdIo受到50点伤害

8_1_BNOGlrVzHs发起攻击, 4_1_bedE7ksqcc受到84点伤害

8_3_kn03spEYH3发起攻击, 2_2_3Y2CHuS2Tl受到26点伤害

 2_2_3Y2CHuS2Tl的潜行被识破

7_4_fFzNsWUg4M发起攻击, 9_4_OKpol4CPvV回避了攻击

9_3_gOu3Qc3TFf发起攻击, 3_2_7NYUaThfG9受到0点伤害

7_3_zC5YeIA0H2发起攻击, 3_4_H58PY9DUd0受到73点伤害

8_0_Hpj3ECqmur使用血祭, 召唤出使魔

0_3_kM8rP6G5Ta发起攻击, 8_3_kn03spEYH3受到33点伤害

7_1_EoFaPIxreV发起攻击, 8_1_BNOGlrVzHs受到99点伤害

4_4_vpuJDEmsAb发起攻击, 2_2_3Y2CHuS2Tl受到113点伤害

2_3_fBubINRSbN发起攻击, 4_1_bedE7ksqcc受到48点伤害

9_2_MlnfYjHeHi发起攻击, 0_1_U13KKqyRie受到26点伤害

8_4_8o2TAT6AzP发起攻击, 7_2_iCxkh7njv5受到94点伤害

0_4_7aWRHl1hzq发起攻击, 4_4_vpuJDEmsAb受到79点伤害

1_2_q7AVVjjuyZ发起攻击, 2_1_ZF0iBbPJva受到94点伤害

5_1_VRSqeIA6wB发起攻击, 1_0_NCm3rTQ6Xi受到84点伤害

0_1_U13KKqyRie使用冰冻术, 8_0_Hpj3ECqmur回避了攻击

6_4_wXHMPGGJuX使用瘟疫, 3_2_7NYUaThfG9体力减少64%

5_2_L6XiOdOfiz发起攻击, 1_1_ttAYKGCEUu受到80点伤害

 1_1_ttAYKGCEUu发起反击, 5_2_L6XiOdOfiz受到51点伤害

9_0_ubNPNYhYfD发动铁壁, 9_0_ubNPNYhYfD防御力大幅上升

6_0_douuJVTCHS发起攻击, 8_0_Hpj3ECqmur受到98点伤害

2_4_HRdDGpmxdi发起攻击, 5_4_lBgDQHEh5b回避了攻击

1_4_Nm8iFY9s8u发起攻击, 8_4_8o2TAT6AzP受到169点伤害

使魔使用火球术, 0_1_U13KKqyRie受到87点伤害

5_3_fq3I5eI0Pt发起攻击, 8_3_kn03spEYH3受到104点伤害

3_3_sMlFjQuDO2发起攻击, 2_4_HRdDGpmxdi受到87点伤害

5_0_8r19XDOwrw发起攻击, 4_0_6LlMu64Hov受到42点伤害

7_0_Nfp7EdR79U发起攻击, 使魔回避了攻击

8_2_hlkU3Dm0ea投毒, 0_1_U13KKqyRie受到50点伤害, 0_1_U13KKqyRie中毒

7_4_fFzNsWUg4M发起攻击, 9_4_OKpol4CPvV受到19点伤害

3_1_BQalD7BQD9发起攻击, 1_4_Nm8iFY9s8u受到56点伤害

3_2_7NYUaThfG9发起攻击, 0_0_QcQymPrUFh受到62点伤害

3_4_H58PY9DUd0发起攻击, 1_2_q7AVVjjuyZ受到98点伤害

8_0_Hpj3ECqmur发起攻击, 6_3_Alc7EY3OZz受到85点伤害

6_1_SJeS4VtieG发起攻击, 5_1_VRSqeIA6wB受到92点伤害

2_1_ZF0iBbPJva发动会心一击, 8_1_BNOGlrVzHs受到97点伤害

2_2_3Y2CHuS2Tl使用狂暴术, 1_3_HUGBHL6r0W受到90点伤害, 1_3_HUGBHL6r0W进入狂暴状态

6_4_wXHMPGGJuX使用魅惑, 7_2_iCxkh7njv5被魅惑了

0_0_QcQymPrUFh发起攻击, 9_2_MlnfYjHeHi受到62点伤害

3_0_OIVQH30pU1使用净化, 6_2_kXNOnc6Gc1受到65点伤害

9_4_OKpol4CPvV使用减速术, 1_4_Nm8iFY9s8u进入迟缓状态

4_0_6LlMu64Hov使用减速术, 5_0_8r19XDOwrw进入迟缓状态

1_1_ttAYKGCEUu发起攻击, 5_1_VRSqeIA6wB受到90点伤害

9_3_gOu3Qc3TFf发起攻击, 8_4_8o2TAT6AzP受到71点伤害

使魔发起攻击, 9_3_gOu3Qc3TFf受到24点伤害

4_2_oL5lbVS1z9使用狂暴术, 2_0_5uVmafjdIo受到106点伤害, 2_0_5uVmafjdIo进入狂暴状态

6_3_Alc7EY3OZz发起攻击, 使魔受到76点伤害, 8_0_Hpj3ECqmur受到38点伤害

7_3_zC5YeIA0H2发起攻击, 6_0_douuJVTCHS受到29点伤害

6_2_kXNOnc6Gc1发起攻击, 8_1_BNOGlrVzHs回避了攻击

 6_2_kXNOnc6Gc1从魅惑中解除

1_0_NCm3rTQ6Xi发起攻击, 9_4_OKpol4CPvV受到39点伤害

6_0_douuJVTCHS发起攻击, 0_2_wUxi88cIoS受到56点伤害

4_3_ByRYMmIQng发起攻击, 8_2_hlkU3Dm0ea回避了攻击

4_4_vpuJDEmsAb发起攻击, 9_0_ubNPNYhYfD受到1点伤害

9_2_MlnfYjHeHi使用火球术, 4_3_ByRYMmIQng受到127点伤害

8_1_BNOGlrVzHs发起攻击, 3_2_7NYUaThfG9受到0点伤害

4_1_bedE7ksqcc发起攻击, 1_2_q7AVVjjuyZ受到69点伤害

0_1_U13KKqyRie发起攻击, 7_3_zC5YeIA0H2受到78点伤害

 0_1_U13KKqyRie毒性发作, 0_1_U13KKqyRie受到46点伤害

3_4_H58PY9DUd0发起攻击, 9_0_ubNPNYhYfD受到1点伤害

0_3_kM8rP6G5Ta使用加速术, 0_3_kM8rP6G5Ta进入疾走状态

2_3_fBubINRSbN使用加速术, 2_3_fBubINRSbN进入疾走状态

0_2_wUxi88cIoS发动铁壁, 0_2_wUxi88cIoS防御力大幅上升

8_3_kn03spEYH3发起攻击, 5_0_8r19XDOwrw受到68点伤害

2_4_HRdDGpmxdi发起攻击, 5_1_VRSqeIA6wB受到88点伤害

 5_1_VRSqeIA6wB被击倒了

0_4_7aWRHl1hzq使用雷击术

 6_2_kXNOnc6Gc1受到13点伤害

 6_2_kXNOnc6Gc1受到13点伤害

 6_2_kXNOnc6Gc1受到15点伤害

 6_2_kXNOnc6Gc1受到22点伤害

1_2_q7AVVjjuyZ发起攻击, 0_4_7aWRHl1hzq受到41点伤害

3_3_sMlFjQuDO2发起攻击, 1_0_NCm3rTQ6Xi受到77点伤害

7_2_iCxkh7njv5发动会心一击, 8_2_hlkU3Dm0ea受到48点伤害

 7_2_iCxkh7njv5从魅惑中解除

1_3_HUGBHL6r0W发起狂暴攻击, 7_4_fFzNsWUg4M受到52点伤害

 1_3_HUGBHL6r0W从狂暴中解除

7_4_fFzNsWUg4M发起攻击, 4_3_ByRYMmIQng回避了攻击

使魔发起攻击, 0_4_7aWRHl1hzq受到20点伤害

6_4_wXHMPGGJuX使用魅惑, 5_3_fq3I5eI0Pt被魅惑了

5_2_L6XiOdOfiz发起攻击, 9_4_OKpol4CPvV受到58点伤害

7_0_Nfp7EdR79U发起攻击, 2_0_5uVmafjdIo回避了攻击

8_4_8o2TAT6AzP发起攻击, 9_3_gOu3Qc3TFf受到78点伤害

2_0_5uVmafjdIo发起狂暴攻击, 5_4_lBgDQHEh5b受到162点伤害

 2_0_5uVmafjdIo从狂暴中解除

2_1_ZF0iBbPJva发起攻击, 9_1_8My1bOMVCr防御, 9_1_8My1bOMVCr受到59点伤害

3_1_BQalD7BQD9发起攻击, 0_0_QcQymPrUFh受到32点伤害

3_2_7NYUaThfG9发起攻击, 0_2_wUxi88cIoS受到1点伤害

 3_2_7NYUaThfG9从铁壁中解除

5_3_fq3I5eI0Pt开始聚气, 5_3_fq3I5eI0Pt攻击力上升

 5_3_fq3I5eI0Pt从魅惑中解除

5_4_lBgDQHEh5b发起攻击, 8_0_Hpj3ECqmur受到133点伤害

8_0_Hpj3ECqmur使用加速术, 8_2_hlkU3Dm0ea进入疾走状态

5_0_8r19XDOwrw开始聚气, 5_0_8r19XDOwrw攻击力上升

6_0_douuJVTCHS发起攻击, 0_2_wUxi88cIoS受到1点伤害

9_1_8My1bOMVCr发起攻击, 7_1_EoFaPIxreV受到72点伤害

8_2_hlkU3Dm0ea发起攻击, 6_1_SJeS4VtieG受到77点伤害

7_1_EoFaPIxreV发起攻击, 2_2_3Y2CHuS2Tl受到37点伤害

4_2_oL5lbVS1z9发起攻击, 6_2_kXNOnc6Gc1受到64点伤害

9_0_ubNPNYhYfD使用瘟疫, 1_1_ttAYKGCEUu体力减少58%

4_3_ByRYMmIQng发起攻击, 1_4_Nm8iFY9s8u受到50点伤害

0_0_QcQymPrUFh发起攻击, 9_1_8My1bOMVCr受到134点伤害

3_0_OIVQH30pU1发起攻击, 2_2_3Y2CHuS2Tl受到27点伤害

9_2_MlnfYjHeHi发起攻击, 幻影受到76点伤害

6_1_SJeS4VtieG发起攻击, 9_4_OKpol4CPvV受到110点伤害

8_1_BNOGlrVzHs发起攻击, 2_1_ZF0iBbPJva回避了攻击

4_0_6LlMu64Hov发起攻击, 2_4_HRdDGpmxdi受到92点伤害

9_3_gOu3Qc3TFf使用治愈魔法, 9_4_OKpol4CPvV回复体力117点

2_2_3Y2CHuS2Tl发起攻击, 5_0_8r19XDOwrw受到54点伤害

6_2_kXNOnc6Gc1发起攻击, 2_1_ZF0iBbPJva受到112点伤害

0_3_kM8rP6G5Ta发起攻击, 1_2_q7AVVjjuyZ受到29点伤害, 1_2_q7AVVjjuyZ发动隐匿

9_4_OKpol4CPvV发起攻击, 6_4_wXHMPGGJuX受到124点伤害

0_2_wUxi88cIoS发起攻击, 7_3_zC5YeIA0H2受到74点伤害

2_4_HRdDGpmxdi使用魅惑, 3_0_OIVQH30pU1被魅惑了

0_4_7aWRHl1hzq发起攻击, 7_0_Nfp7EdR79U受到52点伤害

1_4_Nm8iFY9s8u发起攻击, 9_4_OKpol4CPvV受到45点伤害

使魔使用火球术, 0_1_U13KKqyRie受到146点伤害

 0_1_U13KKqyRie被击倒了

使魔发起攻击, 1_3_HUGBHL6r0W受到24点伤害

7_3_zC5YeIA0H2发起攻击, 8_2_hlkU3Dm0ea受到85点伤害

5_2_L6XiOdOfiz发起攻击, 3_3_sMlFjQuDO2受到16点伤害

3_4_H58PY9DUd0发起攻击, 4_2_oL5lbVS1z9受到66点伤害

2_3_fBubINRSbN发起攻击, 4_4_vpuJDEmsAb受到73点伤害

8_3_kn03spEYH3发起攻击, 0_0_QcQymPrUFh受到67点伤害

1_1_ttAYKGCEUu使用地裂术

 3_4_H58PY9DUd0受到31点伤害

 7_2_iCxkh7njv5受到28点伤害

 5_3_fq3I5eI0Pt受到42点伤害

 5_4_lBgDQHEh5b受到34点伤害

 5_2_L6XiOdOfiz受到7点伤害

3_3_sMlFjQuDO2发起攻击, 0_3_kM8rP6G5Ta受到93点伤害

7_2_iCxkh7njv5发起攻击, 6_0_douuJVTCHS受到25点伤害

1_0_NCm3rTQ6Xi发起攻击, 幻影受到39点伤害

8_2_hlkU3Dm0ea使用地裂术

 5_3_fq3I5eI0Pt受到15点伤害

 5_0_8r19XDOwrw受到16点伤害

 1_1_ttAYKGCEUu受到55点伤害

 9_4_OKpol4CPvV受到44点伤害

 5_2_L6XiOdOfiz受到21点伤害

8_4_8o2TAT6AzP发起攻击, 0_0_QcQymPrUFh受到32点伤害

1_2_q7AVVjjuyZ发起攻击, 6_1_SJeS4VtieG受到76点伤害

幻影发起攻击, 1_4_Nm8iFY9s8u回避了攻击

3_1_BQalD7BQD9发起攻击, 7_1_EoFaPIxreV受到67点伤害

1_3_HUGBHL6r0W发起攻击, 6_4_wXHMPGGJuX受到100点伤害

7_1_EoFaPIxreV发起攻击, 4_2_oL5lbVS1z9受到24点伤害

4_4_vpuJDEmsAb发起攻击, 7_3_zC5YeIA0H2受到122点伤害

 7_3_zC5YeIA0H2被击倒了

6_4_wXHMPGGJuX使用魅惑, 8_2_hlkU3Dm0ea被魅惑了

7_0_Nfp7EdR79U发起攻击, 2_0_5uVmafjdIo回避了攻击

9_1_8My1bOMVCr发起攻击, 7_1_EoFaPIxreV受到66点伤害

4_3_ByRYMmIQng发起攻击, 9_3_gOu3Qc3TFf受到79点伤害

0_0_QcQymPrUFh发动会心一击, 幻影受到99点伤害

 幻影消失了

 0_0_QcQymPrUFh吞噬了幻影, 0_0_QcQymPrUFh属性上升

2_3_fBubINRSbN发起攻击, 7_4_fFzNsWUg4M受到116点伤害

 2_3_fBubINRSbN从疾走中解除

2_1_ZF0iBbPJva发起攻击, 使魔受到47点伤害, 8_0_Hpj3ECqmur受到23点伤害

 使魔消失了

4_1_bedE7ksqcc发起攻击, 0_3_kM8rP6G5Ta受到87点伤害

4_0_6LlMu64Hov使用减速术, 9_0_ubNPNYhYfD进入迟缓状态

3_2_7NYUaThfG9发起攻击, 6_1_SJeS4VtieG受到79点伤害

 6_1_SJeS4VtieG被击倒了

6_0_douuJVTCHS发起攻击, 2_3_fBubINRSbN受到100点伤害

8_2_hlkU3Dm0ea发起攻击, 1_4_Nm8iFY9s8u受到84点伤害

 8_2_hlkU3Dm0ea从疾走中解除

 8_2_hlkU3Dm0ea从魅惑中解除

9_2_MlnfYjHeHi发起攻击, 4_2_oL5lbVS1z9受到37点伤害

8_1_BNOGlrVzHs发起攻击, 7_2_iCxkh7njv5受到27点伤害, 7_2_iCxkh7njv5发动隐匿

6_3_Alc7EY3OZz发起攻击, 1_4_Nm8iFY9s8u使用伤害反弹, 6_3_Alc7EY3OZz受到19点伤害

5_3_fq3I5eI0Pt发起攻击, 3_4_H58PY9DUd0受到101点伤害

8_0_Hpj3ECqmur发起攻击, 3_1_BQalD7BQD9受到52点伤害

0_3_kM8rP6G5Ta发起攻击, 1_0_NCm3rTQ6Xi受到48点伤害

 0_3_kM8rP6G5Ta从疾走中解除

3_0_OIVQH30pU1发起攻击, 3_0_OIVQH30pU1受到54点伤害

 3_0_OIVQH30pU1从魅惑中解除

2_0_5uVmafjdIo使用雷击术

 1_2_q7AVVjjuyZ受到45点伤害, 1_2_q7AVVjjuyZ发动隐匿

 1_2_q7AVVjjuyZ受到31点伤害

 1_2_q7AVVjjuyZ受到21点伤害

0_4_7aWRHl1hzq使用诅咒, 8_4_8o2TAT6AzP受到66点伤害

 8_4_8o2TAT6AzP被击倒了

2_2_3Y2CHuS2Tl使用分身, 出现一个新的2_2_3Y2CHuS2Tl

3_3_sMlFjQuDO2发起攻击, 4_0_6LlMu64Hov受到63点伤害

5_4_lBgDQHEh5b发起攻击, 3_0_OIVQH30pU1受到45点伤害

3_4_H58PY9DUd0发起攻击, 9_0_ubNPNYhYfD回避了攻击

4_4_vpuJDEmsAb发起攻击, 0_3_kM8rP6G5Ta受到73点伤害

 0_3_kM8rP6G5Ta被击倒了, 0_3_kM8rP6G5Ta使用护身符抵挡了一次死亡, 0_3_kM8rP6G5Ta回复体力10点

7_4_fFzNsWUg4M发起攻击, 6_4_wXHMPGGJuX受到88点伤害

 6_4_wXHMPGGJuX被击倒了

9_3_gOu3Qc3TFf发起攻击, 6_3_Alc7EY3OZz受到65点伤害

使魔发起攻击, 0_2_wUxi88cIoS受到1点伤害

4_2_oL5lbVS1z9使用狂暴术, 7_0_Nfp7EdR79U受到112点伤害, 7_0_Nfp7EdR79U进入狂暴状态

5_2_L6XiOdOfiz发起攻击, 7_1_EoFaPIxreV受到114点伤害

 7_1_EoFaPIxreV被击倒了

9_0_ubNPNYhYfD发起攻击, 3_1_BQalD7BQD9受到40点伤害

 9_0_ubNPNYhYfD从铁壁中解除

1_3_HUGBHL6r0W发起攻击, 4_1_bedE7ksqcc受到79点伤害

1_0_NCm3rTQ6Xi发起攻击, 9_2_MlnfYjHeHi受到100点伤害

4_3_ByRYMmIQng使用火球术, 2_0_5uVmafjdIo受到72点伤害

2_1_ZF0iBbPJva发起攻击, 5_4_lBgDQHEh5b受到68点伤害

 5_4_lBgDQHEh5b做出垂死抗争, 5_4_lBgDQHEh5b所有属性上升

5_0_8r19XDOwrw发起攻击

 0_2_wUxi88cIoS的铁壁被打消了, 0_2_wUxi88cIoS受到12点伤害

 5_0_8r19XDOwrw从迟缓中解除

0_0_QcQymPrUFh发起攻击, 使魔受到97点伤害, 3_1_BQalD7BQD9受到48点伤害

 使魔消失了

 0_0_QcQymPrUFh吞噬了使魔, 0_0_QcQymPrUFh属性上升

2_3_fBubINRSbN使用幻术, 召唤出幻影

9_4_OKpol4CPvV发动会心一击, 5_0_8r19XDOwrw受到80点伤害

0_2_wUxi88cIoS发动铁壁, 0_2_wUxi88cIoS防御力大幅上升

2_4_HRdDGpmxdi发起攻击, 3_4_H58PY9DUd0受到32点伤害

2_2_3Y2CHuS2Tl发起攻击, 4_0_6LlMu64Hov受到59点伤害

6_2_kXNOnc6Gc1发起攻击, 7_0_Nfp7EdR79U受到57点伤害

7_2_iCxkh7njv5发起攻击, 8_2_hlkU3Dm0ea使用伤害反弹, 7_2_iCxkh7njv5受到36点伤害, 7_2_iCxkh7njv5发动隐匿

0_0_QcQymPrUFh发动会心一击, 4_2_oL5lbVS1z9受到132点伤害

8_3_kn03spEYH3发起攻击, 4_1_bedE7ksqcc回避了攻击

8_1_BNOGlrVzHs使用魅惑, 0_4_7aWRHl1hzq被魅惑了

1_2_q7AVVjjuyZ发起攻击, 4_3_ByRYMmIQng受到134点伤害

 4_3_ByRYMmIQng被击倒了

2_2_3Y2CHuS2Tl发起攻击, 1_4_Nm8iFY9s8u受到56点伤害

3_2_7NYUaThfG9使用生命之轮, 0_2_wUxi88cIoS的体力值与3_2_7NYUaThfG9互换

7_0_Nfp7EdR79U发起狂暴攻击, 7_2_iCxkh7njv5守护7_4_fFzNsWUg4M, 7_2_iCxkh7njv5受到15点伤害

 7_0_Nfp7EdR79U从狂暴中解除

0_3_kM8rP6G5Ta发动会心一击, 6_0_douuJVTCHS受到90点伤害

 6_0_douuJVTCHS被击倒了

4_4_vpuJDEmsAb发起攻击, 9_0_ubNPNYhYfD受到140点伤害

3_0_OIVQH30pU1发起攻击, 4_2_oL5lbVS1z9受到74点伤害

 4_2_oL5lbVS1z9被击倒了

9_3_gOu3Qc3TFf发起攻击, 1_2_q7AVVjjuyZ回避了攻击

3_1_BQalD7BQD9发起攻击, 0_0_QcQymPrUFh受到94点伤害

6_3_Alc7EY3OZz发起攻击, 4_4_vpuJDEmsAb受到104点伤害

 4_4_vpuJDEmsAb做出垂死抗争, 4_4_vpuJDEmsAb所有属性上升

5_2_L6XiOdOfiz发起攻击, 2_2_3Y2CHuS2Tl受到112点伤害

 2_2_3Y2CHuS2Tl被击倒了

8_2_hlkU3Dm0ea使用魅惑, 3_1_BQalD7BQD9被魅惑了

4_1_bedE7ksqcc发起攻击, 3_3_sMlFjQuDO2使用伤害反弹, 4_1_bedE7ksqcc受到20点伤害

0_4_7aWRHl1hzq发起攻击, 9_2_MlnfYjHeHi回避了攻击

 0_4_7aWRHl1hzq从魅惑中解除

1_1_ttAYKGCEUu发起攻击, 8_2_hlkU3Dm0ea受到57点伤害

3_4_H58PY9DUd0发起攻击, 0_4_7aWRHl1hzq受到44点伤害

8_0_Hpj3ECqmur使用加速术, 8_3_kn03spEYH3进入疾走状态

9_1_8My1bOMVCr发起攻击, 0_3_kM8rP6G5Ta受到41点伤害

 0_3_kM8rP6G5Ta被击倒了

9_2_MlnfYjHeHi使用火球术, 8_3_kn03spEYH3受到82点伤害

8_3_kn03spEYH3发起攻击, 2_4_HRdDGpmxdi受到125点伤害

 2_4_HRdDGpmxdi被击倒了

2_0_5uVmafjdIo发起攻击, 0_4_7aWRHl1hzq受到57点伤害

1_3_HUGBHL6r0W发起攻击, 5_4_lBgDQHEh5b回避了攻击

5_0_8r19XDOwrw投毒

 0_2_wUxi88cIoS的铁壁被打消了, 0_2_wUxi88cIoS受到7点伤害, 0_2_wUxi88cIoS中毒

7_4_fFzNsWUg4M发起攻击, 9_4_OKpol4CPvV受到75点伤害

 9_4_OKpol4CPvV被击倒了

7_2_iCxkh7njv5发起攻击, 3_2_7NYUaThfG9受到63点伤害

2_3_fBubINRSbN发起攻击, 3_1_BQalD7BQD9受到65点伤害

 3_1_BQalD7BQD9被击倒了

0_2_wUxi88cIoS发起攻击, 2_3_fBubINRSbN受到66点伤害

 0_2_wUxi88cIoS毒性发作, 0_2_wUxi88cIoS受到31点伤害

8_3_kn03spEYH3使用魅惑, 幻影被魅惑了

4_0_6LlMu64Hov发起攻击, 3_0_OIVQH30pU1回避了攻击

5_3_fq3I5eI0Pt发起攻击, 1_4_Nm8iFY9s8u受到172点伤害

 1_4_Nm8iFY9s8u被击倒了

6_2_kXNOnc6Gc1发起攻击, 8_2_hlkU3Dm0ea受到58点伤害

 8_2_hlkU3Dm0ea被击倒了

0_0_QcQymPrUFh使用火球术, 2_1_ZF0iBbPJva受到90点伤害

4_4_vpuJDEmsAb发起攻击, 2_2_3Y2CHuS2Tl受到82点伤害

 2_2_3Y2CHuS2Tl被击倒了

2_1_ZF0iBbPJva发动会心一击, 0_4_7aWRHl1hzq受到88点伤害

 0_4_7aWRHl1hzq被击倒了

 2_1_ZF0iBbPJva吞噬了0_4_7aWRHl1hzq, 2_1_ZF0iBbPJva属性上升

3_3_sMlFjQuDO2发起攻击, 5_4_lBgDQHEh5b回避了攻击

5_4_lBgDQHEh5b发起攻击, 幻影受到93点伤害

8_3_kn03spEYH3发起攻击, 1_0_NCm3rTQ6Xi受到38点伤害

 8_3_kn03spEYH3从疾走中解除

2_0_5uVmafjdIo发起攻击, 5_3_fq3I5eI0Pt受到143点伤害

 5_3_fq3I5eI0Pt被击倒了

2_1_ZF0iBbPJva使用生命之轮, 0_0_QcQymPrUFh的体力值与2_1_ZF0iBbPJva互换

1_0_NCm3rTQ6Xi发起攻击, 2_0_5uVmafjdIo受到104点伤害

 2_0_5uVmafjdIo被击倒了

8_0_Hpj3ECqmur开始蓄力

9_1_8My1bOMVCr开始聚气, 9_1_8My1bOMVCr攻击力上升

3_0_OIVQH30pU1使用减速术, 0_0_QcQymPrUFh进入迟缓状态

8_1_BNOGlrVzHs发起攻击, 1_3_HUGBHL6r0W受到78点伤害

1_1_ttAYKGCEUu发动铁壁, 1_1_ttAYKGCEUu防御力大幅上升

9_3_gOu3Qc3TFf发起攻击, 3_3_sMlFjQuDO2受到78点伤害

6_3_Alc7EY3OZz发起攻击, 3_3_sMlFjQuDO2受到78点伤害

 3_3_sMlFjQuDO2被击倒了

3_2_7NYUaThfG9发起攻击, 幻影受到86点伤害

 幻影消失了

3_4_H58PY9DUd0发起攻击, 0_2_wUxi88cIoS受到58点伤害

0_2_wUxi88cIoS发起攻击, 8_0_Hpj3ECqmur受到53点伤害

 8_0_Hpj3ECqmur被击倒了

 0_2_wUxi88cIoS毒性发作, 0_2_wUxi88cIoS受到26点伤害

 0_2_wUxi88cIoS被击倒了

4_1_bedE7ksqcc发起攻击, 7_4_fFzNsWUg4M受到140点伤害

 7_4_fFzNsWUg4M被击倒了

6_2_kXNOnc6Gc1发起攻击, 7_2_iCxkh7njv5受到87点伤害

 7_2_iCxkh7njv5被击倒了

 6_2_kXNOnc6Gc1召唤亡灵, 7_2_iCxkh7njv5变成了丧尸

4_4_vpuJDEmsAb发起攻击, 9_0_ubNPNYhYfD受到131点伤害

 9_0_ubNPNYhYfD被击倒了

4_0_6LlMu64Hov发起攻击, 2_3_fBubINRSbN受到40点伤害

1_2_q7AVVjjuyZ发起攻击, 3_2_7NYUaThfG9受到127点伤害

 3_2_7NYUaThfG9被击倒了

5_2_L6XiOdOfiz发起攻击, 8_1_BNOGlrVzHs受到41点伤害

1_3_HUGBHL6r0W发起攻击, 5_2_L6XiOdOfiz受到21点伤害

7_0_Nfp7EdR79U发起攻击, 2_1_ZF0iBbPJva受到70点伤害

 2_1_ZF0iBbPJva被击倒了

2_3_fBubINRSbN发起攻击, 9_1_8My1bOMVCr受到47点伤害

9_2_MlnfYjHeHi发起攻击, 6_2_kXNOnc6Gc1受到33点伤害

8_1_BNOGlrVzHs使用魅惑, 1_3_HUGBHL6r0W被魅惑了

5_4_lBgDQHEh5b发起攻击

 1_1_ttAYKGCEUu的铁壁被打消了, 1_1_ttAYKGCEUu受到65点伤害

 1_1_ttAYKGCEUu被击倒了

5_0_8r19XDOwrw使用加速术, 5_4_lBgDQHEh5b进入疾走状态

4_1_bedE7ksqcc发起攻击, 0_0_QcQymPrUFh受到49点伤害

 0_0_QcQymPrUFh被击倒了

9_1_8My1bOMVCr使用火球术, 1_2_q7AVVjjuyZ受到212点伤害

 1_2_q7AVVjjuyZ被击倒了

8_3_kn03spEYH3发起攻击, 4_4_vpuJDEmsAb受到59点伤害

 4_4_vpuJDEmsAb被击倒了

9_3_gOu3Qc3TFf发起攻击, 6_2_kXNOnc6Gc1守护6_3_Alc7EY3OZz, 6_2_kXNOnc6Gc1受到32点伤害

 6_2_kXNOnc6Gc1被击倒了

 丧尸消失了

1_0_NCm3rTQ6Xi使用净化, 8_3_kn03spEYH3受到62点伤害

5_0_8r19XDOwrw发起攻击, 4_1_bedE7ksqcc回避了攻击

3_0_OIVQH30pU1发起攻击, 1_0_NCm3rTQ6Xi回避了攻击

5_4_lBgDQHEh5b发起攻击, 9_2_MlnfYjHeHi受到78点伤害

 9_2_MlnfYjHeHi被击倒了

3_4_H58PY9DUd0发起攻击, 6_3_Alc7EY3OZz受到39点伤害

2_3_fBubINRSbN使用幻术, 召唤出幻影

6_3_Alc7EY3OZz使用瘟疫, 5_4_lBgDQHEh5b体力减少48%

7_0_Nfp7EdR79U发起攻击, 9_1_8My1bOMVCr受到38点伤害

 9_1_8My1bOMVCr被击倒了

 7_0_Nfp7EdR79U召唤亡灵, 9_1_8My1bOMVCr变成了丧尸

4_0_6LlMu64Hov使用净化, 1_3_HUGBHL6r0W受到35点伤害

8_1_BNOGlrVzHs使用净化, 4_0_6LlMu64Hov受到53点伤害

 4_0_6LlMu64Hov被击倒了

5_4_lBgDQHEh5b发起攻击, 1_0_NCm3rTQ6Xi受到45点伤害

 1_0_NCm3rTQ6Xi被击倒了

5_0_8r19XDOwrw发起攻击, 9_3_gOu3Qc3TFf受到114点伤害

 9_3_gOu3Qc3TFf被击倒了

4_1_bedE7ksqcc使用冰冻术, 5_4_lBgDQHEh5b回避了攻击

5_2_L6XiOdOfiz发起攻击, 2_3_fBubINRSbN受到42点伤害

 2_3_fBubINRSbN被击倒了

 幻影消失了

5_4_lBgDQHEh5b发起攻击, 3_0_OIVQH30pU1受到92点伤害

 3_0_OIVQH30pU1被击倒了

 5_4_lBgDQHEh5b从疾走中解除

3_4_H58PY9DUd0发起攻击, 丧尸受到43点伤害

1_3_HUGBHL6r0W发起攻击, 丧尸受到37点伤害

 1_3_HUGBHL6r0W从魅惑中解除

7_0_Nfp7EdR79U使用净化, 6_3_Alc7EY3OZz受到102点伤害

 6_3_Alc7EY3OZz被击倒了

5_0_8r19XDOwrw发起攻击, 丧尸受到85点伤害

 丧尸消失了

8_1_BNOGlrVzHs发起攻击, 5_2_L6XiOdOfiz受到67点伤害

 5_2_L6XiOdOfiz被击倒了

5_4_lBgDQHEh5b发起攻击, 8_1_BNOGlrVzHs受到90点伤害

 8_1_BNOGlrVzHs被击倒了

4_1_bedE7ksqcc发起攻击, 5_0_8r19XDOwrw受到37点伤害

 5_0_8r19XDOwrw被击倒了

3_4_H58PY9DUd0发起攻击, 4_1_bedE7ksqcc回避了攻击

1_3_HUGBHL6r0W发起攻击, 4_1_bedE7ksqcc受到21点伤害

7_0_Nfp7EdR79U发起攻击, 1_3_HUGBHL6r0W受到74点伤害

 1_3_HUGBHL6r0W被击倒了

4_1_bedE7ksqcc发起攻击, 3_4_H58PY9DUd0受到59点伤害

 3_4_H58PY9DUd0被击倒了

8_3_kn03spEYH3发起攻击, 4_1_bedE7ksqcc受到61点伤害

 4_1_bedE7ksqcc被击倒了

5_4_lBgDQHEh5b发起攻击, 8_3_kn03spEYH3受到107点伤害

 8_3_kn03spEYH3被击倒了

7_0_Nfp7EdR79U使用净化, 5_4_lBgDQHEh5b受到19点伤害

 5_4_lBgDQHEh5b的垂死属性被打消

5_4_lBgDQHEh5b发起攻击, 7_0_Nfp7EdR79U受到39点伤害

 7_0_Nfp7EdR79U被击倒了

"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 50_000, true);
    assert!(guard < 50_000, "fight_multi_7 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_7 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_7 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

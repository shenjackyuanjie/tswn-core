use super::*;

#[test]
fn fight_multi_1() {
    const FIGHT_CASE: &str = r###"0_0_MOV8K3yJ8E
0_1_MYJUqgcTvf
0_2_G15NqUXxTB
0_3_J51OMFvOxh
0_4_bIt68xArJe
0_5_6AWCcfGY61
0_6_rvI70S292G
0_7_SXwT0ejTet
0_8_OwqOS7ank0
0_9_FpGFUJ6Cnl

1_0_ENaYs3bJZT
1_1_Iop2CXVNnX
1_2_ICRZT2cY7J
1_3_HrV3StVkt7
1_4_V6fCvxrcwV
1_5_3Tw9xcUaNH
1_6_cCko9b1wnq
1_7_SfRxDuvg3d
1_8_mgCAkSQ1oT
1_9_9t1kCJGQcD

2_0_7Y34tqo4Pu
2_1_lFiwQr3SiM
2_2_racwRvGpa0
2_3_h8zwuw0mK2
2_4_8nEP156ThD
2_5_aqsISLfTlQ
2_6_hdlqeUUa5w
2_7_3SgztfGDuC
2_8_VwI1N0viFS
2_9_1TJyqQ1tl0

3_0_pb8RplsSNp
3_1_QX7ImZoMsJ
3_2_xS1HMYoEI2
3_3_ZOsbouvLNk
3_4_ahcxOiV5pS
3_5_t4cWHqOXek
3_6_w0KmJyB9rd
3_7_T9q1SbdruK
3_8_arfQFhTWsc
3_9_cjP9imME8k

4_0_7UXDghueip
4_1_qJ31Q6a0Wj
4_2_hIcbTZmFeW
4_3_LJ3DD76AP6
4_4_KtAftbd97c
4_5_hyHUt0Mnjf
4_6_Rwuax8lPIX
4_7_O0XB9iNpa5
4_8_7vZ6SRwZC1
4_9_gIagwj8QqB


2_5_aqsISLfTlQ发起攻击, 4_8_7vZ6SRwZC1受到32点伤害

 2_5_aqsISLfTlQ连击, 1_6_cCko9b1wnq受到41点伤害

 2_5_aqsISLfTlQ连击, 1_6_cCko9b1wnq受到44点伤害

0_7_SXwT0ejTet发起攻击, 4_2_hIcbTZmFeW受到60点伤害

0_9_FpGFUJ6Cnl使用减速术, 2_0_7Y34tqo4Pu进入迟缓状态

3_3_ZOsbouvLNk发动会心一击, 2_2_racwRvGpa0受到80点伤害

2_4_8nEP156ThD发起攻击, 4_0_7UXDghueip受到35点伤害

1_2_ICRZT2cY7J使用分身, 出现一个新的1_2_ICRZT2cY7J

4_0_7UXDghueip发起攻击, 0_3_J51OMFvOxh受到74点伤害

2_9_1TJyqQ1tl0使用幻术, 召唤出幻影

4_9_gIagwj8QqB发起攻击, 2_3_h8zwuw0mK2受到69点伤害

3_4_ahcxOiV5pS发起攻击, 1_2_ICRZT2cY7J受到65点伤害

1_9_9t1kCJGQcD发起攻击, 2_2_racwRvGpa0受到60点伤害

4_7_O0XB9iNpa5发起攻击, 0_3_J51OMFvOxh受到60点伤害

3_0_pb8RplsSNp发起攻击, 1_5_3Tw9xcUaNH受到98点伤害

3_5_t4cWHqOXek发起攻击, 4_5_hyHUt0Mnjf受到115点伤害

2_1_lFiwQr3SiM发起攻击, 0_5_6AWCcfGY61受到71点伤害

3_9_cjP9imME8k发起攻击, 1_6_cCko9b1wnq受到66点伤害

2_2_racwRvGpa0发起攻击, 3_1_QX7ImZoMsJ受到80点伤害

0_5_6AWCcfGY61发起攻击, 2_5_aqsISLfTlQ受到120点伤害

4_4_KtAftbd97c发起攻击, 2_5_aqsISLfTlQ受到48点伤害

0_6_rvI70S292G发起攻击, 2_1_lFiwQr3SiM受到76点伤害

3_8_arfQFhTWsc发起攻击, 2_0_7Y34tqo4Pu回避了攻击

0_0_MOV8K3yJ8E发起攻击, 1_8_mgCAkSQ1oT受到82点伤害

3_6_w0KmJyB9rd发起攻击, 1_0_ENaYs3bJZT受到42点伤害, 1_0_ENaYs3bJZT发动隐匿

4_1_qJ31Q6a0Wj发起攻击, 2_4_8nEP156ThD受到61点伤害

4_3_LJ3DD76AP6发起攻击, 0_0_MOV8K3yJ8E受到48点伤害

4_5_hyHUt0Mnjf发起攻击, 3_4_ahcxOiV5pS回避了攻击

0_2_G15NqUXxTB发起攻击, 1_7_SfRxDuvg3d受到78点伤害

0_8_OwqOS7ank0发起攻击, 3_8_arfQFhTWsc受到91点伤害

0_1_MYJUqgcTvf发起攻击, 2_7_3SgztfGDuC受到148点伤害, 2_7_3SgztfGDuC发动隐匿

2_7_3SgztfGDuC发起攻击, 0_2_G15NqUXxTB受到65点伤害

4_8_7vZ6SRwZC1发起攻击, 1_6_cCko9b1wnq受到49点伤害

1_3_HrV3StVkt7发起攻击, 4_0_7UXDghueip回避了攻击

1_8_mgCAkSQ1oT发起攻击, 4_7_O0XB9iNpa5受到57点伤害

1_0_ENaYs3bJZT发起攻击, 3_5_t4cWHqOXek受到23点伤害

0_4_bIt68xArJe使用狂暴术, 1_9_9t1kCJGQcD回避了攻击

1_4_V6fCvxrcwV发起攻击, 0_8_OwqOS7ank0受到37点伤害

2_6_hdlqeUUa5w发起攻击, 1_9_9t1kCJGQcD受到31点伤害

3_7_T9q1SbdruK发起攻击, 0_0_MOV8K3yJ8E受到68点伤害

0_3_J51OMFvOxh发起攻击, 3_9_cjP9imME8k受到36点伤害

2_0_7Y34tqo4Pu发起攻击, 0_6_rvI70S292G受到63点伤害

4_6_Rwuax8lPIX发起攻击, 0_1_MYJUqgcTvf回避了攻击

1_7_SfRxDuvg3d发起攻击, 0_5_6AWCcfGY61受到63点伤害

2_3_h8zwuw0mK2发起攻击, 4_8_7vZ6SRwZC1受到131点伤害

1_5_3Tw9xcUaNH发起攻击, 0_3_J51OMFvOxh受到56点伤害

2_8_VwI1N0viFS发起攻击, 3_2_xS1HMYoEI2受到85点伤害

4_2_hIcbTZmFeW发起攻击, 3_4_ahcxOiV5pS受到44点伤害

1_1_Iop2CXVNnX发起攻击, 4_0_7UXDghueip受到70点伤害

3_4_ahcxOiV5pS发起攻击, 0_5_6AWCcfGY61受到47点伤害

1_2_ICRZT2cY7J使用治愈魔法, 1_9_9t1kCJGQcD回复体力31点

0_0_MOV8K3yJ8E使用生命之轮, 1_4_V6fCvxrcwV的体力值与0_0_MOV8K3yJ8E互换

4_7_O0XB9iNpa5发起攻击, 2_9_1TJyqQ1tl0受到52点伤害

0_7_SXwT0ejTet发起攻击, 2_0_7Y34tqo4Pu受到84点伤害, 2_0_7Y34tqo4Pu发动隐匿

3_5_t4cWHqOXek发起攻击, 0_9_FpGFUJ6Cnl受到78点伤害

2_2_racwRvGpa0发起攻击, 0_9_FpGFUJ6Cnl受到50点伤害

2_5_aqsISLfTlQ发起攻击, 1_9_9t1kCJGQcD回避了攻击

2_9_1TJyqQ1tl0发起攻击, 3_0_pb8RplsSNp受到63点伤害

0_5_6AWCcfGY61发起攻击, 2_1_lFiwQr3SiM回避了攻击

3_2_xS1HMYoEI2发起攻击, 0_7_SXwT0ejTet受到67点伤害

2_1_lFiwQr3SiM使用分身, 出现一个新的2_1_lFiwQr3SiM

2_4_8nEP156ThD潜行到1_2_ICRZT2cY7J身后

3_1_QX7ImZoMsJ发起攻击, 0_3_J51OMFvOxh受到102点伤害

4_9_gIagwj8QqB使用雷击术

 2_1_lFiwQr3SiM受到23点伤害, 2_1_lFiwQr3SiM发动隐匿

 2_1_lFiwQr3SiM受到28点伤害

 2_1_lFiwQr3SiM受到17点伤害

0_6_rvI70S292G发起攻击, 1_7_SfRxDuvg3d受到68点伤害

4_3_LJ3DD76AP6使用分身, 出现一个新的4_3_LJ3DD76AP6

3_9_cjP9imME8k发起攻击, 2_8_VwI1N0viFS受到60点伤害

1_0_ENaYs3bJZT发起攻击, 0_0_MOV8K3yJ8E受到61点伤害

4_0_7UXDghueip发起攻击, 2_6_hdlqeUUa5w受到48点伤害

1_4_V6fCvxrcwV发起攻击, 2_5_aqsISLfTlQ受到56点伤害

1_6_cCko9b1wnq发起攻击, 4_9_gIagwj8QqB受到42点伤害

3_6_w0KmJyB9rd潜行到4_9_gIagwj8QqB身后

4_1_qJ31Q6a0Wj发起攻击, 1_2_ICRZT2cY7J受到58点伤害

3_3_ZOsbouvLNk发起攻击, 4_7_O0XB9iNpa5受到111点伤害

0_8_OwqOS7ank0发起攻击, 2_7_3SgztfGDuC受到102点伤害

0_1_MYJUqgcTvf发起攻击, 2_4_8nEP156ThD受到53点伤害

 2_4_8nEP156ThD的潜行被识破

1_2_ICRZT2cY7J发起攻击, 3_6_w0KmJyB9rd受到45点伤害

 3_6_w0KmJyB9rd的潜行被识破

1_5_3Tw9xcUaNH使用冰冻术, 3_5_t4cWHqOXek受到98点伤害, 3_5_t4cWHqOXek被冰冻了

4_6_Rwuax8lPIX发起攻击, 1_9_9t1kCJGQcD受到61点伤害

0_9_FpGFUJ6Cnl发起攻击, 2_6_hdlqeUUa5w防御, 2_6_hdlqeUUa5w受到29点伤害

0_2_G15NqUXxTB发起攻击, 3_3_ZOsbouvLNk受到47点伤害

1_1_Iop2CXVNnX发起攻击, 2_8_VwI1N0viFS受到105点伤害

1_3_HrV3StVkt7使用治愈魔法, 1_0_ENaYs3bJZT回复体力42点

3_7_T9q1SbdruK发起攻击, 4_3_LJ3DD76AP6受到101点伤害

2_1_lFiwQr3SiM发起攻击, 0_4_bIt68xArJe防御, 0_4_bIt68xArJe受到44点伤害

1_8_mgCAkSQ1oT发起攻击, 4_6_Rwuax8lPIX受到57点伤害

3_8_arfQFhTWsc发起攻击, 0_0_MOV8K3yJ8E受到44点伤害

4_5_hyHUt0Mnjf发起攻击, 1_0_ENaYs3bJZT受到40点伤害

4_8_7vZ6SRwZC1发起攻击, 1_3_HrV3StVkt7回避了攻击

2_6_hdlqeUUa5w发起攻击, 3_7_T9q1SbdruK受到67点伤害

3_4_ahcxOiV5pS发起攻击, 2_6_hdlqeUUa5w受到102点伤害

0_3_J51OMFvOxh潜行到1_1_Iop2CXVNnX身后

0_0_MOV8K3yJ8E使用诅咒, 1_6_cCko9b1wnq受到100点伤害, 1_6_cCko9b1wnq被诅咒了

0_7_SXwT0ejTet发起攻击, 1_9_9t1kCJGQcD受到68点伤害

3_0_pb8RplsSNp发起攻击, 2_1_lFiwQr3SiM受到64点伤害, 2_1_lFiwQr3SiM发动隐匿

2_1_lFiwQr3SiM发起攻击, 0_7_SXwT0ejTet受到50点伤害

4_2_hIcbTZmFeW发起攻击, 3_0_pb8RplsSNp受到28点伤害

0_4_bIt68xArJe发起攻击, 4_1_qJ31Q6a0Wj受到12点伤害

2_9_1TJyqQ1tl0发起攻击, 0_0_MOV8K3yJ8E受到102点伤害

4_9_gIagwj8QqB发起攻击, 1_4_V6fCvxrcwV受到97点伤害

0_5_6AWCcfGY61发起攻击, 1_1_Iop2CXVNnX受到159点伤害

4_4_KtAftbd97c发起攻击, 3_9_cjP9imME8k受到85点伤害

4_3_LJ3DD76AP6发起攻击, 1_3_HrV3StVkt7受到85点伤害

2_8_VwI1N0viFS使用加速术, 2_0_7Y34tqo4Pu进入疾走状态

4_7_O0XB9iNpa5发起吸血攻击, 0_0_MOV8K3yJ8E受到212点伤害, 4_7_O0XB9iNpa5回复体力106点

 0_0_MOV8K3yJ8E被击倒了

2_2_racwRvGpa0发起攻击, 1_0_ENaYs3bJZT回避了攻击

1_7_SfRxDuvg3d发起攻击, 3_0_pb8RplsSNp受到27点伤害

2_3_h8zwuw0mK2发起攻击, 1_8_mgCAkSQ1oT回避了攻击

2_7_3SgztfGDuC使用治愈魔法, 2_0_7Y34tqo4Pu回复体力48点

 2_0_7Y34tqo4Pu从迟缓中解除

3_5_t4cWHqOXek从冰冻中解除

2_0_7Y34tqo4Pu使用净化, 3_8_arfQFhTWsc受到90点伤害

4_6_Rwuax8lPIX开始蓄力

2_5_aqsISLfTlQ发起攻击, 4_4_KtAftbd97c受到39点伤害

2_4_8nEP156ThD发起攻击, 4_4_KtAftbd97c受到66点伤害, 4_4_KtAftbd97c发动隐匿

3_1_QX7ImZoMsJ发起攻击, 0_1_MYJUqgcTvf受到74点伤害

幻影发起攻击, 1_1_Iop2CXVNnX受到73点伤害

 1_1_Iop2CXVNnX被击倒了

1_9_9t1kCJGQcD发起攻击, 2_2_racwRvGpa0受到63点伤害

3_2_xS1HMYoEI2发起攻击, 2_2_racwRvGpa0受到63点伤害

4_3_LJ3DD76AP6使用分身, 出现一个新的4_3_LJ3DD76AP6

1_4_V6fCvxrcwV发起攻击, 3_3_ZOsbouvLNk受到38点伤害

0_6_rvI70S292G发起攻击, 1_8_mgCAkSQ1oT受到127点伤害

1_2_ICRZT2cY7J发起攻击, 4_0_7UXDghueip受到52点伤害

2_1_lFiwQr3SiM发起攻击, 0_4_bIt68xArJe回避了攻击

3_5_t4cWHqOXek发起攻击, 2_0_7Y34tqo4Pu受到78点伤害, 2_0_7Y34tqo4Pu发动隐匿

3_9_cjP9imME8k使用瘟疫, 2_3_h8zwuw0mK2体力减少46%

3_3_ZOsbouvLNk发起攻击, 4_7_O0XB9iNpa5防御, 4_7_O0XB9iNpa5受到45点伤害

4_0_7UXDghueip发起攻击, 1_3_HrV3StVkt7受到72点伤害

0_5_6AWCcfGY61发起攻击, 2_4_8nEP156ThD受到43点伤害

1_5_3Tw9xcUaNH使用幻术, 召唤出幻影

1_6_cCko9b1wnq发起攻击, 0_1_MYJUqgcTvf受到95点伤害

3_4_ahcxOiV5pS发起攻击, 4_7_O0XB9iNpa5防御, 4_7_O0XB9iNpa5受到46点伤害

3_0_pb8RplsSNp发起攻击, 2_2_racwRvGpa0受到70点伤害

 2_2_racwRvGpa0被击倒了

3_6_w0KmJyB9rd发起攻击, 4_0_7UXDghueip受到52点伤害

4_1_qJ31Q6a0Wj发起攻击, 3_8_arfQFhTWsc受到38点伤害

2_0_7Y34tqo4Pu使用净化, 3_3_ZOsbouvLNk受到89点伤害

0_1_MYJUqgcTvf发起攻击, 4_9_gIagwj8QqB受到62点伤害

1_2_ICRZT2cY7J发起攻击, 4_3_LJ3DD76AP6受到44点伤害

1_0_ENaYs3bJZT发起攻击, 2_9_1TJyqQ1tl0受到52点伤害

4_8_7vZ6SRwZC1发起攻击, 3_4_ahcxOiV5pS受到105点伤害, 3_4_ahcxOiV5pS发动隐匿

4_9_gIagwj8QqB发起攻击, 3_6_w0KmJyB9rd受到60点伤害

2_6_hdlqeUUa5w使用冰冻术, 1_7_SfRxDuvg3d受到30点伤害, 1_7_SfRxDuvg3d被冰冻了

 1_7_SfRxDuvg3d做出垂死抗争, 1_7_SfRxDuvg3d所有属性上升

3_7_T9q1SbdruK开始聚气, 3_7_T9q1SbdruK攻击力上升

0_3_J51OMFvOxh发起攻击, 3_2_xS1HMYoEI2受到28点伤害

4_6_Rwuax8lPIX发起攻击, 1_0_ENaYs3bJZT受到110点伤害

0_9_FpGFUJ6Cnl发起攻击, 3_1_QX7ImZoMsJ受到68点伤害

4_2_hIcbTZmFeW使用减速术, 3_3_ZOsbouvLNk进入迟缓状态

0_8_OwqOS7ank0发起攻击, 1_6_cCko9b1wnq受到91点伤害

 1_6_cCko9b1wnq被击倒了

2_9_1TJyqQ1tl0发起攻击, 0_6_rvI70S292G受到57点伤害

0_7_SXwT0ejTet发起攻击, 3_7_T9q1SbdruK受到112点伤害

2_1_lFiwQr3SiM发起攻击, 4_6_Rwuax8lPIX受到92点伤害

3_9_cjP9imME8k发起攻击, 0_7_SXwT0ejTet受到39点伤害

4_5_hyHUt0Mnjf发起攻击, 0_9_FpGFUJ6Cnl受到40点伤害

2_7_3SgztfGDuC发起攻击, 4_8_7vZ6SRwZC1受到34点伤害

0_6_rvI70S292G使用血祭, 召唤出使魔

1_8_mgCAkSQ1oT发起攻击, 3_2_xS1HMYoEI2受到53点伤害

3_2_xS1HMYoEI2发起攻击, 0_4_bIt68xArJe受到92点伤害

0_2_G15NqUXxTB发起攻击, 2_8_VwI1N0viFS受到109点伤害

2_3_h8zwuw0mK2发起攻击, 0_2_G15NqUXxTB受到56点伤害

1_3_HrV3StVkt7使用治愈魔法, 1_7_SfRxDuvg3d回复体力137点

 1_7_SfRxDuvg3d从冰冻中解除

3_7_T9q1SbdruK发动会心一击, 4_1_qJ31Q6a0Wj回避了攻击

4_3_LJ3DD76AP6发起攻击, 3_2_xS1HMYoEI2受到62点伤害

2_8_VwI1N0viFS发起攻击, 4_9_gIagwj8QqB受到72点伤害

3_8_arfQFhTWsc发起攻击, 0_6_rvI70S292G受到62点伤害

4_7_O0XB9iNpa5发起攻击, 3_3_ZOsbouvLNk受到121点伤害

 3_3_ZOsbouvLNk被击倒了

4_6_Rwuax8lPIX发起攻击, 3_5_t4cWHqOXek受到69点伤害

4_0_7UXDghueip发起攻击, 3_0_pb8RplsSNp受到56点伤害

4_4_KtAftbd97c发起攻击, 2_8_VwI1N0viFS受到31点伤害

 2_8_VwI1N0viFS被击倒了

3_6_w0KmJyB9rd发起攻击, 2_1_lFiwQr3SiM受到79点伤害

 2_1_lFiwQr3SiM被击倒了

2_0_7Y34tqo4Pu发起攻击, 1_2_ICRZT2cY7J受到83点伤害

 2_0_7Y34tqo4Pu从疾走中解除

0_8_OwqOS7ank0发起攻击, 4_8_7vZ6SRwZC1受到86点伤害

2_5_aqsISLfTlQ发起攻击, 4_7_O0XB9iNpa5受到80点伤害

1_2_ICRZT2cY7J发起攻击, 2_9_1TJyqQ1tl0回避了攻击

0_4_bIt68xArJe发起攻击, 3_1_QX7ImZoMsJ回避了攻击

0_5_6AWCcfGY61使用狂暴术, 4_6_Rwuax8lPIX受到28点伤害, 4_6_Rwuax8lPIX进入狂暴状态

幻影发起攻击, 0_9_FpGFUJ6Cnl受到76点伤害

 0_9_FpGFUJ6Cnl被击倒了

使魔使用自爆, 4_9_gIagwj8QqB受到154点伤害

 4_9_gIagwj8QqB被击倒了

 使魔消失了

1_9_9t1kCJGQcD发起攻击, 0_4_bIt68xArJe回避了攻击

2_1_lFiwQr3SiM发起攻击, 3_0_pb8RplsSNp受到11点伤害

4_5_hyHUt0Mnjf发起攻击, 1_3_HrV3StVkt7受到69点伤害

0_1_MYJUqgcTvf发起攻击, 4_3_LJ3DD76AP6受到54点伤害

3_1_QX7ImZoMsJ发起吸血攻击, 4_5_hyHUt0Mnjf受到66点伤害, 3_1_QX7ImZoMsJ回复体力33点

4_8_7vZ6SRwZC1发起攻击, 3_9_cjP9imME8k受到103点伤害

2_9_1TJyqQ1tl0发起攻击, 0_1_MYJUqgcTvf受到56点伤害

0_6_rvI70S292G投毒, 1_5_3Tw9xcUaNH受到39点伤害, 1_5_3Tw9xcUaNH中毒

3_4_ahcxOiV5pS发起攻击, 4_3_LJ3DD76AP6受到59点伤害

 4_3_LJ3DD76AP6被击倒了

4_3_LJ3DD76AP6发起攻击, 0_8_OwqOS7ank0受到90点伤害

3_0_pb8RplsSNp使用魅惑, 0_5_6AWCcfGY61被魅惑了

2_4_8nEP156ThD发起攻击, 4_5_hyHUt0Mnjf受到81点伤害

1_4_V6fCvxrcwV发起攻击, 3_7_T9q1SbdruK受到28点伤害

0_7_SXwT0ejTet使用火球术, 4_6_Rwuax8lPIX受到51点伤害

4_1_qJ31Q6a0Wj发起攻击, 幻影受到42点伤害

4_3_LJ3DD76AP6发起攻击, 3_2_xS1HMYoEI2受到44点伤害

3_9_cjP9imME8k发起攻击, 4_1_qJ31Q6a0Wj受到34点伤害

4_2_hIcbTZmFeW发起攻击, 0_3_J51OMFvOxh受到38点伤害

 0_3_J51OMFvOxh被击倒了

0_8_OwqOS7ank0发起攻击, 4_6_Rwuax8lPIX受到116点伤害

 4_6_Rwuax8lPIX被击倒了

2_7_3SgztfGDuC发起攻击, 0_2_G15NqUXxTB受到48点伤害

1_5_3Tw9xcUaNH使用冰冻术, 3_9_cjP9imME8k受到47点伤害

 3_9_cjP9imME8k被击倒了

 1_5_3Tw9xcUaNH毒性发作, 1_5_3Tw9xcUaNH受到16点伤害

4_4_KtAftbd97c发起攻击, 1_0_ENaYs3bJZT受到58点伤害, 1_0_ENaYs3bJZT发动隐匿

1_8_mgCAkSQ1oT使用魅惑, 4_1_qJ31Q6a0Wj回避了攻击

3_2_xS1HMYoEI2使用血祭, 召唤出使魔

3_5_t4cWHqOXek发起攻击, 1_7_SfRxDuvg3d受到81点伤害

2_3_h8zwuw0mK2发起攻击, 0_8_OwqOS7ank0受到101点伤害

1_0_ENaYs3bJZT使用瘟疫, 2_6_hdlqeUUa5w体力减少40%

2_9_1TJyqQ1tl0发起攻击, 使魔受到47点伤害, 3_2_xS1HMYoEI2受到23点伤害

 3_2_xS1HMYoEI2被击倒了, 3_2_xS1HMYoEI2使用护身符抵挡了一次死亡, 3_2_xS1HMYoEI2回复体力6点

1_3_HrV3StVkt7使用火球术, 0_1_MYJUqgcTvf受到38点伤害

3_7_T9q1SbdruK发起攻击, 1_3_HrV3StVkt7受到102点伤害

 1_3_HrV3StVkt7被击倒了

幻影使用附体, 1_7_SfRxDuvg3d进入狂暴状态

 幻影消失了

1_9_9t1kCJGQcD使用苏生术, 1_1_Iop2CXVNnX复活了, 1_1_Iop2CXVNnX回复体力80点

0_2_G15NqUXxTB发起攻击, 使魔受到64点伤害, 3_2_xS1HMYoEI2受到32点伤害

 3_2_xS1HMYoEI2被击倒了

 使魔消失了

1_7_SfRxDuvg3d发起攻击, 4_1_qJ31Q6a0Wj受到74点伤害

0_1_MYJUqgcTvf发起攻击, 2_7_3SgztfGDuC受到129点伤害

 2_7_3SgztfGDuC被击倒了

2_5_aqsISLfTlQ潜行到4_4_KtAftbd97c身后

4_0_7UXDghueip发起攻击, 3_7_T9q1SbdruK回避了攻击

0_5_6AWCcfGY61使用诅咒, 幻影受到26点伤害, 幻影被诅咒了

 0_5_6AWCcfGY61从魅惑中解除

3_4_ahcxOiV5pS发起攻击, 1_0_ENaYs3bJZT受到66点伤害

1_2_ICRZT2cY7J发起攻击, 4_2_hIcbTZmFeW受到82点伤害

幻影发起攻击, 3_1_QX7ImZoMsJ受到135点伤害

1_1_Iop2CXVNnX使用狂暴术, 3_0_pb8RplsSNp受到56点伤害, 3_0_pb8RplsSNp进入狂暴状态

2_1_lFiwQr3SiM发起攻击, 4_2_hIcbTZmFeW回避了攻击

2_0_7Y34tqo4Pu开始蓄力

2_6_hdlqeUUa5w发起攻击, 0_1_MYJUqgcTvf回避了攻击

0_7_SXwT0ejTet发起攻击, 4_8_7vZ6SRwZC1受到89点伤害

 4_8_7vZ6SRwZC1被击倒了

3_1_QX7ImZoMsJ发起攻击, 1_4_V6fCvxrcwV受到80点伤害

0_4_bIt68xArJe发起攻击, 1_5_3Tw9xcUaNH回避了攻击

3_8_arfQFhTWsc发起攻击, 4_0_7UXDghueip回避了攻击

3_0_pb8RplsSNp发起狂暴攻击, 2_1_lFiwQr3SiM受到81点伤害

 2_1_lFiwQr3SiM被击倒了

 3_0_pb8RplsSNp从狂暴中解除

3_6_w0KmJyB9rd发起攻击, 0_4_bIt68xArJe受到53点伤害, 0_4_bIt68xArJe发动隐匿

1_7_SfRxDuvg3d发起攻击, 2_9_1TJyqQ1tl0受到96点伤害

1_0_ENaYs3bJZT发起攻击, 3_7_T9q1SbdruK受到120点伤害

 3_7_T9q1SbdruK被击倒了

1_4_V6fCvxrcwV发起攻击, 4_3_LJ3DD76AP6受到101点伤害

 4_3_LJ3DD76AP6被击倒了

0_6_rvI70S292G开始蓄力

4_1_qJ31Q6a0Wj使用治愈魔法, 4_4_KtAftbd97c回复体力80点

3_5_t4cWHqOXek发起攻击, 2_3_h8zwuw0mK2受到67点伤害

2_4_8nEP156ThD发起攻击, 1_7_SfRxDuvg3d受到33点伤害

4_3_LJ3DD76AP6发起攻击, 1_8_mgCAkSQ1oT回避了攻击

1_9_9t1kCJGQcD发起攻击, 2_0_7Y34tqo4Pu受到43点伤害, 2_0_7Y34tqo4Pu发动隐匿

4_7_O0XB9iNpa5发起攻击, 2_9_1TJyqQ1tl0回避了攻击

2_0_7Y34tqo4Pu发起攻击, 0_7_SXwT0ejTet守护0_2_G15NqUXxTB, 0_7_SXwT0ejTet受到114点伤害

 0_7_SXwT0ejTet被击倒了

4_2_hIcbTZmFeW发起攻击, 3_8_arfQFhTWsc受到57点伤害

0_8_OwqOS7ank0使用狂暴术, 幻影回避了攻击

2_5_aqsISLfTlQ发动背刺, 4_4_KtAftbd97c受到358点伤害

 4_4_KtAftbd97c被击倒了

1_2_ICRZT2cY7J发起攻击, 3_4_ahcxOiV5pS受到47点伤害

2_9_1TJyqQ1tl0发起攻击, 1_9_9t1kCJGQcD受到61点伤害

0_5_6AWCcfGY61发起攻击, 2_4_8nEP156ThD受到43点伤害

2_6_hdlqeUUa5w发起攻击, 1_7_SfRxDuvg3d受到26点伤害

1_1_Iop2CXVNnX发起攻击, 4_2_hIcbTZmFeW受到86点伤害

0_2_G15NqUXxTB发动铁壁, 0_2_G15NqUXxTB防御力大幅上升

4_0_7UXDghueip发起攻击, 1_7_SfRxDuvg3d受到92点伤害

 1_7_SfRxDuvg3d被击倒了

1_5_3Tw9xcUaNH发起攻击, 2_5_aqsISLfTlQ受到44点伤害

 1_5_3Tw9xcUaNH毒性发作, 1_5_3Tw9xcUaNH受到13点伤害

0_1_MYJUqgcTvf使用生命之轮, 3_0_pb8RplsSNp回避了攻击

2_3_h8zwuw0mK2发起攻击, 1_9_9t1kCJGQcD回避了攻击

1_0_ENaYs3bJZT使用瘟疫, 4_7_O0XB9iNpa5体力减少46%

0_4_bIt68xArJe使用火球术, 4_1_qJ31Q6a0Wj受到104点伤害

0_6_rvI70S292G投毒, 3_8_arfQFhTWsc受到170点伤害

 3_8_arfQFhTWsc被击倒了

4_5_hyHUt0Mnjf发起攻击, 2_3_h8zwuw0mK2受到81点伤害

 2_3_h8zwuw0mK2被击倒了

3_4_ahcxOiV5pS发起攻击, 0_8_OwqOS7ank0受到94点伤害

 0_8_OwqOS7ank0被击倒了

 3_4_ahcxOiV5pS吞噬了0_8_OwqOS7ank0, 3_4_ahcxOiV5pS属性上升

2_4_8nEP156ThD使用火球术, 3_6_w0KmJyB9rd受到48点伤害

1_2_ICRZT2cY7J发起攻击, 2_9_1TJyqQ1tl0受到98点伤害

 2_9_1TJyqQ1tl0被击倒了

2_0_7Y34tqo4Pu发起吸血攻击, 4_2_hIcbTZmFeW受到68点伤害, 2_0_7Y34tqo4Pu回复体力34点

 4_2_hIcbTZmFeW被击倒了

2_5_aqsISLfTlQ发起攻击, 1_8_mgCAkSQ1oT回避了攻击

3_1_QX7ImZoMsJ发起攻击, 0_2_G15NqUXxTB受到1点伤害

1_5_3Tw9xcUaNH发起攻击, 3_1_QX7ImZoMsJ受到50点伤害

 3_1_QX7ImZoMsJ被击倒了

 1_5_3Tw9xcUaNH毒性发作, 1_5_3Tw9xcUaNH受到11点伤害

1_2_ICRZT2cY7J发起攻击, 0_2_G15NqUXxTB受到1点伤害

3_4_ahcxOiV5pS使用苏生术, 3_8_arfQFhTWsc复活了, 3_8_arfQFhTWsc回复体力109点

1_1_Iop2CXVNnX发起攻击, 4_5_hyHUt0Mnjf受到27点伤害

 4_5_hyHUt0Mnjf被击倒了

3_6_w0KmJyB9rd发起攻击, 1_9_9t1kCJGQcD受到151点伤害

 1_9_9t1kCJGQcD被击倒了

4_0_7UXDghueip发起攻击, 1_5_3Tw9xcUaNH受到61点伤害

 1_5_3Tw9xcUaNH被击倒了

 幻影消失了

4_3_LJ3DD76AP6发起攻击, 3_5_t4cWHqOXek受到73点伤害

3_8_arfQFhTWsc发起攻击, 0_5_6AWCcfGY61受到17点伤害

1_8_mgCAkSQ1oT发起攻击, 3_0_pb8RplsSNp受到65点伤害

 3_0_pb8RplsSNp被击倒了

3_5_t4cWHqOXek发起攻击, 1_1_Iop2CXVNnX受到46点伤害

0_1_MYJUqgcTvf发起攻击, 3_5_t4cWHqOXek受到84点伤害

 3_5_t4cWHqOXek被击倒了

0_6_rvI70S292G发起吸血攻击, 1_8_mgCAkSQ1oT回避了攻击

4_7_O0XB9iNpa5发起吸血攻击, 1_4_V6fCvxrcwV受到171点伤害, 4_7_O0XB9iNpa5回复体力86点

 1_4_V6fCvxrcwV被击倒了

4_1_qJ31Q6a0Wj使用魅惑, 2_0_7Y34tqo4Pu被魅惑了

2_4_8nEP156ThD发起攻击, 0_6_rvI70S292G受到68点伤害

1_0_ENaYs3bJZT发起攻击, 3_4_ahcxOiV5pS回避了攻击

0_4_bIt68xArJe发起攻击, 4_3_LJ3DD76AP6守护4_7_O0XB9iNpa5, 4_3_LJ3DD76AP6受到30点伤害

 4_3_LJ3DD76AP6被击倒了

0_5_6AWCcfGY61发起攻击, 2_6_hdlqeUUa5w受到42点伤害

1_2_ICRZT2cY7J发起攻击, 4_7_O0XB9iNpa5受到54点伤害

1_1_Iop2CXVNnX使用火球术, 3_4_ahcxOiV5pS回避了攻击

0_2_G15NqUXxTB发起攻击, 2_5_aqsISLfTlQ受到40点伤害

 2_5_aqsISLfTlQ被击倒了

4_0_7UXDghueip发起攻击, 1_1_Iop2CXVNnX受到131点伤害

 1_1_Iop2CXVNnX被击倒了

2_6_hdlqeUUa5w使用冰冻术, 4_7_O0XB9iNpa5受到40点伤害, 4_7_O0XB9iNpa5被冰冻了

1_8_mgCAkSQ1oT发起攻击, 2_0_7Y34tqo4Pu受到91点伤害, 2_0_7Y34tqo4Pu发动隐匿

2_0_7Y34tqo4Pu使用净化, 1_2_ICRZT2cY7J受到104点伤害

 1_2_ICRZT2cY7J被击倒了, 1_2_ICRZT2cY7J使用护身符抵挡了一次死亡, 1_2_ICRZT2cY7J回复体力2点

 2_0_7Y34tqo4Pu从魅惑中解除

2_4_8nEP156ThD发起攻击, 0_2_G15NqUXxTB回避了攻击

0_4_bIt68xArJe发起攻击, 1_0_ENaYs3bJZT受到20点伤害

 1_0_ENaYs3bJZT被击倒了

0_6_rvI70S292G发起吸血攻击, 2_0_7Y34tqo4Pu受到47点伤害, 0_6_rvI70S292G回复体力24点, 2_0_7Y34tqo4Pu发动隐匿

3_6_w0KmJyB9rd发起攻击, 2_0_7Y34tqo4Pu受到97点伤害

 2_0_7Y34tqo4Pu被击倒了

1_2_ICRZT2cY7J发起攻击, 0_2_G15NqUXxTB受到1点伤害

1_2_ICRZT2cY7J发起攻击, 4_7_O0XB9iNpa5受到57点伤害

 4_7_O0XB9iNpa5被击倒了

0_2_G15NqUXxTB发起攻击, 4_1_qJ31Q6a0Wj受到76点伤害

 4_1_qJ31Q6a0Wj被击倒了

 0_2_G15NqUXxTB从铁壁中解除

3_8_arfQFhTWsc发起攻击, 0_5_6AWCcfGY61受到61点伤害

0_1_MYJUqgcTvf发起攻击, 2_4_8nEP156ThD受到63点伤害

 2_4_8nEP156ThD被击倒了

 0_1_MYJUqgcTvf召唤亡灵, 2_4_8nEP156ThD变成了丧尸

4_0_7UXDghueip使用地裂术

 3_6_w0KmJyB9rd受到32点伤害

 0_5_6AWCcfGY61使用伤害反弹, 4_0_7UXDghueip受到28点伤害

 0_6_rvI70S292G受到39点伤害

 丧尸受到51点伤害

3_4_ahcxOiV5pS使用狂暴术, 0_2_G15NqUXxTB受到61点伤害, 0_2_G15NqUXxTB进入狂暴状态

1_8_mgCAkSQ1oT使用魅惑, 3_6_w0KmJyB9rd被魅惑了

0_4_bIt68xArJe发起攻击, 3_6_w0KmJyB9rd受到49点伤害

 3_6_w0KmJyB9rd被击倒了

2_6_hdlqeUUa5w使用净化, 丧尸受到123点伤害

 丧尸消失了

0_5_6AWCcfGY61发起攻击, 1_2_ICRZT2cY7J受到63点伤害

0_6_rvI70S292G发起攻击, 1_8_mgCAkSQ1oT受到0点伤害

3_8_arfQFhTWsc发起攻击, 1_2_ICRZT2cY7J受到23点伤害

 1_2_ICRZT2cY7J被击倒了, 1_2_ICRZT2cY7J使用护身符抵挡了一次死亡, 1_2_ICRZT2cY7J回复体力7点

0_1_MYJUqgcTvf发动铁壁, 0_1_MYJUqgcTvf防御力大幅上升

3_4_ahcxOiV5pS发起攻击, 0_2_G15NqUXxTB受到63点伤害

 0_2_G15NqUXxTB被击倒了

1_8_mgCAkSQ1oT发起攻击, 2_6_hdlqeUUa5w受到66点伤害

 2_6_hdlqeUUa5w被击倒了

1_2_ICRZT2cY7J发起攻击, 0_1_MYJUqgcTvf受到1点伤害

1_2_ICRZT2cY7J发起攻击, 0_4_bIt68xArJe受到45点伤害

0_5_6AWCcfGY61使用诅咒, 1_8_mgCAkSQ1oT受到115点伤害

 1_8_mgCAkSQ1oT被击倒了

3_8_arfQFhTWsc发起攻击, 0_1_MYJUqgcTvf受到1点伤害

0_1_MYJUqgcTvf发起攻击, 1_2_ICRZT2cY7J受到149点伤害

 1_2_ICRZT2cY7J被击倒了

 0_1_MYJUqgcTvf召唤亡灵, 1_2_ICRZT2cY7J变成了丧尸

4_0_7UXDghueip发起攻击, 丧尸受到121点伤害

0_4_bIt68xArJe使用火球术, 3_8_arfQFhTWsc受到25点伤害

0_6_rvI70S292G发起攻击, 3_8_arfQFhTWsc受到25点伤害

 0_6_rvI70S292G连击, 3_8_arfQFhTWsc受到44点伤害

3_4_ahcxOiV5pS使用狂暴术, 0_4_bIt68xArJe受到121点伤害

 0_4_bIt68xArJe被击倒了

0_5_6AWCcfGY61发起攻击, 3_4_ahcxOiV5pS受到45点伤害

3_8_arfQFhTWsc发动铁壁, 3_8_arfQFhTWsc防御力大幅上升

1_2_ICRZT2cY7J发起攻击, 0_5_6AWCcfGY61使用伤害反弹, 1_2_ICRZT2cY7J回避了攻击

0_1_MYJUqgcTvf发起攻击, 3_4_ahcxOiV5pS受到41点伤害

 3_4_ahcxOiV5pS被击倒了

 0_1_MYJUqgcTvf召唤亡灵, 3_4_ahcxOiV5pS变成了丧尸

 0_1_MYJUqgcTvf从铁壁中解除

4_0_7UXDghueip发起攻击, 0_1_MYJUqgcTvf受到68点伤害

 0_1_MYJUqgcTvf被击倒了

 丧尸消失了

 丧尸消失了

0_6_rvI70S292G发起吸血攻击, 4_0_7UXDghueip受到56点伤害, 0_6_rvI70S292G回复体力28点

 4_0_7UXDghueip被击倒了

1_2_ICRZT2cY7J使用分身, 出现一个新的1_2_ICRZT2cY7J

0_5_6AWCcfGY61发起攻击, 1_2_ICRZT2cY7J受到64点伤害

 1_2_ICRZT2cY7J被击倒了

3_8_arfQFhTWsc发起攻击, 1_2_ICRZT2cY7J回避了攻击

0_5_6AWCcfGY61使用狂暴术, 3_8_arfQFhTWsc受到1点伤害, 3_8_arfQFhTWsc进入狂暴状态

1_2_ICRZT2cY7J使用治愈魔法, 1_2_ICRZT2cY7J回复体力67点

0_5_6AWCcfGY61发起攻击, 1_2_ICRZT2cY7J受到86点伤害

 1_2_ICRZT2cY7J被击倒了

 0_5_6AWCcfGY61吞噬了1_2_ICRZT2cY7J, 0_5_6AWCcfGY61属性上升

0_6_rvI70S292G投毒, 3_8_arfQFhTWsc受到1点伤害

3_8_arfQFhTWsc发起狂暴攻击, 3_8_arfQFhTWsc受到1点伤害

 3_8_arfQFhTWsc从狂暴中解除

 3_8_arfQFhTWsc从铁壁中解除

0_5_6AWCcfGY61发起攻击, 3_8_arfQFhTWsc受到110点伤害

 3_8_arfQFhTWsc被击倒了"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 50_000, true);
    assert_eq!(total_score, 26018, "fight_multi_1 score mismatch");
    assert!(guard < 50_000, "fight_multi_1 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_1 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_1 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

use super::*;

#[test]
fn fight_multi_3() {
    const FIGHT_CASE: &str = r###"0_0_xgmfGTxsgJ
0_1_EYFqrDPqa6
0_2_R0dQORCsVA
0_3_FBFFeNkNSk
0_4_P1FJT96SJA

1_0_p1p6k5PNcD
1_1_pKBLm6GEAC
1_2_7H1vlZNszA
1_3_pK5scFsXo9
1_4_WVkVlsThvI

2_0_aXLsxIS6Ia
2_1_uysWKZlXH6
2_2_bwVXNklpsw
2_3_tHnczzzMsq
2_4_b6JyAxXZxw

3_0_XTHHn9wEV2
3_1_7zuwHRlIaW
3_2_yYDVJIHkJB
3_3_j51w0pP8RT
3_4_JcaediYDzK

4_0_lVbvLEiXDZ
4_1_ft7jIfTgNj
4_2_NruYBCUIZA
4_3_zceJX8EHfN
4_4_sX1ESd2D2y

5_0_wmIWDtdAYU
5_1_CwY40TkXyv
5_2_t3BK8haK6v
5_3_SrzWF3LbKc
5_4_rYaAhrRwCU

6_0_1wsxbMAkKd
6_1_14AVsCIznH
6_2_3RX0LxbGos
6_3_CR3OsxQ1rZ
6_4_7cFkTEybUG

7_0_xT4XQqpP8G
7_1_lubb3aK7nA
7_2_XjP0Tr6J0Y
7_3_aZRt6BSHCf
7_4_QmVZ678Hg0

8_0_AlkBWAyz1Z
8_1_svHOQH6cia
8_2_Be5zzcfPi7
8_3_Bbqi8P2zLC
8_4_elrW4qH2o1

9_0_t0V0qFkSgB
9_1_gohCojhCZA
9_2_WYbpcKNF82
9_3_hYmz070eWn
9_4_k8iLxetXMp


6_1_14AVsCIznH发起攻击, 4_0_lVbvLEiXDZ受到36点伤害

4_0_lVbvLEiXDZ使用魅惑, 2_3_tHnczzzMsq被魅惑了

5_1_CwY40TkXyv潜行到6_0_1wsxbMAkKd身后

5_0_wmIWDtdAYU发起攻击, 0_1_EYFqrDPqa6受到105点伤害

8_0_AlkBWAyz1Z发起攻击, 6_0_1wsxbMAkKd受到146点伤害

8_4_elrW4qH2o1使用净化, 4_0_lVbvLEiXDZ受到45点伤害

3_1_7zuwHRlIaW发起攻击, 2_3_tHnczzzMsq受到84点伤害

1_2_7H1vlZNszA发起攻击, 9_4_k8iLxetXMp受到71点伤害

8_1_svHOQH6cia发起攻击, 5_0_wmIWDtdAYU回避了攻击

7_0_xT4XQqpP8G使用火球术, 1_2_7H1vlZNszA受到162点伤害

0_4_P1FJT96SJA使用诅咒, 2_4_b6JyAxXZxw受到50点伤害, 2_4_b6JyAxXZxw被诅咒了

4_2_NruYBCUIZA发起攻击, 8_4_elrW4qH2o1受到101点伤害

0_3_FBFFeNkNSk发起攻击, 9_3_hYmz070eWn受到38点伤害, 9_3_hYmz070eWn发动隐匿

7_3_aZRt6BSHCf发起攻击, 3_4_JcaediYDzK受到152点伤害

9_1_gohCojhCZA潜行到1_4_WVkVlsThvI身后

2_1_uysWKZlXH6发起攻击, 7_2_XjP0Tr6J0Y受到33点伤害

4_3_zceJX8EHfN发起攻击, 3_1_7zuwHRlIaW受到52点伤害

7_4_QmVZ678Hg0发起攻击, 0_2_R0dQORCsVA受到82点伤害

9_2_WYbpcKNF82使用雷击术

 7_2_XjP0Tr6J0Y防御, 7_2_XjP0Tr6J0Y受到12点伤害

 7_2_XjP0Tr6J0Y受到20点伤害

 7_2_XjP0Tr6J0Y受到35点伤害

 7_2_XjP0Tr6J0Y受到40点伤害

 7_2_XjP0Tr6J0Y受到40点伤害

8_2_Be5zzcfPi7使用净化, 5_4_rYaAhrRwCU受到46点伤害

9_3_hYmz070eWn发动铁壁, 9_3_hYmz070eWn防御力大幅上升

7_2_XjP0Tr6J0Y发起攻击, 1_4_WVkVlsThvI受到31点伤害

2_4_b6JyAxXZxw使用血祭, 召唤出使魔

4_1_ft7jIfTgNj投毒, 7_2_XjP0Tr6J0Y受到10点伤害, 7_2_XjP0Tr6J0Y中毒

2_2_bwVXNklpsw使用加速术, 2_4_b6JyAxXZxw进入疾走状态

3_2_yYDVJIHkJB发起攻击, 0_4_P1FJT96SJA受到70点伤害

3_0_XTHHn9wEV2使用魅惑, 8_3_Bbqi8P2zLC回避了攻击

6_0_1wsxbMAkKd发起攻击, 0_3_FBFFeNkNSk受到113点伤害, 0_3_FBFFeNkNSk发动隐匿

0_0_xgmfGTxsgJ发起攻击, 1_0_p1p6k5PNcD受到172点伤害

7_1_lubb3aK7nA使用治愈魔法, 7_2_XjP0Tr6J0Y回复体力181点

 7_2_XjP0Tr6J0Y从中毒中解除

5_4_rYaAhrRwCU发起攻击, 3_2_yYDVJIHkJB受到74点伤害

2_3_tHnczzzMsq发起攻击, 3_1_7zuwHRlIaW受到113点伤害

 2_3_tHnczzzMsq从魅惑中解除

1_1_pKBLm6GEAC发起攻击, 7_2_XjP0Tr6J0Y受到85点伤害

3_3_j51w0pP8RT发起攻击, 6_0_1wsxbMAkKd受到63点伤害

6_3_CR3OsxQ1rZ发起攻击, 2_1_uysWKZlXH6受到102点伤害

4_4_sX1ESd2D2y发起攻击, 1_3_pK5scFsXo9受到81点伤害

9_4_k8iLxetXMp发起攻击, 3_0_XTHHn9wEV2受到57点伤害

5_3_SrzWF3LbKc发起攻击, 4_2_NruYBCUIZA受到42点伤害

2_0_aXLsxIS6Ia发起攻击, 7_2_XjP0Tr6J0Y回避了攻击

8_3_Bbqi8P2zLC发起攻击, 7_3_aZRt6BSHCf受到128点伤害

3_4_JcaediYDzK发起攻击, 8_3_Bbqi8P2zLC受到61点伤害

9_0_t0V0qFkSgB发起攻击, 7_2_XjP0Tr6J0Y受到41点伤害

1_4_WVkVlsThvI发起攻击, 9_1_gohCojhCZA受到42点伤害

 9_1_gohCojhCZA的潜行被识破

1_0_p1p6k5PNcD发起攻击, 0_2_R0dQORCsVA受到43点伤害

5_1_CwY40TkXyv发动背刺, 6_0_1wsxbMAkKd受到419点伤害

 6_0_1wsxbMAkKd被击倒了

8_0_AlkBWAyz1Z发起攻击, 0_0_xgmfGTxsgJ回避了攻击

5_2_t3BK8haK6v发起攻击, 9_3_hYmz070eWn受到1点伤害

0_2_R0dQORCsVA发起攻击, 6_1_14AVsCIznH受到58点伤害

4_3_zceJX8EHfN发起攻击, 3_2_yYDVJIHkJB受到98点伤害

1_2_7H1vlZNszA发起攻击, 5_1_CwY40TkXyv受到70点伤害

0_1_EYFqrDPqa6发起攻击, 6_2_3RX0LxbGos受到144点伤害

6_2_3RX0LxbGos发起攻击, 使魔受到65点伤害, 2_4_b6JyAxXZxw受到32点伤害

2_3_tHnczzzMsq使用净化, 7_3_aZRt6BSHCf受到114点伤害

使魔发起攻击, 3_0_XTHHn9wEV2受到58点伤害

5_0_wmIWDtdAYU发起攻击, 7_1_lubb3aK7nA受到65点伤害

6_4_7cFkTEybUG发起攻击, 2_2_bwVXNklpsw受到95点伤害

0_4_P1FJT96SJA开始聚气, 0_4_P1FJT96SJA攻击力上升

2_2_bwVXNklpsw发起攻击, 9_3_hYmz070eWn受到1点伤害

9_1_gohCojhCZA发起攻击, 7_3_aZRt6BSHCf受到114点伤害

 7_3_aZRt6BSHCf被击倒了

3_1_7zuwHRlIaW发起攻击, 8_2_Be5zzcfPi7受到65点伤害

1_3_pK5scFsXo9使用狂暴术, 0_0_xgmfGTxsgJ受到42点伤害, 0_0_xgmfGTxsgJ进入狂暴状态

7_0_xT4XQqpP8G发起攻击, 9_0_t0V0qFkSgB受到33点伤害

0_0_xgmfGTxsgJ发起狂暴攻击, 0_4_P1FJT96SJA受到77点伤害

 0_0_xgmfGTxsgJ从狂暴中解除

5_3_SrzWF3LbKc发起攻击, 0_1_EYFqrDPqa6受到46点伤害

7_1_lubb3aK7nA使用治愈魔法, 7_2_XjP0Tr6J0Y回复体力77点

5_4_rYaAhrRwCU发起攻击, 7_0_xT4XQqpP8G受到50点伤害

1_1_pKBLm6GEAC发起攻击, 9_3_hYmz070eWn受到1点伤害

3_3_j51w0pP8RT使用魅惑, 8_4_elrW4qH2o1回避了攻击

2_4_b6JyAxXZxw发起攻击, 7_1_lubb3aK7nA受到49点伤害

6_1_14AVsCIznH发起攻击, 5_2_t3BK8haK6v受到84点伤害

3_2_yYDVJIHkJB开始蓄力

4_2_NruYBCUIZA发起攻击, 9_1_gohCojhCZA受到112点伤害

0_3_FBFFeNkNSk发起攻击, 3_1_7zuwHRlIaW回避了攻击

8_4_elrW4qH2o1发起攻击, 1_4_WVkVlsThvI受到58点伤害

2_1_uysWKZlXH6发起攻击, 0_0_xgmfGTxsgJ受到36点伤害

9_3_hYmz070eWn发起攻击, 2_3_tHnczzzMsq受到51点伤害

4_4_sX1ESd2D2y发起攻击, 0_4_P1FJT96SJA回避了攻击

3_4_JcaediYDzK发动会心一击, 4_1_ft7jIfTgNj受到133点伤害

8_0_AlkBWAyz1Z发起攻击, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到190点伤害

3_0_XTHHn9wEV2发动铁壁, 3_0_XTHHn9wEV2防御力大幅上升

5_2_t3BK8haK6v使用火球术, 1_1_pKBLm6GEAC受到36点伤害

9_0_t0V0qFkSgB发起攻击, 2_0_aXLsxIS6Ia受到55点伤害

9_2_WYbpcKNF82使用诅咒, 0_3_FBFFeNkNSk受到71点伤害, 0_3_FBFFeNkNSk被诅咒了

8_2_Be5zzcfPi7发起攻击, 1_4_WVkVlsThvI受到82点伤害

7_2_XjP0Tr6J0Y发起攻击, 9_3_hYmz070eWn受到1点伤害

8_1_svHOQH6cia发起攻击, 4_4_sX1ESd2D2y受到74点伤害

9_4_k8iLxetXMp发起攻击, 1_1_pKBLm6GEAC受到33点伤害

4_1_ft7jIfTgNj发起攻击, 3_3_j51w0pP8RT受到81点伤害

4_0_lVbvLEiXDZ发起攻击, 2_0_aXLsxIS6Ia受到69点伤害

0_4_P1FJT96SJA使用瘟疫, 5_4_rYaAhrRwCU体力减少44%

1_0_p1p6k5PNcD发起攻击, 8_3_Bbqi8P2zLC回避了攻击

3_1_7zuwHRlIaW发起攻击, 0_0_xgmfGTxsgJ受到61点伤害

1_4_WVkVlsThvI发动铁壁, 1_4_WVkVlsThvI防御力大幅上升

4_3_zceJX8EHfN发起攻击, 7_0_xT4XQqpP8G受到85点伤害

7_4_QmVZ678Hg0发起攻击, 5_0_wmIWDtdAYU回避了攻击

2_4_b6JyAxXZxw发起攻击, 8_0_AlkBWAyz1Z受到107点伤害

6_4_7cFkTEybUG使用诅咒, 8_2_Be5zzcfPi7受到81点伤害, 8_2_Be5zzcfPi7被诅咒了, 8_2_Be5zzcfPi7发动隐匿

8_3_Bbqi8P2zLC发起攻击, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到126点伤害

 2_4_b6JyAxXZxw被击倒了

 使魔消失了

7_0_xT4XQqpP8G使用魅惑, 9_3_hYmz070eWn回避了攻击

0_1_EYFqrDPqa6发起攻击, 3_2_yYDVJIHkJB受到65点伤害

5_1_CwY40TkXyv发起攻击, 8_1_svHOQH6cia受到99点伤害

2_3_tHnczzzMsq发起攻击, 9_2_WYbpcKNF82受到32点伤害

2_0_aXLsxIS6Ia发起攻击, 8_3_Bbqi8P2zLC回避了攻击

3_3_j51w0pP8RT投毒, 1_2_7H1vlZNszA受到59点伤害, 1_2_7H1vlZNszA中毒

5_0_wmIWDtdAYU发起吸血攻击, 3_3_j51w0pP8RT受到55点伤害, 5_0_wmIWDtdAYU回复体力0点

1_3_pK5scFsXo9发起攻击, 3_3_j51w0pP8RT受到53点伤害

0_0_xgmfGTxsgJ发起攻击, 1_2_7H1vlZNszA受到85点伤害

 1_2_7H1vlZNszA发起反击, 0_0_xgmfGTxsgJ受到77点伤害

6_2_3RX0LxbGos使用狂暴术, 0_0_xgmfGTxsgJ受到54点伤害, 0_0_xgmfGTxsgJ进入狂暴状态

7_1_lubb3aK7nA发起攻击, 6_1_14AVsCIznH受到44点伤害

6_3_CR3OsxQ1rZ发起攻击, 9_2_WYbpcKNF82受到86点伤害

8_0_AlkBWAyz1Z发起攻击, 7_4_QmVZ678Hg0受到54点伤害

0_3_FBFFeNkNSk发起攻击, 9_0_t0V0qFkSgB回避了攻击

9_1_gohCojhCZA发起攻击, 5_0_wmIWDtdAYU受到57点伤害

0_2_R0dQORCsVA发动铁壁, 0_2_R0dQORCsVA防御力大幅上升

4_4_sX1ESd2D2y发起攻击, 9_2_WYbpcKNF82回避了攻击

0_4_P1FJT96SJA使用诅咒, 8_1_svHOQH6cia受到103点伤害, 8_1_svHOQH6cia被诅咒了

2_2_bwVXNklpsw发起攻击, 3_2_yYDVJIHkJB受到70点伤害

 3_2_yYDVJIHkJB被击倒了

8_4_elrW4qH2o1发起攻击, 1_4_WVkVlsThvI受到1点伤害

9_0_t0V0qFkSgB发起攻击, 5_0_wmIWDtdAYU受到47点伤害

5_4_rYaAhrRwCU发起攻击, 2_0_aXLsxIS6Ia受到118点伤害

2_1_uysWKZlXH6发起攻击, 6_4_7cFkTEybUG受到119点伤害, 6_4_7cFkTEybUG发动隐匿

4_3_zceJX8EHfN发起攻击, 9_4_k8iLxetXMp受到55点伤害

4_0_lVbvLEiXDZ发起攻击, 3_4_JcaediYDzK受到33点伤害, 3_4_JcaediYDzK发动隐匿

1_0_p1p6k5PNcD使用诅咒, 4_0_lVbvLEiXDZ受到31点伤害, 4_0_lVbvLEiXDZ被诅咒了

3_4_JcaediYDzK发起攻击, 4_1_ft7jIfTgNj受到43点伤害

1_1_pKBLm6GEAC发起攻击, 8_0_AlkBWAyz1Z受到56点伤害

7_4_QmVZ678Hg0发起攻击, 0_4_P1FJT96SJA回避了攻击

6_1_14AVsCIznH发起攻击, 0_2_R0dQORCsVA受到0点伤害

7_0_xT4XQqpP8G使用魅惑, 9_0_t0V0qFkSgB回避了攻击

4_1_ft7jIfTgNj发起攻击, 7_4_QmVZ678Hg0回避了攻击

5_1_CwY40TkXyv发起攻击, 1_0_p1p6k5PNcD受到146点伤害

 1_0_p1p6k5PNcD被击倒了

3_1_7zuwHRlIaW发起攻击, 9_0_t0V0qFkSgB受到75点伤害

9_2_WYbpcKNF82发起攻击, 8_3_Bbqi8P2zLC受到45点伤害

1_2_7H1vlZNszA发起攻击, 7_2_XjP0Tr6J0Y受到49点伤害

 1_2_7H1vlZNszA毒性发作, 1_2_7H1vlZNszA受到17点伤害

 1_2_7H1vlZNszA被击倒了

6_4_7cFkTEybUG发起攻击, 5_1_CwY40TkXyv受到62点伤害

5_3_SrzWF3LbKc使用分身, 出现一个新的5_3_SrzWF3LbKc

7_1_lubb3aK7nA发起攻击, 5_0_wmIWDtdAYU受到47点伤害

2_0_aXLsxIS6Ia发起攻击, 0_4_P1FJT96SJA受到25点伤害

 2_0_aXLsxIS6Ia连击, 8_2_Be5zzcfPi7受到45点伤害

 2_0_aXLsxIS6Ia连击, 诅咒使伤害加倍, 8_2_Be5zzcfPi7受到44点伤害

7_2_XjP0Tr6J0Y发起攻击, 5_4_rYaAhrRwCU受到98点伤害

8_3_Bbqi8P2zLC发起攻击, 3_3_j51w0pP8RT受到87点伤害

 3_3_j51w0pP8RT被击倒了

0_1_EYFqrDPqa6发起攻击, 5_1_CwY40TkXyv受到56点伤害

8_0_AlkBWAyz1Z发起攻击, 诅咒使伤害加倍, 0_3_FBFFeNkNSk受到258点伤害

 0_3_FBFFeNkNSk被击倒了

5_2_t3BK8haK6v开始蓄力

0_2_R0dQORCsVA发起攻击, 6_3_CR3OsxQ1rZ受到62点伤害

1_4_WVkVlsThvI发起攻击, 0_4_P1FJT96SJA受到35点伤害

6_3_CR3OsxQ1rZ发起攻击, 8_3_Bbqi8P2zLC受到33点伤害

8_1_svHOQH6cia发起攻击, 2_0_aXLsxIS6Ia受到78点伤害

 2_0_aXLsxIS6Ia被击倒了

0_4_P1FJT96SJA使用瘟疫, 4_4_sX1ESd2D2y体力减少50%

3_0_XTHHn9wEV2发起攻击, 9_3_hYmz070eWn受到1点伤害

2_3_tHnczzzMsq发起攻击, 1_1_pKBLm6GEAC受到30点伤害

4_3_zceJX8EHfN发起攻击, 6_2_3RX0LxbGos受到49点伤害

5_0_wmIWDtdAYU发起吸血攻击, 7_0_xT4XQqpP8G受到101点伤害, 5_0_wmIWDtdAYU回复体力51点

8_2_Be5zzcfPi7发起攻击, 4_3_zceJX8EHfN受到60点伤害

9_3_hYmz070eWn使用冰冻术, 4_2_NruYBCUIZA受到10点伤害, 4_2_NruYBCUIZA被冰冻了

 9_3_hYmz070eWn从铁壁中解除

4_0_lVbvLEiXDZ发起攻击, 7_1_lubb3aK7nA受到23点伤害

9_0_t0V0qFkSgB发动会心一击, 5_2_t3BK8haK6v受到104点伤害

9_1_gohCojhCZA发起攻击, 7_4_QmVZ678Hg0受到95点伤害

9_4_k8iLxetXMp发起攻击, 1_3_pK5scFsXo9回避了攻击

7_0_xT4XQqpP8G发起攻击, 9_0_t0V0qFkSgB回避了攻击

8_4_elrW4qH2o1使用生命之轮, 4_0_lVbvLEiXDZ的体力值与8_4_elrW4qH2o1互换

2_1_uysWKZlXH6发起攻击, 6_1_14AVsCIznH受到52点伤害

1_3_pK5scFsXo9发起攻击, 6_4_7cFkTEybUG受到65点伤害

3_4_JcaediYDzK发起攻击, 8_4_elrW4qH2o1受到53点伤害

2_2_bwVXNklpsw发起攻击, 6_3_CR3OsxQ1rZ受到60点伤害

0_0_xgmfGTxsgJ发起狂暴攻击, 6_3_CR3OsxQ1rZ受到100点伤害

 0_0_xgmfGTxsgJ从狂暴中解除

6_2_3RX0LxbGos发起攻击, 4_1_ft7jIfTgNj受到42点伤害

3_1_7zuwHRlIaW发起攻击, 8_4_elrW4qH2o1受到49点伤害

5_3_SrzWF3LbKc发起攻击, 1_3_pK5scFsXo9受到52点伤害

7_4_QmVZ678Hg0使用减速术, 6_4_7cFkTEybUG进入迟缓状态

7_1_lubb3aK7nA开始聚气, 7_1_lubb3aK7nA攻击力上升

1_4_WVkVlsThvI发起攻击, 5_0_wmIWDtdAYU受到70点伤害

 1_4_WVkVlsThvI从铁壁中解除

8_2_Be5zzcfPi7发起攻击, 3_4_JcaediYDzK受到79点伤害

 3_4_JcaediYDzK被击倒了

6_1_14AVsCIznH发起攻击, 9_4_k8iLxetXMp受到46点伤害

4_1_ft7jIfTgNj发起攻击, 7_2_XjP0Tr6J0Y受到65点伤害, 7_2_XjP0Tr6J0Y发动隐匿

8_0_AlkBWAyz1Z发起攻击, 6_2_3RX0LxbGos受到60点伤害

3_0_XTHHn9wEV2发起攻击, 4_2_NruYBCUIZA受到54点伤害

 3_0_XTHHn9wEV2从铁壁中解除

5_4_rYaAhrRwCU使用减速术, 1_1_pKBLm6GEAC进入迟缓状态

5_0_wmIWDtdAYU使用净化, 9_3_hYmz070eWn受到29点伤害

4_4_sX1ESd2D2y发起攻击, 5_1_CwY40TkXyv受到27点伤害

 5_1_CwY40TkXyv被击倒了

4_2_NruYBCUIZA从冰冻中解除

9_0_t0V0qFkSgB发起攻击, 1_4_WVkVlsThvI防御, 1_4_WVkVlsThvI受到33点伤害

5_3_SrzWF3LbKc使用生命之轮, 4_0_lVbvLEiXDZ的体力值与5_3_SrzWF3LbKc互换

2_3_tHnczzzMsq发起攻击, 9_0_t0V0qFkSgB受到57点伤害

6_3_CR3OsxQ1rZ发起攻击, 9_2_WYbpcKNF82受到62点伤害

8_1_svHOQH6cia使用狂暴术, 0_1_EYFqrDPqa6回避了攻击

8_3_Bbqi8P2zLC发起攻击, 6_2_3RX0LxbGos受到73点伤害

0_4_P1FJT96SJA使用瘟疫, 1_1_pKBLm6GEAC体力减少48%

4_2_NruYBCUIZA发起攻击, 5_3_SrzWF3LbKc受到51点伤害

0_2_R0dQORCsVA使用生命之轮, 2_1_uysWKZlXH6回避了攻击

 0_2_R0dQORCsVA从铁壁中解除

4_3_zceJX8EHfN发起攻击, 5_3_SrzWF3LbKc回避了攻击

7_4_QmVZ678Hg0使用减速术, 4_4_sX1ESd2D2y进入迟缓状态

7_2_XjP0Tr6J0Y使用雷击术

 1_1_pKBLm6GEAC受到31点伤害

 1_1_pKBLm6GEAC受到21点伤害

 1_1_pKBLm6GEAC受到40点伤害

 1_1_pKBLm6GEAC受到44点伤害

 1_1_pKBLm6GEAC被击倒了

 7_2_XjP0Tr6J0Y吞噬了1_1_pKBLm6GEAC, 7_2_XjP0Tr6J0Y属性上升

0_1_EYFqrDPqa6发起攻击, 8_2_Be5zzcfPi7受到127点伤害

 8_2_Be5zzcfPi7被击倒了

9_1_gohCojhCZA发起攻击, 4_2_NruYBCUIZA受到111点伤害

2_1_uysWKZlXH6使用狂暴术, 9_2_WYbpcKNF82受到77点伤害, 9_2_WYbpcKNF82进入狂暴状态

9_2_WYbpcKNF82发起狂暴攻击, 5_2_t3BK8haK6v受到29点伤害

 9_2_WYbpcKNF82从狂暴中解除

9_3_hYmz070eWn发起攻击, 4_3_zceJX8EHfN受到69点伤害

9_4_k8iLxetXMp发起攻击, 5_3_SrzWF3LbKc受到43点伤害, 5_3_SrzWF3LbKc发动隐匿

2_2_bwVXNklpsw发起攻击, 6_1_14AVsCIznH使用伤害反弹, 2_2_bwVXNklpsw受到28点伤害

8_4_elrW4qH2o1发起攻击, 7_4_QmVZ678Hg0受到81点伤害

1_3_pK5scFsXo9发起攻击, 2_1_uysWKZlXH6受到57点伤害

7_2_XjP0Tr6J0Y发起攻击, 9_4_k8iLxetXMp受到52点伤害

3_0_XTHHn9wEV2发起攻击, 4_3_zceJX8EHfN受到55点伤害

5_2_t3BK8haK6v使用诅咒, 9_3_hYmz070eWn受到175点伤害, 9_3_hYmz070eWn被诅咒了

7_1_lubb3aK7nA使用治愈魔法, 7_4_QmVZ678Hg0回复体力230点

6_4_7cFkTEybUG发起攻击, 4_2_NruYBCUIZA受到65点伤害

4_1_ft7jIfTgNj投毒, 7_4_QmVZ678Hg0受到56点伤害, 7_4_QmVZ678Hg0中毒

4_0_lVbvLEiXDZ发起攻击, 6_1_14AVsCIznH受到84点伤害

8_0_AlkBWAyz1Z发起攻击, 7_4_QmVZ678Hg0受到43点伤害

9_0_t0V0qFkSgB发动会心一击, 0_1_EYFqrDPqa6受到134点伤害

0_0_xgmfGTxsgJ使用火球术, 5_3_SrzWF3LbKc受到187点伤害

 5_3_SrzWF3LbKc被击倒了

6_2_3RX0LxbGos使用狂暴术, 4_0_lVbvLEiXDZ受到86点伤害, 4_0_lVbvLEiXDZ进入狂暴状态

5_4_rYaAhrRwCU发起攻击, 8_0_AlkBWAyz1Z回避了攻击

3_1_7zuwHRlIaW发起攻击, 6_4_7cFkTEybUG受到60点伤害

7_4_QmVZ678Hg0发起攻击, 4_1_ft7jIfTgNj受到43点伤害

 7_4_QmVZ678Hg0毒性发作, 7_4_QmVZ678Hg0受到16点伤害

7_0_xT4XQqpP8G使用魅惑, 5_0_wmIWDtdAYU被魅惑了

1_4_WVkVlsThvI发起攻击, 7_2_XjP0Tr6J0Y受到67点伤害

2_3_tHnczzzMsq发起攻击, 1_4_WVkVlsThvI受到48点伤害

4_3_zceJX8EHfN发起攻击, 9_1_gohCojhCZA受到42点伤害

0_4_P1FJT96SJA发起攻击, 2_2_bwVXNklpsw受到114点伤害

4_2_NruYBCUIZA使用分身, 出现一个新的4_2_NruYBCUIZA

0_2_R0dQORCsVA发起攻击, 1_4_WVkVlsThvI受到82点伤害

5_3_SrzWF3LbKc发起攻击, 8_3_Bbqi8P2zLC受到72点伤害

6_1_14AVsCIznH发起攻击, 7_0_xT4XQqpP8G受到33点伤害

0_1_EYFqrDPqa6发起攻击, 7_0_xT4XQqpP8G受到73点伤害

 7_0_xT4XQqpP8G被击倒了

2_1_uysWKZlXH6发起攻击, 4_3_zceJX8EHfN受到74点伤害

 4_3_zceJX8EHfN被击倒了

5_0_wmIWDtdAYU发起攻击, 0_4_P1FJT96SJA回避了攻击

 5_0_wmIWDtdAYU从魅惑中解除

6_3_CR3OsxQ1rZ发起攻击, 9_2_WYbpcKNF82受到99点伤害

 9_2_WYbpcKNF82被击倒了

9_4_k8iLxetXMp发起攻击, 诅咒使伤害加倍, 4_0_lVbvLEiXDZ受到52点伤害

 4_0_lVbvLEiXDZ被击倒了

3_0_XTHHn9wEV2发起攻击, 2_1_uysWKZlXH6受到48点伤害

0_0_xgmfGTxsgJ发起攻击, 8_3_Bbqi8P2zLC受到42点伤害

1_3_pK5scFsXo9投毒, 6_4_7cFkTEybUG回避了攻击

6_2_3RX0LxbGos发起攻击, 1_3_pK5scFsXo9受到120点伤害

7_1_lubb3aK7nA使用分身, 出现一个新的7_1_lubb3aK7nA

5_4_rYaAhrRwCU使用诅咒, 0_2_R0dQORCsVA受到88点伤害, 0_2_R0dQORCsVA被诅咒了

3_1_7zuwHRlIaW发起攻击, 8_4_elrW4qH2o1回避了攻击

1_4_WVkVlsThvI发起攻击, 9_1_gohCojhCZA受到66点伤害

2_3_tHnczzzMsq发起攻击, 4_2_NruYBCUIZA受到94点伤害

 4_2_NruYBCUIZA被击倒了

7_4_QmVZ678Hg0发起攻击, 2_3_tHnczzzMsq受到57点伤害

 7_4_QmVZ678Hg0毒性发作, 7_4_QmVZ678Hg0受到13点伤害

7_2_XjP0Tr6J0Y发起攻击, 6_1_14AVsCIznH受到163点伤害

 6_1_14AVsCIznH被击倒了

8_1_svHOQH6cia发起攻击, 1_4_WVkVlsThvI受到73点伤害

 1_4_WVkVlsThvI被击倒了

8_3_Bbqi8P2zLC发起攻击, 4_1_ft7jIfTgNj受到19点伤害

 4_1_ft7jIfTgNj被击倒了

2_2_bwVXNklpsw发起吸血攻击, 7_2_XjP0Tr6J0Y回避了攻击

8_4_elrW4qH2o1发起攻击, 4_4_sX1ESd2D2y受到68点伤害

9_1_gohCojhCZA发起攻击, 5_3_SrzWF3LbKc受到51点伤害

9_3_hYmz070eWn发起攻击, 0_4_P1FJT96SJA受到94点伤害

 0_4_P1FJT96SJA被击倒了

8_0_AlkBWAyz1Z发起攻击, 9_4_k8iLxetXMp受到81点伤害

 9_4_k8iLxetXMp被击倒了

 8_0_AlkBWAyz1Z召唤亡灵, 9_4_k8iLxetXMp变成了丧尸

9_0_t0V0qFkSgB发起攻击, 7_1_lubb3aK7nA受到106点伤害

 7_1_lubb3aK7nA被击倒了

4_2_NruYBCUIZA发起攻击, 丧尸受到68点伤害

7_1_lubb3aK7nA发起攻击, 0_1_EYFqrDPqa6受到102点伤害

 0_1_EYFqrDPqa6被击倒了

5_2_t3BK8haK6v发动会心一击, 8_4_elrW4qH2o1受到126点伤害

 8_4_elrW4qH2o1被击倒了

0_2_R0dQORCsVA发起攻击, 9_3_hYmz070eWn受到18点伤害

3_1_7zuwHRlIaW发起攻击, 2_1_uysWKZlXH6受到124点伤害

 2_1_uysWKZlXH6被击倒了

 3_1_7zuwHRlIaW吞噬了2_1_uysWKZlXH6, 3_1_7zuwHRlIaW属性上升

1_3_pK5scFsXo9开始蓄力

7_2_XjP0Tr6J0Y潜行到0_0_xgmfGTxsgJ身后

3_0_XTHHn9wEV2发起攻击, 8_0_AlkBWAyz1Z受到94点伤害

5_4_rYaAhrRwCU使用减速术, 9_3_hYmz070eWn回避了攻击

3_1_7zuwHRlIaW发起攻击, 0_2_R0dQORCsVA防御, 诅咒使伤害加倍, 0_2_R0dQORCsVA受到96点伤害

 0_2_R0dQORCsVA被击倒了

8_1_svHOQH6cia使用狂暴术, 7_1_lubb3aK7nA回避了攻击

4_4_sX1ESd2D2y发起攻击, 丧尸受到98点伤害

 丧尸消失了

2_2_bwVXNklpsw发起攻击, 3_0_XTHHn9wEV2受到82点伤害

0_0_xgmfGTxsgJ使用火球术, 8_3_Bbqi8P2zLC受到45点伤害

2_3_tHnczzzMsq发起攻击, 5_3_SrzWF3LbKc回避了攻击

5_3_SrzWF3LbKc发起攻击, 7_2_XjP0Tr6J0Y受到70点伤害

 7_2_XjP0Tr6J0Y的潜行被识破

 7_2_XjP0Tr6J0Y被击倒了

5_0_wmIWDtdAYU发起攻击, 6_4_7cFkTEybUG受到68点伤害

 6_4_7cFkTEybUG被击倒了

3_0_XTHHn9wEV2发起攻击, 9_0_t0V0qFkSgB受到50点伤害

9_0_t0V0qFkSgB发起攻击, 5_0_wmIWDtdAYU受到79点伤害

6_2_3RX0LxbGos发起攻击, 3_1_7zuwHRlIaW受到91点伤害

7_4_QmVZ678Hg0发起攻击, 4_4_sX1ESd2D2y受到59点伤害

 4_4_sX1ESd2D2y被击倒了

 7_4_QmVZ678Hg0毒性发作, 7_4_QmVZ678Hg0受到11点伤害

6_3_CR3OsxQ1rZ发起攻击, 1_3_pK5scFsXo9受到109点伤害

 1_3_pK5scFsXo9被击倒了

4_2_NruYBCUIZA发起攻击, 诅咒使伤害加倍, 8_1_svHOQH6cia受到114点伤害

 8_1_svHOQH6cia被击倒了

8_0_AlkBWAyz1Z发起攻击, 2_3_tHnczzzMsq受到73点伤害

5_2_t3BK8haK6v发起攻击, 3_1_7zuwHRlIaW受到55点伤害

 3_1_7zuwHRlIaW被击倒了

7_1_lubb3aK7nA发起攻击, 5_3_SrzWF3LbKc受到97点伤害

 5_3_SrzWF3LbKc被击倒了

8_3_Bbqi8P2zLC发起攻击, 5_2_t3BK8haK6v受到67点伤害

 5_2_t3BK8haK6v被击倒了

2_3_tHnczzzMsq使用苏生术, 2_4_b6JyAxXZxw复活了, 2_4_b6JyAxXZxw回复体力113点

7_4_QmVZ678Hg0发起攻击, 9_1_gohCojhCZA回避了攻击

 7_4_QmVZ678Hg0毒性发作, 7_4_QmVZ678Hg0受到9点伤害

 7_4_QmVZ678Hg0从中毒中解除

9_3_hYmz070eWn发起攻击, 6_3_CR3OsxQ1rZ回避了攻击

2_2_bwVXNklpsw发起攻击, 7_4_QmVZ678Hg0受到40点伤害

9_1_gohCojhCZA发起攻击, 5_4_rYaAhrRwCU受到95点伤害

 5_4_rYaAhrRwCU被击倒了

2_4_b6JyAxXZxw发起攻击, 9_0_t0V0qFkSgB受到40点伤害, 9_0_t0V0qFkSgB发动隐匿

 2_4_b6JyAxXZxw从疾走中解除

8_0_AlkBWAyz1Z使用净化, 诅咒使伤害加倍, 9_3_hYmz070eWn受到266点伤害

 9_3_hYmz070eWn被击倒了

 8_0_AlkBWAyz1Z召唤亡灵, 9_3_hYmz070eWn变成了丧尸

5_0_wmIWDtdAYU发起攻击, 7_4_QmVZ678Hg0受到84点伤害

 7_4_QmVZ678Hg0被击倒了

3_0_XTHHn9wEV2发起攻击, 6_3_CR3OsxQ1rZ受到60点伤害

 6_3_CR3OsxQ1rZ被击倒了

 3_0_XTHHn9wEV2吞噬了6_3_CR3OsxQ1rZ, 3_0_XTHHn9wEV2属性上升

3_0_XTHHn9wEV2使用苏生术, 3_1_7zuwHRlIaW复活了, 3_1_7zuwHRlIaW回复体力85点

0_0_xgmfGTxsgJ使用火球术, 8_0_AlkBWAyz1Z受到134点伤害

 8_0_AlkBWAyz1Z被击倒了

 丧尸消失了

9_1_gohCojhCZA潜行到2_4_b6JyAxXZxw身后

7_1_lubb3aK7nA发起攻击, 9_1_gohCojhCZA回避了攻击

2_3_tHnczzzMsq开始蓄力

4_2_NruYBCUIZA发起攻击, 3_0_XTHHn9wEV2受到41点伤害

6_2_3RX0LxbGos发起攻击, 0_0_xgmfGTxsgJ受到48点伤害

 0_0_xgmfGTxsgJ被击倒了

3_1_7zuwHRlIaW发起攻击, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到108点伤害

9_0_t0V0qFkSgB发起攻击, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到98点伤害

 2_4_b6JyAxXZxw被击倒了

5_0_wmIWDtdAYU发起攻击, 9_0_t0V0qFkSgB受到76点伤害

 9_0_t0V0qFkSgB被击倒了

3_0_XTHHn9wEV2发动铁壁, 3_0_XTHHn9wEV2防御力大幅上升

7_1_lubb3aK7nA发起攻击, 2_2_bwVXNklpsw受到74点伤害

 2_2_bwVXNklpsw被击倒了

6_2_3RX0LxbGos开始聚气, 6_2_3RX0LxbGos攻击力上升

4_2_NruYBCUIZA发起攻击, 3_0_XTHHn9wEV2受到1点伤害

9_1_gohCojhCZA发起攻击, 6_2_3RX0LxbGos受到106点伤害

 6_2_3RX0LxbGos被击倒了

8_3_Bbqi8P2zLC发起攻击, 3_1_7zuwHRlIaW受到35点伤害

 3_1_7zuwHRlIaW发起反击, 8_3_Bbqi8P2zLC受到38点伤害

2_3_tHnczzzMsq使用苏生术, 2_4_b6JyAxXZxw复活了, 2_4_b6JyAxXZxw回复体力313点

5_0_wmIWDtdAYU使用雷击术

 3_0_XTHHn9wEV2受到1点伤害

 3_0_XTHHn9wEV2受到1点伤害

 3_0_XTHHn9wEV2受到1点伤害

 3_0_XTHHn9wEV2受到1点伤害

3_1_7zuwHRlIaW使用狂暴术, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到62点伤害, 2_4_b6JyAxXZxw进入狂暴状态

2_4_b6JyAxXZxw发起狂暴攻击, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到72点伤害

 2_4_b6JyAxXZxw从狂暴中解除

4_2_NruYBCUIZA发起攻击, 2_4_b6JyAxXZxw受到68点伤害

3_0_XTHHn9wEV2发起攻击, 2_4_b6JyAxXZxw受到73点伤害

9_1_gohCojhCZA使用瘟疫, 3_1_7zuwHRlIaW体力减少49%

7_1_lubb3aK7nA发起攻击, 4_2_NruYBCUIZA受到67点伤害

 4_2_NruYBCUIZA被击倒了

2_3_tHnczzzMsq发起攻击, 8_3_Bbqi8P2zLC受到42点伤害

 8_3_Bbqi8P2zLC被击倒了

3_0_XTHHn9wEV2发起攻击, 诅咒使伤害加倍, 2_4_b6JyAxXZxw受到182点伤害

 2_4_b6JyAxXZxw被击倒了

 3_0_XTHHn9wEV2吞噬了2_4_b6JyAxXZxw, 3_0_XTHHn9wEV2属性上升

 3_0_XTHHn9wEV2从铁壁中解除

5_0_wmIWDtdAYU发起攻击, 3_0_XTHHn9wEV2受到46点伤害

 3_0_XTHHn9wEV2被击倒了

3_1_7zuwHRlIaW发起攻击, 7_1_lubb3aK7nA受到72点伤害

 7_1_lubb3aK7nA被击倒了

2_3_tHnczzzMsq发起攻击, 9_1_gohCojhCZA受到154点伤害

 9_1_gohCojhCZA被击倒了

5_0_wmIWDtdAYU发起吸血攻击, 2_3_tHnczzzMsq受到39点伤害, 5_0_wmIWDtdAYU回复体力20点

3_1_7zuwHRlIaW投毒, 5_0_wmIWDtdAYU回避了攻击

2_3_tHnczzzMsq发起攻击, 3_1_7zuwHRlIaW回避了攻击

3_1_7zuwHRlIaW发起攻击, 5_0_wmIWDtdAYU受到101点伤害

 5_0_wmIWDtdAYU被击倒了

2_3_tHnczzzMsq发起攻击, 3_1_7zuwHRlIaW受到43点伤害

 3_1_7zuwHRlIaW被击倒了

"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, 50_000, true);
    assert!(guard < 50_000, "fight_multi_3 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_3 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_3 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

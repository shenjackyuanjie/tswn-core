use super::*;

#[test]
fn fight_multi_2() {
    const FIGHT_CASE: &str = r###"0_0_XJc3TefogX
0_1_rlGNQ5hOKA
0_2_M5Mc5NV1q8
0_3_g4Mliv7h8p
0_4_yiTQ8gtxlq

1_0_dSjTgEqqjn
1_1_5XOMkXw6ZJ
1_2_okhFFQdvkH
1_3_YBPaZB6Dhy
1_4_dA1SONcqVo

2_0_IbeJa2acsU
2_1_e1WWAVAvLp
2_2_8sMzAtrEch
2_3_5L6gnWViBJ
2_4_ZwzTEFviED

3_0_6feNVPHi6I
3_1_pNK1RsauUP
3_2_Qm1Yw54wkB
3_3_CLlY0shqQG
3_4_28zfba4nKR

4_0_JGb1ZmLQVk
4_1_uGBsxTkOtq
4_2_93jwt8T8YN
4_3_R1pR4DgsUY
4_4_UrJENM2ezJ

5_0_7iRQyT8dPL
5_1_vrEpgvq9Zd
5_2_ccCEcs2MAw
5_3_TSqokhGEdE
5_4_CUzJzbZWVF

6_0_gPpnRCjiTn
6_1_XPv6PBKtGI
6_2_m6a59ZPPZu
6_3_xRYfDW2HTr
6_4_m8d2wsmEFH

7_0_lzOMNgIkPL
7_1_QlLuaXiO2M
7_2_GqhUwR5mit
7_3_yXAlHGdeFr
7_4_sCMdy1Bn3a

8_0_lXCyiAxOVK
8_1_K5CiUWkJoh
8_2_BMmSPrdWgn
8_3_P09BI8WP06
8_4_4f0sDTwwp1

9_0_VE6VPhljRS
9_1_N76NRqq3kb
9_2_eQ40qJ3AKy
9_3_vNEkD3Usi4
9_4_9G6Tk9gEZ9


0_3_g4Mliv7h8p发起攻击, 2_1_e1WWAVAvLp受到42点伤害

5_4_CUzJzbZWVF发起攻击, 7_0_lzOMNgIkPL受到109点伤害

5_2_ccCEcs2MAw发起攻击, 7_4_sCMdy1Bn3a受到140点伤害

2_3_5L6gnWViBJ发起攻击, 8_4_4f0sDTwwp1受到88点伤害

9_2_eQ40qJ3AKy发起攻击, 3_3_CLlY0shqQG受到75点伤害

2_1_e1WWAVAvLp发起攻击, 6_0_gPpnRCjiTn受到40点伤害

6_0_gPpnRCjiTn发起攻击, 8_3_P09BI8WP06受到122点伤害

0_2_M5Mc5NV1q8发起攻击, 7_2_GqhUwR5mit受到111点伤害

7_4_sCMdy1Bn3a发起攻击, 3_1_pNK1RsauUP受到27点伤害

0_0_XJc3TefogX发起攻击, 5_3_TSqokhGEdE受到55点伤害

4_0_JGb1ZmLQVk发起攻击, 7_0_lzOMNgIkPL受到76点伤害

9_4_9G6Tk9gEZ9发起攻击, 1_1_5XOMkXw6ZJ受到51点伤害

6_1_XPv6PBKtGI发起攻击, 1_3_YBPaZB6Dhy受到106点伤害

2_2_8sMzAtrEch发起攻击, 3_3_CLlY0shqQG受到40点伤害

4_2_93jwt8T8YN使用冰冻术, 6_2_m6a59ZPPZu回避了攻击

3_2_Qm1Yw54wkB发起攻击, 0_2_M5Mc5NV1q8回避了攻击

8_3_P09BI8WP06使用血祭, 召唤出使魔

1_4_dA1SONcqVo发起攻击, 5_1_vrEpgvq9Zd回避了攻击

3_0_6feNVPHi6I使用诅咒, 6_2_m6a59ZPPZu受到81点伤害, 6_2_m6a59ZPPZu被诅咒了

4_3_R1pR4DgsUY发起攻击, 2_3_5L6gnWViBJ受到32点伤害

4_1_uGBsxTkOtq发起攻击, 5_2_ccCEcs2MAw受到91点伤害

9_0_VE6VPhljRS发起攻击, 0_4_yiTQ8gtxlq受到90点伤害

9_3_vNEkD3Usi4发起攻击, 0_4_yiTQ8gtxlq受到73点伤害

7_2_GqhUwR5mit发起攻击, 9_3_vNEkD3Usi4受到27点伤害

5_0_7iRQyT8dPL使用治愈魔法, 5_2_ccCEcs2MAw回复体力91点

3_3_CLlY0shqQG发起攻击, 8_1_K5CiUWkJoh受到59点伤害

3_4_28zfba4nKR发起攻击, 2_2_8sMzAtrEch受到56点伤害

1_0_dSjTgEqqjn发起攻击, 2_4_ZwzTEFviED受到118点伤害

8_2_BMmSPrdWgn发起攻击, 1_3_YBPaZB6Dhy受到83点伤害

0_3_g4Mliv7h8p使用加速术, 0_3_g4Mliv7h8p进入疾走状态

9_1_N76NRqq3kb发起攻击, 3_4_28zfba4nKR受到37点伤害

7_0_lzOMNgIkPL发起攻击, 4_1_uGBsxTkOtq受到37点伤害

1_2_okhFFQdvkH发起攻击, 9_4_9G6Tk9gEZ9受到61点伤害, 9_4_9G6Tk9gEZ9发动隐匿

5_3_TSqokhGEdE发起攻击, 9_2_eQ40qJ3AKy受到44点伤害

4_4_UrJENM2ezJ发起攻击, 8_2_BMmSPrdWgn受到71点伤害

6_2_m6a59ZPPZu使用瘟疫, 3_2_Qm1Yw54wkB体力减少66%, 3_2_Qm1Yw54wkB发动隐匿

2_4_ZwzTEFviED发起攻击, 5_0_7iRQyT8dPL受到28点伤害

6_4_m8d2wsmEFH发起攻击, 4_3_R1pR4DgsUY受到64点伤害

2_0_IbeJa2acsU开始聚气, 2_0_IbeJa2acsU攻击力上升

8_4_4f0sDTwwp1发起攻击, 5_2_ccCEcs2MAw受到97点伤害

6_3_xRYfDW2HTr发起攻击, 9_2_eQ40qJ3AKy回避了攻击

0_1_rlGNQ5hOKA使用减速术, 6_1_XPv6PBKtGI进入迟缓状态

1_1_5XOMkXw6ZJ发起攻击, 8_1_K5CiUWkJoh受到57点伤害

0_3_g4Mliv7h8p发起攻击, 2_0_IbeJa2acsU受到52点伤害

0_0_XJc3TefogX发起攻击, 1_4_dA1SONcqVo受到72点伤害

5_1_vrEpgvq9Zd发起攻击, 9_0_VE6VPhljRS受到135点伤害

7_3_yXAlHGdeFr发起攻击, 4_3_R1pR4DgsUY受到106点伤害

3_2_Qm1Yw54wkB发起攻击, 0_1_rlGNQ5hOKA受到61点伤害

3_1_pNK1RsauUP发动铁壁, 3_1_pNK1RsauUP防御力大幅上升

2_3_5L6gnWViBJ发起攻击, 5_1_vrEpgvq9Zd受到48点伤害

7_1_QlLuaXiO2M发起攻击, 9_3_vNEkD3Usi4受到91点伤害

1_3_YBPaZB6Dhy发起攻击, 3_0_6feNVPHi6I受到13点伤害

7_4_sCMdy1Bn3a使用血祭, 召唤出使魔

9_0_VE6VPhljRS发起攻击, 7_3_yXAlHGdeFr受到65点伤害

9_4_9G6Tk9gEZ9使用血祭, 召唤出使魔

2_1_e1WWAVAvLp发起攻击, 0_0_XJc3TefogX受到54点伤害

0_4_yiTQ8gtxlq发起攻击, 9_3_vNEkD3Usi4受到19点伤害

4_2_93jwt8T8YN使用净化, 5_1_vrEpgvq9Zd受到49点伤害

2_0_IbeJa2acsU发起攻击, 4_4_UrJENM2ezJ回避了攻击

8_1_K5CiUWkJoh使用狂暴术, 9_2_eQ40qJ3AKy受到26点伤害, 9_2_eQ40qJ3AKy进入狂暴状态

0_2_M5Mc5NV1q8发起攻击, 2_4_ZwzTEFviED受到62点伤害

使魔发起攻击, 5_0_7iRQyT8dPL回避了攻击

5_2_ccCEcs2MAw使用火球术, 9_3_vNEkD3Usi4受到46点伤害

0_3_g4Mliv7h8p发起攻击, 8_2_BMmSPrdWgn受到47点伤害

 0_3_g4Mliv7h8p从疾走中解除

4_3_R1pR4DgsUY发起攻击, 2_0_IbeJa2acsU受到88点伤害

5_4_CUzJzbZWVF发起攻击, 0_3_g4Mliv7h8p受到61点伤害

3_4_28zfba4nKR发起攻击, 5_4_CUzJzbZWVF受到71点伤害

6_0_gPpnRCjiTn开始蓄力

8_2_BMmSPrdWgn发起攻击, 1_3_YBPaZB6Dhy受到23点伤害

3_0_6feNVPHi6I使用血祭, 召唤出使魔

8_0_lXCyiAxOVK发起攻击, 6_2_m6a59ZPPZu受到118点伤害

4_1_uGBsxTkOtq发起攻击, 2_2_8sMzAtrEch受到64点伤害

9_2_eQ40qJ3AKy发起狂暴攻击, 6_0_gPpnRCjiTn受到80点伤害

 9_2_eQ40qJ3AKy从狂暴中解除

9_3_vNEkD3Usi4使用减速术, 6_3_xRYfDW2HTr进入迟缓状态

7_0_lzOMNgIkPL使用火球术, 1_1_5XOMkXw6ZJ回避了攻击

3_3_CLlY0shqQG发起攻击, 1_4_dA1SONcqVo受到76点伤害

4_4_UrJENM2ezJ发起攻击, 3_4_28zfba4nKR回避了攻击

6_2_m6a59ZPPZu发起吸血攻击, 5_1_vrEpgvq9Zd受到73点伤害, 6_2_m6a59ZPPZu回复体力37点

2_4_ZwzTEFviED发起攻击, 7_3_yXAlHGdeFr受到28点伤害

1_1_5XOMkXw6ZJ发起攻击, 4_0_JGb1ZmLQVk受到95点伤害, 4_0_JGb1ZmLQVk发动隐匿

1_0_dSjTgEqqjn使用幻术, 召唤出幻影

8_3_P09BI8WP06发起攻击, 7_4_sCMdy1Bn3a回避了攻击

7_2_GqhUwR5mit发起攻击, 3_2_Qm1Yw54wkB受到47点伤害

7_3_yXAlHGdeFr发起攻击, 0_0_XJc3TefogX回避了攻击

使魔发起攻击, 7_4_sCMdy1Bn3a回避了攻击

9_1_N76NRqq3kb发起攻击, 1_0_dSjTgEqqjn受到52点伤害

5_0_7iRQyT8dPL发起攻击, 4_4_UrJENM2ezJ受到92点伤害

1_3_YBPaZB6Dhy发起攻击, 7_2_GqhUwR5mit受到42点伤害

1_4_dA1SONcqVo发起攻击, 使魔受到60点伤害, 9_4_9G6Tk9gEZ9受到30点伤害

2_3_5L6gnWViBJ发起攻击, 4_1_uGBsxTkOtq受到65点伤害

4_0_JGb1ZmLQVk发起攻击, 6_4_m8d2wsmEFH受到56点伤害

9_4_9G6Tk9gEZ9发起攻击, 3_1_pNK1RsauUP受到1点伤害

1_2_okhFFQdvkH使用分身, 出现一个新的1_2_okhFFQdvkH

6_1_XPv6PBKtGI发起攻击, 9_2_eQ40qJ3AKy受到55点伤害

0_4_yiTQ8gtxlq发起攻击, 6_3_xRYfDW2HTr回避了攻击

2_2_8sMzAtrEch发起攻击, 0_2_M5Mc5NV1q8受到20点伤害

5_4_CUzJzbZWVF发起攻击, 8_0_lXCyiAxOVK受到81点伤害

7_1_QlLuaXiO2M发起攻击, 9_1_N76NRqq3kb受到79点伤害

0_0_XJc3TefogX发起攻击, 5_0_7iRQyT8dPL受到85点伤害

5_1_vrEpgvq9Zd发起攻击, 2_3_5L6gnWViBJ受到44点伤害

5_3_TSqokhGEdE发起攻击, 1_4_dA1SONcqVo受到104点伤害

0_1_rlGNQ5hOKA发起攻击, 4_2_93jwt8T8YN受到61点伤害

6_4_m8d2wsmEFH发起攻击, 0_0_XJc3TefogX受到65点伤害

0_2_M5Mc5NV1q8发起攻击, 7_0_lzOMNgIkPL受到56点伤害

使魔使用火球术, 5_3_TSqokhGEdE回避了攻击

5_2_ccCEcs2MAw发起攻击, 3_4_28zfba4nKR受到32点伤害

9_2_eQ40qJ3AKy投毒, 3_3_CLlY0shqQG受到76点伤害, 3_3_CLlY0shqQG中毒

3_4_28zfba4nKR发起攻击, 7_4_sCMdy1Bn3a受到7点伤害, 7_4_sCMdy1Bn3a发动隐匿

2_4_ZwzTEFviED发起攻击, 5_2_ccCEcs2MAw受到44点伤害

1_1_5XOMkXw6ZJ发起攻击, 2_4_ZwzTEFviED受到47点伤害

8_2_BMmSPrdWgn发起攻击, 1_2_okhFFQdvkH受到98点伤害

使魔发起攻击, 5_1_vrEpgvq9Zd受到44点伤害

3_1_pNK1RsauUP发起攻击, 0_2_M5Mc5NV1q8受到60点伤害

4_3_R1pR4DgsUY发起攻击, 6_3_xRYfDW2HTr受到78点伤害

8_4_4f0sDTwwp1发起攻击, 4_1_uGBsxTkOtq受到50点伤害

7_0_lzOMNgIkPL发起攻击, 5_0_7iRQyT8dPL回避了攻击

6_2_m6a59ZPPZu使用瘟疫, 1_1_5XOMkXw6ZJ体力减少61%

3_2_Qm1Yw54wkB使用治愈魔法, 3_4_28zfba4nKR回复体力67点

8_1_K5CiUWkJoh使用狂暴术, 3_4_28zfba4nKR受到89点伤害, 3_4_28zfba4nKR进入狂暴状态

使魔发起攻击, 3_2_Qm1Yw54wkB受到18点伤害, 3_2_Qm1Yw54wkB发动隐匿

0_3_g4Mliv7h8p投毒, 9_4_9G6Tk9gEZ9受到56点伤害, 9_4_9G6Tk9gEZ9中毒

3_0_6feNVPHi6I使用诅咒, 8_4_4f0sDTwwp1受到36点伤害, 8_4_4f0sDTwwp1被诅咒了

9_0_VE6VPhljRS使用分身, 出现一个新的9_0_VE6VPhljRS

2_1_e1WWAVAvLp发起攻击, 8_3_P09BI8WP06受到53点伤害

4_4_UrJENM2ezJ使用瘟疫, 3_1_pNK1RsauUP体力减少44%

4_2_93jwt8T8YN发起攻击, 0_0_XJc3TefogX受到41点伤害

2_0_IbeJa2acsU发起攻击, 7_3_yXAlHGdeFr受到156点伤害

6_0_gPpnRCjiTn发起攻击, 0_2_M5Mc5NV1q8受到126点伤害

1_0_dSjTgEqqjn发起攻击, 4_0_JGb1ZmLQVk受到63点伤害, 4_0_JGb1ZmLQVk发动隐匿

8_3_P09BI8WP06发起攻击, 3_0_6feNVPHi6I回避了攻击

7_4_sCMdy1Bn3a发起攻击, 1_0_dSjTgEqqjn回避了攻击

3_3_CLlY0shqQG发起攻击, 9_3_vNEkD3Usi4受到22点伤害

 3_3_CLlY0shqQG连击, 0_2_M5Mc5NV1q8受到63点伤害

 3_3_CLlY0shqQG毒性发作, 3_3_CLlY0shqQG受到32点伤害

使魔发起攻击, 2_3_5L6gnWViBJ受到45点伤害

2_3_5L6gnWViBJ发起攻击, 3_1_pNK1RsauUP受到1点伤害

8_0_lXCyiAxOVK发起攻击, 7_1_QlLuaXiO2M回避了攻击

4_1_uGBsxTkOtq发起攻击, 0_3_g4Mliv7h8p受到93点伤害

9_3_vNEkD3Usi4发起攻击, 7_2_GqhUwR5mit受到61点伤害

1_2_okhFFQdvkH发起攻击, 4_2_93jwt8T8YN受到95点伤害

7_3_yXAlHGdeFr发起攻击, 3_4_28zfba4nKR受到62点伤害

0_4_yiTQ8gtxlq发起攻击, 6_0_gPpnRCjiTn受到44点伤害

6_3_xRYfDW2HTr发起攻击, 1_0_dSjTgEqqjn受到25点伤害

6_4_m8d2wsmEFH使用诅咒, 7_0_lzOMNgIkPL受到100点伤害

 7_0_lzOMNgIkPL被击倒了

8_2_BMmSPrdWgn发起攻击, 6_1_XPv6PBKtGI受到32点伤害

1_2_okhFFQdvkH发起攻击, 诅咒使伤害加倍, 8_4_4f0sDTwwp1受到108点伤害

9_1_N76NRqq3kb发起攻击, 6_3_xRYfDW2HTr受到54点伤害

7_2_GqhUwR5mit发起攻击, 6_3_xRYfDW2HTr受到85点伤害

4_0_JGb1ZmLQVk使用分身, 出现一个新的4_0_JGb1ZmLQVk

2_4_ZwzTEFviED使用生命之轮, 8_1_K5CiUWkJoh的体力值与2_4_ZwzTEFviED互换

1_3_YBPaZB6Dhy发起攻击, 6_4_m8d2wsmEFH受到48点伤害

使魔发起攻击, 9_0_VE6VPhljRS受到28点伤害

9_0_VE6VPhljRS发起攻击, 3_3_CLlY0shqQG受到47点伤害

5_2_ccCEcs2MAw发起攻击, 4_2_93jwt8T8YN受到79点伤害

1_4_dA1SONcqVo使用加速术, 1_2_okhFFQdvkH进入疾走状态

9_2_eQ40qJ3AKy发起攻击, 6_4_m8d2wsmEFH受到69点伤害

7_1_QlLuaXiO2M发起攻击, 5_4_CUzJzbZWVF受到84点伤害, 5_4_CUzJzbZWVF发动隐匿

0_0_XJc3TefogX发起攻击, 4_0_JGb1ZmLQVk回避了攻击

5_3_TSqokhGEdE发起攻击, 2_0_IbeJa2acsU受到140点伤害

4_4_UrJENM2ezJ发起攻击, 5_3_TSqokhGEdE受到84点伤害

6_2_m6a59ZPPZu使用瘟疫, 2_2_8sMzAtrEch体力减少64%, 2_2_8sMzAtrEch发动隐匿

使魔发起攻击, 5_3_TSqokhGEdE受到16点伤害

2_2_8sMzAtrEch发起攻击, 使魔受到56点伤害, 8_3_P09BI8WP06受到28点伤害

使魔发起攻击, 6_3_xRYfDW2HTr回避了攻击

3_1_pNK1RsauUP发起攻击, 2_3_5L6gnWViBJ受到40点伤害

 3_1_pNK1RsauUP连击, 2_3_5L6gnWViBJ受到26点伤害

 3_1_pNK1RsauUP从铁壁中解除

8_4_4f0sDTwwp1发起攻击, 0_4_yiTQ8gtxlq受到78点伤害

5_0_7iRQyT8dPL发起攻击, 0_3_g4Mliv7h8p回避了攻击

1_2_okhFFQdvkH发起攻击, 9_1_N76NRqq3kb受到55点伤害

7_3_yXAlHGdeFr发起攻击, 1_3_YBPaZB6Dhy受到72点伤害

3_4_28zfba4nKR发起狂暴攻击, 9_3_vNEkD3Usi4受到74点伤害

 3_4_28zfba4nKR从狂暴中解除

0_1_rlGNQ5hOKA开始蓄力

3_2_Qm1Yw54wkB使用分身, 出现一个新的3_2_Qm1Yw54wkB

2_0_IbeJa2acsU发起攻击, 1_2_okhFFQdvkH受到104点伤害

 1_2_okhFFQdvkH被击倒了

9_1_N76NRqq3kb发起攻击, 1_1_5XOMkXw6ZJ受到68点伤害

4_3_R1pR4DgsUY发起攻击, 使魔受到52点伤害, 3_0_6feNVPHi6I受到26点伤害

5_4_CUzJzbZWVF发起攻击, 7_4_sCMdy1Bn3a受到96点伤害

4_1_uGBsxTkOtq发起攻击, 3_2_Qm1Yw54wkB受到41点伤害

 3_2_Qm1Yw54wkB被击倒了

9_0_VE6VPhljRS发起攻击, 6_4_m8d2wsmEFH受到52点伤害

2_1_e1WWAVAvLp使用净化, 1_4_dA1SONcqVo受到68点伤害

 1_4_dA1SONcqVo被击倒了

 2_1_e1WWAVAvLp吞噬了1_4_dA1SONcqVo, 2_1_e1WWAVAvLp属性上升

4_2_93jwt8T8YN使用狂暴术, 3_0_6feNVPHi6I回避了攻击

8_1_K5CiUWkJoh发起攻击, 5_0_7iRQyT8dPL受到41点伤害

1_0_dSjTgEqqjn发起攻击, 6_4_m8d2wsmEFH受到40点伤害

 1_0_dSjTgEqqjn连击, 6_4_m8d2wsmEFH回避了攻击

8_2_BMmSPrdWgn使用地裂术

 9_0_VE6VPhljRS受到32点伤害

 9_0_VE6VPhljRS被击倒了

 3_3_CLlY0shqQG回避了攻击

 2_4_ZwzTEFviED受到24点伤害

 5_2_ccCEcs2MAw受到14点伤害, 5_2_ccCEcs2MAw发动隐匿

 0_0_XJc3TefogX受到48点伤害

8_3_P09BI8WP06发起攻击, 9_2_eQ40qJ3AKy受到49点伤害

0_3_g4Mliv7h8p使用加速术, 0_3_g4Mliv7h8p进入疾走状态

3_0_6feNVPHi6I发起攻击, 9_4_9G6Tk9gEZ9受到63点伤害

2_3_5L6gnWViBJ发起攻击, 5_1_vrEpgvq9Zd受到53点伤害

7_1_QlLuaXiO2M使用冰冻术, 9_4_9G6Tk9gEZ9受到40点伤害, 9_4_9G6Tk9gEZ9被冰冻了

4_0_JGb1ZmLQVk发起攻击, 7_1_QlLuaXiO2M受到55点伤害

0_4_yiTQ8gtxlq发起攻击, 4_4_UrJENM2ezJ回避了攻击

2_4_ZwzTEFviED发起攻击, 幻影受到8点伤害

6_4_m8d2wsmEFH发起攻击, 7_4_sCMdy1Bn3a受到90点伤害

4_0_JGb1ZmLQVk发起攻击, 3_0_6feNVPHi6I受到35点伤害

5_2_ccCEcs2MAw发起攻击, 9_1_N76NRqq3kb受到98点伤害

9_2_eQ40qJ3AKy发起攻击, 0_0_XJc3TefogX受到53点伤害

5_1_vrEpgvq9Zd发起攻击, 使魔受到65点伤害, 9_4_9G6Tk9gEZ9受到32点伤害

 使魔消失了

3_3_CLlY0shqQG发起攻击, 使魔受到39点伤害, 7_4_sCMdy1Bn3a受到19点伤害

 7_4_sCMdy1Bn3a被击倒了

 使魔消失了

 3_3_CLlY0shqQG毒性发作, 3_3_CLlY0shqQG受到26点伤害

 3_3_CLlY0shqQG被击倒了

2_1_e1WWAVAvLp发起攻击, 0_1_rlGNQ5hOKA受到53点伤害

1_1_5XOMkXw6ZJ发起攻击, 3_0_6feNVPHi6I受到56点伤害

1_2_okhFFQdvkH发起攻击, 4_0_JGb1ZmLQVk受到80点伤害

 4_0_JGb1ZmLQVk被击倒了

3_2_Qm1Yw54wkB发起攻击, 2_2_8sMzAtrEch受到55点伤害, 2_2_8sMzAtrEch发动隐匿

0_3_g4Mliv7h8p发起攻击, 5_4_CUzJzbZWVF受到39点伤害

8_0_lXCyiAxOVK使用生命之轮, 2_1_e1WWAVAvLp回避了攻击

7_2_GqhUwR5mit开始蓄力

7_3_yXAlHGdeFr发起攻击, 8_1_K5CiUWkJoh受到77点伤害

 8_1_K5CiUWkJoh被击倒了

6_1_XPv6PBKtGI发起攻击, 5_2_ccCEcs2MAw受到64点伤害

 6_1_XPv6PBKtGI从迟缓中解除

4_4_UrJENM2ezJ发起攻击, 幻影受到62点伤害

0_2_M5Mc5NV1q8使用诅咒, 5_4_CUzJzbZWVF受到83点伤害

 5_4_CUzJzbZWVF被击倒了

幻影发起攻击, 9_2_eQ40qJ3AKy受到94点伤害

9_1_N76NRqq3kb发起攻击, 2_3_5L6gnWViBJ受到101点伤害

 2_3_5L6gnWViBJ被击倒了

8_4_4f0sDTwwp1发起攻击, 3_2_Qm1Yw54wkB受到27点伤害

 3_2_Qm1Yw54wkB被击倒了

4_1_uGBsxTkOtq投毒, 1_0_dSjTgEqqjn受到68点伤害, 1_0_dSjTgEqqjn中毒

5_0_7iRQyT8dPL发起攻击, 1_3_YBPaZB6Dhy受到77点伤害

 1_3_YBPaZB6Dhy被击倒了

9_4_9G6Tk9gEZ9从冰冻中解除

6_4_m8d2wsmEFH发起攻击, 9_0_VE6VPhljRS受到112点伤害

 9_0_VE6VPhljRS被击倒了

使魔发起攻击, 2_1_e1WWAVAvLp受到40点伤害

1_2_okhFFQdvkH发起攻击, 使魔回避了攻击

0_3_g4Mliv7h8p发起攻击, 7_1_QlLuaXiO2M受到32点伤害

 0_3_g4Mliv7h8p从疾走中解除

9_3_vNEkD3Usi4使用净化, 7_1_QlLuaXiO2M受到57点伤害

7_1_QlLuaXiO2M发起攻击, 3_0_6feNVPHi6I回避了攻击

9_4_9G6Tk9gEZ9发起攻击, 0_1_rlGNQ5hOKA回避了攻击

 9_4_9G6Tk9gEZ9毒性发作, 9_4_9G6Tk9gEZ9受到30点伤害

 9_4_9G6Tk9gEZ9被击倒了

0_1_rlGNQ5hOKA发起攻击, 3_1_pNK1RsauUP回避了攻击

2_2_8sMzAtrEch发起攻击, 1_1_5XOMkXw6ZJ受到34点伤害

2_0_IbeJa2acsU发起攻击, 4_3_R1pR4DgsUY受到149点伤害

 4_3_R1pR4DgsUY被击倒了

8_2_BMmSPrdWgn发起攻击, 2_1_e1WWAVAvLp受到50点伤害

5_2_ccCEcs2MAw发起攻击, 诅咒使伤害加倍, 6_2_m6a59ZPPZu受到152点伤害

 6_2_m6a59ZPPZu被击倒了

3_1_pNK1RsauUP发动铁壁, 3_1_pNK1RsauUP防御力大幅上升

9_2_eQ40qJ3AKy发起攻击, 0_4_yiTQ8gtxlq受到90点伤害

2_1_e1WWAVAvLp开始聚气, 2_1_e1WWAVAvLp攻击力上升

5_3_TSqokhGEdE发起攻击, 6_1_XPv6PBKtGI受到77点伤害

1_0_dSjTgEqqjn发起攻击, 3_1_pNK1RsauUP受到1点伤害

 1_0_dSjTgEqqjn毒性发作, 1_0_dSjTgEqqjn受到18点伤害

使魔发起攻击, 5_3_TSqokhGEdE受到20点伤害

0_0_XJc3TefogX发起攻击, 9_1_N76NRqq3kb受到49点伤害

 9_1_N76NRqq3kb被击倒了

5_1_vrEpgvq9Zd发起攻击, 1_1_5XOMkXw6ZJ回避了攻击

0_4_yiTQ8gtxlq使用火球术, 7_2_GqhUwR5mit受到59点伤害

6_3_xRYfDW2HTr使用火球术, 9_3_vNEkD3Usi4受到163点伤害

 9_3_vNEkD3Usi4被击倒了

 6_3_xRYfDW2HTr从迟缓中解除

4_2_93jwt8T8YN使用诅咒, 3_1_pNK1RsauUP受到1点伤害, 3_1_pNK1RsauUP被诅咒了

6_0_gPpnRCjiTn发起攻击, 5_2_ccCEcs2MAw回避了攻击

1_2_okhFFQdvkH发起攻击, 使魔回避了攻击

 1_2_okhFFQdvkH从疾走中解除

1_1_5XOMkXw6ZJ发起攻击, 5_1_vrEpgvq9Zd受到104点伤害

 5_1_vrEpgvq9Zd被击倒了

4_0_JGb1ZmLQVk发起攻击, 0_2_M5Mc5NV1q8受到53点伤害

 0_2_M5Mc5NV1q8被击倒了

8_4_4f0sDTwwp1使用治愈魔法, 8_0_lXCyiAxOVK回复体力81点

5_0_7iRQyT8dPL发起攻击, 2_4_ZwzTEFviED回避了攻击

3_4_28zfba4nKR使用生命之轮, 2_4_ZwzTEFviED回避了攻击

使魔使用自爆, 5_0_7iRQyT8dPL受到176点伤害

 5_0_7iRQyT8dPL被击倒了

 使魔消失了

3_0_6feNVPHi6I发起攻击, 6_1_XPv6PBKtGI受到55点伤害

7_3_yXAlHGdeFr使用苏生术, 7_0_lzOMNgIkPL复活了, 7_0_lzOMNgIkPL回复体力92点

4_4_UrJENM2ezJ发起攻击, 0_4_yiTQ8gtxlq受到78点伤害

 0_4_yiTQ8gtxlq被击倒了

2_4_ZwzTEFviED发起攻击, 6_3_xRYfDW2HTr受到61点伤害

 6_3_xRYfDW2HTr被击倒了

8_2_BMmSPrdWgn发起攻击, 9_2_eQ40qJ3AKy受到70点伤害

 9_2_eQ40qJ3AKy被击倒了

8_3_P09BI8WP06发起攻击, 7_3_yXAlHGdeFr受到41点伤害

4_1_uGBsxTkOtq发起攻击, 7_1_QlLuaXiO2M受到74点伤害

7_2_GqhUwR5mit使用狂暴术, 5_2_ccCEcs2MAw受到206点伤害

 5_2_ccCEcs2MAw被击倒了

2_2_8sMzAtrEch发起攻击, 6_0_gPpnRCjiTn受到43点伤害

6_4_m8d2wsmEFH发起攻击, 4_0_JGb1ZmLQVk受到102点伤害

 4_0_JGb1ZmLQVk被击倒了

2_0_IbeJa2acsU发起攻击, 4_4_UrJENM2ezJ受到185点伤害

8_0_lXCyiAxOVK发起攻击, 6_1_XPv6PBKtGI受到58点伤害

7_1_QlLuaXiO2M发起攻击, 1_2_okhFFQdvkH受到68点伤害

1_1_5XOMkXw6ZJ发起攻击, 7_0_lzOMNgIkPL受到77点伤害

 7_0_lzOMNgIkPL做出垂死抗争, 7_0_lzOMNgIkPL所有属性上升

1_0_dSjTgEqqjn发起攻击, 8_0_lXCyiAxOVK受到77点伤害

 1_0_dSjTgEqqjn毒性发作, 1_0_dSjTgEqqjn受到15点伤害

幻影使用附体, 4_4_UrJENM2ezJ进入狂暴状态

 幻影消失了

7_0_lzOMNgIkPL发起攻击, 2_1_e1WWAVAvLp受到75点伤害

0_3_g4Mliv7h8p发起攻击, 2_1_e1WWAVAvLp受到47点伤害

2_1_e1WWAVAvLp发起攻击, 7_2_GqhUwR5mit受到175点伤害

 7_2_GqhUwR5mit被击倒了

6_1_XPv6PBKtGI使用加速术, 6_1_XPv6PBKtGI进入疾走状态

1_2_okhFFQdvkH发起攻击, 3_0_6feNVPHi6I回避了攻击

0_0_XJc3TefogX发起攻击, 8_2_BMmSPrdWgn受到116点伤害

 8_2_BMmSPrdWgn被击倒了

0_1_rlGNQ5hOKA发起攻击, 3_4_28zfba4nKR受到69点伤害

4_2_93jwt8T8YN发起攻击, 0_1_rlGNQ5hOKA回避了攻击

6_0_gPpnRCjiTn发起攻击, 8_0_lXCyiAxOVK受到84点伤害

8_3_P09BI8WP06发起攻击, 2_4_ZwzTEFviED受到52点伤害

5_3_TSqokhGEdE发起攻击, 3_0_6feNVPHi6I受到121点伤害

 3_0_6feNVPHi6I被击倒了

 使魔消失了

4_4_UrJENM2ezJ发起狂暴攻击, 6_0_gPpnRCjiTn回避了攻击

6_4_m8d2wsmEFH发起攻击, 0_1_rlGNQ5hOKA受到102点伤害

 0_1_rlGNQ5hOKA被击倒了, 0_1_rlGNQ5hOKA使用护身符抵挡了一次死亡, 0_1_rlGNQ5hOKA回复体力14点

7_0_lzOMNgIkPL发起攻击, 3_4_28zfba4nKR受到50点伤害

3_1_pNK1RsauUP发起攻击, 8_0_lXCyiAxOVK受到123点伤害

 8_0_lXCyiAxOVK被击倒了

8_4_4f0sDTwwp1使用雷击术

 3_4_28zfba4nKR受到13点伤害

 3_4_28zfba4nKR被击倒了

6_1_XPv6PBKtGI使用加速术, 6_4_m8d2wsmEFH进入疾走状态

1_0_dSjTgEqqjn使用狂暴术, 2_4_ZwzTEFviED回避了攻击

 1_0_dSjTgEqqjn毒性发作, 1_0_dSjTgEqqjn受到12点伤害

2_4_ZwzTEFviED发起攻击, 4_1_uGBsxTkOtq受到112点伤害

 4_1_uGBsxTkOtq被击倒了

1_1_5XOMkXw6ZJ发起攻击, 7_3_yXAlHGdeFr受到71点伤害

 7_3_yXAlHGdeFr被击倒了

2_0_IbeJa2acsU发起攻击, 0_3_g4Mliv7h8p受到86点伤害

7_1_QlLuaXiO2M发起攻击, 2_0_IbeJa2acsU受到109点伤害

 2_0_IbeJa2acsU被击倒了

2_1_e1WWAVAvLp发起攻击, 8_3_P09BI8WP06受到88点伤害

 2_1_e1WWAVAvLp连击, 8_3_P09BI8WP06受到61点伤害

 8_3_P09BI8WP06被击倒了

 2_1_e1WWAVAvLp吞噬了8_3_P09BI8WP06, 2_1_e1WWAVAvLp属性上升

6_1_XPv6PBKtGI使用火球术, 2_1_e1WWAVAvLp受到81点伤害

 2_1_e1WWAVAvLp被击倒了

 6_1_XPv6PBKtGI从疾走中解除

2_2_8sMzAtrEch发起攻击, 0_3_g4Mliv7h8p受到38点伤害

 0_3_g4Mliv7h8p被击倒了

6_4_m8d2wsmEFH使用加速术, 6_4_m8d2wsmEFH进入疾走状态

3_1_pNK1RsauUP发起攻击, 6_0_gPpnRCjiTn受到95点伤害

 6_0_gPpnRCjiTn被击倒了

 3_1_pNK1RsauUP从铁壁中解除

1_1_5XOMkXw6ZJ发起攻击, 6_4_m8d2wsmEFH受到91点伤害

 6_4_m8d2wsmEFH被击倒了

7_0_lzOMNgIkPL使用冰冻术, 5_3_TSqokhGEdE受到67点伤害, 5_3_TSqokhGEdE被冰冻了

0_0_XJc3TefogX使用分身, 出现一个新的0_0_XJc3TefogX

1_2_okhFFQdvkH发起攻击, 7_0_lzOMNgIkPL回避了攻击

8_4_4f0sDTwwp1使用诅咒, 2_4_ZwzTEFviED受到78点伤害, 2_4_ZwzTEFviED被诅咒了

7_1_QlLuaXiO2M发起攻击, 1_1_5XOMkXw6ZJ受到67点伤害

 1_1_5XOMkXw6ZJ被击倒了

4_4_UrJENM2ezJ发起狂暴攻击, 4_2_93jwt8T8YN受到119点伤害

 4_2_93jwt8T8YN被击倒了

0_1_rlGNQ5hOKA发起吸血攻击, 4_4_UrJENM2ezJ受到129点伤害, 0_1_rlGNQ5hOKA回复体力65点

 4_4_UrJENM2ezJ被击倒了

2_4_ZwzTEFviED发起攻击, 0_0_XJc3TefogX受到37点伤害

 0_0_XJc3TefogX被击倒了

7_0_lzOMNgIkPL使用火球术, 0_1_rlGNQ5hOKA受到134点伤害

 0_1_rlGNQ5hOKA被击倒了

2_2_8sMzAtrEch发起攻击, 7_0_lzOMNgIkPL回避了攻击

1_0_dSjTgEqqjn发起攻击, 6_1_XPv6PBKtGI受到97点伤害

 6_1_XPv6PBKtGI被击倒了, 6_1_XPv6PBKtGI使用护身符抵挡了一次死亡, 6_1_XPv6PBKtGI回复体力10点

 1_0_dSjTgEqqjn毒性发作, 1_0_dSjTgEqqjn受到10点伤害

 1_0_dSjTgEqqjn从中毒中解除

0_0_XJc3TefogX使用魅惑, 3_1_pNK1RsauUP回避了攻击

5_3_TSqokhGEdE从冰冻中解除

8_4_4f0sDTwwp1发起攻击, 2_2_8sMzAtrEch受到18点伤害

 2_2_8sMzAtrEch被击倒了

7_1_QlLuaXiO2M发起攻击, 诅咒使伤害加倍, 3_1_pNK1RsauUP受到204点伤害

 3_1_pNK1RsauUP被击倒了

5_3_TSqokhGEdE发起攻击, 1_2_okhFFQdvkH受到151点伤害

 1_2_okhFFQdvkH被击倒了

6_1_XPv6PBKtGI发起攻击, 7_0_lzOMNgIkPL受到62点伤害

 7_0_lzOMNgIkPL被击倒了

2_4_ZwzTEFviED发起攻击, 1_0_dSjTgEqqjn受到37点伤害

1_0_dSjTgEqqjn使用狂暴术, 5_3_TSqokhGEdE受到65点伤害

 5_3_TSqokhGEdE被击倒了

8_4_4f0sDTwwp1发起攻击, 诅咒使伤害加倍, 2_4_ZwzTEFviED受到44点伤害

 2_4_ZwzTEFviED被击倒了

0_0_XJc3TefogX使用狂暴术, 8_4_4f0sDTwwp1回避了攻击

7_1_QlLuaXiO2M使用冰冻术, 1_0_dSjTgEqqjn受到50点伤害

 1_0_dSjTgEqqjn被击倒了

6_1_XPv6PBKtGI发起攻击, 7_1_QlLuaXiO2M受到68点伤害

 7_1_QlLuaXiO2M被击倒了

0_0_XJc3TefogX发起攻击, 8_4_4f0sDTwwp1回避了攻击

8_4_4f0sDTwwp1发起攻击, 6_1_XPv6PBKtGI受到31点伤害

 6_1_XPv6PBKtGI被击倒了

0_0_XJc3TefogX使用分身, 出现一个新的0_0_XJc3TefogX

8_4_4f0sDTwwp1使用诅咒, 0_0_XJc3TefogX受到61点伤害

 0_0_XJc3TefogX被击倒了, 0_0_XJc3TefogX使用护身符抵挡了一次死亡, 0_0_XJc3TefogX回复体力5点

0_0_XJc3TefogX发起攻击, 8_4_4f0sDTwwp1回避了攻击

0_0_XJc3TefogX发起攻击, 8_4_4f0sDTwwp1受到54点伤害

"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    const MAX_ROUNDS: usize = 2_000;
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard) = collect_replay_lines(&mut runner, MAX_ROUNDS, true);
    if guard >= MAX_ROUNDS {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(20);
        let ctx_end = (mismatch_idx + 20).min(min_len);
        eprintln!("fight_multi_2 timeout mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_2 combat did not finish in expected rounds: guard={guard}, actual_len={}, expected_len={}, mismatch_idx={mismatch_idx}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx),
        );
    }
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_2 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_2 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

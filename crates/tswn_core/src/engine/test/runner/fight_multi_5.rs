use super::*;

#[test]
fn fight_multi_5() {
    const FIGHT_CASE: &str = r###"0_0_OE041oFbxn
0_1_Qo0QQK1cyp
0_2_Kjs42l7qLc
0_3_3yyBH01Ruk
0_4_KEQagm1yVt

1_0_inTzTYXKLg
1_1_pW74kGLASb
1_2_IBhfLGaNsK
1_3_Q4REgRSRHw
1_4_U2di8J7PIZ

2_0_FgBSKpHg5G
2_1_0m8LcAvZn0
2_2_qmay4KEmCs
2_3_D9eZulzxlN
2_4_qoYVfisd8I

3_0_Ttf3Fzu1fC
3_1_73rrc9RBPk
3_2_JFts4vMKQq
3_3_U9aUlFUkQn
3_4_fnElC2wk4b

4_0_BrpPqO7E2s
4_1_cOwiWoxv9q
4_2_LuaT50Ntjm
4_3_dpiZEedx4b
4_4_LYhigLx9Nw

5_0_1U5ENiqHNp
5_1_SFkR0HDMPc
5_2_xSJnozzuFB
5_3_UKX9rRxhNP
5_4_gZXiHA2roi

6_0_v6aT5f5EUQ
6_1_WPlosQ1BmN
6_2_s2BXaAtOaL
6_3_a4kJg3pBwA
6_4_8bDLxfaHnA

7_0_w6v54BKNxq
7_1_qPwrO17Lw8
7_2_c5mQKcNyuD
7_3_E2RnuLDSwp
7_4_dJxJS4mBh8

8_0_SlWCnIlcXS
8_1_jwcjtdGp2a
8_2_al8yGl6In6
8_3_eBqI1A8FlT
8_4_BxUtRZaxjR

9_0_K3Hq1PAaLO
9_1_MlhfQz4qe0
9_2_OcYSe3uM5D
9_3_XTzlXwKPI3
9_4_xxvEpksjiM


2_0_FgBSKpHg5G发起攻击, 4_4_LYhigLx9Nw受到21点伤害

9_1_MlhfQz4qe0发起攻击, 7_0_w6v54BKNxq受到108点伤害

2_2_qmay4KEmCs发起攻击, 3_3_U9aUlFUkQn受到75点伤害

3_1_73rrc9RBPk发起攻击, 8_1_jwcjtdGp2a受到58点伤害

2_1_0m8LcAvZn0使用狂暴术, 4_4_LYhigLx9Nw受到44点伤害, 4_4_LYhigLx9Nw进入狂暴状态

4_1_cOwiWoxv9q发起攻击, 1_1_pW74kGLASb受到50点伤害

5_0_1U5ENiqHNp发起攻击, 4_4_LYhigLx9Nw受到58点伤害

 4_4_LYhigLx9Nw发起反击, 5_0_1U5ENiqHNp受到73点伤害

3_4_fnElC2wk4b发起攻击, 6_1_WPlosQ1BmN受到96点伤害

4_3_dpiZEedx4b发起攻击, 6_3_a4kJg3pBwA受到86点伤害

7_2_c5mQKcNyuD发起攻击, 0_3_3yyBH01Ruk受到102点伤害

7_1_qPwrO17Lw8发起攻击, 4_3_dpiZEedx4b受到100点伤害

1_0_inTzTYXKLg使用诅咒, 0_3_3yyBH01Ruk受到37点伤害, 0_3_3yyBH01Ruk被诅咒了, 0_3_3yyBH01Ruk发动隐匿

9_4_xxvEpksjiM发起攻击, 6_4_8bDLxfaHnA受到70点伤害

9_2_OcYSe3uM5D发起攻击, 2_3_D9eZulzxlN回避了攻击

6_3_a4kJg3pBwA发起攻击, 4_1_cOwiWoxv9q受到102点伤害

0_0_OE041oFbxn发起攻击, 7_3_E2RnuLDSwp受到68点伤害

2_3_D9eZulzxlN发起攻击, 6_2_s2BXaAtOaL受到57点伤害

2_4_qoYVfisd8I发起攻击, 3_1_73rrc9RBPk回避了攻击

3_3_U9aUlFUkQn使用魅惑, 9_2_OcYSe3uM5D被魅惑了

5_1_SFkR0HDMPc发起攻击, 8_3_eBqI1A8FlT受到16点伤害

0_2_Kjs42l7qLc发起攻击, 8_4_BxUtRZaxjR受到39点伤害

6_0_v6aT5f5EUQ发起攻击, 2_2_qmay4KEmCs受到80点伤害

4_4_LYhigLx9Nw发起狂暴攻击, 1_2_IBhfLGaNsK受到131点伤害

 4_4_LYhigLx9Nw从狂暴中解除

9_0_K3Hq1PAaLO使用净化, 7_4_dJxJS4mBh8受到76点伤害

6_4_8bDLxfaHnA发起攻击, 2_1_0m8LcAvZn0受到63点伤害

1_4_U2di8J7PIZ发起攻击, 3_1_73rrc9RBPk回避了攻击

8_0_SlWCnIlcXS发起攻击, 5_0_1U5ENiqHNp受到72点伤害

1_1_pW74kGLASb发动会心一击, 5_1_SFkR0HDMPc受到100点伤害

6_1_WPlosQ1BmN发起攻击, 3_2_JFts4vMKQq受到89点伤害

8_4_BxUtRZaxjR使用诅咒, 7_2_c5mQKcNyuD受到45点伤害, 7_2_c5mQKcNyuD被诅咒了

5_3_UKX9rRxhNP发起攻击, 8_0_SlWCnIlcXS受到107点伤害

8_1_jwcjtdGp2a使用幻术, 召唤出幻影

7_3_E2RnuLDSwp发起攻击, 0_2_Kjs42l7qLc受到62点伤害

5_2_xSJnozzuFB使用血祭, 召唤出使魔

9_3_XTzlXwKPI3发起攻击, 8_2_al8yGl6In6受到64点伤害

0_3_3yyBH01Ruk发起攻击, 1_0_inTzTYXKLg受到54点伤害

1_2_IBhfLGaNsK发起攻击, 8_3_eBqI1A8FlT受到89点伤害

4_2_LuaT50Ntjm发起攻击, 5_4_gZXiHA2roi受到46点伤害

7_0_w6v54BKNxq发起攻击, 9_1_MlhfQz4qe0受到117点伤害, 9_1_MlhfQz4qe0发动隐匿

4_3_dpiZEedx4b潜行到3_3_U9aUlFUkQn身后

7_4_dJxJS4mBh8发起攻击, 1_1_pW74kGLASb受到82点伤害, 1_1_pW74kGLASb发动隐匿

0_1_Qo0QQK1cyp发起攻击, 6_2_s2BXaAtOaL受到68点伤害

8_3_eBqI1A8FlT发起攻击, 4_1_cOwiWoxv9q受到86点伤害

8_2_al8yGl6In6发起攻击, 2_2_qmay4KEmCs回避了攻击

4_0_BrpPqO7E2s发起攻击, 0_0_OE041oFbxn受到59点伤害

 4_0_BrpPqO7E2s连击, 0_0_OE041oFbxn受到68点伤害

3_0_Ttf3Fzu1fC发起攻击, 5_2_xSJnozzuFB受到71点伤害

3_1_73rrc9RBPk发起攻击, 7_1_qPwrO17Lw8受到122点伤害

9_2_OcYSe3uM5D发起攻击, 4_3_dpiZEedx4b受到21点伤害

 4_3_dpiZEedx4b的潜行被识破

 9_2_OcYSe3uM5D从魅惑中解除

6_2_s2BXaAtOaL发起攻击, 3_1_73rrc9RBPk回避了攻击

4_1_cOwiWoxv9q发起攻击, 9_4_xxvEpksjiM受到69点伤害

0_2_Kjs42l7qLc发起攻击, 2_4_qoYVfisd8I回避了攻击

0_4_KEQagm1yVt发起攻击, 2_4_qoYVfisd8I受到109点伤害

7_1_qPwrO17Lw8发起吸血攻击, 0_2_Kjs42l7qLc受到75点伤害, 7_1_qPwrO17Lw8回复体力38点

2_2_qmay4KEmCs发起攻击, 1_2_IBhfLGaNsK受到56点伤害

5_4_gZXiHA2roi发起攻击, 3_2_JFts4vMKQq受到47点伤害

2_1_0m8LcAvZn0投毒, 使魔受到29点伤害, 使魔中毒, 5_2_xSJnozzuFB受到14点伤害

3_2_JFts4vMKQq发起攻击, 6_4_8bDLxfaHnA回避了攻击

9_1_MlhfQz4qe0使用雷击术

 3_4_fnElC2wk4b受到29点伤害

 3_4_fnElC2wk4b受到41点伤害, 3_4_fnElC2wk4b发动隐匿

 3_4_fnElC2wk4b受到19点伤害

 3_4_fnElC2wk4b受到17点伤害

 3_4_fnElC2wk4b受到14点伤害

1_3_Q4REgRSRHw发起攻击, 6_4_8bDLxfaHnA回避了攻击

1_4_U2di8J7PIZ发起攻击, 3_1_73rrc9RBPk受到80点伤害, 3_1_73rrc9RBPk发动隐匿

7_2_c5mQKcNyuD使用分身, 出现一个新的7_2_c5mQKcNyuD

2_4_qoYVfisd8I开始聚气, 2_4_qoYVfisd8I攻击力上升

5_2_xSJnozzuFB发起攻击, 8_0_SlWCnIlcXS受到79点伤害

6_3_a4kJg3pBwA发起攻击, 0_1_Qo0QQK1cyp受到41点伤害

9_3_XTzlXwKPI3使用治愈魔法, 9_1_MlhfQz4qe0回复体力116点

5_3_UKX9rRxhNP发起攻击, 3_3_U9aUlFUkQn防御, 3_3_U9aUlFUkQn受到46点伤害

3_3_U9aUlFUkQn发动铁壁, 3_3_U9aUlFUkQn防御力大幅上升

2_0_FgBSKpHg5G发起攻击, 6_0_v6aT5f5EUQ受到64点伤害

9_4_xxvEpksjiM使用净化, 幻影受到170点伤害

 幻影消失了

5_1_SFkR0HDMPc使用瘟疫, 1_0_inTzTYXKLg体力减少63%

使魔发起攻击, 9_2_OcYSe3uM5D受到51点伤害

 使魔毒性发作, 使魔受到14点伤害, 5_2_xSJnozzuFB受到7点伤害

4_4_LYhigLx9Nw发起攻击, 3_0_Ttf3Fzu1fC受到61点伤害

0_0_OE041oFbxn发起攻击, 4_4_LYhigLx9Nw受到48点伤害

4_3_dpiZEedx4b使用减速术, 9_1_MlhfQz4qe0进入迟缓状态

6_4_8bDLxfaHnA发起攻击, 9_4_xxvEpksjiM受到99点伤害

1_0_inTzTYXKLg使用诅咒, 7_1_qPwrO17Lw8受到107点伤害, 7_1_qPwrO17Lw8被诅咒了

8_1_jwcjtdGp2a发起攻击, 7_4_dJxJS4mBh8受到31点伤害

2_3_D9eZulzxlN发起攻击, 7_0_w6v54BKNxq受到58点伤害

8_0_SlWCnIlcXS发起攻击, 5_4_gZXiHA2roi回避了攻击

1_2_IBhfLGaNsK发起攻击, 4_2_LuaT50Ntjm受到59点伤害

4_2_LuaT50Ntjm发起攻击, 5_4_gZXiHA2roi受到58点伤害

6_0_v6aT5f5EUQ发起攻击, 3_0_Ttf3Fzu1fC受到94点伤害

5_0_1U5ENiqHNp发动铁壁, 5_0_1U5ENiqHNp防御力大幅上升

2_4_qoYVfisd8I发起攻击, 4_0_BrpPqO7E2s回避了攻击

2_2_qmay4KEmCs发起攻击, 8_3_eBqI1A8FlT受到44点伤害

9_4_xxvEpksjiM发起攻击, 2_1_0m8LcAvZn0受到90点伤害

9_2_OcYSe3uM5D发起攻击, 5_3_UKX9rRxhNP回避了攻击

6_2_s2BXaAtOaL发起攻击, 4_3_dpiZEedx4b受到68点伤害

4_1_cOwiWoxv9q发起攻击, 9_4_xxvEpksjiM受到60点伤害

 9_4_xxvEpksjiM被击倒了

1_1_pW74kGLASb发动会心一击, 8_1_jwcjtdGp2a受到128点伤害

7_2_c5mQKcNyuD发起攻击, 2_4_qoYVfisd8I受到36点伤害

3_0_Ttf3Fzu1fC发起攻击, 4_4_LYhigLx9Nw受到70点伤害

 4_4_LYhigLx9Nw发起反击, 3_0_Ttf3Fzu1fC受到50点伤害

3_4_fnElC2wk4b发起攻击, 2_3_D9eZulzxlN回避了攻击

1_3_Q4REgRSRHw发起攻击, 3_2_JFts4vMKQq回避了攻击

5_4_gZXiHA2roi使用火球术, 7_2_c5mQKcNyuD受到50点伤害

7_3_E2RnuLDSwp使用幻术, 召唤出幻影

8_2_al8yGl6In6发起攻击, 5_1_SFkR0HDMPc受到6点伤害

4_0_BrpPqO7E2s发起攻击, 1_4_U2di8J7PIZ回避了攻击

9_0_K3Hq1PAaLO发起攻击, 5_3_UKX9rRxhNP受到26点伤害, 5_3_UKX9rRxhNP发动隐匿

2_1_0m8LcAvZn0发起攻击, 5_4_gZXiHA2roi受到65点伤害

0_3_3yyBH01Ruk使用狂暴术, 3_3_U9aUlFUkQn受到1点伤害, 3_3_U9aUlFUkQn进入狂暴状态

0_1_Qo0QQK1cyp发起攻击, 4_4_LYhigLx9Nw受到69点伤害

3_2_JFts4vMKQq发起攻击, 5_2_xSJnozzuFB受到106点伤害

8_3_eBqI1A8FlT发起攻击, 7_4_dJxJS4mBh8回避了攻击

6_4_8bDLxfaHnA发起攻击, 7_0_w6v54BKNxq受到84点伤害

8_4_BxUtRZaxjR发起攻击, 6_0_v6aT5f5EUQ受到41点伤害, 6_0_v6aT5f5EUQ发动隐匿

3_1_73rrc9RBPk发起攻击, 9_0_K3Hq1PAaLO受到102点伤害

 9_0_K3Hq1PAaLO发起反击, 3_1_73rrc9RBPk回避了攻击

9_3_XTzlXwKPI3发起攻击, 3_3_U9aUlFUkQn受到1点伤害

6_1_WPlosQ1BmN发起攻击, 8_2_al8yGl6In6受到69点伤害

 6_1_WPlosQ1BmN连击, 8_2_al8yGl6In6受到53点伤害

1_4_U2di8J7PIZ发起攻击, 6_4_8bDLxfaHnA受到89点伤害

7_2_c5mQKcNyuD发起攻击, 4_0_BrpPqO7E2s受到61点伤害

7_1_qPwrO17Lw8使用生命之轮, 4_2_LuaT50Ntjm的体力值与7_1_qPwrO17Lw8互换

2_0_FgBSKpHg5G发起攻击, 诅咒使伤害加倍, 7_2_c5mQKcNyuD受到162点伤害

 7_2_c5mQKcNyuD被击倒了

9_2_OcYSe3uM5D发起攻击, 0_1_Qo0QQK1cyp受到66点伤害

5_1_SFkR0HDMPc使用减速术, 幻影进入迟缓状态

0_2_Kjs42l7qLc发起攻击, 2_2_qmay4KEmCs受到26点伤害

6_0_v6aT5f5EUQ发起攻击, 4_4_LYhigLx9Nw受到91点伤害

 4_4_LYhigLx9Nw被击倒了

3_0_Ttf3Fzu1fC发起攻击, 4_0_BrpPqO7E2s受到38点伤害

0_4_KEQagm1yVt发起攻击, 1_3_Q4REgRSRHw受到65点伤害

4_3_dpiZEedx4b使用减速术, 9_2_OcYSe3uM5D进入迟缓状态

5_3_UKX9rRxhNP发起攻击, 9_1_MlhfQz4qe0受到60点伤害

8_1_jwcjtdGp2a发起攻击, 3_0_Ttf3Fzu1fC受到63点伤害

 3_0_Ttf3Fzu1fC被击倒了

8_0_SlWCnIlcXS使用地裂术

 使魔受到67点伤害, 5_2_xSJnozzuFB受到33点伤害

 使魔消失了

 4_0_BrpPqO7E2s受到54点伤害

 5_0_1U5ENiqHNp受到1点伤害

 1_2_IBhfLGaNsK受到33点伤害

7_4_dJxJS4mBh8发起攻击, 1_3_Q4REgRSRHw受到70点伤害, 1_3_Q4REgRSRHw发动隐匿

4_2_LuaT50Ntjm开始聚气, 4_2_LuaT50Ntjm攻击力上升

7_0_w6v54BKNxq发起攻击, 3_1_73rrc9RBPk受到27点伤害

7_2_c5mQKcNyuD发起攻击, 5_0_1U5ENiqHNp回避了攻击

1_0_inTzTYXKLg发起攻击, 9_2_OcYSe3uM5D受到88点伤害

2_2_qmay4KEmCs发起攻击, 9_1_MlhfQz4qe0受到98点伤害

6_3_a4kJg3pBwA发起攻击, 2_1_0m8LcAvZn0受到73点伤害

0_3_3yyBH01Ruk使用狂暴术, 4_2_LuaT50Ntjm受到66点伤害

 4_2_LuaT50Ntjm被击倒了

4_1_cOwiWoxv9q使用净化, 2_1_0m8LcAvZn0受到83点伤害

5_4_gZXiHA2roi使用瘟疫, 0_4_KEQagm1yVt体力减少60%

7_3_E2RnuLDSwp发起攻击, 5_3_UKX9rRxhNP受到77点伤害, 5_3_UKX9rRxhNP发动隐匿

3_2_JFts4vMKQq发起攻击, 2_3_D9eZulzxlN受到65点伤害

8_2_al8yGl6In6发起攻击, 3_4_fnElC2wk4b受到57点伤害, 3_4_fnElC2wk4b发动隐匿

6_0_v6aT5f5EUQ使用诅咒, 2_4_qoYVfisd8I受到86点伤害, 2_4_qoYVfisd8I被诅咒了

5_0_1U5ENiqHNp使用治愈魔法, 5_0_1U5ENiqHNp回复体力117点

7_1_qPwrO17Lw8发起攻击, 2_2_qmay4KEmCs受到35点伤害

2_3_D9eZulzxlN开始聚气, 2_3_D9eZulzxlN攻击力上升

2_4_qoYVfisd8I发起攻击, 幻影受到61点伤害

6_2_s2BXaAtOaL发起攻击, 0_3_3yyBH01Ruk回避了攻击

1_2_IBhfLGaNsK发起攻击, 8_4_BxUtRZaxjR防御, 8_4_BxUtRZaxjR受到59点伤害

3_4_fnElC2wk4b发起攻击, 6_2_s2BXaAtOaL受到112点伤害

0_0_OE041oFbxn使用治愈魔法, 0_0_OE041oFbxn回复体力127点

9_0_K3Hq1PAaLO使用净化, 4_0_BrpPqO7E2s受到79点伤害

1_3_Q4REgRSRHw发起攻击, 4_3_dpiZEedx4b受到42点伤害

3_3_U9aUlFUkQn发起狂暴攻击, 5_4_gZXiHA2roi受到82点伤害

 3_3_U9aUlFUkQn从狂暴中解除

3_1_73rrc9RBPk发起攻击, 4_0_BrpPqO7E2s受到78点伤害

2_1_0m8LcAvZn0发起攻击, 8_2_al8yGl6In6受到66点伤害

8_0_SlWCnIlcXS发起攻击, 0_0_OE041oFbxn受到115点伤害

0_1_Qo0QQK1cyp发起攻击, 7_4_dJxJS4mBh8受到68点伤害

8_3_eBqI1A8FlT发起攻击, 0_2_Kjs42l7qLc受到89点伤害

0_4_KEQagm1yVt发起攻击, 2_3_D9eZulzxlN回避了攻击

6_4_8bDLxfaHnA发起攻击, 8_1_jwcjtdGp2a受到83点伤害

8_4_BxUtRZaxjR发起攻击, 3_3_U9aUlFUkQn受到1点伤害

5_2_xSJnozzuFB发起攻击, 3_3_U9aUlFUkQn受到1点伤害

1_1_pW74kGLASb发动铁壁, 1_1_pW74kGLASb防御力大幅上升

0_2_Kjs42l7qLc发起攻击, 诅咒使伤害加倍, 7_1_qPwrO17Lw8受到98点伤害

4_3_dpiZEedx4b发起攻击, 9_3_XTzlXwKPI3受到97点伤害

9_3_XTzlXwKPI3发起攻击, 1_4_U2di8J7PIZ受到33点伤害

5_1_SFkR0HDMPc发起攻击, 7_3_E2RnuLDSwp受到50点伤害

5_0_1U5ENiqHNp发起攻击, 0_0_OE041oFbxn受到101点伤害

 5_0_1U5ENiqHNp从铁壁中解除

2_2_qmay4KEmCs发起攻击, 7_2_c5mQKcNyuD受到44点伤害

7_0_w6v54BKNxq发起攻击, 0_4_KEQagm1yVt受到23点伤害

7_2_c5mQKcNyuD发起攻击, 0_0_OE041oFbxn受到86点伤害

6_0_v6aT5f5EUQ使用减速术, 1_4_U2di8J7PIZ进入迟缓状态

5_3_UKX9rRxhNP发起攻击, 0_2_Kjs42l7qLc受到105点伤害

 0_2_Kjs42l7qLc被击倒了

5_4_gZXiHA2roi发起攻击, 2_2_qmay4KEmCs受到30点伤害

2_0_FgBSKpHg5G投毒, 5_3_UKX9rRxhNP受到66点伤害, 5_3_UKX9rRxhNP中毒, 5_3_UKX9rRxhNP发动隐匿

8_0_SlWCnIlcXS发起攻击, 5_1_SFkR0HDMPc受到104点伤害

 5_1_SFkR0HDMPc被击倒了

7_4_dJxJS4mBh8发起攻击, 6_0_v6aT5f5EUQ受到45点伤害, 6_0_v6aT5f5EUQ发动隐匿

3_2_JFts4vMKQq发起攻击, 7_1_qPwrO17Lw8受到43点伤害

4_1_cOwiWoxv9q发起攻击, 2_0_FgBSKpHg5G受到75点伤害

6_1_WPlosQ1BmN发起攻击, 1_4_U2di8J7PIZ回避了攻击

0_0_OE041oFbxn发起攻击, 6_1_WPlosQ1BmN受到45点伤害

9_0_K3Hq1PAaLO使用净化, 2_2_qmay4KEmCs受到98点伤害

9_1_MlhfQz4qe0发起攻击, 7_1_qPwrO17Lw8受到59点伤害

6_4_8bDLxfaHnA发起攻击, 2_0_FgBSKpHg5G受到77点伤害

7_3_E2RnuLDSwp发起攻击, 1_1_pW74kGLASb受到1点伤害

2_1_0m8LcAvZn0发起攻击, 5_0_1U5ENiqHNp受到53点伤害

6_3_a4kJg3pBwA发起攻击, 1_0_inTzTYXKLg受到59点伤害

6_2_s2BXaAtOaL发起攻击, 2_4_qoYVfisd8I回避了攻击

1_3_Q4REgRSRHw发起攻击, 2_2_qmay4KEmCs回避了攻击

8_1_jwcjtdGp2a发起攻击, 3_1_73rrc9RBPk受到42点伤害

3_1_73rrc9RBPk发起攻击, 6_3_a4kJg3pBwA受到70点伤害

8_2_al8yGl6In6使用治愈魔法, 8_0_SlWCnIlcXS回复体力85点

4_0_BrpPqO7E2s发起攻击, 诅咒使伤害加倍, 0_3_3yyBH01Ruk受到116点伤害

0_4_KEQagm1yVt使用减速术, 8_0_SlWCnIlcXS进入迟缓状态

1_4_U2di8J7PIZ发起攻击, 2_3_D9eZulzxlN受到67点伤害

7_1_qPwrO17Lw8发起吸血攻击, 9_2_OcYSe3uM5D回避了攻击

1_0_inTzTYXKLg发起攻击, 5_0_1U5ENiqHNp受到51点伤害

2_2_qmay4KEmCs发起攻击, 6_3_a4kJg3pBwA守护6_1_WPlosQ1BmN, 6_3_a4kJg3pBwA受到21点伤害

9_2_OcYSe3uM5D发起攻击, 5_0_1U5ENiqHNp受到97点伤害

1_1_pW74kGLASb发起攻击, 7_3_E2RnuLDSwp受到107点伤害

 7_3_E2RnuLDSwp被击倒了

 幻影消失了

8_3_eBqI1A8FlT发起攻击, 5_0_1U5ENiqHNp受到75点伤害

 5_0_1U5ENiqHNp被击倒了

3_4_fnElC2wk4b发动铁壁, 3_4_fnElC2wk4b防御力大幅上升

8_4_BxUtRZaxjR发起攻击, 3_3_U9aUlFUkQn受到1点伤害

2_3_D9eZulzxlN发起攻击, 9_0_K3Hq1PAaLO受到90点伤害

3_3_U9aUlFUkQn发起攻击, 2_1_0m8LcAvZn0受到78点伤害

 2_1_0m8LcAvZn0被击倒了

 3_3_U9aUlFUkQn从铁壁中解除

0_3_3yyBH01Ruk使用狂暴术, 1_4_U2di8J7PIZ受到29点伤害, 1_4_U2di8J7PIZ进入狂暴状态

3_2_JFts4vMKQq发起攻击, 9_3_XTzlXwKPI3受到32点伤害

7_0_w6v54BKNxq发起攻击, 9_2_OcYSe3uM5D受到80点伤害

6_0_v6aT5f5EUQ发起攻击, 3_3_U9aUlFUkQn受到71点伤害

6_4_8bDLxfaHnA发起攻击, 7_4_dJxJS4mBh8受到47点伤害

3_1_73rrc9RBPk发起攻击, 6_3_a4kJg3pBwA受到63点伤害

5_2_xSJnozzuFB发起攻击, 3_4_fnElC2wk4b受到0点伤害

6_3_a4kJg3pBwA发起攻击, 1_3_Q4REgRSRHw受到49点伤害

6_2_s2BXaAtOaL发起攻击, 9_1_MlhfQz4qe0受到56点伤害

0_1_Qo0QQK1cyp发起攻击, 6_0_v6aT5f5EUQ受到52点伤害

0_0_OE041oFbxn使用冰冻术, 7_4_dJxJS4mBh8受到16点伤害, 7_4_dJxJS4mBh8被冰冻了

4_3_dpiZEedx4b发起攻击, 1_1_pW74kGLASb受到1点伤害

1_3_Q4REgRSRHw发起攻击, 2_2_qmay4KEmCs回避了攻击

2_4_qoYVfisd8I发起攻击, 6_2_s2BXaAtOaL受到73点伤害

 6_2_s2BXaAtOaL被击倒了

 2_4_qoYVfisd8I吞噬了6_2_s2BXaAtOaL, 2_4_qoYVfisd8I属性上升

5_4_gZXiHA2roi使用瘟疫, 6_4_8bDLxfaHnA体力减少59%

4_1_cOwiWoxv9q发起攻击, 1_1_pW74kGLASb受到1点伤害

1_2_IBhfLGaNsK发起攻击, 8_0_SlWCnIlcXS受到82点伤害

9_0_K3Hq1PAaLO使用生命之轮, 2_3_D9eZulzxlN回避了攻击

1_0_inTzTYXKLg发起攻击, 3_1_73rrc9RBPk受到31点伤害

8_2_al8yGl6In6发起攻击, 0_4_KEQagm1yVt回避了攻击

5_3_UKX9rRxhNP发起攻击, 0_1_Qo0QQK1cyp受到21点伤害

 5_3_UKX9rRxhNP毒性发作, 5_3_UKX9rRxhNP受到13点伤害, 5_3_UKX9rRxhNP发动隐匿

8_0_SlWCnIlcXS发起攻击, 3_1_73rrc9RBPk受到26点伤害, 3_1_73rrc9RBPk发动隐匿

0_4_KEQagm1yVt发动铁壁, 0_4_KEQagm1yVt防御力大幅上升

7_1_qPwrO17Lw8发起吸血攻击, 1_0_inTzTYXKLg受到55点伤害, 7_1_qPwrO17Lw8回复体力28点

 1_0_inTzTYXKLg被击倒了

8_1_jwcjtdGp2a发起攻击, 2_3_D9eZulzxlN受到46点伤害

2_3_D9eZulzxlN发起攻击

 1_1_pW74kGLASb的铁壁被打消了, 1_1_pW74kGLASb受到5点伤害

2_0_FgBSKpHg5G发起攻击, 1_4_U2di8J7PIZ受到101点伤害

 1_4_U2di8J7PIZ发起反击, 2_0_FgBSKpHg5G回避了攻击

0_3_3yyBH01Ruk发起攻击, 7_4_dJxJS4mBh8受到46点伤害

3_2_JFts4vMKQq使用治愈魔法, 3_3_U9aUlFUkQn回复体力90点

8_3_eBqI1A8FlT发起攻击, 6_1_WPlosQ1BmN受到49点伤害

6_1_WPlosQ1BmN发动会心一击, 9_2_OcYSe3uM5D回避了攻击

7_2_c5mQKcNyuD使用净化, 0_1_Qo0QQK1cyp受到97点伤害

0_0_OE041oFbxn使用苏生术, 0_2_Kjs42l7qLc复活了, 0_2_Kjs42l7qLc回复体力114点

2_2_qmay4KEmCs发起攻击, 7_1_qPwrO17Lw8回避了攻击

3_3_U9aUlFUkQn使用魅惑, 6_1_WPlosQ1BmN被魅惑了

3_1_73rrc9RBPk发起攻击, 2_3_D9eZulzxlN受到57点伤害

6_3_a4kJg3pBwA使用瘟疫, 8_0_SlWCnIlcXS体力减少38%

 8_0_SlWCnIlcXS发起反击, 6_3_a4kJg3pBwA受到85点伤害

 6_3_a4kJg3pBwA被击倒了

1_1_pW74kGLASb发起攻击, 8_2_al8yGl6In6受到31点伤害

6_4_8bDLxfaHnA发起攻击, 5_2_xSJnozzuFB受到73点伤害

 5_2_xSJnozzuFB被击倒了

9_3_XTzlXwKPI3发起攻击, 2_0_FgBSKpHg5G受到56点伤害

7_0_w6v54BKNxq发起攻击, 6_4_8bDLxfaHnA受到47点伤害

4_0_BrpPqO7E2s发起攻击, 6_4_8bDLxfaHnA受到50点伤害

 6_4_8bDLxfaHnA被击倒了

4_3_dpiZEedx4b使用生命之轮, 0_4_KEQagm1yVt的体力值与4_3_dpiZEedx4b互换

9_1_MlhfQz4qe0发起攻击, 3_1_73rrc9RBPk回避了攻击

 9_1_MlhfQz4qe0从迟缓中解除

8_4_BxUtRZaxjR发起攻击, 诅咒使伤害加倍, 2_4_qoYVfisd8I受到180点伤害

 2_4_qoYVfisd8I被击倒了

4_1_cOwiWoxv9q发起攻击, 2_2_qmay4KEmCs受到80点伤害

 2_2_qmay4KEmCs被击倒了

6_0_v6aT5f5EUQ发起攻击, 0_0_OE041oFbxn受到44点伤害

 0_0_OE041oFbxn被击倒了

5_3_UKX9rRxhNP发起攻击, 1_4_U2di8J7PIZ受到64点伤害

 5_3_UKX9rRxhNP毒性发作, 5_3_UKX9rRxhNP受到11点伤害

0_1_Qo0QQK1cyp发起攻击, 9_3_XTzlXwKPI3受到75点伤害

8_3_eBqI1A8FlT使用血祭, 召唤出使魔

8_2_al8yGl6In6发起攻击, 3_2_JFts4vMKQq受到70点伤害

0_4_KEQagm1yVt发起攻击, 8_3_eBqI1A8FlT受到135点伤害

 8_3_eBqI1A8FlT被击倒了

 使魔消失了

9_0_K3Hq1PAaLO发起攻击, 1_2_IBhfLGaNsK受到64点伤害

 1_2_IBhfLGaNsK被击倒了

1_4_U2di8J7PIZ发起攻击, 3_4_fnElC2wk4b受到0点伤害

 1_4_U2di8J7PIZ从迟缓中解除

8_1_jwcjtdGp2a发起攻击, 诅咒使伤害加倍, 7_1_qPwrO17Lw8受到56点伤害

0_2_Kjs42l7qLc发起攻击, 9_0_K3Hq1PAaLO受到37点伤害

 9_0_K3Hq1PAaLO被击倒了

3_4_fnElC2wk4b发起攻击, 4_3_dpiZEedx4b受到69点伤害

1_3_Q4REgRSRHw使用治愈魔法, 1_4_U2di8J7PIZ回复体力197点

 1_4_U2di8J7PIZ从狂暴中解除

2_0_FgBSKpHg5G使用火球术, 8_4_BxUtRZaxjR受到74点伤害

7_4_dJxJS4mBh8从冰冻中解除

1_1_pW74kGLASb发动会心一击, 7_2_c5mQKcNyuD受到85点伤害

 7_2_c5mQKcNyuD被击倒了

2_3_D9eZulzxlN发起攻击, 0_4_KEQagm1yVt受到1点伤害

7_4_dJxJS4mBh8发起吸血攻击, 4_0_BrpPqO7E2s受到103点伤害, 7_4_dJxJS4mBh8回复体力52点

 4_0_BrpPqO7E2s被击倒了

9_2_OcYSe3uM5D发起攻击, 7_0_w6v54BKNxq受到72点伤害

 7_0_w6v54BKNxq被击倒了, 7_0_w6v54BKNxq使用护身符抵挡了一次死亡, 7_0_w6v54BKNxq回复体力2点

 9_2_OcYSe3uM5D从迟缓中解除

3_2_JFts4vMKQq使用苏生术, 3_0_Ttf3Fzu1fC复活了, 3_0_Ttf3Fzu1fC回复体力89点

6_1_WPlosQ1BmN发起攻击, 6_1_WPlosQ1BmN受到50点伤害

 6_1_WPlosQ1BmN连击, 6_1_WPlosQ1BmN受到70点伤害

 6_1_WPlosQ1BmN被击倒了

8_4_BxUtRZaxjR发起攻击, 9_2_OcYSe3uM5D受到72点伤害

 9_2_OcYSe3uM5D被击倒了, 9_2_OcYSe3uM5D使用护身符抵挡了一次死亡, 9_2_OcYSe3uM5D回复体力13点

5_4_gZXiHA2roi发起攻击, 9_2_OcYSe3uM5D受到98点伤害

 9_2_OcYSe3uM5D被击倒了

4_1_cOwiWoxv9q发起攻击, 8_0_SlWCnIlcXS受到124点伤害

 8_0_SlWCnIlcXS被击倒了

7_0_w6v54BKNxq发起攻击, 0_2_Kjs42l7qLc受到37点伤害

3_3_U9aUlFUkQn发动铁壁, 3_3_U9aUlFUkQn防御力大幅上升

3_1_73rrc9RBPk发起攻击, 0_2_Kjs42l7qLc受到62点伤害

9_1_MlhfQz4qe0发起攻击, 3_2_JFts4vMKQq受到53点伤害

 3_2_JFts4vMKQq被击倒了

5_3_UKX9rRxhNP使用苏生术, 5_1_SFkR0HDMPc复活了, 5_1_SFkR0HDMPc回复体力58点

 5_3_UKX9rRxhNP毒性发作, 5_3_UKX9rRxhNP受到9点伤害

7_1_qPwrO17Lw8发起攻击, 3_1_73rrc9RBPk受到36点伤害, 3_1_73rrc9RBPk发动隐匿

0_1_Qo0QQK1cyp发起攻击, 9_1_MlhfQz4qe0受到83点伤害

 9_1_MlhfQz4qe0被击倒了

4_3_dpiZEedx4b发起攻击, 1_4_U2di8J7PIZ受到66点伤害

9_3_XTzlXwKPI3发起攻击, 3_0_Ttf3Fzu1fC受到85点伤害

0_2_Kjs42l7qLc发起攻击, 3_4_fnElC2wk4b受到0点伤害

0_4_KEQagm1yVt使用减速术, 1_4_U2di8J7PIZ进入迟缓状态

 0_4_KEQagm1yVt从铁壁中解除

8_1_jwcjtdGp2a发起攻击, 3_4_fnElC2wk4b受到0点伤害

0_3_3yyBH01Ruk发起攻击, 3_3_U9aUlFUkQn受到1点伤害

8_2_al8yGl6In6发起攻击, 1_3_Q4REgRSRHw受到33点伤害, 1_3_Q4REgRSRHw发动隐匿

6_0_v6aT5f5EUQ发起攻击, 5_3_UKX9rRxhNP受到41点伤害

 5_3_UKX9rRxhNP被击倒了

3_4_fnElC2wk4b投毒, 1_4_U2di8J7PIZ受到87点伤害, 1_4_U2di8J7PIZ中毒

 3_4_fnElC2wk4b从铁壁中解除

5_4_gZXiHA2roi发起攻击, 0_2_Kjs42l7qLc回避了攻击

2_0_FgBSKpHg5G发起攻击, 8_4_BxUtRZaxjR回避了攻击

1_1_pW74kGLASb使用地裂术

 0_4_KEQagm1yVt受到41点伤害

 诅咒使伤害加倍, 7_1_qPwrO17Lw8受到58点伤害

 7_1_qPwrO17Lw8被击倒了

 4_3_dpiZEedx4b受到53点伤害

 4_3_dpiZEedx4b被击倒了

 2_0_FgBSKpHg5G回避了攻击

 5_4_gZXiHA2roi受到23点伤害

 5_4_gZXiHA2roi被击倒了

1_4_U2di8J7PIZ发起攻击, 诅咒使伤害加倍, 0_3_3yyBH01Ruk受到98点伤害

 0_3_3yyBH01Ruk被击倒了

 1_4_U2di8J7PIZ毒性发作, 1_4_U2di8J7PIZ受到27点伤害, 1_4_U2di8J7PIZ发动隐匿

 1_4_U2di8J7PIZ发起反击, 3_4_fnElC2wk4b受到111点伤害

 3_4_fnElC2wk4b被击倒了

2_3_D9eZulzxlN发起攻击, 8_2_al8yGl6In6受到112点伤害

 8_2_al8yGl6In6被击倒了

7_4_dJxJS4mBh8发起攻击, 3_3_U9aUlFUkQn受到1点伤害

4_1_cOwiWoxv9q发起攻击, 0_2_Kjs42l7qLc受到44点伤害

 0_2_Kjs42l7qLc被击倒了

7_0_w6v54BKNxq发起攻击, 3_0_Ttf3Fzu1fC回避了攻击

3_0_Ttf3Fzu1fC发起攻击, 7_4_dJxJS4mBh8受到16点伤害

3_3_U9aUlFUkQn使用狂暴术, 2_3_D9eZulzxlN受到31点伤害

 2_3_D9eZulzxlN被击倒了

0_4_KEQagm1yVt发起攻击, 3_3_U9aUlFUkQn受到1点伤害

3_1_73rrc9RBPk发动铁壁, 3_1_73rrc9RBPk防御力大幅上升

0_1_Qo0QQK1cyp发起攻击, 7_4_dJxJS4mBh8回避了攻击

5_1_SFkR0HDMPc发起攻击, 4_1_cOwiWoxv9q受到30点伤害

1_3_Q4REgRSRHw使用治愈魔法, 1_1_pW74kGLASb回复体力138点

8_4_BxUtRZaxjR发起攻击, 1_1_pW74kGLASb受到89点伤害

9_3_XTzlXwKPI3发起攻击, 3_1_73rrc9RBPk受到0点伤害

8_1_jwcjtdGp2a发起攻击, 3_1_73rrc9RBPk受到0点伤害

2_0_FgBSKpHg5G投毒, 8_4_BxUtRZaxjR受到30点伤害, 8_4_BxUtRZaxjR中毒

6_0_v6aT5f5EUQ使用诅咒, 0_4_KEQagm1yVt受到61点伤害

 0_4_KEQagm1yVt被击倒了

7_4_dJxJS4mBh8发起攻击, 4_1_cOwiWoxv9q受到60点伤害

 4_1_cOwiWoxv9q被击倒了

1_1_pW74kGLASb发起攻击, 9_3_XTzlXwKPI3回避了攻击

3_3_U9aUlFUkQn使用狂暴术, 7_4_dJxJS4mBh8回避了攻击

 3_3_U9aUlFUkQn从铁壁中解除

2_0_FgBSKpHg5G使用火球术, 1_1_pW74kGLASb受到55点伤害

9_3_XTzlXwKPI3发起攻击, 3_1_73rrc9RBPk受到0点伤害

3_0_Ttf3Fzu1fC发起攻击, 7_4_dJxJS4mBh8受到40点伤害

 7_4_dJxJS4mBh8被击倒了

6_0_v6aT5f5EUQ发起攻击, 9_3_XTzlXwKPI3受到88点伤害

 9_3_XTzlXwKPI3被击倒了

7_0_w6v54BKNxq发起攻击, 3_3_U9aUlFUkQn回避了攻击

5_1_SFkR0HDMPc使用魅惑, 1_1_pW74kGLASb被魅惑了

1_4_U2di8J7PIZ发起攻击, 3_3_U9aUlFUkQn受到43点伤害

 1_4_U2di8J7PIZ毒性发作, 1_4_U2di8J7PIZ受到22点伤害

 1_4_U2di8J7PIZ从迟缓中解除

8_4_BxUtRZaxjR发起攻击, 2_0_FgBSKpHg5G受到70点伤害

 2_0_FgBSKpHg5G被击倒了

 8_4_BxUtRZaxjR毒性发作, 8_4_BxUtRZaxjR受到13点伤害

3_1_73rrc9RBPk发起攻击, 1_3_Q4REgRSRHw受到37点伤害, 1_3_Q4REgRSRHw发动隐匿

0_1_Qo0QQK1cyp发起攻击, 3_3_U9aUlFUkQn受到85点伤害

6_0_v6aT5f5EUQ使用诅咒, 1_1_pW74kGLASb回避了攻击

1_3_Q4REgRSRHw使用减速术, 7_0_w6v54BKNxq进入迟缓状态

8_1_jwcjtdGp2a发起攻击, 1_3_Q4REgRSRHw受到59点伤害

 1_3_Q4REgRSRHw被击倒了

1_1_pW74kGLASb发动铁壁, 1_1_pW74kGLASb防御力大幅上升

 1_1_pW74kGLASb从魅惑中解除

8_4_BxUtRZaxjR使用诅咒, 3_3_U9aUlFUkQn受到45点伤害, 3_3_U9aUlFUkQn被诅咒了

 8_4_BxUtRZaxjR毒性发作, 8_4_BxUtRZaxjR受到11点伤害

5_1_SFkR0HDMPc发起攻击, 3_0_Ttf3Fzu1fC受到65点伤害

 3_0_Ttf3Fzu1fC被击倒了

0_1_Qo0QQK1cyp发起攻击, 7_0_w6v54BKNxq受到138点伤害

 7_0_w6v54BKNxq被击倒了, 7_0_w6v54BKNxq使用护身符抵挡了一次死亡, 7_0_w6v54BKNxq回复体力2点

1_4_U2di8J7PIZ发起攻击, 5_1_SFkR0HDMPc受到46点伤害

 1_4_U2di8J7PIZ毒性发作, 1_4_U2di8J7PIZ受到18点伤害

 1_4_U2di8J7PIZ被击倒了

3_3_U9aUlFUkQn发动铁壁, 3_3_U9aUlFUkQn防御力大幅上升

3_1_73rrc9RBPk发起攻击, 1_1_pW74kGLASb受到1点伤害

 3_1_73rrc9RBPk从铁壁中解除

6_0_v6aT5f5EUQ发起攻击, 3_3_U9aUlFUkQn受到1点伤害

8_1_jwcjtdGp2a使用瘟疫, 1_1_pW74kGLASb体力减少37%

7_0_w6v54BKNxq发起攻击, 诅咒使伤害加倍, 3_3_U9aUlFUkQn受到2点伤害

5_1_SFkR0HDMPc发起攻击, 3_1_73rrc9RBPk受到89点伤害

 3_1_73rrc9RBPk被击倒了

1_1_pW74kGLASb发起攻击, 6_0_v6aT5f5EUQ受到101点伤害

 6_0_v6aT5f5EUQ被击倒了

8_4_BxUtRZaxjR发起攻击, 7_0_w6v54BKNxq受到48点伤害

 7_0_w6v54BKNxq被击倒了

 8_4_BxUtRZaxjR毒性发作, 8_4_BxUtRZaxjR受到9点伤害

0_1_Qo0QQK1cyp发起攻击, 8_4_BxUtRZaxjR受到81点伤害

 8_4_BxUtRZaxjR被击倒了

8_1_jwcjtdGp2a发起攻击, 0_1_Qo0QQK1cyp受到80点伤害

 0_1_Qo0QQK1cyp被击倒了

5_1_SFkR0HDMPc潜行到8_1_jwcjtdGp2a身后

3_3_U9aUlFUkQn发起攻击, 1_1_pW74kGLASb受到1点伤害

5_1_SFkR0HDMPc发动背刺, 8_1_jwcjtdGp2a受到364点伤害

 8_1_jwcjtdGp2a被击倒了

1_1_pW74kGLASb发起攻击, 诅咒使伤害加倍, 3_3_U9aUlFUkQn受到2点伤害

 1_1_pW74kGLASb从铁壁中解除

5_1_SFkR0HDMPc潜行到3_3_U9aUlFUkQn身后

3_3_U9aUlFUkQn发起攻击, 1_1_pW74kGLASb受到85点伤害

 1_1_pW74kGLASb被击倒了

 3_3_U9aUlFUkQn从铁壁中解除

5_1_SFkR0HDMPc发动背刺, 诅咒使伤害加倍, 3_3_U9aUlFUkQn受到596点伤害

 3_3_U9aUlFUkQn被击倒了

"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 50_000, true);
    assert_eq!(total_score, 27113, "fight_multi_5 score mismatch");
    assert!(guard < 50_000, "fight_multi_5 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_5 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_5 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

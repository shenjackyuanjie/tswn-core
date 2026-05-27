//! 多队混战回放测试分片 8。
//!
//! 分片文件只保存大型 fixture，执行逻辑由 runner 测试公共 helper 负责。

use super::*;

#[test]
fn fight_multi_8() {
    const FIGHT_CASE: &str = r###"0_0_cx2a5TJT1F
0_1_2DSdSOmqo8
0_2_It3MVeWRmJ
0_3_KcXMSazHe2
0_4_1ZEBbcdl5o

1_0_sYdLgIJggu
1_1_WsH32vC2Gu
1_2_8DTBZp20BB
1_3_wDqcIqVe9e
1_4_4MWzi3MzY9

2_0_kldLWRtbum
2_1_gLmombPqNn
2_2_B7yhvE3uMU
2_3_PjQk7ectjH
2_4_evEIvoBPDc

3_0_R2uvu1CTA8
3_1_y0SpN68Tfv
3_2_eXeG9Oadai
3_3_T6IYOYJcav
3_4_CcwaEsL5uT

4_0_7Wx4XzLysw
4_1_T3InxdXePf
4_2_wUPgmCZfvl
4_3_2HmoMyxgxb
4_4_SsPbfKDVD5

5_0_yYQfJ9Egpw
5_1_gdigTVK1ik
5_2_EH5SAkztn2
5_3_xWWZWu2QRJ
5_4_RWsjMhGuyc

6_0_1s1lX0zQkp
6_1_iFwpBCKMan
6_2_tQx9103dxr
6_3_3eIf4PBwzN
6_4_tyTOHhNAUK

7_0_GCmMIAm9mp
7_1_rAnmcUDae9
7_2_V2V8XimRlj
7_3_eMQ6Oc4mqG
7_4_x16AnGvau5

8_0_PUXBd52Ido
8_1_gtPdYsT04c
8_2_PB8dxu381f
8_3_X3KSYuXaSl
8_4_QPtnhzOlUb

9_0_OZ1WnelQTe
9_1_fRCxSAgG8t
9_2_JoFsvqnTja
9_3_G6EWnZq9QG
9_4_T88581tOJe


9_1_fRCxSAgG8t使用冰冻术, 4_1_T3InxdXePf受到66点伤害, 4_1_T3InxdXePf被冰冻了

6_1_iFwpBCKMan发起攻击, 0_0_cx2a5TJT1F受到59点伤害

9_0_OZ1WnelQTe使用瘟疫, 4_3_2HmoMyxgxb体力减少47%

2_2_B7yhvE3uMU发起攻击, 4_3_2HmoMyxgxb受到73点伤害, 4_3_2HmoMyxgxb发动隐匿

1_3_wDqcIqVe9e发起攻击, 3_4_CcwaEsL5uT受到84点伤害

6_2_tQx9103dxr发起攻击, 8_2_PB8dxu381f受到56点伤害

0_0_cx2a5TJT1F发起攻击, 8_0_PUXBd52Ido受到59点伤害

4_0_7Wx4XzLysw发起攻击, 1_3_wDqcIqVe9e受到65点伤害

8_4_QPtnhzOlUb发起攻击, 3_0_R2uvu1CTA8受到12点伤害

3_0_R2uvu1CTA8发起攻击, 9_1_fRCxSAgG8t受到143点伤害

0_1_2DSdSOmqo8发起攻击, 5_0_yYQfJ9Egpw受到69点伤害

6_4_tyTOHhNAUK发起攻击, 5_1_gdigTVK1ik受到70点伤害

7_0_GCmMIAm9mp发起攻击, 2_2_B7yhvE3uMU受到54点伤害

8_3_X3KSYuXaSl发起攻击, 7_1_rAnmcUDae9受到111点伤害

9_2_JoFsvqnTja发起攻击, 8_4_QPtnhzOlUb受到91点伤害

7_2_V2V8XimRlj发起攻击, 5_2_EH5SAkztn2受到144点伤害

0_3_KcXMSazHe2发起攻击, 2_3_PjQk7ectjH受到66点伤害

3_4_CcwaEsL5uT发起攻击, 4_4_SsPbfKDVD5受到84点伤害, 4_4_SsPbfKDVD5发动隐匿

2_0_kldLWRtbum发起攻击, 0_2_It3MVeWRmJ受到94点伤害

7_4_x16AnGvau5发起攻击, 2_1_gLmombPqNn受到47点伤害

8_1_gtPdYsT04c发起攻击, 2_2_B7yhvE3uMU使用伤害反弹, 8_1_gtPdYsT04c受到79点伤害

3_2_eXeG9Oadai发起攻击, 0_3_KcXMSazHe2回避了攻击

6_0_1s1lX0zQkp发起攻击, 3_4_CcwaEsL5uT受到60点伤害

4_3_2HmoMyxgxb发起攻击, 6_3_3eIf4PBwzN受到30点伤害

4_2_wUPgmCZfvl发起攻击, 6_0_1s1lX0zQkp受到98点伤害

6_3_3eIf4PBwzN发起攻击, 2_2_B7yhvE3uMU受到68点伤害

5_4_RWsjMhGuyc发起攻击, 8_2_PB8dxu381f受到76点伤害

2_3_PjQk7ectjH发起攻击, 8_0_PUXBd52Ido受到61点伤害

8_2_PB8dxu381f发起攻击, 7_2_V2V8XimRlj受到139点伤害

1_1_WsH32vC2Gu发起攻击, 7_0_GCmMIAm9mp受到54点伤害

5_3_xWWZWu2QRJ发起攻击, 7_3_eMQ6Oc4mqG受到80点伤害

1_4_4MWzi3MzY9使用血祭, 召唤出使魔

0_4_1ZEBbcdl5o发起攻击, 1_2_8DTBZp20BB受到18点伤害

5_0_yYQfJ9Egpw使用血祭, 召唤出使魔

4_4_SsPbfKDVD5发起攻击, 1_4_4MWzi3MzY9受到66点伤害

5_1_gdigTVK1ik发起攻击, 2_0_kldLWRtbum受到28点伤害

8_0_PUXBd52Ido发起攻击, 7_1_rAnmcUDae9受到57点伤害

5_2_EH5SAkztn2发起攻击, 3_2_eXeG9Oadai受到66点伤害

2_4_evEIvoBPDc使用血祭, 召唤出使魔

3_1_y0SpN68Tfv发起攻击, 6_0_1s1lX0zQkp受到92点伤害

7_1_rAnmcUDae9发起攻击, 2_3_PjQk7ectjH受到66点伤害

9_3_G6EWnZq9QG投毒, 2_0_kldLWRtbum回避了攻击

9_4_T88581tOJe发起攻击, 5_4_RWsjMhGuyc受到35点伤害

7_3_eMQ6Oc4mqG使用加速术, 7_4_x16AnGvau5进入疾走状态

6_4_tyTOHhNAUK发起攻击, 1_3_wDqcIqVe9e受到74点伤害

1_0_sYdLgIJggu发起攻击, 2_4_evEIvoBPDc受到76点伤害

9_1_fRCxSAgG8t发起攻击, 4_2_wUPgmCZfvl回避了攻击

2_1_gLmombPqNn发起攻击, 1_2_8DTBZp20BB受到40点伤害

1_2_8DTBZp20BB发起攻击, 9_3_G6EWnZq9QG受到32点伤害

7_4_x16AnGvau5使用血祭, 召唤出使魔

使魔发起攻击, 8_0_PUXBd52Ido受到43点伤害

0_2_It3MVeWRmJ发起攻击, 使魔受到101点伤害, 1_4_4MWzi3MzY9受到50点伤害

 使魔消失了

8_4_QPtnhzOlUb发起攻击, 0_3_KcXMSazHe2受到65点伤害, 0_3_KcXMSazHe2发动隐匿

0_1_2DSdSOmqo8发起攻击, 6_3_3eIf4PBwzN受到55点伤害

7_0_GCmMIAm9mp发起攻击, 6_3_3eIf4PBwzN受到91点伤害

9_2_JoFsvqnTja发起攻击, 6_3_3eIf4PBwzN回避了攻击

3_3_T6IYOYJcav发起攻击, 6_2_tQx9103dxr受到66点伤害

0_3_KcXMSazHe2发起攻击, 2_3_PjQk7ectjH受到44点伤害

8_1_gtPdYsT04c发起攻击, 3_2_eXeG9Oadai受到77点伤害

5_1_gdigTVK1ik发起攻击, 4_1_T3InxdXePf受到80点伤害

6_1_iFwpBCKMan发起攻击, 5_1_gdigTVK1ik受到59点伤害

4_1_T3InxdXePf从冰冻中解除

4_3_2HmoMyxgxb发起攻击, 3_3_T6IYOYJcav受到80点伤害

8_2_PB8dxu381f发起攻击, 3_0_R2uvu1CTA8受到69点伤害

3_4_CcwaEsL5uT发起攻击, 4_2_wUPgmCZfvl受到66点伤害, 4_2_wUPgmCZfvl发动隐匿

2_0_kldLWRtbum发起攻击, 3_1_y0SpN68Tfv受到44点伤害

2_2_B7yhvE3uMU发起攻击, 4_0_7Wx4XzLysw受到113点伤害

使魔发起攻击, 4_3_2HmoMyxgxb受到50点伤害

使魔发起攻击, 8_1_gtPdYsT04c受到56点伤害

9_3_G6EWnZq9QG发起攻击, 6_4_tyTOHhNAUK受到55点伤害

1_4_4MWzi3MzY9潜行到4_1_T3InxdXePf身后

2_3_PjQk7ectjH发起攻击, 5_4_RWsjMhGuyc受到60点伤害

7_4_x16AnGvau5发起攻击, 5_3_xWWZWu2QRJ回避了攻击

3_2_eXeG9Oadai使用血祭, 召唤出使魔

6_0_1s1lX0zQkp发起攻击, 3_0_R2uvu1CTA8受到61点伤害

5_3_xWWZWu2QRJ使用减速术, 2_0_kldLWRtbum进入迟缓状态

7_1_rAnmcUDae9发起攻击, 2_3_PjQk7ectjH受到25点伤害

4_1_T3InxdXePf发起攻击, 1_2_8DTBZp20BB受到64点伤害

9_0_OZ1WnelQTe发起攻击, 0_1_2DSdSOmqo8受到95点伤害

8_0_PUXBd52Ido发起攻击, 7_3_eMQ6Oc4mqG受到50点伤害

1_3_wDqcIqVe9e开始聚气, 1_3_wDqcIqVe9e攻击力上升

6_2_tQx9103dxr发起攻击, 0_4_1ZEBbcdl5o防御, 0_4_1ZEBbcdl5o受到40点伤害

0_0_cx2a5TJT1F发起攻击, 8_4_QPtnhzOlUb受到75点伤害

4_2_wUPgmCZfvl发起攻击, 5_1_gdigTVK1ik回避了攻击

6_4_tyTOHhNAUK使用血祭, 召唤出使魔

0_4_1ZEBbcdl5o发起攻击, 1_2_8DTBZp20BB守护1_0_sYdLgIJggu, 1_2_8DTBZp20BB受到39点伤害

9_1_fRCxSAgG8t使用冰冻术, 1_0_sYdLgIJggu受到51点伤害, 1_0_sYdLgIJggu被冰冻了

5_0_yYQfJ9Egpw发起攻击, 1_0_sYdLgIJggu受到71点伤害

2_1_gLmombPqNn发起攻击, 9_2_JoFsvqnTja受到44点伤害

3_1_y0SpN68Tfv发起攻击, 2_0_kldLWRtbum受到32点伤害

9_4_T88581tOJe使用火球术, 1_2_8DTBZp20BB受到172点伤害

 1_2_8DTBZp20BB被击倒了

4_0_7Wx4XzLysw发起攻击, 2_4_evEIvoBPDc受到71点伤害

3_0_R2uvu1CTA8发起攻击, 6_1_iFwpBCKMan受到43点伤害

6_3_3eIf4PBwzN发起攻击, 0_0_cx2a5TJT1F受到88点伤害

5_4_RWsjMhGuyc发起攻击, 9_3_G6EWnZq9QG受到55点伤害

7_2_V2V8XimRlj发起攻击, 2_3_PjQk7ectjH受到61点伤害

5_2_EH5SAkztn2开始聚气, 5_2_EH5SAkztn2攻击力上升

2_4_evEIvoBPDc发起攻击, 7_4_x16AnGvau5受到38点伤害

使魔发起攻击, 1_1_WsH32vC2Gu回避了攻击

8_4_QPtnhzOlUb发起攻击, 6_1_iFwpBCKMan回避了攻击

0_1_2DSdSOmqo8发动会心一击, 1_0_sYdLgIJggu受到93点伤害

0_3_KcXMSazHe2发起攻击, 4_2_wUPgmCZfvl受到80点伤害, 4_2_wUPgmCZfvl发动隐匿

1_1_WsH32vC2Gu发起攻击, 8_0_PUXBd52Ido受到55点伤害

4_4_SsPbfKDVD5使用狂暴术, 9_0_OZ1WnelQTe受到70点伤害, 9_0_OZ1WnelQTe进入狂暴状态

7_4_x16AnGvau5发起攻击, 6_2_tQx9103dxr受到43点伤害

 7_4_x16AnGvau5从疾走中解除

8_1_gtPdYsT04c发起攻击, 3_3_T6IYOYJcav受到45点伤害

5_1_gdigTVK1ik使用地裂术

 使魔受到8点伤害, 3_2_eXeG9Oadai受到4点伤害

 使魔受到39点伤害, 7_4_x16AnGvau5受到19点伤害

 使魔受到46点伤害, 2_4_evEIvoBPDc受到23点伤害

 9_4_T88581tOJe受到19点伤害, 9_4_T88581tOJe发动隐匿

 8_0_PUXBd52Ido受到17点伤害

6_1_iFwpBCKMan发起攻击, 9_2_JoFsvqnTja受到71点伤害

8_3_X3KSYuXaSl发起攻击, 1_1_WsH32vC2Gu受到56点伤害

5_3_xWWZWu2QRJ发起攻击, 8_3_X3KSYuXaSl受到108点伤害

4_3_2HmoMyxgxb使用地裂术

 5_0_yYQfJ9Egpw受到7点伤害

 7_0_GCmMIAm9mp受到26点伤害

 3_3_T6IYOYJcav受到30点伤害

 7_1_rAnmcUDae9回避了攻击

 使魔受到34点伤害, 7_4_x16AnGvau5受到17点伤害, 7_4_x16AnGvau5发动隐匿

7_3_eMQ6Oc4mqG发起攻击, 5_0_yYQfJ9Egpw受到64点伤害

7_0_GCmMIAm9mp发起攻击, 4_3_2HmoMyxgxb受到28点伤害

9_2_JoFsvqnTja潜行到5_4_RWsjMhGuyc身后

使魔发起攻击, 3_4_CcwaEsL5uT回避了攻击

使魔发起攻击, 4_1_T3InxdXePf防御, 4_1_T3InxdXePf受到25点伤害, 4_1_T3InxdXePf发动隐匿

7_1_rAnmcUDae9发起攻击, 4_4_SsPbfKDVD5受到57点伤害, 4_4_SsPbfKDVD5发动隐匿

9_3_G6EWnZq9QG发起攻击, 使魔受到81点伤害, 7_4_x16AnGvau5受到40点伤害

 使魔消失了

6_4_tyTOHhNAUK发起攻击, 1_1_WsH32vC2Gu受到20点伤害

2_3_PjQk7ectjH发起攻击, 6_0_1s1lX0zQkp受到58点伤害

3_3_T6IYOYJcav发起攻击, 0_3_KcXMSazHe2受到51点伤害, 0_3_KcXMSazHe2发动隐匿

3_4_CcwaEsL5uT发起攻击, 2_1_gLmombPqNn受到115点伤害

2_2_B7yhvE3uMU发起攻击, 5_2_EH5SAkztn2受到82点伤害

3_2_eXeG9Oadai使用魅惑, 使魔被魅惑了

6_0_1s1lX0zQkp发起攻击, 0_4_1ZEBbcdl5o受到69点伤害

使魔发起攻击, 1_4_4MWzi3MzY9受到67点伤害

 1_4_4MWzi3MzY9的潜行被识破

使魔发起攻击, 1_1_WsH32vC2Gu受到55点伤害

 使魔从魅惑中解除

9_4_T88581tOJe使用火球术, 6_4_tyTOHhNAUK受到141点伤害

0_2_It3MVeWRmJ使用诅咒, 5_1_gdigTVK1ik受到38点伤害, 5_1_gdigTVK1ik被诅咒了

4_1_T3InxdXePf发起攻击, 2_4_evEIvoBPDc受到128点伤害

 2_4_evEIvoBPDc被击倒了

 使魔消失了

1_4_4MWzi3MzY9使用生命之轮, 6_1_iFwpBCKMan回避了攻击

0_1_2DSdSOmqo8发动会心一击, 9_4_T88581tOJe受到84点伤害, 9_4_T88581tOJe发动隐匿

9_0_OZ1WnelQTe发起狂暴攻击, 0_4_1ZEBbcdl5o受到133点伤害

 0_4_1ZEBbcdl5o被击倒了

 9_0_OZ1WnelQTe从狂暴中解除

9_1_fRCxSAgG8t发起攻击, 7_2_V2V8XimRlj受到61点伤害

5_0_yYQfJ9Egpw发起攻击, 9_3_G6EWnZq9QG受到60点伤害

0_3_KcXMSazHe2发起攻击, 诅咒使伤害加倍, 5_1_gdigTVK1ik受到124点伤害

1_3_wDqcIqVe9e发起攻击, 5_3_xWWZWu2QRJ受到63点伤害

4_0_7Wx4XzLysw发起攻击, 8_2_PB8dxu381f受到49点伤害

3_0_R2uvu1CTA8发起攻击, 5_4_RWsjMhGuyc受到81点伤害

4_2_wUPgmCZfvl开始蓄力

7_2_V2V8XimRlj发起攻击, 使魔受到56点伤害, 3_2_eXeG9Oadai受到28点伤害

8_2_PB8dxu381f发起攻击, 4_0_7Wx4XzLysw受到116点伤害, 4_0_7Wx4XzLysw发动隐匿

7_4_x16AnGvau5发起攻击, 3_3_T6IYOYJcav受到54点伤害

8_0_PUXBd52Ido发起攻击, 2_2_B7yhvE3uMU受到69点伤害

5_3_xWWZWu2QRJ发起吸血攻击, 1_4_4MWzi3MzY9回避了攻击

4_4_SsPbfKDVD5发起攻击, 8_4_QPtnhzOlUb受到123点伤害

 8_4_QPtnhzOlUb被击倒了

8_1_gtPdYsT04c发起攻击, 2_1_gLmombPqNn回避了攻击

5_1_gdigTVK1ik使用地裂术

 9_1_fRCxSAgG8t受到35点伤害

 4_4_SsPbfKDVD5受到30点伤害

 6_0_1s1lX0zQkp受到43点伤害

 使魔受到14点伤害, 3_2_eXeG9Oadai受到7点伤害

6_2_tQx9103dxr发起攻击, 3_0_R2uvu1CTA8受到127点伤害

3_1_y0SpN68Tfv发起攻击, 1_3_wDqcIqVe9e受到47点伤害

4_3_2HmoMyxgxb发起攻击, 5_3_xWWZWu2QRJ回避了攻击

6_3_3eIf4PBwzN发起攻击, 4_2_wUPgmCZfvl受到42点伤害, 4_2_wUPgmCZfvl发动隐匿

5_4_RWsjMhGuyc发起攻击, 0_1_2DSdSOmqo8受到80点伤害

2_3_PjQk7ectjH发起攻击, 8_0_PUXBd52Ido受到90点伤害

6_4_tyTOHhNAUK发起攻击, 0_3_KcXMSazHe2受到49点伤害, 0_3_KcXMSazHe2发动隐匿

7_0_GCmMIAm9mp发起攻击, 6_4_tyTOHhNAUK受到78点伤害

 6_4_tyTOHhNAUK被击倒了

 使魔消失了

2_0_kldLWRtbum发起攻击, 9_3_G6EWnZq9QG受到78点伤害

2_1_gLmombPqNn发起攻击, 使魔受到49点伤害, 5_0_yYQfJ9Egpw受到24点伤害

5_2_EH5SAkztn2发起攻击, 8_1_gtPdYsT04c受到78点伤害

6_0_1s1lX0zQkp发起攻击, 9_0_OZ1WnelQTe受到97点伤害

9_3_G6EWnZq9QG投毒, 1_1_WsH32vC2Gu受到57点伤害, 1_1_WsH32vC2Gu中毒

8_3_X3KSYuXaSl发起攻击, 7_0_GCmMIAm9mp受到96点伤害

 8_3_X3KSYuXaSl连击, 7_0_GCmMIAm9mp受到39点伤害

 8_3_X3KSYuXaSl连击, 7_0_GCmMIAm9mp受到27点伤害

3_3_T6IYOYJcav发起攻击, 5_1_gdigTVK1ik受到92点伤害

 5_1_gdigTVK1ik被击倒了

 3_3_T6IYOYJcav吞噬了5_1_gdigTVK1ik, 3_3_T6IYOYJcav属性上升

1_1_WsH32vC2Gu发起攻击, 9_0_OZ1WnelQTe受到90点伤害

 1_1_WsH32vC2Gu毒性发作, 1_1_WsH32vC2Gu受到32点伤害

使魔发起攻击, 9_1_fRCxSAgG8t受到42点伤害

 9_1_fRCxSAgG8t发起反击, 使魔受到60点伤害, 5_0_yYQfJ9Egpw受到30点伤害

 使魔消失了

0_0_cx2a5TJT1F发起攻击, 6_3_3eIf4PBwzN受到53点伤害

7_1_rAnmcUDae9使用地裂术

 6_0_1s1lX0zQkp受到39点伤害

 6_0_1s1lX0zQkp被击倒了

 0_0_cx2a5TJT1F受到37点伤害

 1_4_4MWzi3MzY9守护1_0_sYdLgIJggu, 1_4_4MWzi3MzY9受到0点伤害

 5_4_RWsjMhGuyc受到36点伤害

0_2_It3MVeWRmJ发起攻击, 9_1_fRCxSAgG8t受到71点伤害

7_3_eMQ6Oc4mqG发起攻击, 1_0_sYdLgIJggu受到54点伤害

 1_0_sYdLgIJggu被击倒了

9_2_JoFsvqnTja发动背刺, 5_4_RWsjMhGuyc受到221点伤害

 5_4_RWsjMhGuyc被击倒了

3_3_T6IYOYJcav使用地裂术

 7_1_rAnmcUDae9受到40点伤害

 6_1_iFwpBCKMan受到48点伤害

 7_2_V2V8XimRlj受到35点伤害

 6_3_3eIf4PBwzN受到31点伤害

 8_0_PUXBd52Ido受到11点伤害

7_2_V2V8XimRlj发起攻击, 0_1_2DSdSOmqo8受到123点伤害

 0_1_2DSdSOmqo8被击倒了

4_4_SsPbfKDVD5发起攻击, 5_0_yYQfJ9Egpw受到72点伤害

2_2_B7yhvE3uMU使用血祭, 召唤出使魔

3_2_eXeG9Oadai发起攻击, 2_2_B7yhvE3uMU受到56点伤害

6_1_iFwpBCKMan使用治愈魔法, 6_3_3eIf4PBwzN回复体力20点

4_1_T3InxdXePf发起攻击, 3_3_T6IYOYJcav受到77点伤害

 3_3_T6IYOYJcav被击倒了

1_4_4MWzi3MzY9使用分身, 出现一个新的1_4_4MWzi3MzY9

4_3_2HmoMyxgxb开始聚气, 4_3_2HmoMyxgxb攻击力上升

4_2_wUPgmCZfvl发起攻击, 使魔受到110点伤害, 2_2_B7yhvE3uMU受到55点伤害

 2_2_B7yhvE3uMU被击倒了

 使魔消失了

0_3_KcXMSazHe2发起攻击, 7_3_eMQ6Oc4mqG受到23点伤害

8_1_gtPdYsT04c发起攻击, 使魔受到57点伤害, 3_2_eXeG9Oadai受到28点伤害

 使魔消失了

5_3_xWWZWu2QRJ发起攻击, 6_1_iFwpBCKMan受到82点伤害

7_1_rAnmcUDae9发起攻击, 9_0_OZ1WnelQTe受到64点伤害

 9_0_OZ1WnelQTe被击倒了

9_4_T88581tOJe发起攻击, 4_4_SsPbfKDVD5受到127点伤害

 4_4_SsPbfKDVD5被击倒了

9_1_fRCxSAgG8t使用幻术, 召唤出幻影

5_0_yYQfJ9Egpw使用血祭, 召唤出使魔

2_3_PjQk7ectjH发起攻击, 5_0_yYQfJ9Egpw受到104点伤害

 5_0_yYQfJ9Egpw被击倒了

 使魔消失了

3_4_CcwaEsL5uT发起攻击, 2_0_kldLWRtbum受到91点伤害

7_4_x16AnGvau5使用冰冻术, 3_1_y0SpN68Tfv受到36点伤害, 3_1_y0SpN68Tfv被冰冻了

6_2_tQx9103dxr开始蓄力

3_0_R2uvu1CTA8发起攻击, 7_2_V2V8XimRlj受到76点伤害

7_3_eMQ6Oc4mqG发起攻击, 6_2_tQx9103dxr受到55点伤害

6_3_3eIf4PBwzN发起攻击, 4_1_T3InxdXePf回避了攻击

8_2_PB8dxu381f发起攻击, 5_3_xWWZWu2QRJ受到66点伤害

8_0_PUXBd52Ido发起攻击, 1_4_4MWzi3MzY9受到0点伤害

1_3_wDqcIqVe9e发起攻击, 4_1_T3InxdXePf受到157点伤害

 4_1_T3InxdXePf被击倒了

1_4_4MWzi3MzY9使用雷击术

 9_4_T88581tOJe受到36点伤害

 9_4_T88581tOJe受到30点伤害, 9_4_T88581tOJe发动隐匿

 9_4_T88581tOJe受到20点伤害

 9_4_T88581tOJe受到31点伤害

 9_4_T88581tOJe受到27点伤害

9_3_G6EWnZq9QG使用分身, 出现一个新的9_3_G6EWnZq9QG

0_2_It3MVeWRmJ使用诅咒, 8_0_PUXBd52Ido受到81点伤害

 8_0_PUXBd52Ido被击倒了

7_0_GCmMIAm9mp使用生命之轮, 5_3_xWWZWu2QRJ的体力值与7_0_GCmMIAm9mp互换

9_2_JoFsvqnTja发起攻击, 3_0_R2uvu1CTA8受到90点伤害

 3_0_R2uvu1CTA8被击倒了

0_0_cx2a5TJT1F发起攻击, 8_2_PB8dxu381f受到45点伤害

9_4_T88581tOJe发起攻击, 4_3_2HmoMyxgxb受到42点伤害

 4_3_2HmoMyxgxb被击倒了

4_0_7Wx4XzLysw发起攻击, 2_1_gLmombPqNn受到48点伤害

4_2_wUPgmCZfvl发起攻击, 1_4_4MWzi3MzY9受到135点伤害

 1_4_4MWzi3MzY9被击倒了

1_1_WsH32vC2Gu发起攻击, 8_3_X3KSYuXaSl受到63点伤害

 1_1_WsH32vC2Gu毒性发作, 1_1_WsH32vC2Gu受到26点伤害

8_1_gtPdYsT04c发起攻击, 幻影受到57点伤害

5_2_EH5SAkztn2发起攻击, 7_4_x16AnGvau5受到112点伤害

7_2_V2V8XimRlj发起攻击, 3_2_eXeG9Oadai受到48点伤害

2_1_gLmombPqNn发起攻击, 8_2_PB8dxu381f受到55点伤害

3_2_eXeG9Oadai发起攻击, 7_3_eMQ6Oc4mqG受到11点伤害

1_3_wDqcIqVe9e发起攻击, 0_0_cx2a5TJT1F回避了攻击

9_3_G6EWnZq9QG发起攻击, 2_1_gLmombPqNn受到67点伤害

 2_1_gLmombPqNn被击倒了

2_3_PjQk7ectjH使用诅咒, 幻影受到40点伤害, 幻影被诅咒了

0_3_KcXMSazHe2使用苏生术, 0_4_1ZEBbcdl5o复活了, 0_4_1ZEBbcdl5o回复体力21点

9_3_G6EWnZq9QG发起攻击, 0_3_KcXMSazHe2防御, 0_3_KcXMSazHe2受到23点伤害, 0_3_KcXMSazHe2发动隐匿

0_4_1ZEBbcdl5o发起攻击, 4_2_wUPgmCZfvl受到82点伤害

5_3_xWWZWu2QRJ发起攻击, 9_1_fRCxSAgG8t受到37点伤害

 9_1_fRCxSAgG8t被击倒了

 幻影消失了

6_1_iFwpBCKMan发起攻击, 8_3_X3KSYuXaSl受到81点伤害

7_3_eMQ6Oc4mqG使用瘟疫, 3_1_y0SpN68Tfv体力减少62%

8_3_X3KSYuXaSl发起攻击, 0_2_It3MVeWRmJ受到42点伤害

7_4_x16AnGvau5发起攻击, 6_1_iFwpBCKMan受到41点伤害

7_1_rAnmcUDae9发起攻击, 3_1_y0SpN68Tfv受到60点伤害

6_3_3eIf4PBwzN发起攻击, 7_2_V2V8XimRlj受到56点伤害

 7_2_V2V8XimRlj被击倒了

0_0_cx2a5TJT1F发起攻击, 1_1_WsH32vC2Gu受到62点伤害

5_3_xWWZWu2QRJ发起攻击, 7_0_GCmMIAm9mp受到65点伤害

9_4_T88581tOJe发动铁壁, 9_4_T88581tOJe防御力大幅上升

8_2_PB8dxu381f发起攻击, 7_3_eMQ6Oc4mqG受到65点伤害

1_1_WsH32vC2Gu发起攻击, 8_3_X3KSYuXaSl受到75点伤害

 8_3_X3KSYuXaSl被击倒了

 1_1_WsH32vC2Gu毒性发作, 1_1_WsH32vC2Gu受到22点伤害

 1_1_WsH32vC2Gu被击倒了

8_1_gtPdYsT04c开始聚气, 8_1_gtPdYsT04c攻击力上升

3_2_eXeG9Oadai使用血祭, 召唤出使魔

6_2_tQx9103dxr发起攻击, 7_0_GCmMIAm9mp受到128点伤害

1_4_4MWzi3MzY9潜行到2_0_kldLWRtbum身后

4_2_wUPgmCZfvl使用幻术, 召唤出幻影

2_3_PjQk7ectjH使用魅惑, 9_3_G6EWnZq9QG被魅惑了

2_0_kldLWRtbum使用血祭, 召唤出使魔

 2_0_kldLWRtbum从迟缓中解除

5_2_EH5SAkztn2使用幻术, 召唤出幻影

9_3_G6EWnZq9QG发起攻击, 2_0_kldLWRtbum受到85点伤害

0_2_It3MVeWRmJ发起攻击, 2_0_kldLWRtbum受到82点伤害

9_2_JoFsvqnTja使用苏生术, 9_0_OZ1WnelQTe复活了, 9_0_OZ1WnelQTe回复体力145点

1_3_wDqcIqVe9e发起攻击, 6_3_3eIf4PBwzN受到114点伤害

 6_3_3eIf4PBwzN被击倒了, 6_3_3eIf4PBwzN使用护身符抵挡了一次死亡, 6_3_3eIf4PBwzN回复体力1点

7_3_eMQ6Oc4mqG使用地裂术

 9_0_OZ1WnelQTe受到36点伤害

 6_2_tQx9103dxr受到12点伤害

 1_3_wDqcIqVe9e受到31点伤害

 9_4_T88581tOJe受到1点伤害

 9_3_G6EWnZq9QG受到0点伤害

0_3_KcXMSazHe2发起攻击, 8_1_gtPdYsT04c受到85点伤害

 8_1_gtPdYsT04c被击倒了

3_4_CcwaEsL5uT发起攻击, 0_3_KcXMSazHe2回避了攻击

3_1_y0SpN68Tfv从冰冻中解除

使魔发起攻击, 5_3_xWWZWu2QRJ受到42点伤害

 5_3_xWWZWu2QRJ被击倒了

9_0_OZ1WnelQTe使用苏生术, 9_1_fRCxSAgG8t复活了, 9_1_fRCxSAgG8t回复体力38点

6_1_iFwpBCKMan发起攻击, 3_1_y0SpN68Tfv受到91点伤害

 3_1_y0SpN68Tfv被击倒了

4_0_7Wx4XzLysw发起攻击, 7_3_eMQ6Oc4mqG受到84点伤害

 7_3_eMQ6Oc4mqG被击倒了

7_0_GCmMIAm9mp发起攻击, 9_3_G6EWnZq9QG受到94点伤害

 9_3_G6EWnZq9QG被击倒了

6_3_3eIf4PBwzN发起攻击, 9_3_G6EWnZq9QG受到80点伤害

 9_3_G6EWnZq9QG被击倒了

0_4_1ZEBbcdl5o发起攻击, 3_4_CcwaEsL5uT受到43点伤害

7_1_rAnmcUDae9使用血祭, 召唤出使魔

2_0_kldLWRtbum发起攻击, 9_1_fRCxSAgG8t受到41点伤害

 9_1_fRCxSAgG8t被击倒了

7_4_x16AnGvau5发起攻击, 0_2_It3MVeWRmJ受到57点伤害

6_2_tQx9103dxr发起攻击, 7_0_GCmMIAm9mp回避了攻击

0_0_cx2a5TJT1F发起攻击, 9_2_JoFsvqnTja受到36点伤害

9_4_T88581tOJe使用地裂术

 幻影受到20点伤害

 使魔受到52点伤害, 2_0_kldLWRtbum受到26点伤害

 2_0_kldLWRtbum被击倒了

 使魔消失了

 1_3_wDqcIqVe9e受到20点伤害

 7_0_GCmMIAm9mp受到29点伤害

 7_0_GCmMIAm9mp被击倒了

 3_4_CcwaEsL5uT受到42点伤害

 3_4_CcwaEsL5uT被击倒了, 3_4_CcwaEsL5uT使用护身符抵挡了一次死亡, 3_4_CcwaEsL5uT回复体力14点

8_2_PB8dxu381f发动铁壁, 8_2_PB8dxu381f防御力大幅上升

3_2_eXeG9Oadai使用魅惑, 9_0_OZ1WnelQTe被魅惑了

1_3_wDqcIqVe9e发起攻击, 使魔受到64点伤害, 7_1_rAnmcUDae9受到32点伤害

4_2_wUPgmCZfvl使用生命之轮, 6_2_tQx9103dxr的体力值与4_2_wUPgmCZfvl互换

2_3_PjQk7ectjH发起攻击, 3_4_CcwaEsL5uT受到21点伤害

 3_4_CcwaEsL5uT被击倒了

6_3_3eIf4PBwzN发起攻击, 0_0_cx2a5TJT1F受到44点伤害

6_1_iFwpBCKMan发起攻击, 幻影回避了攻击

0_3_KcXMSazHe2发起攻击, 4_0_7Wx4XzLysw受到63点伤害

 4_0_7Wx4XzLysw被击倒了

1_4_4MWzi3MzY9潜行到3_2_eXeG9Oadai身后

0_4_1ZEBbcdl5o发起攻击, 4_2_wUPgmCZfvl受到30点伤害, 4_2_wUPgmCZfvl发动隐匿

9_0_OZ1WnelQTe使用净化, 6_1_iFwpBCKMan受到74点伤害

 6_1_iFwpBCKMan被击倒了

 9_0_OZ1WnelQTe从魅惑中解除

7_1_rAnmcUDae9发起攻击, 0_2_It3MVeWRmJ受到60点伤害

 0_2_It3MVeWRmJ做出垂死抗争, 0_2_It3MVeWRmJ所有属性上升

0_2_It3MVeWRmJ发起攻击, 幻影受到83点伤害

5_2_EH5SAkztn2发起攻击, 1_3_wDqcIqVe9e回避了攻击

3_2_eXeG9Oadai发起攻击, 0_0_cx2a5TJT1F受到68点伤害

 0_0_cx2a5TJT1F被击倒了

使魔发起攻击, 7_4_x16AnGvau5受到66点伤害

 7_4_x16AnGvau5被击倒了

幻影发起攻击, 9_2_JoFsvqnTja受到33点伤害

使魔发起攻击, 幻影受到22点伤害

4_2_wUPgmCZfvl发起攻击, 3_2_eXeG9Oadai受到89点伤害

 3_2_eXeG9Oadai被击倒了

 使魔消失了

0_2_It3MVeWRmJ发起攻击, 幻影受到85点伤害

 幻影消失了

9_2_JoFsvqnTja发起攻击, 幻影受到151点伤害

 幻影消失了

8_2_PB8dxu381f发起攻击, 1_4_4MWzi3MzY9受到0点伤害

0_3_KcXMSazHe2发起攻击, 6_2_tQx9103dxr受到70点伤害

 6_2_tQx9103dxr被击倒了

9_4_T88581tOJe使用地裂术

 7_1_rAnmcUDae9受到35点伤害

 0_4_1ZEBbcdl5o受到68点伤害

 0_4_1ZEBbcdl5o被击倒了

 使魔受到24点伤害, 7_1_rAnmcUDae9受到12点伤害

 使魔消失了

 2_3_PjQk7ectjH受到40点伤害

 9_4_T88581tOJe从铁壁中解除

1_3_wDqcIqVe9e发起攻击, 7_1_rAnmcUDae9受到185点伤害

 7_1_rAnmcUDae9被击倒了

6_3_3eIf4PBwzN发起攻击, 1_4_4MWzi3MzY9受到166点伤害

 1_4_4MWzi3MzY9的潜行被识破

 1_4_4MWzi3MzY9被击倒了

2_3_PjQk7ectjH使用苏生术, 2_2_B7yhvE3uMU复活了, 2_2_B7yhvE3uMU回复体力55点

9_0_OZ1WnelQTe发起攻击, 0_3_KcXMSazHe2受到72点伤害

4_2_wUPgmCZfvl发起攻击, 0_3_KcXMSazHe2防御, 0_3_KcXMSazHe2受到27点伤害

 0_3_KcXMSazHe2被击倒了

9_2_JoFsvqnTja发起攻击, 2_3_PjQk7ectjH受到107点伤害

 2_3_PjQk7ectjH被击倒了

8_2_PB8dxu381f发起攻击, 9_0_OZ1WnelQTe受到54点伤害

 8_2_PB8dxu381f从铁壁中解除

5_2_EH5SAkztn2使用幻术, 召唤出幻影

1_3_wDqcIqVe9e使用生命之轮, 9_2_JoFsvqnTja的体力值与1_3_wDqcIqVe9e互换

0_2_It3MVeWRmJ发起攻击, 9_4_T88581tOJe受到61点伤害

 9_4_T88581tOJe被击倒了

6_3_3eIf4PBwzN发起攻击, 幻影受到39点伤害

2_2_B7yhvE3uMU发起攻击, 9_0_OZ1WnelQTe受到59点伤害

 9_0_OZ1WnelQTe被击倒了

4_2_wUPgmCZfvl发起攻击, 6_3_3eIf4PBwzN受到109点伤害

 6_3_3eIf4PBwzN被击倒了

8_2_PB8dxu381f使用加速术, 8_2_PB8dxu381f进入疾走状态

5_2_EH5SAkztn2发起攻击, 9_2_JoFsvqnTja受到203点伤害

 9_2_JoFsvqnTja被击倒了

2_2_B7yhvE3uMU发起攻击, 幻影受到135点伤害

 幻影消失了

8_2_PB8dxu381f发起攻击, 4_2_wUPgmCZfvl受到93点伤害

 4_2_wUPgmCZfvl被击倒了

0_2_It3MVeWRmJ发起攻击, 1_3_wDqcIqVe9e受到68点伤害

1_3_wDqcIqVe9e发起攻击, 5_2_EH5SAkztn2受到79点伤害

2_2_B7yhvE3uMU发起攻击, 8_2_PB8dxu381f受到75点伤害

 8_2_PB8dxu381f被击倒了

5_2_EH5SAkztn2发起攻击, 2_2_B7yhvE3uMU受到150点伤害

 2_2_B7yhvE3uMU被击倒了

1_3_wDqcIqVe9e发起攻击, 5_2_EH5SAkztn2受到199点伤害

 5_2_EH5SAkztn2被击倒了

0_2_It3MVeWRmJ使用净化, 1_3_wDqcIqVe9e受到77点伤害

 1_3_wDqcIqVe9e的聚气被打消了

 1_3_wDqcIqVe9e被击倒了

"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 50_000, true);
    assert_eq!(total_score, 26679, "fight_multi_8 score mismatch");
    assert!(guard < 50_000, "fight_multi_8 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_8 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_8 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

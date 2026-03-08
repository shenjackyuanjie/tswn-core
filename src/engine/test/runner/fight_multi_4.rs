use super::*;

#[test]
fn fight_multi_4() {
    const FIGHT_CASE: &str = r###"0_0_GRR4GBI1hh
0_1_37g4QDLOyz
0_2_STUo5Tp7Kz
0_3_jCDQuyd74D
0_4_Y57QRmlGyK

1_0_DlmTuosDRI
1_1_GY08TJk9iN
1_2_rqwR0WKSqd
1_3_NTBiwz0rx6
1_4_mi8rzAZRcL

2_0_5Zov2TRJBS
2_1_dHYrfDPpeG
2_2_a8g5otawV0
2_3_btx3Rd69W4
2_4_SoujNzpqey

3_0_1sJ3FFj0LV
3_1_lg4LD0Shci
3_2_Xj7rbRgqPH
3_3_bPpPTmdyOS
3_4_jvjOfZVnjj

4_0_z0y058yvlz
4_1_wmP15vkZ7h
4_2_K3WFm2YgsC
4_3_viqwtrElRg
4_4_0LbyKvd7Vg

5_0_5WyU5ATBQ3
5_1_9BiX2PJqQl
5_2_g2bYwlWCzg
5_3_nZYGYwt0qW
5_4_7VsGVumPwL

6_0_PdEG3Ieut1
6_1_jm8zg4tIVM
6_2_JJdtzMRYHZ
6_3_PpbaWuA99J
6_4_EsKaedaMdV

7_0_CKOui9yuSJ
7_1_v2sn5XlbXP
7_2_6MfgWGadXp
7_3_gGLztutq24
7_4_5UaSbrJAXo

8_0_dC27Pbwo8e
8_1_68mnmEINjD
8_2_ioo9NMDIMB
8_3_l6EBWql0yg
8_4_62Ti7Vil8U

9_0_RrWNdwNPPO
9_1_ZxgcWsa0Ro
9_2_FBahtpK9qf
9_3_b24Kw9LirO
9_4_5FdCB0xSIV


2_0_5Zov2TRJBS发起攻击, 9_3_b24Kw9LirO受到105点伤害

5_3_nZYGYwt0qW发起攻击, 1_3_NTBiwz0rx6受到119点伤害

8_3_l6EBWql0yg发起攻击, 2_0_5Zov2TRJBS受到66点伤害

5_0_5WyU5ATBQ3发起攻击, 3_0_1sJ3FFj0LV受到146点伤害

9_4_5FdCB0xSIV发起攻击, 0_1_37g4QDLOyz受到65点伤害

0_2_STUo5Tp7Kz使用瘟疫, 6_3_PpbaWuA99J体力减少59%

2_1_dHYrfDPpeG发起攻击, 5_0_5WyU5ATBQ3受到36点伤害

5_1_9BiX2PJqQl使用加速术, 5_1_9BiX2PJqQl进入疾走状态

7_1_v2sn5XlbXP发起攻击, 1_1_GY08TJk9iN受到55点伤害

1_0_DlmTuosDRI发起攻击, 5_0_5WyU5ATBQ3受到48点伤害

7_4_5UaSbrJAXo发起攻击, 1_0_DlmTuosDRI受到42点伤害

8_2_ioo9NMDIMB发起攻击, 3_0_1sJ3FFj0LV受到99点伤害

8_4_62Ti7Vil8U发起攻击, 7_0_CKOui9yuSJ受到47点伤害

9_1_ZxgcWsa0Ro发起攻击, 0_2_STUo5Tp7Kz受到84点伤害

4_0_z0y058yvlz发起攻击, 9_0_RrWNdwNPPO受到47点伤害

2_4_SoujNzpqey发起攻击, 5_4_7VsGVumPwL受到39点伤害

9_0_RrWNdwNPPO使用地裂术

 2_4_SoujNzpqey受到37点伤害

 6_1_jm8zg4tIVM回避了攻击

 7_1_v2sn5XlbXP回避了攻击

 7_0_CKOui9yuSJ受到26点伤害

 8_0_dC27Pbwo8e受到28点伤害

6_3_PpbaWuA99J发起攻击, 9_1_ZxgcWsa0Ro受到44点伤害

3_1_lg4LD0Shci发起攻击, 6_1_jm8zg4tIVM受到38点伤害

1_3_NTBiwz0rx6发动会心一击, 6_0_PdEG3Ieut1受到112点伤害

0_4_Y57QRmlGyK发起攻击, 4_3_viqwtrElRg受到90点伤害

3_3_bPpPTmdyOS发起攻击, 0_0_GRR4GBI1hh回避了攻击

8_0_dC27Pbwo8e使用分身, 出现一个新的8_0_dC27Pbwo8e

7_3_gGLztutq24发起攻击, 2_3_btx3Rd69W4受到130点伤害

1_1_GY08TJk9iN发起攻击, 3_2_Xj7rbRgqPH受到23点伤害

6_4_EsKaedaMdV发起攻击, 4_4_0LbyKvd7Vg受到77点伤害

4_3_viqwtrElRg发起攻击, 9_2_FBahtpK9qf受到73点伤害

6_0_PdEG3Ieut1发起攻击, 0_0_GRR4GBI1hh受到92点伤害

9_2_FBahtpK9qf使用幻术, 召唤出幻影

5_4_7VsGVumPwL投毒, 4_4_0LbyKvd7Vg受到125点伤害, 4_4_0LbyKvd7Vg中毒

9_3_b24Kw9LirO发起攻击, 8_1_68mnmEINjD受到71点伤害

4_2_K3WFm2YgsC发起攻击, 2_1_dHYrfDPpeG受到58点伤害

2_3_btx3Rd69W4发起攻击, 9_4_5FdCB0xSIV受到35点伤害

5_2_g2bYwlWCzg发起攻击, 8_1_68mnmEINjD受到104点伤害

7_2_6MfgWGadXp发起攻击, 0_1_37g4QDLOyz受到73点伤害

8_1_68mnmEINjD发起攻击, 1_2_rqwR0WKSqd受到61点伤害

3_2_Xj7rbRgqPH发起攻击, 1_3_NTBiwz0rx6受到30点伤害

5_3_nZYGYwt0qW发起攻击, 7_3_gGLztutq24受到26点伤害

3_0_1sJ3FFj0LV发起攻击, 0_2_STUo5Tp7Kz受到34点伤害

0_3_jCDQuyd74D使用减速术, 9_1_ZxgcWsa0Ro进入迟缓状态

7_0_CKOui9yuSJ发起攻击, 9_4_5FdCB0xSIV受到61点伤害

0_1_37g4QDLOyz发起攻击, 5_1_9BiX2PJqQl受到64点伤害

4_4_0LbyKvd7Vg发起攻击, 6_0_PdEG3Ieut1受到111点伤害, 6_0_PdEG3Ieut1发动隐匿

 4_4_0LbyKvd7Vg毒性发作, 4_4_0LbyKvd7Vg受到23点伤害

2_2_a8g5otawV0发起攻击, 7_3_gGLztutq24受到75点伤害

0_0_GRR4GBI1hh发起攻击, 1_2_rqwR0WKSqd受到59点伤害

1_0_DlmTuosDRI发起攻击, 2_4_SoujNzpqey受到74点伤害

5_0_5WyU5ATBQ3发起攻击, 9_4_5FdCB0xSIV受到119点伤害

1_4_mi8rzAZRcL发动铁壁, 1_4_mi8rzAZRcL防御力大幅上升

1_2_rqwR0WKSqd发起攻击, 9_1_ZxgcWsa0Ro受到48点伤害

2_0_5Zov2TRJBS使用减速术, 9_2_FBahtpK9qf进入迟缓状态

5_1_9BiX2PJqQl发起攻击, 0_1_37g4QDLOyz守护0_4_Y57QRmlGyK, 0_1_37g4QDLOyz受到37点伤害

8_0_dC27Pbwo8e发起攻击, 0_4_Y57QRmlGyK受到59点伤害

8_3_l6EBWql0yg发起攻击, 3_4_jvjOfZVnjj受到78点伤害

6_2_JJdtzMRYHZ发起攻击, 5_0_5WyU5ATBQ3受到33点伤害

9_3_b24Kw9LirO发起攻击, 4_1_wmP15vkZ7h受到88点伤害

3_3_bPpPTmdyOS发起攻击, 1_2_rqwR0WKSqd受到110点伤害

6_0_PdEG3Ieut1发起攻击, 幻影受到32点伤害

3_4_jvjOfZVnjj发起攻击, 5_1_9BiX2PJqQl受到59点伤害

9_4_5FdCB0xSIV发起攻击, 8_4_62Ti7Vil8U回避了攻击

7_2_6MfgWGadXp发起攻击, 2_0_5Zov2TRJBS受到60点伤害

9_0_RrWNdwNPPO发起攻击, 2_2_a8g5otawV0受到70点伤害

6_3_PpbaWuA99J发起攻击, 0_3_jCDQuyd74D受到75点伤害

6_1_jm8zg4tIVM发起攻击, 4_1_wmP15vkZ7h受到98点伤害

1_1_GY08TJk9iN发动会心一击, 7_0_CKOui9yuSJ受到80点伤害

7_1_v2sn5XlbXP使用血祭, 召唤出使魔

8_4_62Ti7Vil8U发起攻击, 7_4_5UaSbrJAXo受到59点伤害

4_0_z0y058yvlz发起吸血攻击, 6_3_PpbaWuA99J受到98点伤害, 4_0_z0y058yvlz回复体力0点

 6_3_PpbaWuA99J做出垂死抗争, 6_3_PpbaWuA99J所有属性上升

2_4_SoujNzpqey发起攻击, 7_0_CKOui9yuSJ受到68点伤害

6_4_EsKaedaMdV发起攻击, 7_2_6MfgWGadXp受到68点伤害

3_1_lg4LD0Shci发起攻击, 8_3_l6EBWql0yg受到34点伤害

 3_1_lg4LD0Shci连击, 9_2_FBahtpK9qf受到39点伤害, 9_2_FBahtpK9qf发动隐匿

4_3_viqwtrElRg发起攻击, 8_4_62Ti7Vil8U受到115点伤害

8_2_ioo9NMDIMB发起攻击, 3_1_lg4LD0Shci回避了攻击

5_2_g2bYwlWCzg发起攻击, 0_4_Y57QRmlGyK受到39点伤害

8_1_68mnmEINjD发起攻击, 9_2_FBahtpK9qf回避了攻击

3_2_Xj7rbRgqPH使用分身, 出现一个新的3_2_Xj7rbRgqPH

0_2_STUo5Tp7Kz发起攻击, 8_4_62Ti7Vil8U受到90点伤害

2_1_dHYrfDPpeG发起攻击, 3_0_1sJ3FFj0LV受到62点伤害

 3_0_1sJ3FFj0LV被击倒了

4_4_0LbyKvd7Vg开始聚气, 4_4_0LbyKvd7Vg攻击力上升

 4_4_0LbyKvd7Vg毒性发作, 4_4_0LbyKvd7Vg受到19点伤害

6_2_JJdtzMRYHZ发起攻击, 4_0_z0y058yvlz受到71点伤害

4_1_wmP15vkZ7h使用诅咒, 2_1_dHYrfDPpeG受到49点伤害, 2_1_dHYrfDPpeG被诅咒了

5_4_7VsGVumPwL开始聚气, 5_4_7VsGVumPwL攻击力上升

7_4_5UaSbrJAXo使用治愈魔法, 7_2_6MfgWGadXp回复体力68点

4_2_K3WFm2YgsC开始蓄力

7_0_CKOui9yuSJ发起攻击, 2_1_dHYrfDPpeG回避了攻击

2_3_btx3Rd69W4发起攻击, 5_3_nZYGYwt0qW回避了攻击

0_0_GRR4GBI1hh发起攻击, 5_0_5WyU5ATBQ3受到26点伤害

8_0_dC27Pbwo8e发起攻击, 5_1_9BiX2PJqQl受到71点伤害

7_3_gGLztutq24发起攻击, 8_2_ioo9NMDIMB受到126点伤害

5_1_9BiX2PJqQl发起攻击, 4_3_viqwtrElRg回避了攻击

 5_1_9BiX2PJqQl从疾走中解除

5_0_5WyU5ATBQ3发起攻击, 9_1_ZxgcWsa0Ro回避了攻击

2_2_a8g5otawV0发起攻击, 9_0_RrWNdwNPPO受到87点伤害

1_4_mi8rzAZRcL发起攻击, 9_0_RrWNdwNPPO受到61点伤害

0_1_37g4QDLOyz发起攻击, 6_2_JJdtzMRYHZ受到71点伤害

3_3_bPpPTmdyOS发起攻击, 1_1_GY08TJk9iN受到112点伤害

2_0_5Zov2TRJBS发起攻击, 7_1_v2sn5XlbXP受到77点伤害

使魔发起攻击, 9_1_ZxgcWsa0Ro受到41点伤害

5_3_nZYGYwt0qW发起攻击, 8_1_68mnmEINjD受到46点伤害

0_4_Y57QRmlGyK发起攻击, 2_3_btx3Rd69W4受到80点伤害

7_2_6MfgWGadXp发起攻击, 6_4_EsKaedaMdV回避了攻击

1_2_rqwR0WKSqd发起攻击, 0_1_37g4QDLOyz受到21点伤害

6_3_PpbaWuA99J发起攻击, 4_3_viqwtrElRg受到103点伤害

7_1_v2sn5XlbXP发起攻击, 3_4_jvjOfZVnjj受到62点伤害

3_4_jvjOfZVnjj使用分身, 出现一个新的3_4_jvjOfZVnjj

1_3_NTBiwz0rx6发起攻击, 7_4_5UaSbrJAXo受到25点伤害

9_3_b24Kw9LirO发起攻击, 3_2_Xj7rbRgqPH受到72点伤害

0_3_jCDQuyd74D发起攻击, 1_1_GY08TJk9iN受到25点伤害

8_0_dC27Pbwo8e发起攻击, 9_0_RrWNdwNPPO受到54点伤害

3_2_Xj7rbRgqPH发起攻击, 6_2_JJdtzMRYHZ受到38点伤害

3_1_lg4LD0Shci发起攻击, 2_3_btx3Rd69W4受到50点伤害

6_0_PdEG3Ieut1发起攻击, 0_1_37g4QDLOyz回避了攻击

4_0_z0y058yvlz发起吸血攻击, 8_2_ioo9NMDIMB受到109点伤害, 4_0_z0y058yvlz回复体力55点

 8_2_ioo9NMDIMB被击倒了

9_4_5FdCB0xSIV发起攻击, 7_2_6MfgWGadXp受到39点伤害

9_0_RrWNdwNPPO使用地裂术

 6_1_jm8zg4tIVM回避了攻击

 2_4_SoujNzpqey受到22点伤害

 0_4_Y57QRmlGyK受到53点伤害

 7_0_CKOui9yuSJ受到30点伤害

 3_4_jvjOfZVnjj回避了攻击

3_2_Xj7rbRgqPH发起攻击, 5_0_5WyU5ATBQ3受到25点伤害

2_1_dHYrfDPpeG发起攻击, 3_3_bPpPTmdyOS受到153点伤害

4_4_0LbyKvd7Vg发起攻击, 6_1_jm8zg4tIVM受到96点伤害

 4_4_0LbyKvd7Vg毒性发作, 4_4_0LbyKvd7Vg受到16点伤害

1_0_DlmTuosDRI发起攻击, 7_1_v2sn5XlbXP受到95点伤害

6_4_EsKaedaMdV发起攻击, 7_4_5UaSbrJAXo受到61点伤害

4_3_viqwtrElRg发起攻击, 0_4_Y57QRmlGyK受到54点伤害

9_2_FBahtpK9qf发起攻击, 0_2_STUo5Tp7Kz受到68点伤害

7_4_5UaSbrJAXo使用生命之轮, 6_4_EsKaedaMdV的体力值与7_4_5UaSbrJAXo互换

9_1_ZxgcWsa0Ro发起攻击, 1_0_DlmTuosDRI受到79点伤害

6_1_jm8zg4tIVM发起攻击, 3_1_lg4LD0Shci回避了攻击

1_1_GY08TJk9iN发起攻击, 8_4_62Ti7Vil8U受到44点伤害

3_4_jvjOfZVnjj使用雷击术

 5_4_7VsGVumPwL受到25点伤害

 5_4_7VsGVumPwL受到27点伤害

 5_4_7VsGVumPwL受到18点伤害

5_4_7VsGVumPwL发起攻击, 6_4_EsKaedaMdV受到113点伤害

 6_4_EsKaedaMdV发起反击, 5_4_7VsGVumPwL受到63点伤害

8_4_62Ti7Vil8U发起攻击, 9_4_5FdCB0xSIV受到10点伤害

4_2_K3WFm2YgsC发起攻击, 0_3_jCDQuyd74D受到231点伤害

 0_3_jCDQuyd74D被击倒了

8_0_dC27Pbwo8e发起攻击, 7_4_5UaSbrJAXo受到29点伤害

2_0_5Zov2TRJBS发起攻击, 5_2_g2bYwlWCzg受到74点伤害

5_1_9BiX2PJqQl发起攻击, 2_1_dHYrfDPpeG受到125点伤害

8_3_l6EBWql0yg使用血祭, 召唤出使魔

4_1_wmP15vkZ7h发动会心一击, 9_2_FBahtpK9qf受到167点伤害, 9_2_FBahtpK9qf发动隐匿

7_0_CKOui9yuSJ发起攻击, 9_2_FBahtpK9qf受到40点伤害

 9_2_FBahtpK9qf被击倒了

 幻影消失了

 7_0_CKOui9yuSJ吞噬了9_2_FBahtpK9qf, 7_0_CKOui9yuSJ属性上升

1_4_mi8rzAZRcL发起攻击, 3_1_lg4LD0Shci受到37点伤害

 1_4_mi8rzAZRcL从铁壁中解除

0_1_37g4QDLOyz发起攻击, 3_1_lg4LD0Shci受到28点伤害

3_3_bPpPTmdyOS使用狂暴术, 4_2_K3WFm2YgsC受到100点伤害, 4_2_K3WFm2YgsC进入狂暴状态

7_1_v2sn5XlbXP发起攻击, 5_0_5WyU5ATBQ3受到49点伤害

5_0_5WyU5ATBQ3发起攻击, 8_3_l6EBWql0yg受到57点伤害

0_4_Y57QRmlGyK发起攻击, 3_1_lg4LD0Shci受到88点伤害

2_2_a8g5otawV0发起攻击, 6_1_jm8zg4tIVM受到57点伤害

0_0_GRR4GBI1hh发起攻击, 9_0_RrWNdwNPPO受到62点伤害

 9_0_RrWNdwNPPO被击倒了

7_2_6MfgWGadXp发起攻击, 3_1_lg4LD0Shci受到68点伤害

0_2_STUo5Tp7Kz使用冰冻术, 9_4_5FdCB0xSIV受到33点伤害, 9_4_5FdCB0xSIV被冰冻了

2_1_dHYrfDPpeG发起攻击, 9_1_ZxgcWsa0Ro受到73点伤害

7_3_gGLztutq24发起攻击, 3_3_bPpPTmdyOS受到129点伤害

7_4_5UaSbrJAXo使用治愈魔法, 7_0_CKOui9yuSJ回复体力75点

9_3_b24Kw9LirO发动铁壁, 9_3_b24Kw9LirO防御力大幅上升

2_3_btx3Rd69W4发起攻击, 1_0_DlmTuosDRI受到101点伤害

8_1_68mnmEINjD发起攻击, 4_0_z0y058yvlz受到111点伤害

 4_0_z0y058yvlz发起反击, 8_1_68mnmEINjD受到23点伤害

2_4_SoujNzpqey使用地裂术

 1_2_rqwR0WKSqd受到22点伤害

 7_0_CKOui9yuSJ受到20点伤害

 7_3_gGLztutq24受到12点伤害

 7_4_5UaSbrJAXo受到23点伤害

 0_1_37g4QDLOyz守护0_2_STUo5Tp7Kz, 0_1_37g4QDLOyz受到12点伤害

1_2_rqwR0WKSqd发起攻击, 6_2_JJdtzMRYHZ回避了攻击

6_3_PpbaWuA99J使用分身, 出现一个新的6_3_PpbaWuA99J

使魔使用火球术, 4_0_z0y058yvlz受到59点伤害

6_4_EsKaedaMdV发起攻击, 4_2_K3WFm2YgsC守护4_1_wmP15vkZ7h, 4_2_K3WFm2YgsC受到19点伤害

6_0_PdEG3Ieut1发起攻击, 7_3_gGLztutq24受到41点伤害

3_4_jvjOfZVnjj发起攻击, 8_0_dC27Pbwo8e受到70点伤害

5_2_g2bYwlWCzg发起攻击, 2_1_dHYrfDPpeG受到90点伤害

 2_1_dHYrfDPpeG被击倒了

8_0_dC27Pbwo8e发起攻击, 7_0_CKOui9yuSJ回避了攻击

3_2_Xj7rbRgqPH发起攻击, 0_2_STUo5Tp7Kz受到91点伤害

6_2_JJdtzMRYHZ发起攻击, 5_2_g2bYwlWCzg受到83点伤害

7_0_CKOui9yuSJ使用瘟疫, 5_1_9BiX2PJqQl体力减少43%

3_2_Xj7rbRgqPH发起攻击, 0_0_GRR4GBI1hh受到28点伤害

1_1_GY08TJk9iN发起攻击, 7_1_v2sn5XlbXP回避了攻击

5_3_nZYGYwt0qW发起攻击, 2_0_5Zov2TRJBS受到85点伤害

1_0_DlmTuosDRI发起攻击, 0_0_GRR4GBI1hh受到91点伤害

8_3_l6EBWql0yg投毒, 6_1_jm8zg4tIVM回避了攻击

5_0_5WyU5ATBQ3发起攻击, 8_3_l6EBWql0yg受到85点伤害

8_4_62Ti7Vil8U发起攻击, 5_2_g2bYwlWCzg受到17点伤害

1_4_mi8rzAZRcL发起攻击, 2_2_a8g5otawV0受到110点伤害

6_1_jm8zg4tIVM使用治愈魔法, 6_0_PdEG3Ieut1回复体力174点

6_3_PpbaWuA99J发起攻击, 7_2_6MfgWGadXp回避了攻击

5_4_7VsGVumPwL使用加速术, 5_3_nZYGYwt0qW进入疾走状态

1_3_NTBiwz0rx6发起攻击, 9_3_b24Kw9LirO受到1点伤害

4_2_K3WFm2YgsC发起攻击, 3_2_Xj7rbRgqPH受到87点伤害

 3_2_Xj7rbRgqPH被击倒了

4_0_z0y058yvlz发起攻击, 6_3_PpbaWuA99J受到31点伤害

 6_3_PpbaWuA99J被击倒了

 4_0_z0y058yvlz连击, 1_0_DlmTuosDRI受到57点伤害

 1_0_DlmTuosDRI被击倒了

 4_0_z0y058yvlz连击, 2_0_5Zov2TRJBS受到54点伤害

5_1_9BiX2PJqQl发起攻击, 6_2_JJdtzMRYHZ受到32点伤害

3_4_jvjOfZVnjj发动铁壁, 3_4_jvjOfZVnjj防御力大幅上升

6_4_EsKaedaMdV发起攻击, 5_4_7VsGVumPwL受到61点伤害

4_3_viqwtrElRg发起攻击, 9_4_5FdCB0xSIV受到61点伤害

2_4_SoujNzpqey发起攻击, 5_3_nZYGYwt0qW受到54点伤害

8_0_dC27Pbwo8e发起攻击, 6_0_PdEG3Ieut1受到86点伤害

3_4_jvjOfZVnjj使用雷击术

 1_4_mi8rzAZRcL受到24点伤害

 1_4_mi8rzAZRcL受到21点伤害

 1_4_mi8rzAZRcL受到16点伤害

 1_4_mi8rzAZRcL受到32点伤害

9_3_b24Kw9LirO发起攻击, 1_2_rqwR0WKSqd受到82点伤害

4_2_K3WFm2YgsC开始蓄力

2_2_a8g5otawV0发起攻击, 4_0_z0y058yvlz受到57点伤害

9_4_5FdCB0xSIV从冰冻中解除

3_3_bPpPTmdyOS使用狂暴术, 7_0_CKOui9yuSJ受到92点伤害, 7_0_CKOui9yuSJ进入狂暴状态, 7_0_CKOui9yuSJ发动隐匿

7_3_gGLztutq24发起攻击, 8_0_dC27Pbwo8e受到80点伤害

1_1_GY08TJk9iN发起攻击, 使魔受到41点伤害, 7_1_v2sn5XlbXP受到20点伤害

使魔使用自爆, 0_4_Y57QRmlGyK受到95点伤害

 0_4_Y57QRmlGyK被击倒了

 使魔消失了

5_3_nZYGYwt0qW发起攻击, 2_3_btx3Rd69W4受到53点伤害

 2_3_btx3Rd69W4被击倒了

7_1_v2sn5XlbXP发起攻击, 8_1_68mnmEINjD受到48点伤害

4_1_wmP15vkZ7h发起攻击, 5_2_g2bYwlWCzg受到50点伤害

 5_2_g2bYwlWCzg做出垂死抗争, 5_2_g2bYwlWCzg所有属性上升

9_1_ZxgcWsa0Ro发动铁壁, 9_1_ZxgcWsa0Ro防御力大幅上升

 9_1_ZxgcWsa0Ro从迟缓中解除

9_4_5FdCB0xSIV发起攻击, 6_2_JJdtzMRYHZ受到83点伤害

0_0_GRR4GBI1hh发起攻击, 4_1_wmP15vkZ7h受到43点伤害

5_2_g2bYwlWCzg发起攻击, 4_3_viqwtrElRg受到54点伤害

2_0_5Zov2TRJBS发起攻击, 6_3_PpbaWuA99J受到59点伤害

 6_3_PpbaWuA99J被击倒了

使魔发起攻击, 5_0_5WyU5ATBQ3受到23点伤害

5_3_nZYGYwt0qW发起攻击, 3_4_jvjOfZVnjj受到49点伤害

8_3_l6EBWql0yg发起攻击, 7_2_6MfgWGadXp受到57点伤害

6_0_PdEG3Ieut1使用幻术, 召唤出幻影

7_4_5UaSbrJAXo使用分身, 出现一个新的7_4_5UaSbrJAXo

8_1_68mnmEINjD发起攻击, 1_3_NTBiwz0rx6受到58点伤害

1_2_rqwR0WKSqd发起攻击, 3_1_lg4LD0Shci受到21点伤害

8_0_dC27Pbwo8e发起攻击, 幻影受到32点伤害

3_1_lg4LD0Shci发起攻击, 1_1_GY08TJk9iN受到77点伤害

4_3_viqwtrElRg发起攻击, 6_2_JJdtzMRYHZ受到52点伤害

7_0_CKOui9yuSJ使用地裂术

 幻影受到45点伤害

 9_1_ZxgcWsa0Ro受到1点伤害

 5_1_9BiX2PJqQl受到7点伤害

 2_2_a8g5otawV0受到45点伤害

1_4_mi8rzAZRcL发起攻击, 5_3_nZYGYwt0qW受到36点伤害

0_1_37g4QDLOyz发起攻击, 9_3_b24Kw9LirO受到1点伤害

5_0_5WyU5ATBQ3发起攻击, 8_1_68mnmEINjD受到46点伤害

 8_1_68mnmEINjD被击倒了

8_4_62Ti7Vil8U发起攻击, 7_2_6MfgWGadXp受到30点伤害

9_3_b24Kw9LirO使用加速术, 9_3_b24Kw9LirO进入疾走状态

 9_3_b24Kw9LirO从铁壁中解除

5_2_g2bYwlWCzg发起攻击, 7_3_gGLztutq24受到80点伤害

7_2_6MfgWGadXp发起攻击, 3_4_jvjOfZVnjj受到1点伤害

3_2_Xj7rbRgqPH发起攻击, 5_2_g2bYwlWCzg受到62点伤害

 5_2_g2bYwlWCzg被击倒了

0_2_STUo5Tp7Kz使用冰冻术, 6_2_JJdtzMRYHZ受到34点伤害

 6_2_JJdtzMRYHZ被击倒了

3_4_jvjOfZVnjj使用雷击术

 7_2_6MfgWGadXp受到35点伤害

 7_2_6MfgWGadXp受到29点伤害

 7_2_6MfgWGadXp受到21点伤害

 7_2_6MfgWGadXp受到13点伤害

 7_2_6MfgWGadXp受到9点伤害

5_3_nZYGYwt0qW使用瘟疫, 3_4_jvjOfZVnjj体力减少62%

 5_3_nZYGYwt0qW从疾走中解除

4_0_z0y058yvlz发起攻击, 9_1_ZxgcWsa0Ro受到1点伤害

2_2_a8g5otawV0使用加速术, 2_2_a8g5otawV0进入疾走状态

2_4_SoujNzpqey发起攻击, 8_3_l6EBWql0yg受到49点伤害

4_4_0LbyKvd7Vg发起攻击, 1_2_rqwR0WKSqd受到65点伤害

 1_2_rqwR0WKSqd被击倒了

 4_4_0LbyKvd7Vg毒性发作, 4_4_0LbyKvd7Vg受到13点伤害

 4_4_0LbyKvd7Vg从中毒中解除

7_4_5UaSbrJAXo发起攻击, 4_4_0LbyKvd7Vg受到64点伤害

 4_4_0LbyKvd7Vg被击倒了

6_4_EsKaedaMdV发起攻击, 8_0_dC27Pbwo8e受到97点伤害

 8_0_dC27Pbwo8e被击倒了

5_4_7VsGVumPwL发起攻击, 3_2_Xj7rbRgqPH受到110点伤害

4_2_K3WFm2YgsC发起攻击, 0_2_STUo5Tp7Kz回避了攻击

3_3_bPpPTmdyOS发起攻击, 1_3_NTBiwz0rx6受到116点伤害

 1_3_NTBiwz0rx6被击倒了

1_1_GY08TJk9iN发起攻击, 使魔受到49点伤害, 8_3_l6EBWql0yg受到24点伤害

5_1_9BiX2PJqQl发起攻击, 幻影受到110点伤害

 幻影消失了

使魔发起攻击, 7_1_v2sn5XlbXP受到24点伤害

8_3_l6EBWql0yg发起攻击, 0_0_GRR4GBI1hh受到54点伤害

 0_0_GRR4GBI1hh做出垂死抗争, 0_0_GRR4GBI1hh所有属性上升

6_0_PdEG3Ieut1开始蓄力

7_4_5UaSbrJAXo发起攻击, 6_1_jm8zg4tIVM受到18点伤害

2_2_a8g5otawV0发起攻击, 5_4_7VsGVumPwL受到61点伤害

0_0_GRR4GBI1hh使用雷击术

 3_3_bPpPTmdyOS受到22点伤害

 3_3_bPpPTmdyOS被击倒了

6_1_jm8zg4tIVM使用治愈魔法, 6_4_EsKaedaMdV回复体力89点

1_4_mi8rzAZRcL潜行到9_3_b24Kw9LirO身后

7_3_gGLztutq24发起攻击, 8_4_62Ti7Vil8U受到79点伤害

 8_4_62Ti7Vil8U被击倒了

2_0_5Zov2TRJBS发起攻击, 4_2_K3WFm2YgsC受到114点伤害

 4_2_K3WFm2YgsC被击倒了

3_4_jvjOfZVnjj发起攻击, 5_3_nZYGYwt0qW受到56点伤害

9_4_5FdCB0xSIV使用瘟疫, 4_1_wmP15vkZ7h体力减少60%

3_2_Xj7rbRgqPH发动会心一击, 7_4_5UaSbrJAXo受到93点伤害

3_1_lg4LD0Shci发动会心一击, 0_0_GRR4GBI1hh回避了攻击

4_3_viqwtrElRg发起攻击, 7_4_5UaSbrJAXo受到15点伤害

 7_4_5UaSbrJAXo被击倒了

9_3_b24Kw9LirO发起攻击, 7_1_v2sn5XlbXP受到122点伤害

 7_1_v2sn5XlbXP被击倒了

8_0_dC27Pbwo8e发起攻击, 7_3_gGLztutq24受到55点伤害

6_4_EsKaedaMdV发起攻击, 3_4_jvjOfZVnjj受到1点伤害

6_0_PdEG3Ieut1发动会心一击, 3_4_jvjOfZVnjj受到278点伤害

 3_4_jvjOfZVnjj被击倒了

5_4_7VsGVumPwL发起攻击, 1_4_mi8rzAZRcL受到99点伤害

 1_4_mi8rzAZRcL的潜行被识破

5_3_nZYGYwt0qW发起攻击, 1_4_mi8rzAZRcL受到59点伤害

4_1_wmP15vkZ7h发起攻击, 6_0_PdEG3Ieut1受到46点伤害, 6_0_PdEG3Ieut1发动隐匿

5_0_5WyU5ATBQ3发起攻击, 4_3_viqwtrElRg受到120点伤害

 4_3_viqwtrElRg被击倒了

4_0_z0y058yvlz发起攻击, 9_1_ZxgcWsa0Ro受到1点伤害

0_0_GRR4GBI1hh发起攻击, 9_4_5FdCB0xSIV受到108点伤害

 9_4_5FdCB0xSIV被击倒了

9_1_ZxgcWsa0Ro发起攻击, 6_1_jm8zg4tIVM受到33点伤害

2_2_a8g5otawV0发起吸血攻击, 使魔受到114点伤害, 2_2_a8g5otawV0回复体力57点, 8_3_l6EBWql0yg受到57点伤害

 8_3_l6EBWql0yg被击倒了

 使魔消失了

 2_2_a8g5otawV0从疾走中解除

7_0_CKOui9yuSJ发起攻击, 5_3_nZYGYwt0qW受到57点伤害

7_2_6MfgWGadXp发起攻击, 0_1_37g4QDLOyz受到70点伤害

0_1_37g4QDLOyz发起攻击, 1_1_GY08TJk9iN受到40点伤害

0_2_STUo5Tp7Kz使用诅咒, 5_3_nZYGYwt0qW受到43点伤害, 5_3_nZYGYwt0qW被诅咒了

1_1_GY08TJk9iN使用地裂术

 3_2_Xj7rbRgqPH受到40点伤害

 3_2_Xj7rbRgqPH被击倒了

 5_4_7VsGVumPwL受到17点伤害

 7_4_5UaSbrJAXo受到63点伤害

 4_1_wmP15vkZ7h受到52点伤害

 4_1_wmP15vkZ7h被击倒了

3_4_jvjOfZVnjj发起攻击, 2_2_a8g5otawV0受到78点伤害

 3_4_jvjOfZVnjj从铁壁中解除

6_0_PdEG3Ieut1发起攻击, 0_1_37g4QDLOyz回避了攻击

9_3_b24Kw9LirO使用加速术, 9_1_ZxgcWsa0Ro进入疾走状态

 9_3_b24Kw9LirO从疾走中解除

1_4_mi8rzAZRcL发起攻击, 5_3_nZYGYwt0qW受到59点伤害

 5_3_nZYGYwt0qW被击倒了

2_0_5Zov2TRJBS发起攻击, 7_4_5UaSbrJAXo受到78点伤害

 7_4_5UaSbrJAXo被击倒了

9_1_ZxgcWsa0Ro潜行到7_2_6MfgWGadXp身后

 9_1_ZxgcWsa0Ro从铁壁中解除

6_1_jm8zg4tIVM使用加速术, 6_4_EsKaedaMdV进入疾走状态

5_1_9BiX2PJqQl发起攻击, 0_0_GRR4GBI1hh回避了攻击

2_2_a8g5otawV0使用加速术, 2_4_SoujNzpqey进入疾走状态

2_4_SoujNzpqey使用冰冻术, 6_0_PdEG3Ieut1受到20点伤害, 6_0_PdEG3Ieut1被冰冻了

8_0_dC27Pbwo8e发起攻击, 9_3_b24Kw9LirO受到30点伤害

7_3_gGLztutq24使用减速术, 9_3_b24Kw9LirO回避了攻击

6_4_EsKaedaMdV发起攻击, 9_1_ZxgcWsa0Ro使用伤害反弹, 6_4_EsKaedaMdV受到42点伤害

5_0_5WyU5ATBQ3使用苏生术, 5_2_g2bYwlWCzg复活了, 5_2_g2bYwlWCzg回复体力78点

5_4_7VsGVumPwL发起攻击, 3_4_jvjOfZVnjj受到161点伤害

 3_4_jvjOfZVnjj被击倒了

 5_4_7VsGVumPwL吞噬了3_4_jvjOfZVnjj, 5_4_7VsGVumPwL属性上升

5_4_7VsGVumPwL使用加速术, 5_2_g2bYwlWCzg进入疾走状态

9_3_b24Kw9LirO发起攻击, 6_4_EsKaedaMdV受到32点伤害

4_0_z0y058yvlz发起攻击, 5_4_7VsGVumPwL受到28点伤害

 5_4_7VsGVumPwL被击倒了

2_4_SoujNzpqey使用地裂术

 4_0_z0y058yvlz受到18点伤害

 3_1_lg4LD0Shci受到68点伤害

 3_1_lg4LD0Shci被击倒了

 7_2_6MfgWGadXp受到26点伤害

 1_4_mi8rzAZRcL受到23点伤害

 4_0_z0y058yvlz发起反击, 2_4_SoujNzpqey使用伤害反弹, 4_0_z0y058yvlz受到14点伤害

 4_0_z0y058yvlz做出垂死抗争, 4_0_z0y058yvlz所有属性上升

9_1_ZxgcWsa0Ro发动背刺, 7_2_6MfgWGadXp受到288点伤害

 7_2_6MfgWGadXp被击倒了

9_1_ZxgcWsa0Ro发起攻击, 2_2_a8g5otawV0受到48点伤害

 2_2_a8g5otawV0被击倒了

 9_1_ZxgcWsa0Ro从疾走中解除

0_0_GRR4GBI1hh使用雷击术

 1_4_mi8rzAZRcL受到15点伤害

 1_4_mi8rzAZRcL受到14点伤害

 1_4_mi8rzAZRcL被击倒了

5_2_g2bYwlWCzg发起攻击, 9_3_b24Kw9LirO受到143点伤害

 9_3_b24Kw9LirO被击倒了

7_0_CKOui9yuSJ发起攻击, 6_1_jm8zg4tIVM回避了攻击

0_1_37g4QDLOyz发起攻击, 8_0_dC27Pbwo8e使用伤害反弹, 0_1_37g4QDLOyz受到38点伤害

 0_1_37g4QDLOyz做出垂死抗争, 0_1_37g4QDLOyz所有属性上升

1_1_GY08TJk9iN发起攻击, 0_0_GRR4GBI1hh回避了攻击

6_1_jm8zg4tIVM发起攻击, 0_0_GRR4GBI1hh受到23点伤害

5_2_g2bYwlWCzg发起攻击, 6_0_PdEG3Ieut1受到70点伤害

 6_0_PdEG3Ieut1被击倒了

4_0_z0y058yvlz发起攻击, 6_4_EsKaedaMdV受到109点伤害

 6_4_EsKaedaMdV被击倒了

2_4_SoujNzpqey发起攻击, 5_1_9BiX2PJqQl受到45点伤害

 2_4_SoujNzpqey从疾走中解除

2_0_5Zov2TRJBS发起攻击, 0_0_GRR4GBI1hh回避了攻击

5_0_5WyU5ATBQ3发起攻击, 2_4_SoujNzpqey受到52点伤害

5_1_9BiX2PJqQl发起吸血攻击, 6_1_jm8zg4tIVM受到102点伤害, 5_1_9BiX2PJqQl回复体力51点

 6_1_jm8zg4tIVM被击倒了

0_1_37g4QDLOyz发起攻击, 5_2_g2bYwlWCzg受到56点伤害

8_0_dC27Pbwo8e使用幻术, 召唤出幻影

7_3_gGLztutq24发起攻击, 0_0_GRR4GBI1hh受到17点伤害

5_2_g2bYwlWCzg发起攻击, 幻影受到62点伤害

 5_2_g2bYwlWCzg从疾走中解除

9_1_ZxgcWsa0Ro使用火球术, 0_0_GRR4GBI1hh受到11点伤害

 0_0_GRR4GBI1hh发起反击, 9_1_ZxgcWsa0Ro受到45点伤害

0_0_GRR4GBI1hh发起攻击, 2_0_5Zov2TRJBS受到52点伤害

 2_0_5Zov2TRJBS被击倒了

0_2_STUo5Tp7Kz发起攻击, 9_1_ZxgcWsa0Ro回避了攻击

1_1_GY08TJk9iN使用地裂术

 2_4_SoujNzpqey受到47点伤害

 幻影受到17点伤害

 8_0_dC27Pbwo8e受到47点伤害

 8_0_dC27Pbwo8e被击倒了

 幻影消失了

 7_0_CKOui9yuSJ受到22点伤害

 7_0_CKOui9yuSJ被击倒了, 7_0_CKOui9yuSJ使用护身符抵挡了一次死亡, 7_0_CKOui9yuSJ回复体力5点

 9_1_ZxgcWsa0Ro使用伤害反弹, 1_1_GY08TJk9iN受到26点伤害

 1_1_GY08TJk9iN被击倒了

4_0_z0y058yvlz发起攻击, 5_1_9BiX2PJqQl受到58点伤害

5_0_5WyU5ATBQ3使用苏生术, 5_3_nZYGYwt0qW复活了, 5_3_nZYGYwt0qW回复体力81点

7_0_CKOui9yuSJ发起攻击, 0_0_GRR4GBI1hh回避了攻击

0_1_37g4QDLOyz发起攻击, 5_2_g2bYwlWCzg受到45点伤害

 5_2_g2bYwlWCzg被击倒了

9_1_ZxgcWsa0Ro发起攻击, 7_3_gGLztutq24受到29点伤害

 7_3_gGLztutq24被击倒了

5_1_9BiX2PJqQl发起攻击, 4_0_z0y058yvlz受到67点伤害

 4_0_z0y058yvlz被击倒了

0_2_STUo5Tp7Kz发起攻击, 5_3_nZYGYwt0qW受到44点伤害

0_0_GRR4GBI1hh发起攻击, 5_3_nZYGYwt0qW受到60点伤害

 5_3_nZYGYwt0qW被击倒了

9_1_ZxgcWsa0Ro发动铁壁, 9_1_ZxgcWsa0Ro防御力大幅上升

2_4_SoujNzpqey使用地裂术

 7_0_CKOui9yuSJ回避了攻击

 9_1_ZxgcWsa0Ro受到1点伤害

 0_0_GRR4GBI1hh受到36点伤害

 0_0_GRR4GBI1hh被击倒了

 0_2_STUo5Tp7Kz受到14点伤害

7_0_CKOui9yuSJ使用冰冻术, 5_0_5WyU5ATBQ3受到44点伤害, 5_0_5WyU5ATBQ3被冰冻了

0_1_37g4QDLOyz发起攻击, 2_4_SoujNzpqey回避了攻击

5_1_9BiX2PJqQl发起攻击, 2_4_SoujNzpqey受到103点伤害

 2_4_SoujNzpqey被击倒了

0_2_STUo5Tp7Kz使用冰冻术, 9_1_ZxgcWsa0Ro受到1点伤害, 9_1_ZxgcWsa0Ro被冰冻了

5_1_9BiX2PJqQl发起攻击, 9_1_ZxgcWsa0Ro受到1点伤害

7_0_CKOui9yuSJ发起攻击, 5_1_9BiX2PJqQl受到63点伤害

 5_1_9BiX2PJqQl被击倒了

 7_0_CKOui9yuSJ吞噬了5_1_9BiX2PJqQl, 7_0_CKOui9yuSJ属性上升

5_0_5WyU5ATBQ3从冰冻中解除

5_0_5WyU5ATBQ3发起攻击, 9_1_ZxgcWsa0Ro受到1点伤害

0_1_37g4QDLOyz使用诅咒, 9_1_ZxgcWsa0Ro受到1点伤害, 9_1_ZxgcWsa0Ro被诅咒了

9_1_ZxgcWsa0Ro从冰冻中解除

9_1_ZxgcWsa0Ro发起攻击, 0_2_STUo5Tp7Kz受到21点伤害

 0_2_STUo5Tp7Kz被击倒了

7_0_CKOui9yuSJ发起攻击, 9_1_ZxgcWsa0Ro受到1点伤害

0_1_37g4QDLOyz发起攻击, 7_0_CKOui9yuSJ受到62点伤害

 7_0_CKOui9yuSJ被击倒了

5_0_5WyU5ATBQ3发起攻击, 诅咒使伤害加倍, 9_1_ZxgcWsa0Ro受到2点伤害

9_1_ZxgcWsa0Ro发起攻击, 5_0_5WyU5ATBQ3受到46点伤害

 5_0_5WyU5ATBQ3被击倒了

 9_1_ZxgcWsa0Ro从铁壁中解除

0_1_37g4QDLOyz发起攻击, 诅咒使伤害加倍, 9_1_ZxgcWsa0Ro受到118点伤害

 9_1_ZxgcWsa0Ro被击倒了

"###;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines(&mut runner, 50_000, true);
    assert_eq!(total_score, 25799, "fight_multi_4 score mismatch");
    assert!(guard < 50_000, "fight_multi_4 combat did not finish in expected rounds");
    assert_trace_with_context("fight_multi_4", &actual_lines, &expected_lines);
}

use super::*;

pub fn large_38<E: crate::EngineAdapter>() {
    const CASE: &str = r####"0_4RnhQrYzbq
1_MuW07xoMus
2_TLcfyIEn9Y
3_9mK8kLaw2V
4_FHUSuY76qq
5_BYAZ8S59GN
6_p7eVP0gAnh
7_qckCHq8x8z
8_Lr85xny6it
9_tfwtw3QPmO


1_MuW07xoMus发起攻击, 9_tfwtw3QPmO受到79点伤害

5_BYAZ8S59GN发起攻击, 4_FHUSuY76qq受到118点伤害

6_p7eVP0gAnh发起攻击, 7_qckCHq8x8z受到44点伤害

2_TLcfyIEn9Y使用加速术, 2_TLcfyIEn9Y进入疾走状态

0_4RnhQrYzbq使用火球术, 2_TLcfyIEn9Y受到129点伤害

7_qckCHq8x8z发起攻击, 0_4RnhQrYzbq受到137点伤害

8_Lr85xny6it发起攻击, 6_p7eVP0gAnh受到46点伤害

3_9mK8kLaw2V发起攻击, 6_p7eVP0gAnh受到133点伤害

1_MuW07xoMus发起攻击, 4_FHUSuY76qq受到78点伤害

2_TLcfyIEn9Y发起攻击, 0_4RnhQrYzbq受到75点伤害

9_tfwtw3QPmO发起攻击, 0_4RnhQrYzbq受到112点伤害

 0_4RnhQrYzbq被击倒了

4_FHUSuY76qq发起攻击, 3_9mK8kLaw2V受到40点伤害

7_qckCHq8x8z发起攻击, 1_MuW07xoMus受到54点伤害

8_Lr85xny6it发起攻击, 5_BYAZ8S59GN受到106点伤害

5_BYAZ8S59GN发起攻击, 4_FHUSuY76qq受到39点伤害

6_p7eVP0gAnh发起攻击, 9_tfwtw3QPmO受到41点伤害

2_TLcfyIEn9Y发起攻击, 3_9mK8kLaw2V受到72点伤害

 2_TLcfyIEn9Y从疾走中解除

3_9mK8kLaw2V发起攻击, 5_BYAZ8S59GN受到158点伤害

1_MuW07xoMus使用治愈魔法, 1_MuW07xoMus回复体力54点

7_qckCHq8x8z发起攻击, 4_FHUSuY76qq回避了攻击

8_Lr85xny6it发起攻击, 1_MuW07xoMus受到88点伤害

9_tfwtw3QPmO发起攻击, 8_Lr85xny6it受到45点伤害

6_p7eVP0gAnh发起攻击, 7_qckCHq8x8z受到74点伤害

5_BYAZ8S59GN发起攻击, 1_MuW07xoMus受到72点伤害

4_FHUSuY76qq发起攻击, 9_tfwtw3QPmO受到70点伤害

2_TLcfyIEn9Y发起攻击, 8_Lr85xny6it受到67点伤害

7_qckCHq8x8z发起攻击, 2_TLcfyIEn9Y回避了攻击

1_MuW07xoMus发起攻击, 3_9mK8kLaw2V受到74点伤害

3_9mK8kLaw2V发起攻击, 7_qckCHq8x8z受到57点伤害

4_FHUSuY76qq使用减速术, 7_qckCHq8x8z回避了攻击

8_Lr85xny6it发起攻击, 1_MuW07xoMus受到29点伤害

5_BYAZ8S59GN发起攻击, 7_qckCHq8x8z受到95点伤害

7_qckCHq8x8z使用净化, 9_tfwtw3QPmO受到34点伤害

6_p7eVP0gAnh发起攻击, 2_TLcfyIEn9Y受到59点伤害

4_FHUSuY76qq发起攻击, 1_MuW07xoMus受到95点伤害

9_tfwtw3QPmO发起攻击, 5_BYAZ8S59GN受到89点伤害

 5_BYAZ8S59GN被击倒了

1_MuW07xoMus发起攻击, 4_FHUSuY76qq受到57点伤害

2_TLcfyIEn9Y发起攻击, 6_p7eVP0gAnh受到152点伤害

 6_p7eVP0gAnh被击倒了

3_9mK8kLaw2V发起吸血攻击, 2_TLcfyIEn9Y受到46点伤害, 3_9mK8kLaw2V回复体力23点

7_qckCHq8x8z发起攻击, 8_Lr85xny6it受到58点伤害

8_Lr85xny6it发起攻击, 2_TLcfyIEn9Y受到41点伤害

 2_TLcfyIEn9Y被击倒了

4_FHUSuY76qq发起攻击, 3_9mK8kLaw2V受到53点伤害

1_MuW07xoMus发起攻击, 3_9mK8kLaw2V受到66点伤害

 3_9mK8kLaw2V被击倒了

9_tfwtw3QPmO发起攻击, 8_Lr85xny6it受到64点伤害

 8_Lr85xny6it被击倒了

1_MuW07xoMus发起攻击, 9_tfwtw3QPmO受到152点伤害

 9_tfwtw3QPmO被击倒了

7_qckCHq8x8z发起攻击, 4_FHUSuY76qq受到91点伤害

 4_FHUSuY76qq被击倒了

7_qckCHq8x8z发起攻击, 1_MuW07xoMus受到102点伤害

 1_MuW07xoMus被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-38 must contain a blank separator between input and trace",
        "sampled case-38 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 3961, "large_38 score mismatch");
    assert!(guard < 20_000, "sampled case-38 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-38", &actual_lines, &expected_lines);
}

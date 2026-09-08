use super::*;

pub fn large_37<E: crate::EngineAdapter>() {
    const CASE: &str = r####"0_n0foAiLMcc
1_Bvpbn3b55R
2_pg6O17IbDF
3_zA5mtxMcPi
4_P0revuk8ms
5_nr70Kikkf8
6_LSUD2CsfGo
7_rRZN5tNMjb
8_7J291lHaC6
9_tZOkO2s8JU


0_n0foAiLMcc发起攻击, 1_Bvpbn3b55R受到76点伤害

3_zA5mtxMcPi投毒, 4_P0revuk8ms回避了攻击

5_nr70Kikkf8发起攻击, 8_7J291lHaC6受到88点伤害

8_7J291lHaC6发起攻击, 4_P0revuk8ms受到119点伤害

2_pg6O17IbDF发起攻击, 6_LSUD2CsfGo受到66点伤害

9_tZOkO2s8JU发起攻击, 5_nr70Kikkf8受到77点伤害

4_P0revuk8ms发起攻击, 3_zA5mtxMcPi受到90点伤害

7_rRZN5tNMjb发起攻击, 4_P0revuk8ms回避了攻击

6_LSUD2CsfGo发动铁壁, 6_LSUD2CsfGo防御力大幅上升

3_zA5mtxMcPi发起攻击, 8_7J291lHaC6受到36点伤害

5_nr70Kikkf8发起攻击, 0_n0foAiLMcc受到79点伤害

8_7J291lHaC6发起攻击, 6_LSUD2CsfGo受到1点伤害

1_Bvpbn3b55R发起攻击, 7_rRZN5tNMjb受到36点伤害

2_pg6O17IbDF发起攻击, 4_P0revuk8ms受到107点伤害

9_tZOkO2s8JU发起攻击, 6_LSUD2CsfGo防御, 6_LSUD2CsfGo受到0点伤害

4_P0revuk8ms使用生命之轮, 2_pg6O17IbDF的体力值与4_P0revuk8ms互换

7_rRZN5tNMjb发起攻击, 1_Bvpbn3b55R受到103点伤害

0_n0foAiLMcc发起攻击, 2_pg6O17IbDF受到46点伤害

6_LSUD2CsfGo发起攻击, 9_tZOkO2s8JU受到59点伤害

3_zA5mtxMcPi发起攻击, 4_P0revuk8ms受到67点伤害

5_nr70Kikkf8发起攻击, 4_P0revuk8ms回避了攻击

9_tZOkO2s8JU发起攻击, 4_P0revuk8ms受到53点伤害

7_rRZN5tNMjb投毒, 0_n0foAiLMcc回避了攻击

2_pg6O17IbDF发起攻击, 7_rRZN5tNMjb受到49点伤害

8_7J291lHaC6发起攻击, 4_P0revuk8ms受到38点伤害

 8_7J291lHaC6连击, 6_LSUD2CsfGo受到1点伤害

 8_7J291lHaC6连击, 6_LSUD2CsfGo受到1点伤害

1_Bvpbn3b55R使用雷击术

 0_n0foAiLMcc受到22点伤害

 0_n0foAiLMcc受到15点伤害

 0_n0foAiLMcc受到7点伤害

4_P0revuk8ms发起攻击, 6_LSUD2CsfGo受到1点伤害

6_LSUD2CsfGo开始聚气, 6_LSUD2CsfGo攻击力上升

 6_LSUD2CsfGo从铁壁中解除

9_tZOkO2s8JU使用魅惑, 5_nr70Kikkf8回避了攻击

0_n0foAiLMcc发起攻击, 8_7J291lHaC6回避了攻击

3_zA5mtxMcPi发动会心一击, 6_LSUD2CsfGo受到89点伤害

7_rRZN5tNMjb发起攻击, 5_nr70Kikkf8受到81点伤害

8_7J291lHaC6发起攻击, 9_tZOkO2s8JU受到42点伤害

6_LSUD2CsfGo发起攻击, 9_tZOkO2s8JU受到179点伤害

 9_tZOkO2s8JU被击倒了

4_P0revuk8ms发起攻击, 3_zA5mtxMcPi受到66点伤害

5_nr70Kikkf8发起攻击, 6_LSUD2CsfGo受到101点伤害

2_pg6O17IbDF发起攻击, 0_n0foAiLMcc受到106点伤害

0_n0foAiLMcc发起攻击, 4_P0revuk8ms受到72点伤害

3_zA5mtxMcPi发起攻击, 5_nr70Kikkf8受到66点伤害

1_Bvpbn3b55R发起攻击, 4_P0revuk8ms回避了攻击

2_pg6O17IbDF使用净化, 1_Bvpbn3b55R防御, 1_Bvpbn3b55R受到36点伤害

6_LSUD2CsfGo发起攻击, 7_rRZN5tNMjb受到84点伤害

7_rRZN5tNMjb发起攻击, 5_nr70Kikkf8受到109点伤害

 5_nr70Kikkf8被击倒了

8_7J291lHaC6发起攻击, 3_zA5mtxMcPi受到57点伤害

 3_zA5mtxMcPi被击倒了

 8_7J291lHaC6吞噬了3_zA5mtxMcPi, 8_7J291lHaC6属性上升

4_P0revuk8ms使用狂暴术, 6_LSUD2CsfGo受到73点伤害

 6_LSUD2CsfGo被击倒了

0_n0foAiLMcc发起攻击, 2_pg6O17IbDF受到22点伤害

8_7J291lHaC6发起攻击, 1_Bvpbn3b55R受到50点伤害

2_pg6O17IbDF发起攻击, 0_n0foAiLMcc受到62点伤害

 0_n0foAiLMcc被击倒了

 2_pg6O17IbDF召唤亡灵, 0_n0foAiLMcc变成了丧尸

7_rRZN5tNMjb发起攻击, 1_Bvpbn3b55R受到61点伤害

 1_Bvpbn3b55R被击倒了

8_7J291lHaC6发起攻击, 丧尸受到103点伤害

4_P0revuk8ms使用狂暴术, 7_rRZN5tNMjb受到119点伤害

 7_rRZN5tNMjb被击倒了

丧尸发起攻击, 4_P0revuk8ms受到43点伤害

8_7J291lHaC6发起攻击, 丧尸受到60点伤害

 丧尸消失了

4_P0revuk8ms发起攻击, 2_pg6O17IbDF受到69点伤害

 2_pg6O17IbDF被击倒了

8_7J291lHaC6使用净化, 4_P0revuk8ms受到53点伤害

 4_P0revuk8ms被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-37 must contain a blank separator between input and trace",
        "sampled case-37 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 4371, "large_37 score mismatch");
    assert!(guard < 20_000, "sampled case-37 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-37", &actual_lines, &expected_lines);
}

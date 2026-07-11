use super::*;

pub fn large_40<E: crate::EngineAdapter>() {
    const CASE: &str = r####"0_N47q8QanNZ
1_kQLcV7rC4y
2_i20PdmDzEF
3_CVQot3gtn1
4_OoOMU4fqOA
5_EAFtoxdOB2
6_tRcahIayM4
7_T0zr6asNlZ
8_23KqaLUKKD
9_OjPjUlTNxb


2_i20PdmDzEF发起攻击, 5_EAFtoxdOB2受到86点伤害

4_OoOMU4fqOA发起攻击, 0_N47q8QanNZ受到78点伤害

8_23KqaLUKKD发起攻击, 3_CVQot3gtn1受到51点伤害

9_OjPjUlTNxb发起攻击, 7_T0zr6asNlZ回避了攻击

0_N47q8QanNZ发起攻击, 6_tRcahIayM4受到94点伤害

7_T0zr6asNlZ投毒, 2_i20PdmDzEF受到40点伤害, 2_i20PdmDzEF中毒

3_CVQot3gtn1使用净化, 1_kQLcV7rC4y受到68点伤害

5_EAFtoxdOB2发起攻击, 4_OoOMU4fqOA受到91点伤害

6_tRcahIayM4发起攻击, 1_kQLcV7rC4y受到27点伤害

 6_tRcahIayM4连击, 1_kQLcV7rC4y受到29点伤害

2_i20PdmDzEF发起攻击, 7_T0zr6asNlZ受到112点伤害

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到14点伤害

1_kQLcV7rC4y发起攻击, 8_23KqaLUKKD受到124点伤害

0_N47q8QanNZ发动铁壁, 0_N47q8QanNZ防御力大幅上升

7_T0zr6asNlZ发起攻击, 4_OoOMU4fqOA受到104点伤害

6_tRcahIayM4发起攻击, 3_CVQot3gtn1受到44点伤害

4_OoOMU4fqOA使用地裂术

 9_OjPjUlTNxb受到24点伤害

 3_CVQot3gtn1受到27点伤害

 1_kQLcV7rC4y受到26点伤害

 8_23KqaLUKKD受到40点伤害

8_23KqaLUKKD发起攻击, 4_OoOMU4fqOA受到82点伤害

 4_OoOMU4fqOA被击倒了

3_CVQot3gtn1使用净化, 7_T0zr6asNlZ受到45点伤害

1_kQLcV7rC4y发起攻击, 9_OjPjUlTNxb受到95点伤害

9_OjPjUlTNxb发起攻击, 5_EAFtoxdOB2受到39点伤害

2_i20PdmDzEF使用加速术, 2_i20PdmDzEF进入疾走状态

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到12点伤害

3_CVQot3gtn1使用净化, 7_T0zr6asNlZ受到86点伤害

8_23KqaLUKKD发起攻击, 5_EAFtoxdOB2回避了攻击

2_i20PdmDzEF发起攻击, 0_N47q8QanNZ受到1点伤害

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到10点伤害

0_N47q8QanNZ发起攻击, 2_i20PdmDzEF回避了攻击

7_T0zr6asNlZ发起攻击, 2_i20PdmDzEF受到18点伤害

2_i20PdmDzEF发起攻击, 0_N47q8QanNZ受到1点伤害

 2_i20PdmDzEF毒性发作, 2_i20PdmDzEF受到8点伤害

 2_i20PdmDzEF从中毒中解除

 2_i20PdmDzEF从疾走中解除

3_CVQot3gtn1使用雷击术

 0_N47q8QanNZ受到1点伤害

 0_N47q8QanNZ受到1点伤害

 0_N47q8QanNZ受到1点伤害

 0_N47q8QanNZ回避了攻击

5_EAFtoxdOB2发起攻击, 0_N47q8QanNZ受到1点伤害

6_tRcahIayM4发起攻击, 9_OjPjUlTNxb回避了攻击

8_23KqaLUKKD发起攻击, 3_CVQot3gtn1受到25点伤害

0_N47q8QanNZ发起攻击, 5_EAFtoxdOB2回避了攻击

 0_N47q8QanNZ从铁壁中解除

1_kQLcV7rC4y发起攻击, 6_tRcahIayM4受到92点伤害

2_i20PdmDzEF发动会心一击, 0_N47q8QanNZ受到84点伤害

7_T0zr6asNlZ发起攻击, 3_CVQot3gtn1受到63点伤害

9_OjPjUlTNxb发起攻击, 5_EAFtoxdOB2受到46点伤害

6_tRcahIayM4发起攻击, 3_CVQot3gtn1受到47点伤害

3_CVQot3gtn1发起攻击, 8_23KqaLUKKD受到68点伤害

 8_23KqaLUKKD被击倒了

5_EAFtoxdOB2发起攻击, 2_i20PdmDzEF受到85点伤害

1_kQLcV7rC4y投毒, 0_N47q8QanNZ受到36点伤害, 0_N47q8QanNZ中毒

9_OjPjUlTNxb发起攻击, 2_i20PdmDzEF回避了攻击

7_T0zr6asNlZ发起攻击, 3_CVQot3gtn1受到79点伤害

 3_CVQot3gtn1被击倒了

0_N47q8QanNZ使用火球术, 9_OjPjUlTNxb受到126点伤害

 0_N47q8QanNZ毒性发作, 0_N47q8QanNZ受到26点伤害

2_i20PdmDzEF发起攻击, 5_EAFtoxdOB2受到85点伤害

5_EAFtoxdOB2使用火球术, 6_tRcahIayM4受到96点伤害

6_tRcahIayM4发起攻击, 0_N47q8QanNZ受到44点伤害

1_kQLcV7rC4y发起攻击, 2_i20PdmDzEF受到47点伤害

7_T0zr6asNlZ发起攻击, 1_kQLcV7rC4y受到48点伤害

9_OjPjUlTNxb使用幻术, 召唤出幻影

6_tRcahIayM4发起攻击, 7_T0zr6asNlZ受到110点伤害

 7_T0zr6asNlZ被击倒了

0_N47q8QanNZ发起攻击, 9_OjPjUlTNxb受到116点伤害

 9_OjPjUlTNxb被击倒了

 幻影消失了

 0_N47q8QanNZ毒性发作, 0_N47q8QanNZ受到22点伤害

 0_N47q8QanNZ被击倒了

2_i20PdmDzEF发起攻击, 1_kQLcV7rC4y回避了攻击

5_EAFtoxdOB2发起攻击, 1_kQLcV7rC4y受到40点伤害

1_kQLcV7rC4y发起攻击, 5_EAFtoxdOB2防御, 5_EAFtoxdOB2受到21点伤害

6_tRcahIayM4发起攻击, 2_i20PdmDzEF受到86点伤害

 2_i20PdmDzEF被击倒了

1_kQLcV7rC4y发起攻击, 6_tRcahIayM4受到128点伤害

 6_tRcahIayM4被击倒了

5_EAFtoxdOB2使用雷击术

 1_kQLcV7rC4y受到11点伤害

 1_kQLcV7rC4y受到29点伤害

 1_kQLcV7rC4y被击倒了
"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-40 must contain a blank separator between input and trace",
        "sampled case-40 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 4189, "large_40 score mismatch");
    assert!(guard < 20_000, "sampled case-40 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-40", &actual_lines, &expected_lines);
}

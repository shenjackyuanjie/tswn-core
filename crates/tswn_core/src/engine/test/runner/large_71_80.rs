use super::*;

#[test]
fn large_71() {
    const CASE: &str = r#"南峰 }bRMSYZX@Shabby_fish
反袭 #YN785ClJ3@Shabby_fish


反袭使用幻术, 召唤出幻影

南峰潜行到反袭身后

南峰发动背刺, 反袭受到329点伤害

反袭发起攻击, 南峰受到87点伤害

反袭使用火球术, 南峰回避了攻击

南峰发起攻击, 幻影受到61点伤害

幻影使用附体, 南峰进入狂暴状态

 幻影消失了

南峰发起狂暴攻击, 反袭受到23点伤害

反袭发起攻击, 南峰受到0点伤害

反袭发起攻击, 南峰受到58点伤害

反袭发起攻击, 南峰回避了攻击

南峰发起狂暴攻击, 南峰回避了攻击

反袭发起攻击, 南峰受到39点伤害

南峰发起狂暴攻击, 反袭受到65点伤害

 反袭被击倒了
"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-71 must contain a blank separator between input and trace",
        "sampled case-71 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-71 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-71", &actual_lines, &expected_lines);
}


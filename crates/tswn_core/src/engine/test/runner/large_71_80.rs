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

#[test]
fn large_72() {
    const CASE: &str = r#"Parallel #yVjONa74t@Shabby_fish
合成机油 #ofkyO9UxU@Shabby_fish
极寒陷阱 #46fdemQ6i@Shabby_fish
与神对话 #PQMhRNLYO@Shabby_fish


Parallel发动铁壁, Parallel防御力大幅上升

合成机油发起攻击, Parallel受到0点伤害

极寒陷阱潜行到与神对话身后

Parallel发起攻击, 与神对话受到55点伤害

与神对话发起攻击, Parallel受到0点伤害

极寒陷阱发动背刺, 与神对话受到280点伤害

合成机油使用幻术, 召唤出幻影

与神对话潜行到合成机油身后

Parallel发起攻击, 合成机油受到57点伤害

 Parallel从铁壁中解除

极寒陷阱使用诅咒, 合成机油受到51点伤害, 合成机油被诅咒了

与神对话发动背刺, 诅咒使伤害加倍, 合成机油受到482点伤害

 合成机油被击倒了

 幻影消失了

 与神对话吞噬了合成机油, 与神对话属性上升

与神对话潜行到Parallel身后

极寒陷阱使用雷击术

 Parallel受到19点伤害

 Parallel受到16点伤害

 Parallel受到12点伤害

 Parallel受到35点伤害

Parallel发起攻击, 极寒陷阱受到57点伤害

与神对话发动背刺, Parallel受到298点伤害

 Parallel被击倒了

 与神对话吞噬了Parallel, 与神对话属性上升

极寒陷阱发起攻击, 与神对话回避了攻击

与神对话使用幻术, 召唤出幻影

极寒陷阱发起攻击, 与神对话回避了攻击

与神对话发动铁壁, 与神对话防御力大幅上升

幻影使用附体, 极寒陷阱进入狂暴状态

 幻影消失了

极寒陷阱发起狂暴攻击, 极寒陷阱受到63点伤害

与神对话潜行到极寒陷阱身后

极寒陷阱发起狂暴攻击, 极寒陷阱回避了攻击

与神对话发动背刺, 极寒陷阱受到325点伤害

 极寒陷阱被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-72 must contain a blank separator between input and trace",
        "sampled case-72 trace is empty",
    );
    let mut runner = runners::Runner::new_from_namerena_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-72 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-72", &actual_lines, &expected_lines);
}
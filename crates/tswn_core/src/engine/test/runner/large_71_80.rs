use super::*;

#[test]
fn large_71() {
    const CASE: &str = r#"南峰 }bRMSYZX@Shabby_fish
反袭 #YN785ClJ3@Shabby_fish


反袭 #YN785ClJ3@Shabby_fish使用幻术, 召唤出反袭 #YN785ClJ3?0@Shabby_fish

南峰 }bRMSYZX@Shabby_fish潜行到反袭 #YN785ClJ3@Shabby_fish身后

南峰 }bRMSYZX@Shabby_fish发动背刺, 反袭 #YN785ClJ3@Shabby_fish受到329点伤害

反袭 #YN785ClJ3@Shabby_fish发起攻击, 南峰 }bRMSYZX@Shabby_fish受到87点伤害

反袭 #YN785ClJ3@Shabby_fish使用火球术, 南峰 }bRMSYZX@Shabby_fish回避了攻击

南峰 }bRMSYZX@Shabby_fish发起攻击, 反袭 #YN785ClJ3?0@Shabby_fish受到61点伤害

反袭 #YN785ClJ3?0@Shabby_fish使用附体, 南峰 }bRMSYZX@Shabby_fish进入狂暴状态

反袭 #YN785ClJ3?0@Shabby_fish消失了

南峰 }bRMSYZX@Shabby_fish发起狂暴攻击, 反袭 #YN785ClJ3@Shabby_fish受到23点伤害

反袭 #YN785ClJ3@Shabby_fish发起攻击, 南峰 }bRMSYZX@Shabby_fish受到0点伤害

反袭 #YN785ClJ3@Shabby_fish发起攻击, 南峰 }bRMSYZX@Shabby_fish受到58点伤害

反袭 #YN785ClJ3@Shabby_fish发起攻击, 南峰 }bRMSYZX@Shabby_fish回避了攻击

南峰 }bRMSYZX@Shabby_fish发起狂暴攻击, 南峰 }bRMSYZX@Shabby_fish回避了攻击

反袭 #YN785ClJ3@Shabby_fish发起攻击, 南峰 }bRMSYZX@Shabby_fish受到39点伤害

南峰 }bRMSYZX@Shabby_fish发起狂暴攻击, 反袭 #YN785ClJ3@Shabby_fish受到65点伤害

反袭 #YN785ClJ3@Shabby_fish被击倒了
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


Parallel #yVjONa74t@Shabby_fish发动铁壁, Parallel #yVjONa74t@Shabby_fish防御力大幅上升

合成机油 #ofkyO9UxU@Shabby_fish发起攻击, Parallel #yVjONa74t@Shabby_fish受到0点伤害

极寒陷阱 #46fdemQ6i@Shabby_fish潜行到与神对话 #PQMhRNLYO@Shabby_fish身后

Parallel #yVjONa74t@Shabby_fish发起攻击, 与神对话 #PQMhRNLYO@Shabby_fish受到55点伤害

与神对话 #PQMhRNLYO@Shabby_fish发起攻击, Parallel #yVjONa74t@Shabby_fish受到0点伤害

极寒陷阱 #46fdemQ6i@Shabby_fish发动背刺, 与神对话 #PQMhRNLYO@Shabby_fish受到280点伤害

合成机油 #ofkyO9UxU@Shabby_fish使用幻术, 召唤出合成机油 #ofkyO9UxU?0@Shabby_fish

与神对话 #PQMhRNLYO@Shabby_fish潜行到合成机油 #ofkyO9UxU@Shabby_fish身后

Parallel #yVjONa74t@Shabby_fish发起攻击, 合成机油 #ofkyO9UxU@Shabby_fish受到57点伤害

Parallel #yVjONa74t@Shabby_fish从铁壁中解除

极寒陷阱 #46fdemQ6i@Shabby_fish使用诅咒, 合成机油 #ofkyO9UxU@Shabby_fish受到51点伤害, 合成机油 #ofkyO9UxU@Shabby_fish被诅咒了

与神对话 #PQMhRNLYO@Shabby_fish发动背刺, 诅咒使伤害加倍, 合成机油 #ofkyO9UxU@Shabby_fish受到482点伤害

合成机油 #ofkyO9UxU@Shabby_fish被击倒了

合成机油 #ofkyO9UxU?0@Shabby_fish消失了

与神对话 #PQMhRNLYO@Shabby_fish吞噬了合成机油 #ofkyO9UxU@Shabby_fish, 与神对话 #PQMhRNLYO@Shabby_fish属性上升

与神对话 #PQMhRNLYO@Shabby_fish潜行到Parallel #yVjONa74t@Shabby_fish身后

极寒陷阱 #46fdemQ6i@Shabby_fish使用雷击术

Parallel #yVjONa74t@Shabby_fish受到19点伤害

Parallel #yVjONa74t@Shabby_fish受到16点伤害

Parallel #yVjONa74t@Shabby_fish受到12点伤害

Parallel #yVjONa74t@Shabby_fish受到35点伤害

Parallel #yVjONa74t@Shabby_fish发起攻击, 极寒陷阱 #46fdemQ6i@Shabby_fish受到57点伤害

与神对话 #PQMhRNLYO@Shabby_fish发动背刺, Parallel #yVjONa74t@Shabby_fish受到298点伤害

Parallel #yVjONa74t@Shabby_fish被击倒了

与神对话 #PQMhRNLYO@Shabby_fish吞噬了Parallel #yVjONa74t@Shabby_fish, 与神对话 #PQMhRNLYO@Shabby_fish属性上升

极寒陷阱 #46fdemQ6i@Shabby_fish发起攻击, 与神对话 #PQMhRNLYO@Shabby_fish回避了攻击

与神对话 #PQMhRNLYO@Shabby_fish使用幻术, 召唤出与神对话 #PQMhRNLYO?0@Shabby_fish

极寒陷阱 #46fdemQ6i@Shabby_fish发起攻击, 与神对话 #PQMhRNLYO@Shabby_fish回避了攻击

与神对话 #PQMhRNLYO@Shabby_fish发动铁壁, 与神对话 #PQMhRNLYO@Shabby_fish防御力大幅上升

与神对话 #PQMhRNLYO?0@Shabby_fish使用附体, 极寒陷阱 #46fdemQ6i@Shabby_fish进入狂暴状态

与神对话 #PQMhRNLYO?0@Shabby_fish消失了

极寒陷阱 #46fdemQ6i@Shabby_fish发起狂暴攻击, 极寒陷阱 #46fdemQ6i@Shabby_fish受到63点伤害

与神对话 #PQMhRNLYO@Shabby_fish潜行到极寒陷阱 #46fdemQ6i@Shabby_fish身后

极寒陷阱 #46fdemQ6i@Shabby_fish发起狂暴攻击, 极寒陷阱 #46fdemQ6i@Shabby_fish回避了攻击

与神对话 #PQMhRNLYO@Shabby_fish发动背刺, 极寒陷阱 #46fdemQ6i@Shabby_fish受到325点伤害

极寒陷阱 #46fdemQ6i@Shabby_fish被击倒了"#;
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
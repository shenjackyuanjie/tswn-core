use super::*;

pub fn large_12<E: crate::EngineAdapter>() {
    const CASE: &str = r####"WmG4iW0iZI
L6x5GQXq47
PzFvkx7lP7
m6SPYplZoz
m8iy8R0bkF


L6x5GQXq47发起攻击, WmG4iW0iZI回避了攻击

WmG4iW0iZI发起攻击, PzFvkx7lP7受到81点伤害

PzFvkx7lP7发起攻击, L6x5GQXq47受到32点伤害

m6SPYplZoz发起攻击, L6x5GQXq47受到97点伤害

m8iy8R0bkF发起攻击, L6x5GQXq47受到104点伤害

L6x5GQXq47发起攻击, WmG4iW0iZI受到83点伤害

WmG4iW0iZI发起攻击, PzFvkx7lP7受到79点伤害

PzFvkx7lP7发起攻击, m8iy8R0bkF受到61点伤害

m6SPYplZoz发起攻击, L6x5GQXq47受到62点伤害

L6x5GQXq47发起攻击, m8iy8R0bkF受到53点伤害

WmG4iW0iZI投毒, L6x5GQXq47受到42点伤害

 L6x5GQXq47被击倒了

m8iy8R0bkF发起攻击, PzFvkx7lP7受到106点伤害

PzFvkx7lP7发起攻击, m6SPYplZoz受到33点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI受到28点伤害

m8iy8R0bkF发起攻击, m6SPYplZoz受到61点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI回避了攻击

WmG4iW0iZI发起攻击, PzFvkx7lP7受到44点伤害

 PzFvkx7lP7被击倒了

WmG4iW0iZI发起攻击, m8iy8R0bkF受到0点伤害

m8iy8R0bkF发起攻击, WmG4iW0iZI受到83点伤害

m6SPYplZoz发起攻击, m8iy8R0bkF受到131点伤害

m8iy8R0bkF发起攻击, m6SPYplZoz受到72点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI受到85点伤害

WmG4iW0iZI发起攻击, m8iy8R0bkF受到61点伤害

 m8iy8R0bkF被击倒了

m6SPYplZoz发起攻击, WmG4iW0iZI受到31点伤害

WmG4iW0iZI发起攻击, m6SPYplZoz回避了攻击

m6SPYplZoz发起攻击, WmG4iW0iZI受到29点伤害

 WmG4iW0iZI被击倒了, WmG4iW0iZI使用护身符抵挡了一次死亡, WmG4iW0iZI回复体力10点

WmG4iW0iZI发起攻击, m6SPYplZoz受到70点伤害

m6SPYplZoz发起攻击, WmG4iW0iZI受到85点伤害

 WmG4iW0iZI被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-12 must contain a blank separator between input and trace",
        "sampled case-12 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 2014, "large_12 score mismatch");

    assert!(guard < 20_000, "sampled case-12 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-12", &actual_lines, &expected_lines);
}

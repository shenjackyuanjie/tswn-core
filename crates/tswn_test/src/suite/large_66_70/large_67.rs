use super::*;

/// 这个好像是用来check ts的
pub fn large_67<E: crate::EngineAdapter>() {
    const CASE: &str = r#"Stupefy #rkISERW8@Shabby_fish
日落·日出 #Pd3J7shds@Shabby_fish


Stupefy潜行到日落·日出身后

日落·日出使用血祭, 召唤出使魔

使魔发起攻击, Stupefy受到31点伤害

 Stupefy的潜行被识破

Stupefy潜行到日落·日出身后

日落·日出使用分身, 出现一个新的日落·日出

Stupefy发动背刺, 日落·日出受到440点伤害

 日落·日出被击倒了

 使魔消失了

 Stupefy吞噬了日落·日出, Stupefy属性上升

日落·日出发起攻击, Stupefy受到50点伤害

Stupefy发起攻击, 日落·日出受到22点伤害

日落·日出发起攻击, Stupefy受到37点伤害

Stupefy发起攻击, 日落·日出受到79点伤害

日落·日出潜行到Stupefy身后

Stupefy使用血祭, 召唤出使魔

使魔发起攻击, 日落·日出防御, 日落·日出受到11点伤害

 日落·日出的潜行被识破

日落·日出发起攻击, Stupefy防御, Stupefy受到24点伤害

Stupefy发起攻击, 日落·日出受到49点伤害

使魔发起攻击, 日落·日出受到63点伤害

 日落·日出被击倒了"#;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-67 must contain a blank separator between input and trace",
        "sampled case-67 trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, _total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert!(guard < 20_000, "sampled case-67 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-67", &actual_lines, &expected_lines);
}

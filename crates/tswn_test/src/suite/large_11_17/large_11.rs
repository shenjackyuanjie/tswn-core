use super::*;

pub fn large_11<E: crate::EngineAdapter>() {
    const CASE: &str = r####"abc
aaaa
adwada
asdds
fdgs
dfgwat
sdc


fdgs发起攻击, sdc受到37点伤害

adwada发起攻击, fdgs受到60点伤害

aaaa发起攻击, abc受到36点伤害

asdds使用地裂术

 dfgwat受到22点伤害

 abc受到35点伤害

 sdc受到14点伤害

 adwada受到18点伤害

 fdgs受到21点伤害

abc发起攻击, dfgwat回避了攻击

dfgwat发起攻击, aaaa受到77点伤害

fdgs发起攻击, dfgwat回避了攻击

sdc发起攻击, dfgwat受到46点伤害

adwada发起攻击, aaaa受到91点伤害

abc发起攻击, adwada受到92点伤害

aaaa发起攻击, dfgwat受到58点伤害

dfgwat发起攻击, asdds受到34点伤害

sdc发起攻击, fdgs使用伤害反弹, sdc受到18点伤害

asdds发起攻击, fdgs受到40点伤害

fdgs发起攻击, sdc受到80点伤害

adwada发起攻击, sdc受到64点伤害

abc发起攻击, dfgwat受到109点伤害

dfgwat发起攻击, abc受到69点伤害

asdds使用血祭, 召唤出使魔

aaaa使用瘟疫, abc体力减少51%

使魔使用火球术, fdgs受到84点伤害

sdc发起攻击, asdds受到92点伤害

dfgwat使用魅惑, asdds被魅惑了

abc发起攻击, adwada受到102点伤害

adwada发起攻击, aaaa使用伤害反弹, adwada受到19点伤害

asdds发起攻击, abc回避了攻击

 asdds从魅惑中解除

fdgs使用火球术, asdds受到70点伤害

使魔发起攻击, abc受到35点伤害

dfgwat发起攻击, aaaa受到69点伤害

abc发起攻击, dfgwat受到39点伤害

fdgs使用净化, abc受到48点伤害

 abc被击倒了

adwada使用幻术, 召唤出幻影

asdds发起攻击, dfgwat受到88点伤害

 dfgwat被击倒了

sdc发起攻击, 使魔受到70点伤害, asdds受到35点伤害

使魔发起攻击, aaaa受到75点伤害

 aaaa被击倒了

adwada发起攻击, 使魔受到62点伤害, asdds受到31点伤害

 asdds被击倒了

 使魔消失了

fdgs发起攻击, adwada受到81点伤害

 adwada被击倒了

 幻影消失了

sdc使用狂暴术, fdgs受到44点伤害, fdgs进入狂暴状态

sdc发起攻击, fdgs受到53点伤害

 fdgs被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-11 must contain a blank separator between input and trace",
        "sampled case-11 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 3021, "large_11 score mismatch");

    assert!(guard < 20_000, "sampled case-11 combat did not finish in expected rounds");
    if actual_lines != expected_lines {
        eprintln!("Mismatch found!");
        eprintln!("Actual lines ({}):", actual_lines.len());
        for (i, line) in actual_lines.iter().enumerate() {
            eprintln!("  {}: {}", i, line);
        }
        eprintln!("Expected lines ({}):", expected_lines.len());
        for (i, line) in expected_lines.iter().enumerate() {
            eprintln!("  {}: {}", i, line);
        }
    }
    assert_trace_with_context("sampled case-11", &actual_lines, &expected_lines);
}

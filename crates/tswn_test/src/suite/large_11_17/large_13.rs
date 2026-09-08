use super::*;

pub fn large_13<E: crate::EngineAdapter>() {
    const CASE: &str = r####"qmAhJQzAVj
VuRY86K5Fy
YbhecuG73P
bFtbzLCkX3
rc6tMFMk7z


qmAhJQzAVj发起攻击, bFtbzLCkX3受到101点伤害

bFtbzLCkX3发起攻击, VuRY86K5Fy受到82点伤害

rc6tMFMk7z发起攻击, qmAhJQzAVj受到52点伤害

VuRY86K5Fy发起攻击, qmAhJQzAVj受到46点伤害

qmAhJQzAVj发起攻击, rc6tMFMk7z受到75点伤害

bFtbzLCkX3发起攻击, YbhecuG73P受到70点伤害

YbhecuG73P发起攻击, VuRY86K5Fy防御, VuRY86K5Fy受到13点伤害

VuRY86K5Fy发起攻击, bFtbzLCkX3受到120点伤害

rc6tMFMk7z发起攻击, VuRY86K5Fy受到65点伤害

YbhecuG73P使用加速术, YbhecuG73P进入疾走状态

bFtbzLCkX3发起攻击, rc6tMFMk7z受到38点伤害

YbhecuG73P发起攻击, qmAhJQzAVj受到52点伤害

qmAhJQzAVj发起攻击, VuRY86K5Fy受到31点伤害

VuRY86K5Fy发起攻击, rc6tMFMk7z受到101点伤害

YbhecuG73P发起攻击, rc6tMFMk7z受到103点伤害

 rc6tMFMk7z被击倒了

 YbhecuG73P从疾走中解除

bFtbzLCkX3使用净化, YbhecuG73P受到121点伤害

VuRY86K5Fy发起攻击, qmAhJQzAVj受到102点伤害

YbhecuG73P发起攻击, qmAhJQzAVj回避了攻击

qmAhJQzAVj发起攻击, VuRY86K5Fy受到61点伤害

bFtbzLCkX3发起攻击, qmAhJQzAVj受到66点伤害

 qmAhJQzAVj被击倒了

VuRY86K5Fy发起攻击, YbhecuG73P受到60点伤害

 YbhecuG73P被击倒了

bFtbzLCkX3发起攻击, VuRY86K5Fy受到21点伤害

VuRY86K5Fy发起攻击, bFtbzLCkX3受到113点伤害

 bFtbzLCkX3被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-13 must contain a blank separator between input and trace",
        "sampled case-13 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1833, "large_13 score mismatch");

    assert!(guard < 20_000, "sampled case-13 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-13", &actual_lines, &expected_lines);
}

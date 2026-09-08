use super::*;

pub fn large_20<E: crate::EngineAdapter>() {
    const CASE: &str = r####"k9brYO9ljp
5fh9ir4AaE
905nLuadjH
u05tVLWa49
kil5DzKTCb


k9brYO9ljp发起攻击, 905nLuadjH受到24点伤害

kil5DzKTCb发起攻击, k9brYO9ljp受到55点伤害

u05tVLWa49投毒, 905nLuadjH受到71点伤害, 905nLuadjH中毒

k9brYO9ljp发起攻击, 5fh9ir4AaE回避了攻击

905nLuadjH发起攻击, 5fh9ir4AaE受到43点伤害

 905nLuadjH毒性发作, 905nLuadjH受到26点伤害

5fh9ir4AaE发起攻击, kil5DzKTCb回避了攻击

5fh9ir4AaE发起攻击, k9brYO9ljp受到27点伤害

u05tVLWa49发起攻击, 5fh9ir4AaE受到71点伤害

kil5DzKTCb发起攻击, u05tVLWa49受到80点伤害

905nLuadjH发起攻击, kil5DzKTCb受到56点伤害

 905nLuadjH毒性发作, 905nLuadjH受到21点伤害

k9brYO9ljp发起攻击, u05tVLWa49受到33点伤害

kil5DzKTCb发起攻击, k9brYO9ljp受到95点伤害

5fh9ir4AaE发起攻击, k9brYO9ljp受到59点伤害

u05tVLWa49发起攻击, kil5DzKTCb受到80点伤害

k9brYO9ljp发起攻击, 5fh9ir4AaE受到16点伤害

5fh9ir4AaE发起攻击, 905nLuadjH受到62点伤害

kil5DzKTCb发起攻击, u05tVLWa49受到134点伤害

905nLuadjH发起攻击, 5fh9ir4AaE受到56点伤害

 905nLuadjH毒性发作, 905nLuadjH受到18点伤害

u05tVLWa49发起攻击, 5fh9ir4AaE受到28点伤害

905nLuadjH发起攻击, 5fh9ir4AaE受到47点伤害

 905nLuadjH毒性发作, 905nLuadjH受到15点伤害

 905nLuadjH从中毒中解除

k9brYO9ljp发起攻击, u05tVLWa49防御, u05tVLWa49受到23点伤害

 u05tVLWa49被击倒了

5fh9ir4AaE使用地裂术

 kil5DzKTCb受到43点伤害

 905nLuadjH受到78点伤害

 905nLuadjH被击倒了

 k9brYO9ljp受到41点伤害

 k9brYO9ljp被击倒了

kil5DzKTCb发起攻击, 5fh9ir4AaE受到42点伤害

5fh9ir4AaE发起攻击, kil5DzKTCb回避了攻击

kil5DzKTCb发起攻击, 5fh9ir4AaE受到90点伤害

 5fh9ir4AaE被击倒了"####;
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        CASE,
        "sampled case-20 must contain a blank separator between input and trace",
        "sampled case-20 trace is empty",
    );

    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 20_000, true);
    assert_eq!(total_score, 1796, "large_20 score mismatch");

    assert!(guard < 20_000, "sampled case-20 combat did not finish in expected rounds");
    assert_trace_with_context("sampled case-20", &actual_lines, &expected_lines);
}

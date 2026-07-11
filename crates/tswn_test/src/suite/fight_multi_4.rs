//! 多队混战回放测试分片 4。
//!
//! 分片文件只保存大型 fixture，执行逻辑由 runner 测试公共 helper 负责。

use super::*;

pub fn fight_multi_4<E: crate::EngineAdapter>() {
    const FIGHT_CASE: &str = include_str!("fixtures/fight_multi_4.md");
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, 50_000, true);
    assert_eq!(total_score, 25799, "fight_multi_4 score mismatch");
    assert!(guard < 50_000, "fight_multi_4 combat did not finish in expected rounds");
    assert_trace_with_context("fight_multi_4", &actual_lines, &expected_lines);
}

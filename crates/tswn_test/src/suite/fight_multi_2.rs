//! 多队混战回放测试分片 2。
//!
//! 分片文件只保存大型 fixture，执行逻辑由 runner 测试公共 helper 负责。

use super::*;

pub fn fight_multi_2<E: crate::EngineAdapter>() {
    const FIGHT_CASE: &str = include_str!("fixtures/fight_multi_2.md");
    let (raw_input, expected_lines) = parse_embedded_fight_case(
        FIGHT_CASE,
        "embedded fight case must contain a blank separator between input and trace",
        "embedded fight trace is empty",
    );
    const MAX_ROUNDS: usize = 2_000;
    let mut runner = E::new_from_raw(raw_input).unwrap();
    let (actual_lines, guard, total_score) = collect_replay_lines::<E>(&mut runner, MAX_ROUNDS, true);
    assert_eq!(total_score, 24592, "fight_multi_2 score mismatch");
    if guard >= MAX_ROUNDS {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(20);
        let ctx_end = (mismatch_idx + 20).min(min_len);
        eprintln!("fight_multi_2 timeout mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_2 combat did not finish in expected rounds: guard={guard}, actual_len={}, expected_len={}, mismatch_idx={mismatch_idx}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx),
        );
    }
    if actual_lines != expected_lines {
        let min_len = actual_lines.len().min(expected_lines.len());
        let mismatch_idx = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .position(|(lhs, rhs)| lhs != rhs)
            .unwrap_or(min_len);
        let ctx_start = mismatch_idx.saturating_sub(5);
        let ctx_end = (mismatch_idx + 5).min(min_len);
        eprintln!("fight_multi_2 mismatch context [{ctx_start}..{ctx_end}):");
        for idx in ctx_start..ctx_end {
            eprintln!(
                "  idx={idx}: actual={:?} | expected={:?}",
                actual_lines.get(idx),
                expected_lines.get(idx)
            );
        }
        panic!(
            "fight_multi_2 mismatch at idx={mismatch_idx}, actual_len={}, expected_len={}, actual={:?}, expected={:?}",
            actual_lines.len(),
            expected_lines.len(),
            actual_lines.get(mismatch_idx),
            expected_lines.get(mismatch_idx)
        );
    }
}

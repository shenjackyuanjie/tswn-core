use crate::random::Random;
use tswn_core::cli_api::battle::{BattleModelFrame, BattleModelOutcome, BattleStopReason};

/// 返回待导出帧的下标，按时间排序；初始状态由调用方单独处理。
pub fn select(frames: &[BattleModelFrame], outcome: &BattleModelOutcome, count: usize, seed: &str) -> Vec<usize> {
    let wanted = count.saturating_sub(1);
    if wanted == 0 || outcome.rounds_advanced == 0 {
        return Vec::new();
    }
    let eligible: Vec<_> = frames
        .iter()
        .enumerate()
        .filter(|(_, frame)| outcome.stop_reason != BattleStopReason::Winner || frame.rounds_advanced != outcome.rounds_advanced)
        .map(|(index, _)| index)
        .collect();
    let mut random = Random::new(&[b"sampling-v1", seed.as_bytes()]);
    // 稀疏分组避免用户设置很大上限时按上限分配空桶。
    let mut bins = std::collections::BTreeMap::<usize, Vec<usize>>::new();
    for index in &eligible {
        let bin = ((frames[*index].rounds_advanced as u128 * wanted as u128 / outcome.rounds_advanced as u128) as usize)
            .min(wanted - 1);
        bins.entry(bin).or_default().push(*index);
    }
    let mut selected: Vec<_> = bins.values().map(|bin| bin[random.index(bin.len())]).collect();
    let mut remaining: Vec<_> = eligible.into_iter().filter(|index| !selected.contains(index)).collect();
    random.shuffle(&mut remaining);
    selected.extend(remaining.into_iter().take(wanted.saturating_sub(selected.len())));
    selected.sort_unstable();
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fills_empty_bins_without_duplicates_or_winner_frame() {
        let frames: Vec<_> = [1, 2, 3, 4, 5, 6, 7, 100]
            .into_iter()
            .enumerate()
            .map(|(frame_index, rounds_advanced)| BattleModelFrame {
                frame_index,
                round_index: rounds_advanced - 1,
                rounds_advanced,
            })
            .collect();
        let mut outcome = BattleModelOutcome {
            stop_reason: BattleStopReason::Winner,
            rounds_advanced: 100,
            frames_emitted: 8,
            winner_team_indices: vec![0],
        };
        assert_eq!(select(&frames, &outcome, 8, "x"), (0..7).collect::<Vec<_>>());
        assert_eq!(select(&frames[..2], &outcome, 8, "x"), vec![0, 1]);
        assert!(select(&frames, &outcome, 1, "x").is_empty());
        outcome.stop_reason = BattleStopReason::MaxRounds;
        outcome.winner_team_indices.clear();
        assert_eq!(select(&frames, &outcome, 20, "x"), (0..8).collect::<Vec<_>>());
    }
}

//! 对局候选、配额、分布统计与预期分差。
//!
//! 先围绕当前分数排序位置搜索，再混入全局随机候选；两阶段共用无序配对键，避免重复对局。
//! 生成候选时就累计玩家的计划场次，失败对局不会退回这些配额。handicap 按玩家 ID
//! 的升序方向保存，查询相反方向时取负；分差先转为浮点数，覆盖完整 i32 分数范围。

use super::*;

impl Updator {
    pub(super) fn generate_base_matches(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &HashMap<PairKey, HandicapData>,
    ) -> Vec<MatchPair> {
        let n = players.len();
        let mut matches = Vec::new();
        let mut matched_pairs = HashSet::new();
        let mut order = (0..n).collect::<Vec<_>>();
        self.shuffle(&mut order);

        // 匹配期间分数不变，先拷一份给「必然被门拒」的预筛用
        let scores = players.iter().map(|p| p.score).collect::<Vec<i32>>();
        let max_adjustment = handicap_adjustment_bound(handicap_map);

        for i in order {
            let candidates = self.build_candidate_list(i, n, &matched_pairs, &scores, max_adjustment, |a, b| {
                let win_prob = expected_win_prob_with_handicap(a, b, players, handicap_map, true);
                (0.45..=0.55).contains(&win_prob)
                    && players[a].base_matches < MAX_ACTIVE_MATCHES
                    && players[b].base_matches < MAX_PASSIVE_MATCHES
                    && players[a].total_matches < PRE_SA_MAX_MATCHES
                    && players[b].total_matches < PRE_SA_MAX_MATCHES
            });

            for j in candidates {
                if players[i].base_matches >= MAX_ACTIVE_MATCHES || players[i].total_matches >= PRE_SA_MAX_MATCHES {
                    break;
                }
                if players[j].base_matches >= MAX_PASSIVE_MATCHES || players[j].total_matches >= PRE_SA_MAX_MATCHES {
                    continue;
                }

                let pair = MatchPair(i, j);
                if !matched_pairs.insert(pair.key()) {
                    continue;
                }

                matches.push(pair);
                players[i].total_matches += 1;
                players[j].total_matches += 1;
                players[i].base_matches += 1;
                players[j].base_matches += 1;
            }
        }

        matches
    }

    pub(super) fn generate_second_matches(
        &mut self,
        players: &mut [PlayerRow],
        handicap_map: &HashMap<PairKey, HandicapData>,
        first_matches: &[MatchPair],
    ) -> Vec<MatchPair> {
        let n = players.len();
        let mut matches = first_matches.to_vec();
        let mut matched_pairs = first_matches.iter().map(|pair| pair.key()).collect::<HashSet<_>>();
        let mut order = (0..n).collect::<Vec<_>>();
        self.shuffle(&mut order);

        let scores = players.iter().map(|p| p.score).collect::<Vec<i32>>();
        let max_adjustment = handicap_adjustment_bound(handicap_map);

        for i in order {
            let candidates = self.build_candidate_list(i, n, &matched_pairs, &scores, max_adjustment, |a, b| {
                let win_prob = expected_win_prob_with_handicap(a, b, players, handicap_map, true);
                (0.40..=0.60).contains(&win_prob)
                    && players[a].second_matches < SECOND_ACTIVE_MATCHES
                    && players[b].second_matches < SECOND_PASSIVE_MATCHES
                    && players[a].total_matches < PRE_SA_MAX_MATCHES
                    && players[b].total_matches < PRE_SA_MAX_MATCHES
            });

            for j in candidates {
                if players[i].second_matches >= SECOND_ACTIVE_MATCHES || players[i].total_matches >= PRE_SA_MAX_MATCHES {
                    break;
                }
                if players[j].second_matches >= SECOND_PASSIVE_MATCHES || players[j].total_matches >= PRE_SA_MAX_MATCHES {
                    continue;
                }

                let pair = MatchPair(i, j);
                if !matched_pairs.insert(pair.key()) {
                    continue;
                }

                matches.push(pair);
                players[i].total_matches += 1;
                players[j].total_matches += 1;
                players[i].base_matches += 1;
                players[j].base_matches += 1;
                players[i].second_matches += 1;
                players[j].second_matches += 1;
            }
        }

        matches
    }

    pub(super) fn build_candidate_list<F>(
        &mut self,
        i: usize,
        n: usize,
        matched_pairs: &HashSet<(usize, usize)>,
        scores: &[i32],
        max_adjustment: f64,
        is_eligible_match: F,
    ) -> Vec<usize>
    where
        F: Fn(usize, usize) -> bool,
    {
        let mut local_candidates = Vec::new();
        let mut candidate_set = HashSet::new();
        let search_radius = if n < 30 { 25.min(n.saturating_sub(1)) } else { 40 };

        // 先按分数差把「不管 handicap 怎么取都过不了门」的探测筛掉。
        // 这一步只跳过必然被拒的探测，候选集与随机数消耗都不变，所以行为完全等价。
        let gap_blocked = |a: usize, b: usize| -> bool {
            let gap = (scores[a] as f64 - scores[b] as f64).abs();
            probe_cannot_pass(gap, max_adjustment)
        };

        for offset in 1..=search_radius {
            let j1 = (i + offset) % n;
            if i != j1 && !gap_blocked(i, j1) {
                let pair_key = ordered_usize_pair(i, j1);
                if !matched_pairs.contains(&pair_key) && !candidate_set.contains(&j1) && is_eligible_match(i, j1) {
                    local_candidates.push(j1);
                    candidate_set.insert(j1);
                }
            }

            let j2 = (i + n - (offset % n)) % n;
            if i != j2 && !gap_blocked(i, j2) {
                let pair_key = ordered_usize_pair(i, j2);
                if !matched_pairs.contains(&pair_key) && !candidate_set.contains(&j2) && is_eligible_match(i, j2) {
                    local_candidates.push(j2);
                    candidate_set.insert(j2);
                }
            }
        }

        let exploration_quota = GLOBAL_EXPLORATION_MIN
            .max((local_candidates.len() as f64 * GLOBAL_EXPLORATION_RATIO).ceil() as usize)
            .min(n.saturating_sub(1).saturating_sub(candidate_set.len()));
        let mut random_candidates = Vec::new();
        let max_attempts = 20.max(exploration_quota * 8);

        for _ in 0..max_attempts {
            if random_candidates.len() >= exploration_quota {
                break;
            }
            let j = self.rng.usize(..n);
            if j == i || candidate_set.contains(&j) {
                continue;
            }
            let pair_key = ordered_usize_pair(i, j);
            if matched_pairs.contains(&pair_key) || !is_eligible_match(i, j) {
                continue;
            }
            random_candidates.push(j);
            candidate_set.insert(j);
        }

        let mut merged = self.blend_candidates(&local_candidates, &random_candidates, GLOBAL_EXPLORATION_RATIO);
        self.shuffle(&mut merged);
        merged
    }

    pub(super) fn blend_candidates(
        &mut self,
        local_candidates: &[usize],
        random_candidates: &[usize],
        exploration_ratio: f64,
    ) -> Vec<usize> {
        let mut merged = Vec::with_capacity(local_candidates.len() + random_candidates.len());
        let mut local_idx = 0usize;
        let mut random_idx = 0usize;

        while local_idx < local_candidates.len() || random_idx < random_candidates.len() {
            let use_random = random_idx < random_candidates.len()
                && (local_idx >= local_candidates.len() || self.rng.f64() < exploration_ratio);

            if use_random {
                merged.push(random_candidates[random_idx]);
                random_idx += 1;
            } else if local_idx < local_candidates.len() {
                merged.push(local_candidates[local_idx]);
                local_idx += 1;
            }
        }

        merged
    }

    pub(super) fn analyze_match_distribution(
        &self,
        matches: &[MatchPair],
        players: &[PlayerRow],
        base_match_count: usize,
        handicap_map: &HashMap<PairKey, HandicapData>,
    ) -> MatchStats {
        let mut min_matches = usize::MAX;
        let mut max_matches = 0usize;
        let mut total_matches = 0usize;

        for player in players {
            min_matches = min_matches.min(player.total_matches);
            max_matches = max_matches.max(player.total_matches);
            total_matches += player.total_matches;
        }

        let base_only_matches = &matches[..base_match_count.min(matches.len())];
        let mut deltas = self.calculate_all_expected_deltas(players, handicap_map, base_only_matches);
        for pair in &matches[base_match_count.min(matches.len())..] {
            let delta_change = calculate_expected_delta(pair.0, pair.1, players, handicap_map);
            deltas[pair.0] += delta_change;
            deltas[pair.1] -= delta_change;
        }

        MatchStats {
            min_matches: if min_matches == usize::MAX { 0 } else { min_matches },
            max_matches,
            avg_matches: total_matches as f64 / players.len().max(1) as f64,
            final_squared_loss: calculate_total_squared_loss(&deltas),
        }
    }

    pub(super) fn calculate_all_expected_deltas(
        &self,
        players: &[PlayerRow],
        handicap_map: &HashMap<PairKey, HandicapData>,
        matches: &[MatchPair],
    ) -> Vec<f64> {
        let mut deltas = vec![0.0; players.len()];
        for pair in matches {
            let delta_change = calculate_expected_delta(pair.0, pair.1, players, handicap_map);
            deltas[pair.0] += delta_change;
            deltas[pair.1] -= delta_change;
        }
        deltas
    }
}

/// 本轮 handicap 对「分数差」的最大可能调整量（分数尺度）。
///
/// 资格门比较的是 `分数差 + 克制调整`，而克制调整用的是 `disturbed_value`
///（已含 ±40 的探索扰动），所以取本轮 map 里的最大绝对值就是安全上界。
pub(super) fn handicap_adjustment_bound(handicap_map: &HashMap<PairKey, HandicapData>) -> f64 {
    handicap_map.values().map(|d| d.disturbed_value.abs()).fold(0.0_f64, f64::max) * HANDICAP_SCALE
}

pub(super) fn ordered_usize_pair(a: usize, b: usize) -> (usize, usize) { if a < b { (a, b) } else { (b, a) } }

pub(super) fn expected_win_prob_with_handicap(
    i: usize,
    j: usize,
    players: &[PlayerRow],
    handicap_map: &HashMap<PairKey, HandicapData>,
    disturbed: bool,
) -> f64 {
    let p1 = &players[i];
    let p2 = &players[j];
    let key = PairKey::new(p1.id, p2.id);
    let handicap = handicap_map
        .get(&key)
        .map(|data| if disturbed { data.disturbed_value } else { data.basic_value })
        .unwrap_or(0.0);
    let score_diff = (p1.score as f64 - p2.score as f64)
        + if p1.id < p2.id {
            handicap * HANDICAP_SCALE
        } else {
            -handicap * HANDICAP_SCALE
        };
    logistic_expected(score_diff)
}

pub(super) fn calculate_expected_delta(
    i: usize,
    j: usize,
    players: &[PlayerRow],
    handicap_map: &HashMap<PairKey, HandicapData>,
) -> f64 {
    let p1 = &players[i];
    let p2 = &players[j];
    let expected_with_handicap = expected_win_prob_with_handicap(i, j, players, handicap_map, true);
    let expected_base = logistic_expected(p1.score as f64 - p2.score as f64);

    BASE_K_FACTOR * ((1.0 - expected_base) * expected_with_handicap - expected_base * (1.0 - expected_with_handicap))
}

pub(super) fn calculate_total_squared_loss(deltas: &[f64]) -> f64 { deltas.iter().map(|delta| delta * delta).sum() }

pub(super) fn logistic_expected(score_diff: f64) -> f64 { 1.0 / (1.0 + 10_f64.powf(-score_diff / ELO_SCALE_FACTOR)) }

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: i32, score: i32) -> PlayerRow {
        PlayerRow {
            id,
            name: format!("player-{id}"),
            loaded_score: score as f64,
            score,
            original_score: score,
            initial_rank: id,
            rank: id,
            total_matches: 0,
            base_matches: 0,
            second_matches: 0,
            supplement_matches: 0,
        }
    }

    #[test]
    fn extreme_scores_preserve_expected_probability_direction() {
        let players = [player(1, i32::MAX), player(2, i32::MIN)];
        let handicaps = HashMap::new();
        assert_eq!(expected_win_prob_with_handicap(0, 1, &players, &handicaps, false), 1.0);
        assert_eq!(expected_win_prob_with_handicap(1, 0, &players, &handicaps, false), 0.0);
        assert_eq!(calculate_expected_delta(0, 1, &players, &handicaps), 0.0);
        assert_eq!(calculate_expected_delta(1, 0, &players, &handicaps), 0.0);
    }

    #[test]
    fn ordinary_scores_keep_existing_expected_probability() {
        let players = [player(1, 1200), player(2, 1000)];
        let handicaps = HashMap::new();
        let forward = expected_win_prob_with_handicap(0, 1, &players, &handicaps, true);
        let reverse = expected_win_prob_with_handicap(1, 0, &players, &handicaps, true);
        assert_eq!(forward, logistic_expected(200.0));
        assert!((forward + reverse - 1.0).abs() < f64::EPSILON);
        assert_eq!(calculate_expected_delta(0, 1, &players, &handicaps), 0.0);
    }
}

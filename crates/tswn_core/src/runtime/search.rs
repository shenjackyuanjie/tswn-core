//! CPU 搜索的预算与一次决策结果；不改变默认自动战斗规则。
use super::*;

#[derive(Debug, Clone, Copy)]
pub struct SearchConfig {
    /// 只接管指定运行时队伍；None 表示双方均接管。
    pub team: Option<usize>,
    pub shortlist: usize,
    pub samples: usize,
    pub horizon: usize,
    /// 独立搜索种子，禁止从真实战斗随机状态推导未来结果。
    pub seed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn runner() -> RuntimeRunner {
        RuntimeRunner::new_from_groups_with_seed(&[vec!["alpha".into()], vec!["beta".into()]], &["search-test".into()]).unwrap()
    }
    #[test]
    fn same_budget_and_seed_reproduce_real_battle() {
        let mut a = runner();
        let mut b = a.clone();
        let config = SearchConfig {
            samples: 2,
            horizon: 4,
            ..SearchConfig::default()
        };
        for _ in 0..12 {
            a.runtime.run_search_round(config);
            b.runtime.run_search_round(config);
        }
        assert_eq!(a.model_state().unwrap(), b.model_state().unwrap());
    }
    #[test]
    fn probing_does_not_mutate_original_world_or_rng() {
        let a = runner();
        let before = a.model_state().unwrap();
        let mut probe = a.clone();
        probe.runtime.scratch.search_turn = Some(SearchTurn::new(None, None));
        for _ in 0..20 {
            probe.runtime.run_minimal_round_no_capture();
            if probe.runtime.scratch.search_turn.as_ref().is_some_and(|turn| turn.actor.is_some()) {
                break;
            }
        }
        assert_eq!(a.model_state().unwrap(), before);
        assert!(probe.runtime.scratch.search_turn.unwrap().actor.is_some());
    }
    #[test]
    fn simulations_do_not_advance_real_round_counter() {
        let mut a = runner();
        let mut b = a.clone();
        a.runtime.run_search_round(SearchConfig {
            samples: 2,
            horizon: 10,
            ..SearchConfig::default()
        });
        b.runtime.run_minimal_round();
        assert_eq!(a.runtime.round, b.runtime.round);
        assert!(a.runtime.scratch.search_turn.is_none());
    }
    #[test]
    fn unselected_team_follows_exact_legacy_state() {
        let mut a = runner();
        let mut b = a.clone();
        for _ in 0..20 {
            a.runtime.run_search_round(SearchConfig {
                team: Some(usize::MAX),
                ..SearchConfig::default()
            });
            b.runtime.run_minimal_round();
        }
        assert_eq!(a.model_state().unwrap(), b.model_state().unwrap());
    }
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            team: None,
            shortlist: 4,
            samples: 16,
            horizon: 128,
            seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchCandidate {
    pub action: PreparedPlainAction,
    pub cost: i32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchTurn {
    pub chosen: Option<usize>,
    pub simulation_seed: Option<u64>,
    pub candidates: Vec<SearchCandidate>,
    pub actor: Option<EntityIdx>,
}
impl SearchTurn {
    pub(crate) fn new(chosen: Option<usize>, simulation_seed: Option<u64>) -> Self {
        Self {
            chosen,
            simulation_seed,
            candidates: Vec::new(),
            actor: None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct SearchEstimate {
    pub action: PreparedPlainAction,
    pub value: f64,
    pub wins: usize,
    pub completed: usize,
    pub samples: usize,
}
#[derive(Debug, Clone)]
pub struct SearchRound {
    pub outcome: RoundOutcome,
    pub estimates: Vec<SearchEstimate>,
}

impl CombatRuntime {
    /// 完整状态启发式，未终局值仅是排序分数，不能解释成胜率。
    pub fn search_position_value(&self, team: usize) -> f64 {
        let mut ally = 0.0;
        let mut enemy = 0.0;
        for (_, entity) in self.entities.iter() {
            let r = &entity.runtime;
            if !r.alive {
                continue;
            }
            let health = f64::from(r.hp.max(0)) / f64::from(entity.template.max_hp.max(1));
            let power = f64::from(r.attack.max(r.magic).max(0));
            let value = 100.0
                + 100.0 * health
                + power * health.sqrt()
                + f64::from(r.magic_point.clamp(0, 128)) * 0.15
                + f64::from(r.move_state.speed_points.clamp(0, 2048)) * 0.005;
            if r.team == team {
                ally += value;
            } else {
                enemy += value;
            }
        }
        (ally - enemy) / (ally + enemy).max(1.0)
    }

    /// 在回合起点复制状态，所有分支仍执行正式回合入口，包括前后置效果。
    pub fn run_search_round(&mut self, config: SearchConfig) -> SearchRound {
        let mut probe = self.clone();
        probe.scratch.search_turn = Some(SearchTurn::new(None, None));
        probe.run_minimal_round_no_capture();
        let turn = probe.scratch.search_turn.take().unwrap_or_else(|| SearchTurn::new(None, None));
        let Some(actor) = turn.actor else {
            return SearchRound {
                outcome: self.run_minimal_round(),
                estimates: Vec::new(),
            };
        };
        let team = probe.entities.get(actor).unwrap().runtime.team;
        if config.team.is_some_and(|selected| selected != team) {
            return SearchRound {
                outcome: self.run_minimal_round(),
                estimates: Vec::new(),
            };
        }
        let mut ranked = Vec::new();
        for index in 0..turn.candidates.len() {
            // 所有候选使用同一批搜索种子，降低比较方差。
            let mut trial = self.clone();
            trial.scratch.search_turn = Some(SearchTurn::new(Some(index), Some(config.seed)));
            let outcome = trial.run_minimal_round_no_capture();
            let score = outcome.winner_team.map_or_else(
                || trial.search_position_value(team),
                |winner| if winner == team { 2.0 } else { -2.0 },
            );
            ranked.push((index, score));
        }
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        ranked.truncate(config.shortlist.max(1));
        let mut estimates = Vec::new();
        let mut best = None;
        for (index, _) in ranked {
            let mut total = 0.0;
            let mut wins = 0;
            let mut completed = 0;
            for sample in 0..config.samples.max(1) {
                let seed = config.seed.wrapping_add((sample as u64).wrapping_mul(0x9e3779b97f4a7c15));
                let mut trial = self.clone();
                trial.scratch.search_turn = Some(SearchTurn::new(Some(index), Some(seed)));
                let mut winner = trial.run_minimal_round_no_capture().winner_team;
                trial.scratch.search_turn = None;
                for _ in 0..config.horizon {
                    if winner.is_some() {
                        break;
                    }
                    // 续局使用原版策略，避免模拟内递归启动搜索。
                    winner = trial.run_minimal_round_no_capture().winner_team;
                }
                total += match winner {
                    Some(winner) => {
                        completed += 1;
                        if winner == team {
                            wins += 1;
                            1.0
                        } else {
                            -1.0
                        }
                    }
                    None => trial.search_position_value(team) * 0.5,
                };
            }
            let value = total / config.samples.max(1) as f64;
            if best.is_none_or(|(_, score)| value > score) {
                best = Some((index, value));
            }
            estimates.push(SearchEstimate {
                action: turn.candidates[index].action.clone(),
                value,
                wins,
                completed,
                samples: config.samples.max(1),
            });
        }
        self.scratch.search_turn = Some(SearchTurn::new(best.map(|(index, _)| index), None));
        let outcome = self.run_minimal_round();
        self.scratch.search_turn = None;
        SearchRound { outcome, estimates }
    }
}

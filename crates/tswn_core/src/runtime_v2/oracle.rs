use crate::engine::update::{DEFAULT_DELAY0_MS, DEFAULT_DELAY1_MS, UpdateType};
use crate::runtime_v2::{CombatRuntime, EntityIdx, PreparedCombatTemplate, RoundOutcome};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NormalizedRngCheckpoint {
    pub i: u32,
    pub j: u32,
    #[cfg(not(feature = "no_debug"))]
    pub byte_count: u64,
}

impl NormalizedRngCheckpoint {
    pub fn from_runtime(runtime: &CombatRuntime) -> Self {
        Self {
            i: runtime.rng.i,
            j: runtime.rng.j,
            #[cfg(not(feature = "no_debug"))]
            byte_count: runtime.rng.byte_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedActionBoundary {
    pub round: u64,
    pub actor: usize,
    pub target: usize,
    pub amount: i32,
}

impl NormalizedActionBoundary {
    pub fn from_outcome(runtime: &CombatRuntime, outcome: &RoundOutcome) -> Vec<Self> {
        outcome
            .action
            .map(|action| {
                vec![Self {
                    round: runtime.round,
                    actor: action.actor.0 as usize,
                    target: action.target.0 as usize,
                    amount: action.amount,
                }]
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUpdateFrame {
    pub message: String,
    pub caster: usize,
    pub target: usize,
    pub targets: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub update_type: UpdateType,
}

impl NormalizedUpdateFrame {
    pub fn from_outcome(outcome: &RoundOutcome) -> Vec<Self> {
        outcome
            .frame
            .as_ref()
            .map(|frame| {
                frame
                    .updates
                    .updates
                    .iter()
                    .map(|update| Self {
                        message: update.message.to_string(),
                        caster: update.caster,
                        target: update.target,
                        targets: update.targets.iter().copied().collect(),
                        param: update.param,
                        score: update.score,
                        delay0: update.delay0,
                        delay1: update.delay1,
                        update_type: update.update_type,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedOutcome {
    pub winner_team: Option<usize>,
    pub round: u64,
    pub total_score: u64,
    pub rng: NormalizedRngCheckpoint,
    pub entity_ids: Vec<usize>,
    pub teams: Vec<usize>,
    pub hp: Vec<i32>,
    pub alive: Vec<bool>,
    pub round_order: Vec<usize>,
    pub flat_alive: Vec<usize>,
    pub team_alive: Vec<Vec<usize>>,
    pub alive_group_count: usize,
    pub actions: Vec<NormalizedActionBoundary>,
    pub frames: Vec<NormalizedUpdateFrame>,
}

impl NormalizedOutcome {
    pub fn from_runtime(runtime: &CombatRuntime, outcome: &RoundOutcome) -> Self {
        let team_count = runtime
            .entities
            .iter()
            .map(|(_, entity)| entity.runtime.team)
            .max()
            .map_or(0, |team| team + 1);
        Self {
            winner_team: outcome.winner_team,
            round: runtime.round,
            total_score: outcome.frame.as_ref().map_or(0, |frame| {
                frame.updates.updates.iter().map(|update| u64::from(update.score)).sum()
            }),
            rng: NormalizedRngCheckpoint::from_runtime(runtime),
            entity_ids: runtime.entities.iter().map(|(_, entity)| entity.template.id).collect(),
            teams: runtime.entities.iter().map(|(_, entity)| entity.runtime.team).collect(),
            hp: runtime.entities.iter().map(|(_, entity)| entity.runtime.hp).collect(),
            alive: runtime.entities.iter().map(|(_, entity)| entity.runtime.alive).collect(),
            round_order: runtime.world.round_order().iter().map(|idx| idx.0 as usize).collect(),
            flat_alive: runtime.world.flat_alive().iter().map(|idx| idx.0 as usize).collect(),
            team_alive: (0..team_count)
                .map(|team| {
                    runtime
                        .world
                        .team_alive(team)
                        .unwrap_or_default()
                        .iter()
                        .map(|idx| idx.0 as usize)
                        .collect()
                })
                .collect(),
            alive_group_count: runtime.world.alive_group_count(),
            actions: NormalizedActionBoundary::from_outcome(runtime, outcome),
            frames: NormalizedUpdateFrame::from_outcome(outcome),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictDiff {
    Winner {
        expected: Option<usize>,
        actual: Option<usize>,
    },
    Round {
        expected: u64,
        actual: u64,
    },
    Score {
        expected: u64,
        actual: u64,
    },
    Rng {
        expected: NormalizedRngCheckpoint,
        actual: NormalizedRngCheckpoint,
    },
    Hp {
        expected: Vec<i32>,
        actual: Vec<i32>,
    },
    Alive {
        expected: Vec<bool>,
        actual: Vec<bool>,
    },
    RoundOrder {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    FlatAlive {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    TeamAlive {
        expected: Vec<Vec<usize>>,
        actual: Vec<Vec<usize>>,
    },
    AliveGroupCount {
        expected: usize,
        actual: usize,
    },
    EntityIds {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    Teams {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    ActionCount {
        expected: usize,
        actual: usize,
    },
    Action {
        index: usize,
        expected: NormalizedActionBoundary,
        actual: NormalizedActionBoundary,
    },
    FrameCount {
        expected: usize,
        actual: usize,
    },
    Frame {
        index: usize,
        expected: NormalizedUpdateFrame,
        actual: NormalizedUpdateFrame,
    },
}

pub fn strict_diff(expected: &NormalizedOutcome, actual: &NormalizedOutcome) -> Result<(), StrictDiff> {
    if expected.winner_team != actual.winner_team {
        return Err(StrictDiff::Winner {
            expected: expected.winner_team,
            actual: actual.winner_team,
        });
    }
    if expected.round != actual.round {
        return Err(StrictDiff::Round {
            expected: expected.round,
            actual: actual.round,
        });
    }
    if expected.total_score != actual.total_score {
        return Err(StrictDiff::Score {
            expected: expected.total_score,
            actual: actual.total_score,
        });
    }
    if expected.rng != actual.rng {
        return Err(StrictDiff::Rng {
            expected: expected.rng.clone(),
            actual: actual.rng.clone(),
        });
    }
    if expected.entity_ids != actual.entity_ids {
        return Err(StrictDiff::EntityIds {
            expected: expected.entity_ids.clone(),
            actual: actual.entity_ids.clone(),
        });
    }
    if expected.teams != actual.teams {
        return Err(StrictDiff::Teams {
            expected: expected.teams.clone(),
            actual: actual.teams.clone(),
        });
    }
    if expected.hp != actual.hp {
        return Err(StrictDiff::Hp {
            expected: expected.hp.clone(),
            actual: actual.hp.clone(),
        });
    }
    if expected.alive != actual.alive {
        return Err(StrictDiff::Alive {
            expected: expected.alive.clone(),
            actual: actual.alive.clone(),
        });
    }
    if expected.round_order != actual.round_order {
        return Err(StrictDiff::RoundOrder {
            expected: expected.round_order.clone(),
            actual: actual.round_order.clone(),
        });
    }
    if expected.flat_alive != actual.flat_alive {
        return Err(StrictDiff::FlatAlive {
            expected: expected.flat_alive.clone(),
            actual: actual.flat_alive.clone(),
        });
    }
    if expected.team_alive != actual.team_alive {
        return Err(StrictDiff::TeamAlive {
            expected: expected.team_alive.clone(),
            actual: actual.team_alive.clone(),
        });
    }
    if expected.alive_group_count != actual.alive_group_count {
        return Err(StrictDiff::AliveGroupCount {
            expected: expected.alive_group_count,
            actual: actual.alive_group_count,
        });
    }
    if expected.actions.len() != actual.actions.len() {
        return Err(StrictDiff::ActionCount {
            expected: expected.actions.len(),
            actual: actual.actions.len(),
        });
    }
    for (index, (expected_action, actual_action)) in expected.actions.iter().zip(&actual.actions).enumerate() {
        if expected_action != actual_action {
            return Err(StrictDiff::Action {
                index,
                expected: expected_action.clone(),
                actual: actual_action.clone(),
            });
        }
    }
    if expected.frames.len() != actual.frames.len() {
        return Err(StrictDiff::FrameCount {
            expected: expected.frames.len(),
            actual: actual.frames.len(),
        });
    }
    for (index, (expected_frame, actual_frame)) in expected.frames.iter().zip(&actual.frames).enumerate() {
        if expected_frame != actual_frame {
            return Err(StrictDiff::Frame {
                index,
                expected: expected_frame.clone(),
                actual: actual_frame.clone(),
            });
        }
    }
    Ok(())
}

pub fn run_minimal_v2_once(template: PreparedCombatTemplate) -> NormalizedOutcome {
    let mut runtime = CombatRuntime::from_template(template);
    let outcome = runtime.run_minimal_round();
    NormalizedOutcome::from_runtime(&runtime, &outcome)
}

pub fn minimal_1v1_expected_after_one_round(left_hp: i32, right_hp: i32, attack: i32) -> NormalizedOutcome {
    let right_alive = right_hp > attack;
    NormalizedOutcome {
        winner_team: (right_hp <= attack).then_some(0),
        round: 1,
        total_score: attack.max(0) as u64,
        rng: NormalizedRngCheckpoint::default(),
        entity_ids: vec![1, 2],
        teams: vec![0, 1],
        hp: vec![left_hp, (right_hp - attack).max(0)],
        alive: vec![true, right_alive],
        round_order: if right_alive { vec![0, 1] } else { vec![0] },
        flat_alive: if right_alive { vec![0, 1] } else { vec![0] },
        team_alive: if right_alive {
            vec![vec![0], vec![1]]
        } else {
            vec![vec![0], vec![]]
        },
        alive_group_count: if right_alive { 2 } else { 1 },
        actions: vec![NormalizedActionBoundary {
            round: 1,
            actor: EntityIdx(0).0 as usize,
            target: EntityIdx(1).0 as usize,
            amount: attack,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: EntityIdx(0).0 as usize,
            target: EntityIdx(1).0 as usize,
            targets: Vec::new(),
            param: None,
            score: attack.max(0) as u32,
            delay0: DEFAULT_DELAY0_MS,
            delay1: DEFAULT_DELAY1_MS,
            update_type: UpdateType::None,
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_diff_harness_accepts_minimal_v2_outcome() {
        let expected = minimal_1v1_expected_after_one_round(10, 10, 3);
        let actual = run_minimal_v2_once(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn strict_diff_harness_reports_first_frame_mismatch() {
        let expected = minimal_1v1_expected_after_one_round(10, 10, 3);
        let mut actual = expected.clone();
        actual.frames[0].param = Some(4);
        actual.frames[0].delay0 = 250;
        actual.frames[0].delay1 = 50;
        actual.frames[0].targets = vec![0, 1];
        actual.frames[0].update_type = UpdateType::NextLine;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::Frame {
                index: 0,
                expected: expected.frames[0].clone(),
                actual: actual.frames[0].clone(),
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_action_boundary_mismatch_before_frames() {
        let expected = minimal_1v1_expected_after_one_round(10, 10, 3);
        let mut actual = expected.clone();
        actual.actions[0].target = 0;
        actual.frames[0].target = 0;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::Action {
                index: 0,
                expected: expected.actions[0].clone(),
                actual: actual.actions[0].clone(),
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_alive_mismatch_after_hp_matches() {
        let expected = minimal_1v1_expected_after_one_round(10, 3, 3);
        let mut actual = expected.clone();
        actual.alive[1] = true;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::Alive {
                expected: vec![true, false],
                actual: vec![true, true],
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_world_view_mismatch_after_alive_matches() {
        let expected = minimal_1v1_expected_after_one_round(10, 3, 3);
        let mut actual = expected.clone();
        actual.round_order = vec![0, 1];
        actual.flat_alive = vec![0, 1];
        actual.team_alive = vec![vec![0], vec![1]];
        actual.alive_group_count = 2;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::RoundOrder {
                expected: vec![0],
                actual: vec![0, 1],
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_flat_alive_before_team_alive() {
        let expected = minimal_1v1_expected_after_one_round(10, 3, 3);
        let mut actual = expected.clone();
        actual.flat_alive = vec![0, 1];
        actual.team_alive = vec![vec![0], vec![1]];
        actual.alive_group_count = 2;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::FlatAlive {
                expected: vec![0],
                actual: vec![0, 1],
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_team_alive_before_group_count() {
        let expected = minimal_1v1_expected_after_one_round(10, 3, 3);
        let mut actual = expected.clone();
        actual.team_alive = vec![vec![0], vec![1]];
        actual.alive_group_count = 2;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::TeamAlive {
                expected: vec![vec![0], vec![]],
                actual: vec![vec![0], vec![1]],
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_alive_group_count_before_actions() {
        let expected = minimal_1v1_expected_after_one_round(10, 3, 3);
        let mut actual = expected.clone();
        actual.alive_group_count = 2;
        actual.actions[0].target = 0;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::AliveGroupCount { expected: 1, actual: 2 })
        );
    }

    #[test]
    fn strict_diff_harness_reports_entity_identity_mismatch_before_hp() {
        let expected = minimal_1v1_expected_after_one_round(10, 10, 3);
        let mut actual = expected.clone();
        actual.entity_ids[1] = 9;
        actual.teams[1] = 0;
        actual.hp[1] = 9;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::EntityIds {
                expected: vec![1, 2],
                actual: vec![1, 9],
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_rng_mismatch_before_entities() {
        let expected = minimal_1v1_expected_after_one_round(10, 10, 3);
        let mut actual = expected.clone();
        actual.rng.i = 1;
        actual.entity_ids[1] = 9;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::Rng {
                expected: expected.rng.clone(),
                actual: actual.rng.clone(),
            })
        );
    }

    #[test]
    fn strict_diff_harness_reports_score_mismatch_before_rng() {
        let expected = minimal_1v1_expected_after_one_round(10, 10, 3);
        let mut actual = expected.clone();
        actual.total_score = 4;
        actual.rng.i = 1;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::Score { expected: 3, actual: 4 })
        );
    }

    #[test]
    fn strict_diff_harness_reports_outcome_mismatch_after_frames_match() {
        let expected = minimal_1v1_expected_after_one_round(10, 3, 3);
        let mut actual = expected.clone();
        actual.winner_team = None;

        assert_eq!(
            strict_diff(&expected, &actual),
            Err(StrictDiff::Winner {
                expected: Some(0),
                actual: None,
            })
        );
    }
}

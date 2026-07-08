use crate::engine::update::{DEFAULT_DELAY0_MS, DEFAULT_DELAY1_MS, UpdateType};
use crate::runtime_v2::{CombatRuntime, EntityIdx, PreparedCombatTemplate, RoundOutcome};

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
    pub entity_ids: Vec<usize>,
    pub teams: Vec<usize>,
    pub hp: Vec<i32>,
    pub alive: Vec<bool>,
    pub actions: Vec<NormalizedActionBoundary>,
    pub frames: Vec<NormalizedUpdateFrame>,
}

impl NormalizedOutcome {
    pub fn from_runtime(runtime: &CombatRuntime, outcome: &RoundOutcome) -> Self {
        Self {
            winner_team: outcome.winner_team,
            round: runtime.round,
            entity_ids: runtime.entities.iter().map(|(_, entity)| entity.template.id).collect(),
            teams: runtime.entities.iter().map(|(_, entity)| entity.template.team).collect(),
            hp: runtime.entities.iter().map(|(_, entity)| entity.runtime.hp).collect(),
            alive: runtime.entities.iter().map(|(_, entity)| entity.runtime.alive).collect(),
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
    Hp {
        expected: Vec<i32>,
        actual: Vec<i32>,
    },
    Alive {
        expected: Vec<bool>,
        actual: Vec<bool>,
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
    NormalizedOutcome {
        winner_team: (right_hp <= attack).then_some(0),
        round: 1,
        entity_ids: vec![1, 2],
        teams: vec![0, 1],
        hp: vec![left_hp, (right_hp - attack).max(0)],
        alive: vec![true, right_hp > attack],
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

use crate::runtime_v2::{CombatRuntime, EntityIdx, PreparedCombatTemplate, RoundOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUpdateFrame {
    pub message: String,
    pub caster: usize,
    pub target: usize,
    pub score: u32,
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
                        score: update.score,
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
    pub hp: Vec<i32>,
    pub frames: Vec<NormalizedUpdateFrame>,
}

impl NormalizedOutcome {
    pub fn from_runtime(runtime: &CombatRuntime, outcome: &RoundOutcome) -> Self {
        Self {
            winner_team: outcome.winner_team,
            round: runtime.round,
            hp: runtime.entities.iter().map(|(_, entity)| entity.runtime.hp).collect(),
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
    if expected.hp != actual.hp {
        return Err(StrictDiff::Hp {
            expected: expected.hp.clone(),
            actual: actual.hp.clone(),
        });
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
        hp: vec![left_hp, (right_hp - attack).max(0)],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: EntityIdx(0).0 as usize,
            target: EntityIdx(1).0 as usize,
            score: attack.max(0) as u32,
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
        actual.frames[0].score = 4;

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

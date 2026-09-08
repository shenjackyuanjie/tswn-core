use super::{BattleOptions, BattleStopReason, driver::BattleDriver};
use crate::cli_api::{CliApiError, CliApiResult};
use crate::runtime::model_state::{BattleModelState, ModelStateError};
use crate::runtime::{PreparedRuntimeRunner, RuntimeRunner};
use serde::{Deserialize, Serialize};

/// 只描述可见帧末边界，不携带回放、图标或玩家展示快照。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleModelFrame {
    pub frame_index: usize,
    pub round_index: usize,
    pub rounds_advanced: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleModelOutcome {
    pub stop_reason: BattleStopReason,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub winner_team_indices: Vec<usize>,
}

/// 与回放会话使用同一推进和终止策略的数据生成会话。
#[derive(Debug)]
pub struct BattleModelSession {
    driver: BattleDriver,
}

impl BattleModelSession {
    pub fn new(raw: &str, options: BattleOptions) -> CliApiResult<Self> {
        Ok(Self {
            driver: BattleDriver::from_raw(raw, options)?,
        })
    }
    /// eval_rq 已固化在 prepared 中，构造时只接收本场 seed 和回合上限。
    pub fn from_prepared(prepared: &PreparedRuntimeRunner, seed: &[String], max_rounds: usize) -> CliApiResult<Self> {
        let runner =
            RuntimeRunner::new_from_prepared_with_seed(prepared, seed).map_err(|e| CliApiError::RunnerInit(e.to_string()))?;
        Ok(Self {
            driver: BattleDriver::new(runner, max_rounds)?,
        })
    }
    pub fn model_state(&self) -> Result<BattleModelState, ModelStateError> { self.driver.model_state() }
    pub fn next_frame(&mut self) -> CliApiResult<Option<BattleModelFrame>> {
        while let Some(step) = self.driver.advance_round()? {
            if let Some(frame_index) = step.frame_index {
                return Ok(Some(BattleModelFrame {
                    frame_index,
                    round_index: step.round_index,
                    rounds_advanced: self.driver.rounds_advanced,
                }));
            }
        }
        Ok(None)
    }
    pub fn is_done(&self) -> bool { self.driver.stop_reason.is_some() }
    pub fn is_failed(&self) -> bool { self.driver.failure.is_some() }
    pub fn result(&self) -> Option<BattleModelOutcome> {
        Some(BattleModelOutcome {
            stop_reason: self.driver.stop_reason?,
            rounds_advanced: self.driver.rounds_advanced,
            frames_emitted: self.driver.frames_emitted,
            winner_team_indices: self.driver.runner.winner_team_indices(),
        })
    }
    #[cfg(any(test, feature = "battle-test-support"))]
    #[doc(hidden)]
    pub fn invalidate_runtime_for_test(&mut self) { self.driver.runner.runtime_mut().skill_handlers = Default::default(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_api::battle::BattleSession;

    #[test]
    fn headless_and_replay_match_at_every_boundary() {
        let overlay = concat!(
            r#"owner@red+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklshadow":10,"sklsummon":10,"sklzombie":10}}"#,
            "\n\ntarget@blue"
        );
        for raw in ["left@red\n\nright@blue", "a\nb\n\nc\nd\n\ne", overlay] {
            for max_rounds in [1, 500] {
                let options = BattleOptions {
                    max_rounds,
                    ..BattleOptions::default()
                };
                let mut replay = BattleSession::new(raw, options).unwrap();
                let mut headless = BattleModelSession::new(raw, options).unwrap();
                let mut untouched = BattleModelSession::new(raw, options).unwrap();
                assert_eq!(replay.model_state().unwrap(), headless.model_state().unwrap());
                loop {
                    let state = headless.model_state().unwrap();
                    assert_eq!(state, headless.model_state().unwrap());
                    let frame = headless.next_frame().unwrap();
                    assert_eq!(frame, untouched.next_frame().unwrap());
                    let display = replay.next_frame().unwrap();
                    assert_eq!(frame.is_some(), display.is_some());
                    if let (Some(frame), Some(display)) = (frame, display) {
                        assert_eq!(
                            (frame.frame_index, frame.round_index),
                            (display.frame_index, display.round_index)
                        );
                    }
                    assert_eq!(replay.model_state().unwrap(), headless.model_state().unwrap());
                    assert_eq!(headless.model_state().unwrap(), untouched.model_state().unwrap());
                    if frame.is_none() {
                        break;
                    }
                }
                let actual = headless.result().unwrap();
                let expected = replay.result().unwrap();
                assert_eq!(actual.stop_reason, expected.stop_reason);
                assert_eq!(actual.rounds_advanced, expected.rounds_advanced);
                assert_eq!(actual.frames_emitted, expected.frames_emitted);
                assert_eq!(actual.winner_team_indices, expected.winner_team_indices);
            }
        }
    }

    #[test]
    fn failure_is_sticky_and_state_export_rejects_failed_session() {
        let mut session = BattleModelSession::new("alpha@red+bed2[3000]\n\nbeta@blue", BattleOptions::default()).unwrap();
        session.invalidate_runtime_for_test();
        let error = session.next_frame().unwrap_err().to_string();
        assert_eq!(error, session.next_frame().unwrap_err().to_string());
        assert!(session.is_failed());
        assert!(session.model_state().is_err());
        assert!(session.result().is_none());
    }

    #[test]
    fn no_progress_counts_entities_and_winner_beats_last_round() {
        let mut session = BattleModelSession::new("a\n\nb", BattleOptions::default()).unwrap();
        for id in session.driver.runner.all_player_ids() {
            session
                .driver
                .runner
                .runtime
                .world
                .remove_round_actor(crate::runtime::EntityIdx(id as u32));
        }
        assert!(session.next_frame().unwrap().is_none());
        assert_eq!(session.result().unwrap().stop_reason, BattleStopReason::NoProgress);
        assert_eq!(session.result().unwrap().rounds_advanced, 32);

        let mut full = BattleModelSession::new("a\n\nb", BattleOptions::default()).unwrap();
        while full.next_frame().unwrap().is_some() {}
        let expected = full.result().unwrap();
        assert_eq!(expected.stop_reason, BattleStopReason::Winner);
        let mut limited = BattleModelSession::new(
            "a\n\nb",
            BattleOptions {
                max_rounds: expected.rounds_advanced,
                ..BattleOptions::default()
            },
        )
        .unwrap();
        while limited.next_frame().unwrap().is_some() {}
        assert_eq!(limited.result().unwrap(), expected);
    }
}

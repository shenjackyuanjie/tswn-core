use super::{BattleOptions, BattleStopReason};
use crate::cli_api::{CliApiError, CliApiResult, invalid_input};
use crate::runtime::model_state::{BattleModelState, ModelStateError};
use crate::runtime::{RunUpdates, RuntimeRunner};

/// 两种会话共享的逐回合驱动；展示层仍在每个空回合后更新差分快照。
#[derive(Debug)]
pub(super) struct BattleDriver {
    pub runner: RuntimeRunner,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub stop_reason: Option<BattleStopReason>,
    pub failure: Option<CliApiError>,
    max_rounds: usize,
    no_progress_rounds: usize,
}

pub(super) struct RoundStep {
    pub updates: RunUpdates,
    pub round_index: usize,
    pub frame_index: Option<usize>,
}

impl BattleDriver {
    pub fn from_raw(raw: &str, options: BattleOptions) -> CliApiResult<Self> {
        if raw.trim().is_empty() {
            return Err(invalid_input("raw_input is empty"));
        }
        if options.max_rounds == 0 {
            return Err(CliApiError::InvalidArgument("battle max_rounds must be positive".into()));
        }
        if !options.eval_rq.is_finite() {
            return Err(CliApiError::InvalidArgument("battle eval_rq must be finite".into()));
        }
        let (groups, seed) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
        let runner = RuntimeRunner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, options.eval_rq)
            .map_err(|e| CliApiError::RunnerInit(e.to_string()))?;
        Self::new(runner, options.max_rounds)
    }

    pub fn new(runner: RuntimeRunner, max_rounds: usize) -> CliApiResult<Self> {
        if max_rounds == 0 {
            return Err(CliApiError::InvalidArgument("battle max_rounds must be positive".into()));
        }
        Ok(Self {
            stop_reason: runner.have_winner().then_some(BattleStopReason::Winner),
            runner,
            max_rounds,
            rounds_advanced: 0,
            frames_emitted: 0,
            no_progress_rounds: 0,
            failure: None,
        })
    }

    pub fn model_state(&self) -> Result<BattleModelState, ModelStateError> {
        if let Some(error) = &self.failure {
            return Err(ModelStateError(error.to_string()));
        }
        self.runner.model_state()
    }

    pub fn advance_round(&mut self) -> CliApiResult<Option<RoundStep>> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.stop_reason.is_some() {
            return Ok(None);
        }
        if let Err(error) = self.runner.validate_ready() {
            let error = CliApiError::Runtime(error.to_string());
            self.failure = Some(error.clone());
            return Err(error);
        }
        let round_index = self.rounds_advanced;
        let updates = self.runner.main_round();
        self.rounds_advanced += 1;
        let visible = !updates.updates.is_empty() || self.runner.have_winner();
        self.no_progress_rounds = if visible { 0 } else { self.no_progress_rounds + 1 };
        self.stop_reason = if self.runner.have_winner() {
            Some(BattleStopReason::Winner)
        } else if self.rounds_advanced >= self.max_rounds {
            Some(BattleStopReason::MaxRounds)
        } else if self.no_progress_rounds >= self.runner.runtime.entities.iter().count().max(1).saturating_mul(16) {
            Some(BattleStopReason::NoProgress)
        } else {
            None
        };
        let frame_index = visible.then(|| {
            let index = self.frames_emitted;
            self.frames_emitted += 1;
            index
        });
        Ok(Some(RoundStep {
            updates,
            round_index,
            frame_index,
        }))
    }
}

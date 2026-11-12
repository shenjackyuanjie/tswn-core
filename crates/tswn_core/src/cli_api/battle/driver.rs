use super::{BattleOptions, BattleStopReason};
use crate::cli_api::{CliApiError, CliApiResult, invalid_input};
use crate::runtime::model_state::{BattleModelState, ModelStateError};
use crate::runtime::{RunUpdates, RuntimeRunner};

/// 两种会话共享的逐回合驱动；展示层仍在每个空回合后更新差分快照。
#[derive(Debug)]
pub(super) struct BattleDriver {
    pub runner: RuntimeRunner,
    /// cards 的队列和暂停标记必须属于会话，不能放在全局 TLS 中长期保存。
    pub forced: crate::runtime::forced_action::ForcedState,
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
        let mut runner = RuntimeRunner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, options.eval_rq)
            .map_err(|e| CliApiError::RunnerInit(e.to_string()))?;
        // 无效控制队伍必须拒绝，双方控制使用既有的最大 u32 标记。
        if let Some(team) = options.controlled_team {
            if team != u32::MAX as usize && !runner.runtime().entities.iter().any(|(_, e)| e.runtime.team == team) {
                return Err(CliApiError::InvalidArgument("controlled_team does not exist".into()));
            }
        }
        if let Some(pct) = options.start_hp_pct {
            // 百分比开局生命只服务 cards 的受控队伍；无队伍时不得悄悄改双方状态。
            if !pct.is_finite() || pct <= 0.0 || options.controlled_team.is_none() {
                return Err(CliApiError::InvalidArgument(
                    "start_hp_pct must be finite and positive; bonuses require controlled_team".into(),
                ));
            }
            let team = options.controlled_team;
            let runtime = runner.runtime_mut();
            let ids: Vec<_> = runtime
                .entities
                .iter()
                .filter(|(_, entity)| entity.runtime.alive && (team.is_none() || team == Some(entity.runtime.team)))
                .map(|(id, _)| id)
                .collect();
            for id in ids {
                let entity = runtime.entities.get_mut(id).expect("controlled start-hp target disappeared");
                entity.runtime.hp = (entity.template.max_hp as f64 * pct).round().clamp(1.0, i32::MAX as f64) as i32;
            }
        }
        // 创建时接管暂存指令；会话之间不共享后续动作。
        let mut forced = crate::runtime::forced_action::ForcedState::default();
        forced.queue = crate::runtime::forced_action::take_pending_actions();
        forced.controlled_team = options.controlled_team;
        Self::new_with_forced(runner, options.max_rounds, forced)
    }

    pub fn new(runner: RuntimeRunner, max_rounds: usize) -> CliApiResult<Self> {
        Self::new_with_forced(runner, max_rounds, crate::runtime::forced_action::ForcedState::default())
    }

    fn new_with_forced(
        runner: RuntimeRunner,
        max_rounds: usize,
        forced: crate::runtime::forced_action::ForcedState,
    ) -> CliApiResult<Self> {
        if max_rounds == 0 {
            return Err(CliApiError::InvalidArgument("battle max_rounds must be positive".into()));
        }
        Ok(Self {
            stop_reason: runner.have_winner().then_some(BattleStopReason::Winner),
            runner,
            forced,
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
        // 暂停不能消耗行动条或随机流；补交指令后需要重放同一待行动作。
        let checkpoint = self.forced.controlled_team.map(|_| (self.runner.clone(), self.forced.clone()));
        let updates = crate::runtime::forced_action::with_state(&mut self.forced, || self.runner.main_round());
        if self.forced.paused_actor.is_some() {
            if let Some((runner, forced)) = checkpoint {
                let paused_actor = self.forced.paused_actor;
                self.runner = runner;
                self.forced = forced;
                self.forced.paused_actor = paused_actor;
            }
            return Ok(None);
        }
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

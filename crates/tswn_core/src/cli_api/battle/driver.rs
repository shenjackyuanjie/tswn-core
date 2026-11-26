use super::{BattleOptions, BattleStopReason};
use crate::cli_api::{CliApiError, CliApiResult, invalid_input};
use crate::runtime::model_state::{BattleModelState, ModelStateError};
use crate::runtime::{ForcedAction, PlrId, RunUpdates, RuntimeRunner, forced_action};

/// 两种会话共享的逐回合驱动；展示层仍在每个空回合后更新差分快照。
#[derive(Debug)]
pub struct BattleDriver {
    pub runner: RuntimeRunner,
    /// cards 的能力卡快照；指令队列与暂停标记在 runtime 的 control 上，随 runner 一起回滚。
    pub forced: forced_action::ForcedState,
    /// 因缺指令暂停的实体与强制行动原因；补交指令后清空。
    pub paused: Option<(PlrId, Option<ForcedAction>)>,
    /// 主动暂停发生时的只读预览：保留行动前护盾、行动条和随机位置供面板查询。
    /// `runner` 仍保存回合前检查点，续行时只执行一次行动前逻辑。
    pub paused_view: Option<RuntimeRunner>,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub stop_reason: Option<BattleStopReason>,
    pub failure: Option<CliApiError>,
    /// 可选 Clever 搜索配置，普通对局保持原版推进。
    pub search: Option<crate::runtime::search::SearchConfig>,
    max_rounds: usize,
    no_progress_rounds: usize,
}

/// 单步推进结果：本回合更新集合、回合序号与可选帧序号。
pub struct RoundStep {
    pub updates: RunUpdates,
    pub round_index: usize,
    pub frame_index: Option<usize>,
}

impl BattleDriver {
    /// 从 namerena 文本构建驱动：校验输入非空、`max_rounds` 为正、`eval_rq` 有限，
    /// 解析队伍与 seed 后构造 `RuntimeRunner`。
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
        let runner = RuntimeRunner::new_from_groups_with_options(&groups, &seed, options.eval_rq, options.team_upgrades)
            .map_err(|e| CliApiError::RunnerInit(e.to_string()))?;
        Self::new(runner, options.max_rounds)
    }

    /// 用已构造的 `RuntimeRunner` 创建驱动；`max_rounds` 必须为正。
    /// 构造时若对局已产生胜者，直接记 `Winner` 停止原因。
    pub fn new(runner: RuntimeRunner, max_rounds: usize) -> CliApiResult<Self> {
        if max_rounds == 0 {
            return Err(CliApiError::InvalidArgument("battle max_rounds must be positive".into()));
        }
        Ok(Self {
            stop_reason: runner.have_winner().then_some(BattleStopReason::Winner),
            runner,
            forced: forced_action::ForcedState::default(),
            paused: None,
            paused_view: None,
            max_rounds,
            rounds_advanced: 0,
            frames_emitted: 0,
            no_progress_rounds: 0,
            failure: None,
            search: None,
        })
    }

    /// 取当前机制状态快照；已发生 sticky failure 时返回该错误而不是快照。
    pub fn model_state(&self) -> Result<BattleModelState, ModelStateError> {
        if let Some(error) = &self.failure {
            return Err(ModelStateError(error.to_string()));
        }
        self.runner.model_state()
    }

    /// 取当前供查询的 runner：主动暂停时返回行动前预览，否则返回真实 runner。
    pub fn observed_runner(&self) -> &RuntimeRunner { self.paused_view.as_ref().unwrap_or(&self.runner) }

    /// [`observed_runner`](Self::observed_runner) 的可变版本，供调用方修改预览或真实 runner。
    pub fn observed_runner_mut(&mut self) -> &mut RuntimeRunner { self.paused_view.as_mut().unwrap_or(&mut self.runner) }

    /// 推进一个 Runtime 回合。
    ///
    /// 受控对局可能停在半个回合里，因此在推进前留检查点：一旦有单位缺指令，整回合回滚，
    /// 只保留暂停标记。补交指令后再调一次即重放同一行动，行动条与随机流都不被消耗。
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
        let checkpoint = (!self.runner.runtime().control.controlled_teams().is_empty()).then(|| self.runner.clone());
        // 机制层按队伍查询能力卡，进入推进前把会话快照借给 Runtime。
        self.forced.controlled_team = self.runner.runtime().control.controlled_team();
        let updates = forced_action::with_state(&mut self.forced, || {
            if let Some(config) = self.search {
                self.runner
                    .runtime_mut()
                    .run_search_round(config)
                    .outcome
                    .frame
                    .map_or_else(RunUpdates::new, |frame| frame.updates)
            } else {
                self.runner.main_round_prevalidated()
            }
        });
        if let Some(actor) = self.runner.runtime().control.awaiting() {
            let forced = self.runner.runtime().control.awaiting_forced_action();
            if let Some(runner) = checkpoint {
                self.paused_view = Some(std::mem::replace(&mut self.runner, runner));
            }
            self.paused = Some((actor, forced));
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

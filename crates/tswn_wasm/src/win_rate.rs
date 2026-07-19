//! WASM 胜率计算逻辑。
//!
//! 提供 `WinRateSession`（分批次步进执行，支持进度轮询）及 `run_win_rate_sync`
//! 一次性同步函数，计算第一组玩家对其余组的胜率百分比。

use tswn_core::Runner;
use tswn_core::runtime::{PreparedRuntimeRunner, default_custom_runtime_import_config, prepared_runtime_win_rate_range};
use wasm_bindgen::prelude::*;

use crate::error::{WasmResult, invalid_input, runner_init_failed, win_rate_invalid_groups};
use crate::model::{WinRateOptions, WinRateProgress, WinRateResult, WinRateTiming};

fn build_prepared_runner(raw_input: String, eval_rq: f64) -> WasmResult<PreparedRuntimeRunner> {
    if raw_input.trim().is_empty() {
        return Err(invalid_input("rawInput is empty"));
    }

    let (groups, _) = Runner::split_namerena_into_groups(raw_input);
    let group_count = groups.iter().filter(|group| !group.is_empty()).count();
    if group_count < 2 {
        return Err(win_rate_invalid_groups());
    }

    let config = default_custom_runtime_import_config().map_err(|error| runner_init_failed(format!("{error:?}")))?;
    PreparedRuntimeRunner::from_custom_mixed_roster_with_eval_rq(&groups, eval_rq, config)
        .map_err(|error| runner_init_failed(format!("{error:?}")))
}

fn nanos_to_u64(value: u128) -> u64 { u64::try_from(value).unwrap_or(u64::MAX) }

#[wasm_bindgen]
pub struct WinRateSession {
    prepared: PreparedRuntimeRunner,
    total_rounds: usize,
    next_round: usize,
    wins: usize,
    eval_rq: f64,
    init_nanos: u64,
    fight_nanos: u64,
}

impl WinRateSession {
    fn progress_value(&self) -> WinRateProgress {
        let rounds_done = self.next_round;
        let percent = if rounds_done == 0 {
            0.0
        } else {
            self.wins as f64 * 100.0 / rounds_done as f64
        };

        WinRateProgress {
            done: rounds_done >= self.total_rounds,
            rounds_done,
            total_rounds: self.total_rounds,
            wins: self.wins,
            percent,
        }
    }

    pub fn result_value(&self) -> WinRateResult {
        let progress = self.progress_value();
        WinRateResult {
            done: progress.done,
            rounds_done: progress.rounds_done,
            total_rounds: progress.total_rounds,
            wins: progress.wins,
            percent: progress.percent,
            timing: Some(WinRateTiming {
                init_nanos: self.init_nanos,
                fight_nanos: self.fight_nanos,
            }),
        }
    }

    fn step_internal(&mut self, batch_size: usize) -> WasmResult<WinRateProgress> {
        let batch_size = batch_size.max(1);
        let batch_end = self.total_rounds.min(self.next_round.saturating_add(batch_size));
        let summary = prepared_runtime_win_rate_range(&self.prepared, self.next_round, batch_end)
            .map_err(|error| runner_init_failed(error.to_string()))?;
        self.wins += summary.wins;
        self.init_nanos = self.init_nanos.saturating_add(nanos_to_u64(summary.timing.init_nanos));
        self.fight_nanos = self.fight_nanos.saturating_add(nanos_to_u64(summary.timing.fight_nanos));

        self.next_round = batch_end;
        Ok(self.progress_value())
    }

    pub fn new_internal(raw_input: String, total_rounds: usize, options: WinRateOptions) -> WasmResult<Self> {
        let eval_rq = options.resolved_eval_rq();
        let _thread = options.resolved_thread();
        let prepared = build_prepared_runner(raw_input, eval_rq)?;
        Ok(Self {
            prepared,
            total_rounds,
            next_round: 0,
            wins: 0,
            eval_rq,
            init_nanos: 0,
            fight_nanos: 0,
        })
    }
}

pub fn run_win_rate_sync(raw_input: String, total_rounds: usize, options: WinRateOptions) -> WasmResult<WinRateResult> {
    let mut session = WinRateSession::new_internal(raw_input, total_rounds, options)?;
    let _ = session.step_internal(total_rounds.max(1))?;
    Ok(session.result_value())
}

#[wasm_bindgen]
impl WinRateSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, total_rounds: usize, options: Option<WinRateOptions>) -> WasmResult<WinRateSession> {
        crate::install_panic_hook();
        let options = options.unwrap_or_default();
        Self::new_internal(raw_input, total_rounds, options)
    }

    pub fn is_finished(&self) -> bool { self.next_round >= self.total_rounds }

    pub fn progress(&self) -> WinRateProgress { self.progress_value() }

    pub fn step(&mut self, batch_size: Option<usize>) -> WasmResult<WinRateProgress> {
        self.step_internal(batch_size.unwrap_or(100))
    }

    pub fn result(&self) -> WinRateResult { self.result_value() }

    pub fn eval_rq(&self) -> f64 { self.eval_rq }
}

//! 应用全局状态定义。
//!
//! 定义工具枚举 [`Tool`] 及各工具的独立状态结构体（`ToDiyState`、`NamerPfState` 等），
//! 以及聚合所有状态的顶层 [`OpenboxApp`] 结构体。

use std::sync::mpsc::Receiver;
use std::time::Instant;

use crate::backend::{OutputMode, ProgressEvent};

use super::source::TextSource;
use super::widgets::{BenchOutputConfig, OptionalFileOutput};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    ToDiy,
    NamerPf,
    BatchRate,
    Pair,
}

impl Tool {
    pub(crate) const ALL: [Self; 4] = [Self::ToDiy, Self::NamerPf, Self::BatchRate, Self::Pair];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ToDiy => "to-diy",
            Self::NamerPf => "namer-pf",
            Self::BatchRate => "batch-rate",
            Self::Pair => "pair",
        }
    }
}

pub(crate) struct ToDiyState {
    pub(crate) names: TextSource,
    pub(crate) old: bool,
    pub(crate) details: bool,
    pub(crate) output: OptionalFileOutput,
}

impl Default for ToDiyState {
    fn default() -> Self {
        Self {
            names: TextSource::inline("mario@team+fire"),
            old: false,
            details: true,
            output: OptionalFileOutput::default(),
        }
    }
}

pub(crate) struct NamerPfState {
    pub(crate) names: TextSource,
    pub(crate) count: usize,
    pub(crate) threads: usize,
    pub(crate) output: OptionalFileOutput,
}

impl Default for NamerPfState {
    fn default() -> Self {
        Self {
            names: TextSource::inline("mario\nluigi+peach"),
            count: 1000,
            threads: 0,
            output: OptionalFileOutput::default(),
        }
    }
}

pub(crate) struct BatchRateState {
    pub(crate) targets: TextSource,
    pub(crate) players: TextSource,
    pub(crate) count: usize,
    pub(crate) threads: usize,
    pub(crate) keep_rq: bool,
    pub(crate) verbose: bool,
    pub(crate) perf: bool,
    pub(crate) double_plus: bool,
    pub(crate) output: BenchOutputConfig,
}

impl Default for BatchRateState {
    fn default() -> Self {
        Self {
            targets: TextSource::inline("luigi\npeach"),
            players: TextSource::inline("mario\nbowser"),
            count: 1000,
            threads: 0,
            keep_rq: false,
            verbose: false,
            perf: false,
            double_plus: false,
            output: BenchOutputConfig {
                file_output: OptionalFileOutput::default(),
                mode: OutputMode::Log,
                min_screen: String::new(),
                min_file: String::new(),
                precision: 3,
            },
        }
    }
}

pub(crate) struct PairState {
    pub(crate) targets: TextSource,
    pub(crate) players: TextSource,
    pub(crate) teammates: TextSource,
    pub(crate) head: usize,
    pub(crate) count: usize,
    pub(crate) threads: usize,
    pub(crate) keep_rq: bool,
    pub(crate) verbose: bool,
    pub(crate) perf: bool,
    pub(crate) output: BenchOutputConfig,
}

impl Default for PairState {
    fn default() -> Self {
        Self {
            targets: TextSource::inline("luigi\npeach"),
            players: TextSource::inline("mario\nbowser"),
            teammates: TextSource::inline("yoshi\ntoad"),
            head: 3,
            count: 1000,
            threads: 0,
            keep_rq: false,
            verbose: false,
            perf: false,
            output: BenchOutputConfig {
                file_output: OptionalFileOutput::default(),
                mode: OutputMode::Log,
                min_screen: String::new(),
                min_file: String::new(),
                precision: 3,
            },
        }
    }
}

pub(crate) struct OpenboxApp {
    pub(crate) tool: Tool,
    pub(crate) log: String,
    pub(crate) status: String,
    pub(crate) running: bool,
    pub(crate) done: usize,
    pub(crate) total: usize,
    pub(crate) started_at: Option<Instant>,
    pub(crate) rate_text: String,
    pub(crate) eta_text: String,
    pub(crate) rx: Option<Receiver<ProgressEvent>>,
    pub(crate) to_diy: ToDiyState,
    pub(crate) namer_pf: NamerPfState,
    pub(crate) batch_rate: BatchRateState,
    pub(crate) pair: PairState,
}

impl Default for OpenboxApp {
    fn default() -> Self {
        Self {
            tool: Tool::ToDiy,
            log: String::new(),
            status: "就绪".to_string(),
            running: false,
            done: 0,
            total: 0,
            started_at: None,
            rate_text: "--".to_string(),
            eta_text: "--".to_string(),
            rx: None,
            to_diy: ToDiyState::default(),
            namer_pf: NamerPfState::default(),
            batch_rate: BatchRateState::default(),
            pair: PairState::default(),
        }
    }
}

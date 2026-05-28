//! 应用全局状态定义。
//!
//! 定义工具枚举 [`Tool`] 及各工具的独立状态结构体（`ToDiyState`、`NamerPfState` 等），
//! 以及聚合所有状态的顶层 [`OpenboxApp`] 结构体。

use std::collections::HashSet;
use std::sync::{Arc, atomic::AtomicBool, mpsc::Receiver};
use std::time::Instant;

use eframe::egui;

use crate::backend::{NamerPfMetric, OutputMode, PairDetailMode, ProgressEvent};

use super::source::TextSource;
use super::target_presets::{TargetPresetState, TeammatePresetState};
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
            Self::BatchRate => "cqd/cqp",
            Self::Pair => "pair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CountMode {
    Accuracy,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccuracyPreset {
    One,
    Ten,
    Hundred,
}

impl AccuracyPreset {
    pub(crate) const ALL: [Self; 3] = [Self::One, Self::Ten, Self::Hundred];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::One => "1%",
            Self::Ten => "10%",
            Self::Hundred => "100%",
        }
    }

    pub(crate) fn count(self) -> usize {
        match self {
            Self::One => 100,
            Self::Ten => 1000,
            Self::Hundred => 10000,
        }
    }
}

pub(crate) struct ToDiyState {
    pub(crate) names: TextSource,
    pub(crate) old: bool,
    pub(crate) minions: bool,
    pub(crate) details: bool,
    pub(crate) output: OptionalFileOutput,
}

impl Default for ToDiyState {
    fn default() -> Self {
        Self {
            names: TextSource::inline("mario@team+fire"),
            old: false,
            minions: false,
            details: true,
            output: OptionalFileOutput::default(),
        }
    }
}

pub(crate) struct NamerPfState {
    pub(crate) names: TextSource,
    pub(crate) count_mode: CountMode,
    pub(crate) accuracy: AccuracyPreset,
    pub(crate) count: usize,
    pub(crate) auto_threads: bool,
    pub(crate) threads: usize,
    pub(crate) keep_rq: bool,
    pub(crate) metrics: Vec<NamerPfMetricState>,
}

pub(crate) struct NamerPfMetricState {
    pub(crate) metric: NamerPfMetric,
    pub(crate) screen: bool,
    pub(crate) min_screen: String,
    pub(crate) highlight_delta: String,
    pub(crate) file_output: OptionalFileOutput,
    pub(crate) min_file: String,
}

impl Default for NamerPfState {
    fn default() -> Self {
        Self {
            names: TextSource::inline("mario\nluigi+peach"),
            count_mode: CountMode::Accuracy,
            accuracy: AccuracyPreset::Hundred,
            count: AccuracyPreset::Hundred.count(),
            auto_threads: true,
            threads: 0,
            keep_rq: false,
            metrics: NamerPfMetric::ALL
                .into_iter()
                .map(|metric| NamerPfMetricState {
                    metric,
                    screen: true,
                    min_screen: String::new(),
                    highlight_delta: default_namer_pf_highlight_delta(metric).to_string(),
                    file_output: OptionalFileOutput::default(),
                    min_file: String::new(),
                })
                .collect(),
        }
    }
}

pub(crate) struct BatchRateState {
    pub(crate) targets: TextSource,
    pub(crate) target_presets: TargetPresetState,
    pub(crate) manual_targets: bool,
    pub(crate) players: TextSource,
    pub(crate) count_mode: CountMode,
    pub(crate) accuracy: AccuracyPreset,
    pub(crate) count: usize,
    pub(crate) auto_threads: bool,
    pub(crate) threads: usize,
    pub(crate) keep_rq: bool,
    pub(crate) double_plus: bool,
    pub(crate) show_matchups: bool,
    pub(crate) highlight_delta: String,
    pub(crate) output: BenchOutputConfig,
}

impl Default for BatchRateState {
    fn default() -> Self {
        Self {
            targets: TextSource::inline("luigi\npeach"),
            target_presets: TargetPresetState::load(),
            manual_targets: false,
            players: TextSource::inline("mario\nbowser"),
            count_mode: CountMode::Accuracy,
            accuracy: AccuracyPreset::Hundred,
            count: AccuracyPreset::Hundred.count(),
            auto_threads: true,
            threads: 0,
            keep_rq: true,
            double_plus: false,
            show_matchups: false,
            highlight_delta: "1".to_string(),
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
    pub(crate) target_presets: TargetPresetState,
    pub(crate) manual_targets: bool,
    pub(crate) players: TextSource,
    pub(crate) teammates: TextSource,
    pub(crate) teammate_presets: TeammatePresetState,
    pub(crate) manual_teammates: bool,
    pub(crate) head: usize,
    pub(crate) count_mode: CountMode,
    pub(crate) accuracy: AccuracyPreset,
    pub(crate) count: usize,
    pub(crate) auto_threads: bool,
    pub(crate) threads: usize,
    pub(crate) keep_rq: bool,
    pub(crate) detail_mode: PairDetailMode,
    pub(crate) detail_min: String,
    pub(crate) highlight_delta: String,
    pub(crate) output: BenchOutputConfig,
}

impl Default for PairState {
    fn default() -> Self {
        Self {
            targets: TextSource::inline("luigi\npeach"),
            target_presets: TargetPresetState::load_with_preferred_id(Some(2)),
            manual_targets: false,
            players: TextSource::inline("mario\nbowser"),
            teammates: TextSource::inline("yoshi\ntoad"),
            teammate_presets: TeammatePresetState::load(),
            manual_teammates: false,
            head: 3,
            count_mode: CountMode::Accuracy,
            accuracy: AccuracyPreset::Ten,
            count: AccuracyPreset::Ten.count(),
            auto_threads: true,
            threads: 0,
            keep_rq: true,
            detail_mode: PairDetailMode::None,
            detail_min: String::new(),
            highlight_delta: "4".to_string(),
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

fn default_namer_pf_highlight_delta(metric: NamerPfMetric) -> u64 { if metric == NamerPfMetric::Sum { 300 } else { 100 } }

pub(crate) struct OpenboxApp {
    pub(crate) theme_preference: egui::ThemePreference,
    pub(crate) tool: Tool,
    pub(crate) more_settings_open: bool,
    pub(crate) log: String,
    pub(crate) highlight_lines: HashSet<usize>,
    pub(crate) status: String,
    pub(crate) running: bool,
    pub(crate) cancel_requested: bool,
    pub(crate) cancel_token: Option<Arc<AtomicBool>>,
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
            theme_preference: egui::ThemePreference::System,
            tool: Tool::ToDiy,
            more_settings_open: false,
            log: String::new(),
            highlight_lines: HashSet::new(),
            status: "就绪".to_string(),
            running: false,
            cancel_requested: false,
            cancel_token: None,
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

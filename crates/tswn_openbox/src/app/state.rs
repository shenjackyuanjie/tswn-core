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
pub enum Tool {
    ToDiy,
    NamerPf,
    BatchRate,
    Pair,
}

impl Tool {
    pub const ALL: [Self; 4] = [Self::ToDiy, Self::NamerPf, Self::BatchRate, Self::Pair];

    pub fn label(self) -> &'static str {
        match self {
            Self::ToDiy => "to-diy",
            Self::NamerPf => "namer-pf",
            Self::BatchRate => "cqd/cqp",
            Self::Pair => "pair",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CountMode {
    Accuracy,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccuracyPreset {
    One,
    Ten,
    Hundred,
}

impl AccuracyPreset {
    pub const ALL: [Self; 3] = [Self::One, Self::Ten, Self::Hundred];

    pub fn label(self) -> &'static str {
        match self {
            Self::One => "1%",
            Self::Ten => "10%",
            Self::Hundred => "100%",
        }
    }

    pub fn count(self) -> usize {
        match self {
            Self::One => 100,
            Self::Ten => 1000,
            Self::Hundred => 10000,
        }
    }
}

pub struct ToDiyState {
    pub names: TextSource,
    pub old: bool,
    pub minions: bool,
    pub details: bool,
    pub output: OptionalFileOutput,
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

pub struct NamerPfState {
    pub names: TextSource,
    pub count_mode: CountMode,
    pub accuracy: AccuracyPreset,
    pub count: usize,
    pub auto_threads: bool,
    pub threads: usize,
    pub keep_rq: bool,
    pub metrics: Vec<NamerPfMetricState>,
    pub skill_board: NamerPfSkillBoardState,
}

pub struct NamerPfMetricState {
    pub metric: NamerPfMetric,
    pub screen: bool,
    pub min_screen: String,
    pub highlight_delta: String,
    pub file_output: OptionalFileOutput,
    pub min_file: String,
}

pub struct NamerPfSkillBoardState {
    pub screen: bool,
    pub file_output: OptionalFileOutput,
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
            skill_board: NamerPfSkillBoardState {
                screen: false,
                file_output: OptionalFileOutput::default(),
            },
        }
    }
}

pub struct BatchRateState {
    pub targets: TextSource,
    pub target_presets: TargetPresetState,
    pub manual_targets: bool,
    pub players: TextSource,
    pub count_mode: CountMode,
    pub accuracy: AccuracyPreset,
    pub count: usize,
    pub auto_threads: bool,
    pub threads: usize,
    pub keep_rq: bool,
    pub double_plus: bool,
    pub show_matchups: bool,
    pub highlight_delta: String,
    pub output: BenchOutputConfig,
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

pub struct PairState {
    pub targets: TextSource,
    pub target_presets: TargetPresetState,
    pub manual_targets: bool,
    pub players: TextSource,
    pub teammates: TextSource,
    pub teammate_presets: TeammatePresetState,
    pub manual_teammates: bool,
    pub head: usize,
    pub count_mode: CountMode,
    pub accuracy: AccuracyPreset,
    pub count: usize,
    pub auto_threads: bool,
    pub threads: usize,
    pub keep_rq: bool,
    pub detail_mode: PairDetailMode,
    pub detail_min: String,
    pub highlight_delta: String,
    pub output: BenchOutputConfig,
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

pub struct OpenboxApp {
    pub theme_preference: egui::ThemePreference,
    pub tool: Tool,
    pub more_settings_open: bool,
    pub log: String,
    pub highlight_lines: HashSet<usize>,
    pub skill_board_lines: HashSet<usize>,
    pub status: String,
    pub running: bool,
    pub cancel_requested: bool,
    pub cancel_token: Option<Arc<AtomicBool>>,
    pub done: usize,
    pub total: usize,
    pub started_at: Option<Instant>,
    pub rate_text: String,
    pub eta_text: String,
    pub rx: Option<Receiver<ProgressEvent>>,
    pub to_diy: ToDiyState,
    pub namer_pf: NamerPfState,
    pub batch_rate: BatchRateState,
    pub pair: PairState,
}

impl Default for OpenboxApp {
    fn default() -> Self {
        Self {
            theme_preference: egui::ThemePreference::System,
            tool: Tool::ToDiy,
            more_settings_open: false,
            log: String::new(),
            highlight_lines: HashSet::new(),
            skill_board_lines: HashSet::new(),
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

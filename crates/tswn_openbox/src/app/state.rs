//! 应用全局状态定义。
//!
//! 定义工具枚举 [`Tool`] 及各工具的独立状态结构体（`ToDiyState`、`NamerPfState` 等），
//! 以及聚合所有状态的顶层 [`OpenboxApp`] 结构体。

use std::sync::{Arc, atomic::AtomicBool};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tswn_openbox::backend::{NamerPfMetric, OutputMode, PairDetailMode};

use super::help::HelpTopic;
use super::log::LogBuffer;

use super::source::TextSource;
use super::widgets::{BenchOutputConfig, OptionalFileOutput};
use tswn_openbox::presets::{TargetPresetState, TeammatePresetState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    ToDiy,
    NamerPf,
    BatchRate,
    Pair,
    Ds4,
}

impl Tool {
    pub const ALL: [Self; 5] = [Self::ToDiy, Self::NamerPf, Self::BatchRate, Self::Pair, Self::Ds4];

    pub fn label(self) -> &'static str {
        match self {
            Self::ToDiy => "to-diy",
            Self::NamerPf => "namer-pf",
            Self::BatchRate => "cqd/cqp",
            Self::Pair => "pair",
            Self::Ds4 => "DS4",
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
    pub precision: usize,
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
            precision: 0,
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
    pub manual_target_double_plus: bool,
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
            manual_target_double_plus: false,
            players: TextSource::inline("mario\nbowser"),
            count_mode: CountMode::Accuracy,
            accuracy: AccuracyPreset::Hundred,
            count: AccuracyPreset::Hundred.count(),
            auto_threads: true,
            threads: 0,
            keep_rq: true,
            double_plus: false,
            show_matchups: true,
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
    pub player_double_plus: bool,
    pub teammates: TextSource,
    pub teammate_presets: TeammatePresetState,
    pub manual_teammates: bool,
    pub teammate_double_plus: bool,
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
            player_double_plus: false,
            teammates: TextSource::inline("yoshi\ntoad"),
            teammate_presets: TeammatePresetState::load(),
            manual_teammates: false,
            teammate_double_plus: true,
            head: 3,
            count_mode: CountMode::Accuracy,
            accuracy: AccuracyPreset::Ten,
            count: AccuracyPreset::Ten.count(),
            auto_threads: true,
            threads: 0,
            keep_rq: true,
            detail_mode: PairDetailMode::Every,
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
    pub about_open: bool,
    pub(crate) active_help: Option<HelpTopic>,
    /// 各工具页独立的日志与结果，索引与 [`Tool::ALL`] 一致；渲染时只看当前页。
    pub(crate) logs: [LogBuffer; 5],
    pub(crate) views: [super::results::ResultsView; 5],
    /// 当前任务所属的工具页；轮询到的事件始终写入这一页，切页不会串输出。
    pub(crate) task_tool: Tool,
    pub status: String,
    pub running: bool,
    pub cancel_requested: bool,
    pub cancel_token: Option<Arc<AtomicBool>>,
    pub done: usize,
    pub total: usize,
    pub started_at: Option<Instant>,
    pub rate_text: String,
    pub eta_text: String,
    pub(crate) live_feed: Option<tswn_openbox::backend::live::LiveFeed>,
    pub(crate) pending_live: tswn_openbox::backend::live::LiveBatch,
    pub(crate) last_live_poll: Instant,
    pub to_diy: ToDiyState,
    pub namer_pf: NamerPfState,
    pub batch_rate: BatchRateState,
    pub pair: PairState,
    pub ds4: super::ds4::Ds4State,
}

impl Default for OpenboxApp {
    fn default() -> Self {
        Self {
            theme_preference: egui::ThemePreference::System,
            tool: Tool::ToDiy,
            more_settings_open: false,
            about_open: false,
            active_help: None,
            logs: std::array::from_fn(|_| LogBuffer::default()),
            views: std::array::from_fn(|index| super::results::ResultsView::with_mode(super::results::DEFAULT_VIEW_MODES[index])),
            task_tool: Tool::ToDiy,
            status: "就绪".to_string(),
            running: false,
            cancel_requested: false,
            cancel_token: None,
            done: 0,
            total: 0,
            started_at: None,
            rate_text: "--".to_string(),
            eta_text: "--".to_string(),
            live_feed: None,
            pending_live: Default::default(),
            last_live_poll: Instant::now(),
            to_diy: ToDiyState::default(),
            namer_pf: NamerPfState::default(),
            batch_rate: BatchRateState::default(),
            pair: PairState::default(),
            ds4: Default::default(),
        }
    }
}

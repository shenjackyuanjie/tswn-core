//! WASM 层数据模型定义。
//!
//! 定义从 Rust 向 JavaScript 暴露的所有数据结构，包括战斗选项、玩家元数据、
//! 每帧状态快照、战斗回放、胜率结果等，均通过 `tsify` 生成对应的 TypeScript 类型声明。

use serde::{Deserialize, Serialize};
use tsify::Tsify;
use tswn_core::cli_api as core_cli_api;
use tswn_core::engine::update::UpdateType;
use tswn_core::player::skill::act::minion::MinionKind;

#[derive(Debug, Clone, Default, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct FightOptions {
    #[tsify(optional)]
    pub eval_rq: Option<f64>,
    #[tsify(optional)]
    pub include_icons: Option<bool>,
    #[tsify(optional)]
    pub capture_replay: Option<bool>,
}

impl FightOptions {
    pub fn resolved_eval_rq(&self) -> f64 { self.eval_rq.unwrap_or(tswn_core::player::eval_name::DEFAULT_EVAL_RQ) }

    pub fn include_icons(&self) -> bool { self.include_icons.unwrap_or(false) }

    pub fn capture_replay(&self) -> bool { self.capture_replay.unwrap_or(true) }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct WinRateOptions {
    #[tsify(optional)]
    pub eval_rq: Option<f64>,
    #[tsify(optional)]
    pub thread: Option<u32>,
}

impl WinRateOptions {
    pub fn resolved_eval_rq(&self) -> f64 { self.eval_rq.unwrap_or(tswn_core::player::eval_name::WIN_RATE_EVAL_RQ) }

    pub fn resolved_thread(&self) -> u32 {
        let _ = self.thread;
        1
    }
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct PlayerMeta {
    pub id: usize,
    pub team_index: usize,
    pub id_name: String,
    pub icon_key: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct PlayerState {
    pub id: usize,
    pub team_index: usize,
    pub id_name: String,
    pub icon_key: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_png_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minion_kind: Option<MinionKindView>,
    pub hp: i32,
    pub max_hp: i32,
    pub magic_point: i32,
    pub move_point: i32,
    pub attack: i32,
    pub defense: i32,
    pub speed: i32,
    pub agility: i32,
    pub magic: i32,
    pub resistance: i32,
    pub wisdom: i32,
    pub point: u32,
    pub all_sum: u32,
    pub name_factor: f64,
    pub at_boost: f64,
    pub attract: f64,
    pub frozen: bool,
    pub alive: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status_labels: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "snake_case")]
pub enum MinionKindView {
    Clone,
    Summon,
    Shadow,
    Zombie,
}

impl From<MinionKind> for MinionKindView {
    fn from(value: MinionKind) -> Self {
        match value {
            MinionKind::Clone => Self::Clone,
            MinionKind::Summon => Self::Summon,
            MinionKind::Shadow => Self::Shadow,
            MinionKind::Zombie => Self::Zombie,
        }
    }
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "snake_case")]
pub enum UpdateTypeView {
    Win,
    None,
    NextLine,
}

impl From<UpdateType> for UpdateTypeView {
    fn from(value: UpdateType) -> Self {
        match value {
            UpdateType::Win => Self::Win,
            UpdateType::None => Self::None,
            UpdateType::NextLine => Self::NextLine,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "snake_case")]
pub enum MessageTone {
    /// 普通消息（无特殊视觉效果）
    Normal,
    /// 伤害消息
    Damage,
    /// 回复消息
    Recover,
    /// 击倒消息
    Knockout,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct UpdateView {
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub caster_id: usize,
    pub target_id: usize,
    pub target_ids: Vec<usize>,
    pub update_type: UpdateTypeView,
    pub message_template: String,
    pub message_rendered: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hp_delta: Option<i32>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub status_change_tokens: Vec<String>,
    /// 消息色调，由 WASM 根据模板内容判定，JS 无需再通过关键词反推。
    pub tone: MessageTone,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "snake_case")]
pub enum ReplayTextPartKind {
    Text,
    Highlight,
    Player,
    Data,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct ReplayTextPart {
    pub kind: ReplayTextPartKind,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_id: Option<usize>,
    pub show_hp: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub death_effect: bool,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct ReplayClip {
    pub delay: i32,
    pub text_template: String,
    pub color: MessageTone,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub player_id: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    pub show_hp: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub death_effect: bool,
    pub emoji: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<ReplayTextPart>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caster_ids: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_ids: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidebar_states: Vec<PlayerState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidebar_previous_states: Vec<PlayerState>,
    pub winner: bool,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct ReplayRow {
    pub indent: bool,
    pub clips: Vec<ReplayClip>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct RoundFrame {
    pub finished: bool,
    pub winner_ids: Vec<usize>,
    pub updates: Vec<UpdateView>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<ReplayRow>,
    pub states: Vec<PlayerState>,
    /// 帧内所有可见 update 的原始等待总和（毫秒），按混淆版 md5.js 的 delay 规则计算，未按角色数量缩放。
    pub total_delay: i32,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct FightReplay {
    pub players: Vec<PlayerMeta>,
    pub frames: Vec<RoundFrame>,
    pub winner_ids: Vec<usize>,
    pub final_states: Vec<PlayerState>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct FightSummary {
    pub finished: bool,
    pub players: Vec<PlayerMeta>,
    pub winner_ids: Vec<usize>,
    pub final_states: Vec<PlayerState>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[serde(transparent)]
#[tsify(into_wasm_abi)]
pub struct WinnerIds(pub Vec<usize>);

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WinRateProgress {
    pub done: bool,
    pub rounds_done: usize,
    pub total_rounds: usize,
    pub wins: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WinRateTiming {
    pub init_nanos: u64,
    pub fight_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct WinRateResult {
    pub done: bool,
    pub rounds_done: usize,
    pub total_rounds: usize,
    pub wins: usize,
    pub percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<WinRateTiming>,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct GroupWinRateResult {
    pub opponent: String,
    pub result: WinRateResult,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliWinRateResult {
    pub wins: usize,
    pub total: usize,
    pub win_rate: f64,
    pub init_nanos: u64,
    pub fight_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliGroupWinRateResult {
    pub opponent: String,
    pub result: CliWinRateResult,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliScoreResult {
    pub score: f64,
    pub wins: usize,
    pub total: usize,
    pub errors: usize,
    pub init_nanos: u64,
    pub fight_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliNamerPfResult {
    pub group: Vec<String>,
    pub modes: Vec<String>,
    pub scores: Vec<f64>,
    pub total_score: f64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliBatchRateResult {
    pub label: String,
    pub avg_win_rate: f64,
    pub aggregate_win_rate: f64,
    pub wins: usize,
    pub total: usize,
    pub valid_matchups: usize,
    pub skipped_matchups: usize,
    pub init_nanos: u64,
    pub fight_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliPairRateEntry {
    pub name: String,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliPairRateResult {
    pub label: String,
    pub final_score: f64,
    pub head: usize,
    pub selected: usize,
    pub top_pairs: Vec<CliPairRateEntry>,
    pub aggregate_win_rate: f64,
    pub wins: usize,
    pub total: usize,
    pub valid_matchups: usize,
    pub skipped_matchups: usize,
    pub init_nanos: u64,
    pub fight_nanos: u64,
}

#[derive(Debug, Clone, Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CliIconInfo {
    pub border_style: usize,
    pub shapes: Vec<usize>,
    pub bg_color_idx: usize,
    pub bg_color: [u8; 3],
    pub fg_color_indices: Vec<usize>,
    pub fg_colors: Vec<[u8; 3]>,
    pub colors_consumed: usize,
}

fn nanos_to_u64(value: u128) -> u64 { u64::try_from(value).unwrap_or(u64::MAX) }

impl From<core_cli_api::WinRateResult> for CliWinRateResult {
    fn from(value: core_cli_api::WinRateResult) -> Self {
        Self {
            wins: value.wins,
            total: value.total,
            win_rate: value.win_rate,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::GroupWinRateResult> for CliGroupWinRateResult {
    fn from(value: core_cli_api::GroupWinRateResult) -> Self {
        Self {
            opponent: value.opponent,
            result: value.result.into(),
        }
    }
}

impl From<core_cli_api::ScoreResult> for CliScoreResult {
    fn from(value: core_cli_api::ScoreResult) -> Self {
        Self {
            score: value.score,
            wins: value.wins,
            total: value.total,
            errors: value.errors,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::NamerPfResult> for CliNamerPfResult {
    fn from(value: core_cli_api::NamerPfResult) -> Self {
        Self {
            group: value.group,
            modes: value.modes,
            scores: value.scores,
            total_score: value.total_score,
        }
    }
}

impl From<core_cli_api::BatchRateResult> for CliBatchRateResult {
    fn from(value: core_cli_api::BatchRateResult) -> Self {
        Self {
            label: value.label,
            avg_win_rate: value.avg_win_rate,
            aggregate_win_rate: value.aggregate_win_rate,
            wins: value.wins,
            total: value.total,
            valid_matchups: value.valid_matchups,
            skipped_matchups: value.skipped_matchups,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::PairRateEntry> for CliPairRateEntry {
    fn from(value: core_cli_api::PairRateEntry) -> Self {
        Self {
            name: value.name,
            rate: value.rate,
        }
    }
}

impl From<core_cli_api::PairRateResult> for CliPairRateResult {
    fn from(value: core_cli_api::PairRateResult) -> Self {
        Self {
            label: value.label,
            final_score: value.final_score,
            head: value.head,
            selected: value.selected,
            top_pairs: value.top_pairs.into_iter().map(Into::into).collect(),
            aggregate_win_rate: value.aggregate_win_rate,
            wins: value.wins,
            total: value.total,
            valid_matchups: value.valid_matchups,
            skipped_matchups: value.skipped_matchups,
            init_nanos: nanos_to_u64(value.init_nanos),
            fight_nanos: nanos_to_u64(value.fight_nanos),
        }
    }
}

impl From<core_cli_api::IconInfo> for CliIconInfo {
    fn from(value: core_cli_api::IconInfo) -> Self {
        Self {
            border_style: value.border_style,
            shapes: value.shapes,
            bg_color_idx: value.bg_color_idx,
            bg_color: value.bg_color,
            fg_color_indices: value.fg_color_indices,
            fg_colors: value.fg_colors,
            colors_consumed: value.colors_consumed,
        }
    }
}

use serde::{Deserialize, Serialize};
use tswn_core::engine::update::UpdateType;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FightOptions {
    pub eval_rq: Option<f64>,
    pub include_icons: Option<bool>,
    pub capture_replay: Option<bool>,
}

impl FightOptions {
    pub fn resolved_eval_rq(&self) -> f64 {
        self.eval_rq
            .unwrap_or(tswn_core::player::eval_name::DEFAULT_EVAL_RQ)
    }

    pub fn include_icons(&self) -> bool { self.include_icons.unwrap_or(false) }

    pub fn capture_replay(&self) -> bool { self.capture_replay.unwrap_or(true) }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinRateOptions {
    pub eval_rq: Option<f64>,
    pub thread: Option<u32>,
}

impl WinRateOptions {
    pub fn resolved_eval_rq(&self) -> f64 {
        self.eval_rq
            .unwrap_or(tswn_core::player::eval_name::WIN_RATE_EVAL_RQ)
    }

    pub fn resolved_thread(&self) -> u32 {
        let _ = self.thread;
        1
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerMeta {
    pub id: usize,
    pub team_index: usize,
    pub id_name: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub id: usize,
    pub team_index: usize,
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
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
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundFrame {
    pub finished: bool,
    pub winner_ids: Vec<usize>,
    pub updates: Vec<UpdateView>,
    pub states: Vec<PlayerState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FightReplay {
    pub players: Vec<PlayerMeta>,
    pub frames: Vec<RoundFrame>,
    pub winner_ids: Vec<usize>,
    pub final_states: Vec<PlayerState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FightSummary {
    pub finished: bool,
    pub players: Vec<PlayerMeta>,
    pub winner_ids: Vec<usize>,
    pub final_states: Vec<PlayerState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinRateProgress {
    pub done: bool,
    pub rounds_done: usize,
    pub total_rounds: usize,
    pub wins: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinRateTiming {
    pub init_nanos: u64,
    pub fight_nanos: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinRateResult {
    pub done: bool,
    pub rounds_done: usize,
    pub total_rounds: usize,
    pub wins: usize,
    pub percent: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<WinRateTiming>,
}
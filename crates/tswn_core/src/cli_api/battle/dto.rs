use crate::replay_view::ReplayState;
use crate::runtime::{BINDING_COMPLETION_MAX_ROUNDS, PlrId};
use serde::Serialize;

/// Options for the user-facing, complete battle replay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleOptions {
    pub eval_rq: f64,
    pub include_icons: bool,
    pub max_rounds: usize,
}

impl Default for BattleOptions {
    fn default() -> Self {
        Self {
            eval_rq: crate::namerena::eval_name::DEFAULT_EVAL_RQ,
            include_icons: false,
            max_rounds: BINDING_COMPLETION_MAX_ROUNDS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleReplay {
    pub status: BattleStatus,
    pub stop_reason: BattleStopReason,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub finished: bool,
    pub truncated: bool,
    pub initial_states: Vec<BattlePlayerState>,
    pub frames: Vec<BattleReplayFrame>,
    pub final_states: Vec<BattlePlayerState>,
    pub winner_ids: Vec<usize>,
    pub winner_team_indices: Vec<usize>,
    pub state_granularity: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattlePlayerState {
    pub id: usize,
    pub team_index: usize,
    pub input_team_index: Option<usize>,
    pub owner_id: Option<usize>,
    pub source_id: Option<usize>,
    pub id_name: String,
    pub id_key_name: String,
    pub icon_key: String,
    pub display_name: String,
    pub display_index: usize,
    pub base_name: String,
    pub player_type: String,
    pub minion_kind: Option<&'static str>,
    pub icon_png_base64: Option<String>,
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
    pub active: bool,
    pub status_labels: Vec<String>,
}

impl ReplayState for BattlePlayerState {
    fn id(&self) -> PlrId { self.id }

    fn hp(&self) -> i32 { self.hp }

    fn max_hp(&self) -> i32 { self.max_hp }

    fn alive(&self) -> bool { self.alive }

    fn with_hp_alive(&self, hp: i32, alive: bool) -> Self {
        Self {
            hp,
            alive,
            ..self.clone()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleReplayFrame {
    pub frame_index: usize,
    pub round_index: usize,
    pub finished: bool,
    pub winner_ids: Vec<usize>,
    pub updates: Vec<BattleUpdate>,
    pub rows: Vec<BattleReplayRow>,
    pub states: Vec<BattlePlayerState>,
    pub total_delay: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleUpdate {
    pub update_type: &'static str,
    pub tone: &'static str,
    pub message_template: String,
    pub message_rendered: String,
    pub caster_id: Option<usize>,
    pub target_id: Option<usize>,
    pub target_ids: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
    pub delay0: i32,
    pub delay1: i32,
    pub hp_delta: Option<i32>,
    pub is_win: bool,
    pub is_next_line: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleReplayRow {
    pub indent: bool,
    pub clips: Vec<BattleReplayClip>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleReplayClip {
    pub delay: i32,
    pub color: String,
    pub tone: &'static str,
    pub parts: Vec<BattleReplayTextPart>,
    pub caster_ids: Vec<usize>,
    pub target_ids: Vec<usize>,
    pub sidebar_states: Vec<BattlePlayerState>,
    pub sidebar_previous_states: Vec<BattlePlayerState>,
    pub winner: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleReplayTextPart {
    pub kind: &'static str,
    pub text: String,
    pub player_id: Option<usize>,
    pub show_hp: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub death_effect: bool,
    pub emoji: Option<String>,
}

#[deprecated(note = "use BattleOptions")]
pub type BattleReplayOptions = BattleOptions;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleStatus {
    Running,
    Finished,
    Truncated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleStopReason {
    Winner,
    MaxRounds,
    NoProgress,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleResult {
    pub status: BattleStatus,
    pub stop_reason: BattleStopReason,
    pub finished: bool,
    pub truncated: bool,
    pub rounds_advanced: usize,
    pub frames_emitted: usize,
    pub winner_ids: Vec<usize>,
    pub winner_team_indices: Vec<usize>,
    pub final_states: Vec<BattlePlayerState>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli_api::CliApiError;

    #[test]
    fn public_enum_values_and_error_codes_are_stable() {
        for (value, expected) in [
            (BattleStatus::Running, "running"),
            (BattleStatus::Finished, "finished"),
            (BattleStatus::Truncated, "truncated"),
        ] {
            assert_eq!(serde_json::to_value(value).unwrap(), expected);
        }
        for (value, expected) in [
            (BattleStopReason::Winner, "winner"),
            (BattleStopReason::MaxRounds, "max_rounds"),
            (BattleStopReason::NoProgress, "no_progress"),
        ] {
            assert_eq!(serde_json::to_value(value).unwrap(), expected);
        }
        for (error, expected) in [
            (CliApiError::InvalidInput("input".into()), "INVALID_INPUT"),
            (CliApiError::InvalidArgument("argument".into()), "INVALID_ARGUMENT"),
            (CliApiError::UnsupportedOption("option".into()), "UNSUPPORTED_OPTION"),
            (CliApiError::RunnerInit("init".into()), "RUNNER_INIT_FAILED"),
            (CliApiError::Runtime("runtime".into()), "RUNTIME_FAILED"),
            (CliApiError::Internal("internal".into()), "INTERNAL_ERROR"),
        ] {
            assert_eq!(error.code().as_str(), expected);
            assert_eq!(serde_json::to_value(error.code()).unwrap(), expected);
            assert!(!error.to_string().is_empty());
        }
    }

    #[test]
    fn battle_options_defaults_match_binding_contract() {
        let options = BattleOptions::default();
        assert_eq!(options.max_rounds, 20_000);
        assert!(!options.include_icons);
        assert_eq!(options.eval_rq, crate::namerena::eval_name::DEFAULT_EVAL_RQ);
    }
}

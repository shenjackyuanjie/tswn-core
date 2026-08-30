use std::collections::HashMap;

use serde::Serialize;

use crate::namerena::icon_render::render_icon_b64_from_name;
use crate::replay_view::{
    ReplayClip, ReplayEventView, ReplayRow, ReplayState, ReplayTextPart, ReplayTextPartKind, ReplayTone, build_replay_view_frame,
    hp_delta_for_tone, render_update_message,
};
use crate::runtime::update::{RunUpdates, UpdateType};
use crate::runtime::{BINDING_COMPLETION_MAX_ROUNDS, PlrId, RuntimeMinionKind, RuntimePlayerSnapshot, RuntimeRunner};

use super::{CliApiError, CliApiResult, invalid_input};

/// Options for the user-facing, complete battle replay.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleReplayOptions {
    pub eval_rq: f64,
    pub include_icons: bool,
    pub max_rounds: usize,
}

impl Default for BattleReplayOptions {
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

pub fn battle_replay(raw: &str, options: BattleReplayOptions) -> CliApiResult<BattleReplay> {
    if raw.trim().is_empty() {
        return Err(invalid_input("raw_input is empty"));
    }
    if options.max_rounds == 0 {
        return Err(invalid_input("battle_replay max_rounds must be positive"));
    }

    let (groups, seed) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
    let mut runner = RuntimeRunner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, options.eval_rq)
        .map_err(|err| CliApiError::Runner(err.to_string()))?;
    let initial_states = states_from_runner(&runner, options.include_icons);
    let mut previous_states = initial_states.clone();
    let mut frames = Vec::new();
    let mut idle_rounds = 0usize;

    while !runner.have_winner() && frames.len() < options.max_rounds {
        let updates = runner.main_round();
        if updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16usize.saturating_mul(runner.all_player_ids().len().max(1)) {
                break;
            }
            previous_states = states_from_runner(&runner, options.include_icons);
            continue;
        }
        idle_rounds = 0;
        let states = states_from_runner(&runner, options.include_icons);
        frames.push(build_frame(&updates, &previous_states, &states, &runner));
        previous_states = states;
    }

    let finished = runner.have_winner();
    Ok(BattleReplay {
        finished,
        truncated: !finished,
        initial_states,
        frames,
        final_states: states_from_runner(&runner, options.include_icons),
        winner_ids: runner.winner_ids(),
        winner_team_indices: runner.winner_team_indices(),
        state_granularity: "round",
    })
}

fn states_from_runner(runner: &RuntimeRunner, include_icons: bool) -> Vec<BattlePlayerState> {
    runner
        .player_snapshots()
        .into_iter()
        .map(|snapshot| state_from_snapshot(snapshot, include_icons))
        .collect()
}

fn state_from_snapshot(snapshot: RuntimePlayerSnapshot, include_icons: bool) -> BattlePlayerState {
    let icon_key = snapshot.id_key_name.clone();
    BattlePlayerState {
        id: snapshot.id,
        team_index: snapshot.team_index,
        input_team_index: snapshot.input_team_index,
        owner_id: snapshot.owner_id,
        source_id: snapshot.root_owner_id,
        id_name: snapshot.id_name,
        id_key_name: snapshot.id_key_name,
        icon_key: icon_key.clone(),
        display_name: snapshot.display_name,
        display_index: snapshot.display_index,
        base_name: snapshot.base_name,
        player_type: snapshot.player_type.to_owned(),
        minion_kind: snapshot.minion_kind.map(RuntimeMinionKind::as_str),
        icon_png_base64: include_icons.then(|| render_icon_b64_from_name(&icon_key)),
        hp: snapshot.hp,
        max_hp: snapshot.max_hp,
        magic_point: snapshot.magic_point,
        move_point: snapshot.move_point,
        attack: snapshot.attack,
        defense: snapshot.defense,
        speed: snapshot.speed,
        agility: snapshot.agility,
        magic: snapshot.magic,
        resistance: snapshot.resistance,
        wisdom: snapshot.wisdom,
        point: snapshot.point,
        all_sum: snapshot.all_sum,
        name_factor: snapshot.name_factor,
        at_boost: snapshot.at_boost,
        attract: snapshot.attract,
        frozen: snapshot.frozen,
        alive: snapshot.alive,
        active: snapshot.active,
        status_labels: snapshot.status_labels,
    }
}

fn build_frame(
    updates: &RunUpdates,
    previous_states: &[BattlePlayerState],
    states: &[BattlePlayerState],
    runner: &RuntimeRunner,
) -> BattleReplayFrame {
    let names = player_names(states);
    let converted = updates
        .updates
        .iter()
        .map(|update| update_from_runtime(update, &names))
        .collect::<Vec<_>>();
    let events = updates
        .updates
        .iter()
        .zip(converted.iter())
        .map(|(update, view)| ReplayEventView {
            update,
            tone: tone_from_name(view.tone),
            message_rendered: &view.message_rendered,
        })
        .collect::<Vec<_>>();
    let winner_ids = runner.winner_ids();
    let replay = build_replay_view_frame(&events, previous_states, states, &names, runner.have_winner(), &winner_ids);
    BattleReplayFrame {
        finished: runner.have_winner(),
        winner_ids,
        updates: converted,
        rows: replay.rows.into_iter().map(row_from_core).collect(),
        states: states.to_vec(),
        total_delay: replay.total_delay,
    }
}

fn player_names(states: &[BattlePlayerState]) -> HashMap<PlrId, String> {
    states
        .iter()
        .map(|state| {
            let name = if state.minion_kind == Some("clone") && state.display_index > 0 {
                format!("{} #{}", state.display_name, state.display_index)
            } else {
                state.display_name.clone()
            };
            (state.id, name)
        })
        .collect()
}

fn update_from_runtime(update: &crate::RunUpdate, names: &HashMap<PlrId, String>) -> BattleUpdate {
    let tone = classify_tone(update);
    let is_next_line = matches!(update.update_type, UpdateType::NextLine);
    BattleUpdate {
        update_type: update_type_name(update.update_type),
        tone: tone_name(tone),
        message_template: update.message.to_string(),
        message_rendered: render_update_message(update, names),
        caster_id: (!is_next_line).then_some(update.caster),
        target_id: (!is_next_line).then_some(update.target),
        target_ids: update.targets.iter().copied().collect(),
        param: update.param,
        score: update.score,
        delay0: update.delay0,
        delay1: update.delay1,
        hp_delta: hp_delta_for_tone(tone, update),
        is_win: matches!(update.update_type, UpdateType::Win),
        is_next_line,
    }
}

fn classify_tone(update: &crate::RunUpdate) -> ReplayTone {
    if matches!(update.update_type, UpdateType::Win) {
        return ReplayTone::Knockout;
    }
    let message = update.message.as_ref();
    if message.contains("回复体力") {
        ReplayTone::Recover
    } else if message.contains("被击倒") || message.contains("消失了") {
        ReplayTone::Knockout
    } else if message.contains("点伤害") {
        ReplayTone::Damage
    } else if is_status_exit_message(message) {
        ReplayTone::StatusExit
    } else {
        ReplayTone::Normal
    }
}

fn is_status_exit_message(message: &str) -> bool {
    ["中解除", "被识破", "被中止", "被打消", "属性被打消"]
        .iter()
        .any(|token| message.contains(token))
}

fn update_type_name(value: UpdateType) -> &'static str {
    match value {
        UpdateType::Win => "win",
        UpdateType::None => "none",
        UpdateType::NextLine => "next_line",
    }
}

fn tone_name(value: ReplayTone) -> &'static str {
    match value {
        ReplayTone::Normal => "normal",
        ReplayTone::Damage => "damage",
        ReplayTone::Recover => "recover",
        ReplayTone::Knockout => "knockout",
        ReplayTone::StatusExit => "status_exit",
    }
}

fn tone_from_name(value: &str) -> ReplayTone {
    match value {
        "damage" => ReplayTone::Damage,
        "recover" => ReplayTone::Recover,
        "knockout" => ReplayTone::Knockout,
        "status_exit" => ReplayTone::StatusExit,
        _ => ReplayTone::Normal,
    }
}

fn row_from_core(row: ReplayRow<BattlePlayerState>) -> BattleReplayRow {
    BattleReplayRow {
        indent: row.indent,
        clips: row.clips.into_iter().map(clip_from_core).collect(),
    }
}

fn clip_from_core(clip: ReplayClip<BattlePlayerState>) -> BattleReplayClip {
    BattleReplayClip {
        delay: clip.delay,
        color: clip.color,
        tone: tone_name(clip.tone),
        parts: clip.parts.into_iter().map(part_from_core).collect(),
        caster_ids: clip.caster_ids,
        target_ids: clip.target_ids,
        sidebar_states: clip.sidebar_states,
        sidebar_previous_states: clip.sidebar_previous_states,
        winner: clip.winner,
    }
}

fn part_from_core(part: ReplayTextPart) -> BattleReplayTextPart {
    BattleReplayTextPart {
        kind: match part.kind {
            ReplayTextPartKind::Text => "text",
            ReplayTextPartKind::Highlight => "highlight",
            ReplayTextPartKind::Player => "player",
            ReplayTextPartKind::Data => "data",
        },
        text: part.text,
        player_id: part.player_id,
        show_hp: part.show_hp,
        hp_before: part.hp_before,
        hp_after: part.hp_after,
        death_effect: part.death_effect,
        emoji: part.emoji,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battle_replay_returns_render_ready_frames() {
        let replay = battle_replay("left@red\n\nright@blue\n", BattleReplayOptions::default()).unwrap();

        assert!(replay.finished);
        assert!(!replay.frames.is_empty());
        assert_eq!(replay.initial_states.len(), 2);
        assert!(!replay.winner_ids.is_empty());
        assert!(replay.frames.iter().any(|frame| !frame.rows.is_empty()));
    }

    #[test]
    fn battle_replay_rejects_zero_max_rounds() {
        let options = BattleReplayOptions {
            max_rounds: 0,
            ..BattleReplayOptions::default()
        };
        let err = battle_replay("left\n\nright", options).unwrap_err();
        assert_eq!(err.to_string(), "battle_replay max_rounds must be positive");
    }
}

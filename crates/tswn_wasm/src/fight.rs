//! WASM 战斗驱动逻辑。
//!
//! 提供 `FightSession`（逐帧驱动）及 `fight`/`fight_summary` 一次性函数，
//! 将引擎每个主回合的输出整理为 `RoundFrame`（含玩家状态、消息更新、延迟信息），
//! 供 JavaScript 侧逐帧播放或一次性取得完整回放。

use std::collections::HashMap;

use tswn_core::player::PlrId;
use tswn_core::replay_view::{
    ReplayEventView, ReplayState, ReplayTextPart as CoreReplayTextPart, ReplayTextPartKind as CoreReplayTextPartKind, ReplayTone,
    ReplayViewFrame, build_replay_view_frame,
};
use tswn_core::runtime::{BINDING_COMPLETION_MAX_ROUNDS, RuntimePlayerSnapshot};
use tswn_core::{RunUpdates, Runner};
use wasm_bindgen::prelude::*;

use crate::error::{WasmResult, internal_error, invalid_input, runner_init_failed};
use crate::model::{
    FightOptions, FightReplay, FightSummary, MessageTone, MinionKindView, PlayerMeta, PlayerState, ReplayClip, ReplayRow,
    ReplayTextPart, ReplayTextPartKind, RoundFrame, UpdateView, WinnerIds,
};
use crate::render::{classify_message_tone, render_update_message, status_change_tokens};

fn build_runner(raw_input: String, eval_rq: f64) -> WasmResult<Runner> {
    if raw_input.trim().is_empty() {
        return Err(invalid_input("rawInput is empty"));
    }

    let (groups, seed) = Runner::split_namerena_into_groups(raw_input);
    Runner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, eval_rq).map_err(|err| runner_init_failed(err.to_string()))
}

fn collect_players(
    runner: &Runner,
    player_order: &[PlrId],
    include_icons: bool,
) -> WasmResult<(Vec<PlayerMeta>, HashMap<PlrId, String>)> {
    let mut players = Vec::with_capacity(player_order.len());
    let mut names = HashMap::with_capacity(player_order.len());

    for player_id in player_order {
        let Some(player) = runner.player_snapshot(*player_id) else {
            return Err(internal_error(format!("player {player_id} missing from runtime")));
        };
        let display_name = player.display_name;
        let team_index = player.team_index;
        let icon_key = player.id_key_name;
        let icon_png_base64 = if include_icons {
            Some(tswn_core::player::icon_render::render_icon_b64_from_name(&icon_key))
        } else {
            None
        };
        names.insert(*player_id, display_name.clone());
        players.push(PlayerMeta {
            id: *player_id,
            team_index,
            id_name: player.id_name,
            icon_key,
            display_name,
            icon_png_base64,
        });
    }

    Ok((players, names))
}

fn state_from_runtime(player: RuntimePlayerSnapshot, include_icons: bool) -> PlayerState {
    let icon_png_base64 = if include_icons && player.root_owner_id.is_some() {
        Some(tswn_core::player::icon_render::render_icon_b64_from_name(&player.id_key_name))
    } else {
        None
    };
    PlayerState {
        id: player.id,
        team_index: player.team_index,
        id_name: player.id_name,
        icon_key: player.id_key_name,
        display_name: player.display_name,
        display_index: player.display_index,
        icon_png_base64,
        owner_id: player.root_owner_id,
        minion_kind: player.minion_kind.map(Into::into),
        hp: player.hp,
        max_hp: player.max_hp,
        magic_point: player.magic_point,
        move_point: player.move_point,
        attack: player.attack,
        defense: player.defense,
        speed: player.speed,
        agility: player.agility,
        magic: player.magic,
        resistance: player.resistance,
        wisdom: player.wisdom,
        point: player.point,
        all_sum: player.all_sum,
        name_factor: player.name_factor,
        at_boost: player.at_boost,
        attract: player.attract,
        frozen: player.frozen,
        alive: player.alive,
        status_labels: player.status_labels,
    }
}

fn collect_states(runner: &Runner, _player_order: &[PlrId], include_icons: bool) -> WasmResult<Vec<PlayerState>> {
    Ok(runner
        .player_snapshots()
        .into_iter()
        .map(|player| state_from_runtime(player, include_icons))
        .collect())
}

fn display_name_for_state(state: &PlayerState) -> String {
    if matches!(state.minion_kind, Some(MinionKindView::Clone)) && state.display_index > 0 {
        format!("{} #{}", state.display_name, state.display_index)
    } else {
        state.display_name.clone()
    }
}

fn player_names_from_states(states: &[PlayerState]) -> HashMap<PlrId, String> {
    states.iter().map(|state| (state.id, display_name_for_state(state))).collect()
}

fn u32_to_i32_saturating(value: u32) -> i32 { value.min(i32::MAX as u32) as i32 }

fn update_hp_delta(tone: MessageTone, update: &tswn_core::RunUpdate) -> Option<i32> {
    let value = update_hp_delta_value(update);
    match tone {
        MessageTone::Damage => Some(-u32_to_i32_saturating(value)),
        MessageTone::Recover => Some(u32_to_i32_saturating(value)),
        _ => None,
    }
}

fn update_hp_delta_value(update: &tswn_core::RunUpdate) -> u32 {
    if update.message.contains("体力减少") && update.message.contains("[2]%") {
        update.score
    } else {
        update.param.unwrap_or(update.score)
    }
}

impl ReplayState for PlayerState {
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

fn tone_to_core(tone: MessageTone) -> ReplayTone {
    match tone {
        MessageTone::Normal => ReplayTone::Normal,
        MessageTone::Damage => ReplayTone::Damage,
        MessageTone::Recover => ReplayTone::Recover,
        MessageTone::Knockout => ReplayTone::Knockout,
        MessageTone::StatusExit => ReplayTone::StatusExit,
    }
}

fn tone_from_core(tone: ReplayTone) -> MessageTone {
    match tone {
        ReplayTone::Normal => MessageTone::Normal,
        ReplayTone::Damage => MessageTone::Damage,
        ReplayTone::Recover => MessageTone::Recover,
        ReplayTone::Knockout => MessageTone::Knockout,
        ReplayTone::StatusExit => MessageTone::StatusExit,
    }
}

fn part_kind_from_core(kind: CoreReplayTextPartKind) -> ReplayTextPartKind {
    match kind {
        CoreReplayTextPartKind::Text => ReplayTextPartKind::Text,
        CoreReplayTextPartKind::Highlight => ReplayTextPartKind::Highlight,
        CoreReplayTextPartKind::Player => ReplayTextPartKind::Player,
        CoreReplayTextPartKind::Data => ReplayTextPartKind::Data,
    }
}

fn part_from_core(part: CoreReplayTextPart) -> ReplayTextPart {
    ReplayTextPart {
        kind: part_kind_from_core(part.kind),
        text: part.text,
        player_id: part.player_id,
        show_hp: part.show_hp,
        hp_before: part.hp_before,
        hp_after: part.hp_after,
        death_effect: part.death_effect,
        emoji: part.emoji,
    }
}

fn rows_from_core(view: ReplayViewFrame<PlayerState>) -> (Vec<ReplayRow>, i32) {
    let rows = view
        .rows
        .into_iter()
        .map(|row| ReplayRow {
            indent: row.indent,
            clips: row
                .clips
                .into_iter()
                .map(|clip| ReplayClip {
                    delay: clip.delay,
                    color: clip.color,
                    tone: tone_from_core(clip.tone),
                    parts: clip.parts.into_iter().map(part_from_core).collect(),
                    caster_ids: clip.caster_ids,
                    target_ids: clip.target_ids,
                    sidebar_states: clip.sidebar_states,
                    sidebar_previous_states: clip.sidebar_previous_states,
                    winner: clip.winner,
                })
                .collect(),
        })
        .collect();
    (rows, view.total_delay)
}

fn convert_updates(updates: &RunUpdates, player_names: &HashMap<PlrId, String>) -> Vec<UpdateView> {
    updates
        .updates
        .iter()
        .map(|update| {
            let tone = classify_message_tone(&update.message);
            let hp_delta = update_hp_delta(tone, update);
            let status_change_tokens = status_change_tokens(&update.message);
            let message_rendered = render_update_message(update, player_names);
            UpdateView {
                score: update.score,
                delay0: update.delay0,
                delay1: update.delay1,
                caster_id: update.caster,
                target_id: update.target,
                target_ids: update.targets.iter().copied().collect(),
                update_type: update.update_type.into(),
                message_template: update.message.to_string(),
                message_rendered,
                param: update.param,
                hp_delta,
                status_change_tokens,
                tone,
            }
        })
        .collect()
}

fn winner_ids(runner: &Runner) -> Vec<usize> { runner.winner_ids() }

#[wasm_bindgen]
pub struct FightSession {
    runner: Runner,
    player_order: Vec<PlrId>,
    players: Vec<PlayerMeta>,
    last_states: Vec<PlayerState>,
    include_icons: bool,
    capture_replay: bool,
}

impl FightSession {
    pub fn new_internal(raw_input: String, options: FightOptions) -> WasmResult<Self> {
        let runner = build_runner(raw_input, options.resolved_eval_rq())?;
        let player_order = runner.all_player_ids();
        let (players, _player_names) = collect_players(&runner, &player_order, options.include_icons())?;
        let last_states = collect_states(&runner, &player_order, options.include_icons())?;
        Ok(Self {
            runner,
            player_order,
            players,
            last_states,
            include_icons: options.include_icons(),
            capture_replay: options.capture_replay(),
        })
    }

    fn build_frame(&mut self, updates: RunUpdates) -> WasmResult<RoundFrame> {
        let states = collect_states(&self.runner, &self.player_order, self.include_icons)?;
        let converted = convert_updates(&updates, &player_names_from_states(&states));
        let winner_ids = winner_ids(&self.runner);
        let player_names = player_names_from_states(&states);
        let replay_events = converted
            .iter()
            .zip(updates.updates.iter())
            .map(|(view, update)| ReplayEventView {
                update,
                tone: tone_to_core(view.tone),
                message_rendered: view.message_rendered.as_str(),
            })
            .collect::<Vec<_>>();
        let (rows, total_delay) = rows_from_core(build_replay_view_frame(
            &replay_events,
            &self.last_states,
            &states,
            &player_names,
            self.runner.have_winner(),
            &winner_ids,
        ));
        self.last_states = states.clone();
        Ok(RoundFrame {
            finished: self.runner.have_winner(),
            winner_ids,
            updates: converted,
            rows,
            states,
            total_delay,
        })
    }

    pub fn run_to_end_internal(&mut self, limit: Option<usize>) -> WasmResult<FightReplay> {
        let max_frames = limit.unwrap_or(BINDING_COMPLETION_MAX_ROUNDS);
        let mut frames = Vec::new();
        let mut idle_rounds = 0usize;

        while !self.runner.have_winner() && frames.len() < max_frames {
            let updates = self.runner.main_round();
            if updates.updates.is_empty() {
                idle_rounds += 1;
                if idle_rounds > 16 {
                    break;
                }
                continue;
            }

            idle_rounds = 0;
            if self.capture_replay {
                frames.push(self.build_frame(updates)?);
            }
        }

        Ok(FightReplay {
            players: self.players.clone(),
            frames,
            winner_ids: winner_ids(&self.runner),
            final_states: collect_states(&self.runner, &self.player_order, self.include_icons)?,
        })
    }
}

#[wasm_bindgen]
impl FightSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, options: Option<FightOptions>) -> WasmResult<FightSession> {
        crate::install_panic_hook();
        let options = options.unwrap_or_default();
        Self::new_internal(raw_input, options)
    }

    pub fn players(&self) -> Vec<PlayerMeta> { self.players.clone() }

    pub fn state(&self) -> WasmResult<Vec<PlayerState>> { collect_states(&self.runner, &self.player_order, self.include_icons) }

    pub fn is_finished(&self) -> bool { self.runner.have_winner() }

    pub fn winner_ids(&self) -> WinnerIds { WinnerIds(winner_ids(&self.runner)) }

    pub fn step(&mut self) -> WasmResult<RoundFrame> {
        let frame = if self.runner.have_winner() {
            self.build_frame(RunUpdates::new())?
        } else {
            let updates = self.runner.main_round();
            self.build_frame(updates)?
        };
        Ok(frame)
    }

    pub fn run_to_end(&mut self, limit: Option<usize>) -> WasmResult<FightReplay> { self.run_to_end_internal(limit) }
}

pub fn fight_impl(raw_input: String, options: FightOptions) -> WasmResult<FightReplay> {
    let mut session = FightSession::new_internal(raw_input, options)?;
    session.run_to_end_internal(None)
}

pub fn fight_summary_impl(raw_input: String, options: FightOptions) -> WasmResult<FightSummary> {
    let mut session = FightSession::new_internal(raw_input, options)?;
    let replay = session.run_to_end_internal(None)?;
    Ok(FightSummary {
        finished: session.runner.have_winner(),
        players: replay.players,
        winner_ids: replay.winner_ids,
        final_states: replay.final_states,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fight_session_uses_runtime_and_keeps_parts_only_clip_contract() {
        let mut session = FightSession::new_internal("left@red\n\nright@blue\n".to_owned(), FightOptions::default()).unwrap();
        assert_eq!(session.players.iter().map(|player| player.id).collect::<Vec<_>>(), vec![0, 1]);

        let frame = session.step().unwrap();
        assert!(!frame.updates.is_empty());
        let clip = frame
            .rows
            .iter()
            .flat_map(|row| &row.clips)
            .next()
            .expect("first runtime frame should render");
        let ReplayClip {
            delay: _,
            color: _,
            tone: _,
            parts,
            caster_ids: _,
            target_ids: _,
            sidebar_states: _,
            sidebar_previous_states: _,
            winner: _,
        } = clip.clone();
        assert!(!parts.is_empty());
    }

    #[test]
    fn unguarded_fight_reaches_a_runtime_winner() {
        let replay = fight_impl("left@red\n\nright@blue\n".to_owned(), FightOptions::default()).unwrap();
        assert!(!replay.winner_ids.is_empty());
        assert!(!replay.frames.is_empty());
    }
}

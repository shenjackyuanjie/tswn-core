//! 从规范 BattleSession 投影出的高级兼容 API。
use crate::error::{WasmResult, cli_api_error};
use crate::model::{
    FightOptions, FightReplay, FightSummary, MessageTone, MinionKindView, PlayerMeta, PlayerState, ReplayClip, ReplayRow,
    ReplayTextPart, ReplayTextPartKind, RoundFrame, UpdateTypeView, UpdateView, WinnerIds,
};
use crate::render::status_change_tokens;
use tswn_core::cli_api::battle::{BattleOptions, BattlePlayerState, BattleReplayFrame, BattleSession, BattleUpdate};
use wasm_bindgen::prelude::*;

fn state_from_core(state: &BattlePlayerState) -> PlayerState {
    PlayerState {
        id: state.id,
        team_index: state.team_index,
        id_name: state.id_name.clone(),
        icon_key: state.icon_key.clone(),
        display_name: state.display_name.clone(),
        display_index: state.display_index,
        icon_png_base64: state.icon_png_base64.clone(),
        owner_id: state.owner_id,
        minion_kind: match state.minion_kind {
            Some("clone") => Some(MinionKindView::Clone),
            Some("summon") => Some(MinionKindView::Summon),
            Some("shadow") => Some(MinionKindView::Shadow),
            Some("zombie") => Some(MinionKindView::Zombie),
            _ => None,
        },
        hp: state.hp,
        max_hp: state.max_hp,
        magic_point: state.magic_point,
        move_point: state.move_point,
        attack: state.attack,
        defense: state.defense,
        speed: state.speed,
        agility: state.agility,
        magic: state.magic,
        resistance: state.resistance,
        wisdom: state.wisdom,
        point: state.point,
        all_sum: state.all_sum,
        name_factor: state.name_factor,
        at_boost: state.at_boost,
        attract: state.attract,
        frozen: state.frozen,
        alive: state.alive,
        status_labels: state.status_labels.clone(),
    }
}
fn tone(value: &str) -> MessageTone {
    match value {
        "damage" => MessageTone::Damage,
        "recover" => MessageTone::Recover,
        "knockout" => MessageTone::Knockout,
        "status_exit" => MessageTone::StatusExit,
        _ => MessageTone::Normal,
    }
}
fn update_from_core(update: BattleUpdate) -> UpdateView {
    UpdateView {
        score: update.score,
        delay0: update.delay0,
        delay1: update.delay1,
        caster_id: update.caster_id.unwrap_or(0),
        target_id: update.target_id.unwrap_or(0),
        target_ids: update.target_ids,
        update_type: match update.update_type {
            "win" => UpdateTypeView::Win,
            "next_line" => UpdateTypeView::NextLine,
            _ => UpdateTypeView::None,
        },
        status_change_tokens: status_change_tokens(&update.message_template),
        message_template: update.message_template,
        message_rendered: update.message_rendered,
        param: update.param,
        hp_delta: update.hp_delta,
        tone: tone(update.tone),
    }
}
fn frame_from_core(frame: BattleReplayFrame) -> RoundFrame {
    RoundFrame {
        finished: frame.finished,
        winner_ids: frame.winner_ids,
        updates: frame.updates.into_iter().map(update_from_core).collect(),
        states: frame.states.iter().map(state_from_core).collect(),
        total_delay: frame.total_delay,
        rows: frame
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
                        tone: tone(clip.tone),
                        caster_ids: clip.caster_ids,
                        target_ids: clip.target_ids,
                        sidebar_states: clip.sidebar_states.iter().map(state_from_core).collect(),
                        sidebar_previous_states: clip.sidebar_previous_states.iter().map(state_from_core).collect(),
                        winner: clip.winner,
                        parts: clip
                            .parts
                            .into_iter()
                            .map(|part| ReplayTextPart {
                                kind: match part.kind {
                                    "highlight" => ReplayTextPartKind::Highlight,
                                    "player" => ReplayTextPartKind::Player,
                                    "data" => ReplayTextPartKind::Data,
                                    _ => ReplayTextPartKind::Text,
                                },
                                text: part.text,
                                player_id: part.player_id,
                                show_hp: part.show_hp,
                                hp_before: part.hp_before,
                                hp_after: part.hp_after,
                                death_effect: part.death_effect,
                                emoji: part.emoji,
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[wasm_bindgen]
pub struct FightSession {
    session: BattleSession,
    players: Vec<PlayerMeta>,
    capture_replay: bool,
}
impl FightSession {
    pub fn new_internal(raw_input: String, options: FightOptions) -> WasmResult<Self> {
        let session = BattleSession::new(
            &raw_input,
            BattleOptions {
                eval_rq: options.resolved_eval_rq(),
                include_icons: options.include_icons(),
                ..BattleOptions::default()
            },
        )
        .map_err(cli_api_error)?;
        let players = session
            .initial_states()
            .iter()
            .map(|state| PlayerMeta {
                id: state.id,
                team_index: state.team_index,
                id_name: state.id_name.clone(),
                icon_key: state.icon_key.clone(),
                display_name: state.display_name.clone(),
                icon_png_base64: state.icon_png_base64.clone(),
            })
            .collect();
        Ok(Self {
            session,
            players,
            capture_replay: options.capture_replay(),
        })
    }

    /// `limit` 限制本次收集批次，但不改变规范会话预算。
    pub fn run_to_end_internal(&mut self, limit: Option<usize>) -> WasmResult<FightReplay> {
        let mut frames = Vec::new();
        for _ in 0..limit.unwrap_or(usize::MAX) {
            let Some(frame) = self.session.next_frame().map_err(cli_api_error)? else {
                break;
            };
            if self.capture_replay {
                frames.push(frame_from_core(frame));
            }
        }
        Ok(FightReplay {
            players: self.players.clone(),
            frames,
            winner_ids: self.winner_ids().0,
            final_states: self.state()?,
        })
    }
}

#[wasm_bindgen]
impl FightSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, options: Option<FightOptions>) -> WasmResult<FightSession> {
        crate::install_panic_hook();
        Self::new_internal(raw_input, options.unwrap_or_default())
    }
    pub fn players(&self) -> Vec<PlayerMeta> { self.players.clone() }
    pub fn state(&self) -> WasmResult<Vec<PlayerState>> {
        Ok(self.session.current_states().iter().map(state_from_core).collect())
    }
    pub fn is_finished(&self) -> bool { self.session.is_finished() }
    pub fn is_done(&self) -> bool { self.session.is_done() }
    pub fn winner_ids(&self) -> WinnerIds { WinnerIds(self.session.result().map(|result| result.winner_ids).unwrap_or_default()) }
    pub fn step(&mut self) -> WasmResult<RoundFrame> {
        match self.session.next_frame().map_err(cli_api_error)? {
            Some(frame) => Ok(frame_from_core(frame)),
            None => Ok(RoundFrame {
                finished: self.session.is_finished(),
                winner_ids: self.winner_ids().0,
                updates: Vec::new(),
                rows: Vec::new(),
                states: self.state()?,
                total_delay: 0,
            }),
        }
    }
    pub fn run_to_end(&mut self, limit: Option<usize>) -> WasmResult<FightReplay> { self.run_to_end_internal(limit) }
}

pub fn fight_impl(raw_input: String, options: FightOptions) -> WasmResult<FightReplay> {
    FightSession::new_internal(raw_input, options)?.run_to_end_internal(None)
}
pub fn fight_summary_impl(raw_input: String, options: FightOptions) -> WasmResult<FightSummary> {
    let mut session = FightSession::new_internal(raw_input, options)?;
    let replay = session.run_to_end_internal(None)?;
    Ok(FightSummary {
        finished: session.is_finished(),
        players: replay.players,
        winner_ids: replay.winner_ids,
        final_states: replay.final_states,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_player_state_preserves_direct_owner_id() {
        let state = BattlePlayerState {
            id: 9,
            team_index: 0,
            input_team_index: Some(0),
            owner_id: Some(7),
            source_id: Some(3),
            id_name: "minion".into(),
            id_key_name: "minion".into(),
            icon_key: "minion".into(),
            display_name: "minion".into(),
            display_index: 0,
            base_name: "minion".into(),
            player_type: "minion".into(),
            minion_kind: Some("clone"),
            icon_png_base64: None,
            hp: 100,
            max_hp: 100,
            magic_point: 0,
            move_point: 0,
            attack: 0,
            defense: 0,
            speed: 0,
            agility: 0,
            magic: 0,
            resistance: 0,
            wisdom: 0,
            point: 0,
            all_sum: 0,
            name_factor: 1.0,
            at_boost: 1.0,
            attract: 0.0,
            frozen: false,
            alive: true,
            active: true,
            status_labels: Vec::new(),
        };
        let original = state.clone();
        let legacy = state_from_core(&state);
        assert_eq!(legacy.owner_id, Some(7));
        assert_eq!(state, original);
    }

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

    #[test]
    fn compatibility_batches_and_step_share_canonical_progress() {
        let raw = "left@red\n\nright@blue\n";
        let mut legacy = FightSession::new_internal(raw.into(), FightOptions::default()).unwrap();
        let mut canonical = BattleSession::new(raw, BattleOptions::default()).unwrap();
        let first = legacy.run_to_end_internal(Some(1)).unwrap();
        assert_eq!(first.frames.len(), 1);
        assert_eq!(
            first.frames[0].total_delay,
            canonical.next_frame().unwrap().unwrap().total_delay
        );
        assert_eq!(legacy.session.rounds_advanced(), canonical.rounds_advanced());
        while let Some(expected) = canonical.next_frame().unwrap() {
            let actual = legacy.step().unwrap();
            assert_eq!(actual.finished, expected.finished);
            assert_eq!(actual.winner_ids, expected.winner_ids);
            assert_eq!(actual.total_delay, expected.total_delay);
            assert_eq!(actual.rows.len(), expected.rows.len());
            for (row, expected_row) in actual.rows.iter().zip(&expected.rows) {
                for (clip, expected_clip) in row.clips.iter().zip(&expected_row.clips) {
                    assert_eq!(clip.parts.len(), expected_clip.parts.len());
                    for (part, expected_part) in clip.parts.iter().zip(&expected_clip.parts) {
                        assert_eq!(part.text, expected_part.text);
                        assert_eq!(part.hp_before, expected_part.hp_before);
                        assert_eq!(part.hp_after, expected_part.hp_after);
                        assert_eq!(part.death_effect, expected_part.death_effect);
                    }
                }
            }
        }
        assert!(legacy.is_done() && legacy.is_finished());
        assert!(legacy.step().unwrap().updates.is_empty());
        assert_eq!(legacy.session.result(), canonical.result());
    }

    #[test]
    fn capture_disabled_still_obeys_batch_limit_and_completes() {
        let mut legacy = FightSession::new_internal(
            "left@red\n\nright@blue\n".into(),
            FightOptions {
                capture_replay: Some(false),
                ..FightOptions::default()
            },
        )
        .unwrap();
        let partial = legacy.run_to_end_internal(Some(1)).unwrap();
        assert!(partial.frames.is_empty());
        assert_eq!(legacy.session.frames_emitted(), 1);
        let complete = legacy.run_to_end_internal(None).unwrap();
        assert!(complete.frames.is_empty());
        assert!(!complete.winner_ids.is_empty());
        assert!(legacy.is_finished());
    }
}

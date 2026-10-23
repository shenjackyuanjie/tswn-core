//! 从规范 BattleSession 投影出的旧页面兼容 API。
//!
//! 保留旧 PlayerState/UpdateView/空终止帧形状，规则和预算仍由 core 会话决定。
//! 所有推进同步执行，JS 显式 free 释放会话；返回的 DTO 为独立副本。
//! 需要区分截断原因或检查 sticky failure 时使用规范 BattleSession。
use crate::convert::{ts_in, ts_out, ts_out_vec};
use crate::error::{WasmResult, cli_api_error};
use crate::model::{
    FightOptions, FightReplay, FightSummary, MessageTone, MinionKindView, PlayerMeta, PlayerState, ReplayClip, ReplayRow,
    ReplayTextPart, ReplayTextPartKind, RoundFrame, UpdateTypeView, UpdateView, WinnerIds,
};
use crate::render::status_change_tokens;
use tsify::Ts;
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
        shield: state.shield,
        max_shield: state.max_shield,
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
/// 持有规范会话的兼容包装；capture_replay 仅控制批量收集是否保存帧，不阻止计算。
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

    /// 从当前状态同步收集至多 limit 个规范帧，不改变会话总回合预算，也不重新开局。
    /// capture_replay=false 时结果 frames 为空，但仍按相同 limit 推进运行时。
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
            winner_ids: self.winner_ids_value().0,
            final_states: self.state_vec(),
        })
    }

    /// 当前全部玩家状态的 Rust 视图，供兼容包装与测试复用。
    fn state_vec(&self) -> Vec<PlayerState> { self.session.current_states().iter().map(state_from_core).collect() }

    /// 当前胜者 id 的 Rust 视图，供兼容包装与测试复用。
    fn winner_ids_value(&self) -> WinnerIds {
        WinnerIds(self.session.result().map(|result| result.winner_ids).unwrap_or_default())
    }

    /// 推进一步并返回该帧的 Rust 视图；已结束且无新帧时返回占位空帧。
    fn step_internal(&mut self) -> WasmResult<RoundFrame> {
        match self.session.next_frame().map_err(cli_api_error)? {
            Some(frame) => Ok(frame_from_core(frame)),
            None => Ok(RoundFrame {
                finished: self.session.is_finished(),
                winner_ids: self.winner_ids_value().0,
                updates: Vec::new(),
                rows: Vec::new(),
                states: self.state_vec(),
                total_delay: 0,
            }),
        }
    }
}

#[wasm_bindgen]
impl FightSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, options: Option<Ts<FightOptions>>) -> WasmResult<FightSession> {
        crate::install_panic_hook();
        let options = ts_in(options)?;
        Self::new_internal(raw_input, options)
    }
    pub fn players(&self) -> WasmResult<Vec<Ts<PlayerMeta>>> { ts_out_vec(&self.players) }
    pub fn state(&self) -> WasmResult<Vec<Ts<PlayerState>>> { ts_out_vec(&self.state_vec()) }
    pub fn is_finished(&self) -> bool { self.session.is_finished() }
    pub fn is_done(&self) -> bool { self.session.is_done() }
    pub fn winner_ids(&self) -> WasmResult<Ts<WinnerIds>> { ts_out(&self.winner_ids_value()) }
    pub fn step(&mut self) -> WasmResult<Ts<RoundFrame>> { self.step_internal().and_then(|frame| ts_out(&frame)) }
    pub fn run_to_end(&mut self, limit: Option<usize>) -> WasmResult<Ts<FightReplay>> {
        self.run_to_end_internal(limit).and_then(|replay| ts_out(&replay))
    }
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
            shield: 25,
            max_shield: 80,
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
        assert_eq!(legacy.shield, 25);
        assert_eq!(legacy.max_shield, 80);
        assert_eq!(state, original);
    }

    #[test]
    fn legacy_shield_projection_covers_initial_frame_sidebar_and_batch_final_states() {
        let raw = r#"left@red+ol:{"skills":{"shield":80}}

right@blue+ol:{"skills":{}}
seed:7@!"#;
        let mut legacy = FightSession::new_internal(raw.into(), FightOptions::default()).unwrap();
        assert!(legacy.state_vec().iter().all(|state| state.shield == 0));
        assert_eq!(legacy.state_vec()[0].max_shield, 80);
        let mut canonical = BattleSession::new(raw, BattleOptions::default()).unwrap();
        let batch = legacy.run_to_end_internal(Some(3)).unwrap();
        assert_eq!(batch.frames.len(), 3);
        for frame in &batch.frames {
            let expected = canonical.next_frame().unwrap().unwrap();
            assert_eq!(
                frame.states.iter().map(|state| state.max_shield).collect::<Vec<_>>(),
                expected.states.iter().map(|state| state.max_shield).collect::<Vec<_>>()
            );
            assert_eq!(
                frame.states.iter().map(|state| state.shield).collect::<Vec<_>>(),
                expected.states.iter().map(|state| state.shield).collect::<Vec<_>>()
            );
            for (row, expected_row) in frame.rows.iter().zip(&expected.rows) {
                for (clip, expected_clip) in row.clips.iter().zip(&expected_row.clips) {
                    assert_eq!(
                        clip.sidebar_states.iter().map(|state| state.max_shield).collect::<Vec<_>>(),
                        expected_clip.sidebar_states.iter().map(|state| state.max_shield).collect::<Vec<_>>()
                    );
                    assert_eq!(
                        clip.sidebar_previous_states.iter().map(|state| state.max_shield).collect::<Vec<_>>(),
                        expected_clip
                            .sidebar_previous_states
                            .iter()
                            .map(|state| state.max_shield)
                            .collect::<Vec<_>>()
                    );
                    assert_eq!(
                        clip.sidebar_states.iter().map(|state| state.shield).collect::<Vec<_>>(),
                        expected_clip.sidebar_states.iter().map(|state| state.shield).collect::<Vec<_>>()
                    );
                    assert_eq!(
                        clip.sidebar_previous_states.iter().map(|state| state.shield).collect::<Vec<_>>(),
                        expected_clip.sidebar_previous_states.iter().map(|state| state.shield).collect::<Vec<_>>()
                    );
                }
            }
        }
        assert!(batch.final_states.iter().any(|state| state.shield > 0));
        assert_eq!(batch.final_states[0].max_shield, 80);
        assert_eq!(
            batch.final_states.iter().map(|state| state.shield).collect::<Vec<_>>(),
            canonical.current_states().iter().map(|state| state.shield).collect::<Vec<_>>()
        );
        let complete = legacy.run_to_end_internal(None).unwrap();
        assert!(complete.final_states.iter().filter(|state| !state.alive).all(|state| state.shield == 0));
        assert!(<PlayerState as tsify::Tsify>::DECL.contains("shield: number"));
    }

    #[test]
    fn legacy_half_percent_message_preserves_damage_tone_and_hp_parts() {
        let raw =
            "My_Generation lrSP5W8@芒萁\nUnbound v7mEDujiDSLN@芒萁\n\n潮汐-糖藷愠颏@涵虚\n风驰-xtxxgwcb@涵虚\nseed:33554538@!";
        let mut canonical = BattleSession::new(raw, BattleOptions::default()).unwrap();
        while let Some(frame) = canonical.next_frame().unwrap() {
            let Some(update_index) = frame
                .updates
                .iter()
                .position(|update| update.message_template == "[1]体力减少[2]%" && update.score > 0)
            else {
                continue;
            };
            let actual = frame_from_core(frame.clone());
            let update = &actual.updates[update_index];
            assert!(matches!(update.tone, MessageTone::Damage));
            assert_eq!(update.hp_delta, frame.updates[update_index].hp_delta);
            let expected = frame
                .rows
                .iter()
                .flat_map(|row| &row.clips)
                .find(|clip| {
                    clip.tone == "damage"
                        && clip.parts.iter().any(|part| part.player_id == Some(update.target_id) && part.show_hp)
                })
                .unwrap();
            let clip = actual
                .rows
                .iter()
                .flat_map(|row| &row.clips)
                .find(|clip| {
                    matches!(clip.tone, MessageTone::Damage)
                        && clip.parts.iter().any(|part| part.player_id == Some(update.target_id) && part.show_hp)
                })
                .unwrap();
            for (part, expected_part) in clip.parts.iter().zip(&expected.parts) {
                assert_eq!(part.show_hp, expected_part.show_hp);
                assert_eq!(
                    (part.hp_before, part.hp_after),
                    (expected_part.hp_before, expected_part.hp_after)
                );
            }
            return;
        }
        panic!("固定输入应产生瘟疫百分比伤害句");
    }

    #[test]
    fn fight_session_uses_runtime_and_keeps_parts_only_clip_contract() {
        let mut session = FightSession::new_internal("left@red\n\nright@blue\n".to_owned(), FightOptions::default()).unwrap();
        assert_eq!(session.players.iter().map(|player| player.id).collect::<Vec<_>>(), vec![0, 1]);

        let frame = session.step_internal().unwrap();
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
            let actual = legacy.step_internal().unwrap();
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
        assert!(legacy.step_internal().unwrap().updates.is_empty());
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

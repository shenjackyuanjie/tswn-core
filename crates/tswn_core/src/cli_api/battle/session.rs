use std::collections::HashMap;

use crate::namerena::icon_render::render_icon_b64_from_name;
use crate::replay_view::{
    ReplayClip, ReplayEventView, ReplayRow, ReplayTextPart, ReplayTextPartKind, ReplayTone, build_replay_view_frame,
    hp_delta_for_tone, render_update_message,
};
use crate::runtime::update::{RunUpdates, UpdateType};
use crate::runtime::{PlrId, RuntimeMinionKind, RuntimePlayerSnapshot, RuntimeRunner};

use super::dto::*;
use crate::cli_api::{CliApiError, CliApiResult, invalid_input};

/// 有状态、增量式用户 API。所有回合限制和终止决策均在此处处理。
#[derive(Debug)]
pub struct BattleSession {
    runner: RuntimeRunner,
    options: BattleOptions,
    initial_states: Vec<BattlePlayerState>,
    current_states: Vec<BattlePlayerState>,
    rounds_advanced: usize,
    frames_emitted: usize,
    no_progress_rounds: usize,
    stop_reason: Option<BattleStopReason>,
    failure: Option<CliApiError>,
    icon_cache: HashMap<String, String>,
}

const NO_PROGRESS_ROUNDS_PER_ENTITY: usize = 16;

impl BattleSession {
    pub fn new(raw: &str, options: BattleOptions) -> CliApiResult<Self> {
        if raw.trim().is_empty() {
            return Err(invalid_input("raw_input is empty"));
        }
        if options.max_rounds == 0 {
            return Err(CliApiError::InvalidArgument("battle max_rounds must be positive".into()));
        }
        if !options.eval_rq.is_finite() {
            return Err(CliApiError::InvalidArgument("battle eval_rq must be finite".into()));
        }
        let (groups, seed) = RuntimeRunner::split_namerena_into_groups(raw.to_owned());
        let runner = RuntimeRunner::new_from_groups_with_seed_and_eval_rq(&groups, &seed, options.eval_rq)
            .map_err(|err| CliApiError::RunnerInit(err.to_string()))?;
        let mut icon_cache = HashMap::new();
        let initial_states = states_from_runner(&runner, options.include_icons, &mut icon_cache);
        let stop_reason = runner.have_winner().then_some(BattleStopReason::Winner);
        Ok(Self {
            runner,
            options,
            current_states: initial_states.clone(),
            initial_states,
            rounds_advanced: 0,
            frames_emitted: 0,
            no_progress_rounds: 0,
            stop_reason,
            failure: None,
            icon_cache,
        })
    }

    /// 仅用于测试：破坏 skill handlers，让下一次推进经过真实 Runtime validation 失败。
    #[cfg(any(test, feature = "battle-test-support"))]
    #[doc(hidden)]
    pub fn invalidate_runtime_for_test(&mut self) { self.runner.runtime_mut().skill_handlers = Default::default(); }

    pub fn initial_states(&self) -> &[BattlePlayerState] { &self.initial_states }
    pub fn current_states(&self) -> &[BattlePlayerState] { &self.current_states }
    pub fn status(&self) -> BattleStatus {
        match self.stop_reason {
            None => BattleStatus::Running,
            Some(BattleStopReason::Winner) => BattleStatus::Finished,
            Some(_) => BattleStatus::Truncated,
        }
    }
    pub fn stop_reason(&self) -> Option<BattleStopReason> { self.stop_reason }
    /// 是否已产生正常 terminal BattleResult；Runtime failure 不属于正常终止。
    pub fn is_done(&self) -> bool { self.stop_reason.is_some() }
    /// Runtime error 后为 true；后续推进返回相同错误，调用方应停止推进并释放会话。
    pub fn is_failed(&self) -> bool { self.failure.is_some() }
    pub fn is_finished(&self) -> bool { self.status() == BattleStatus::Finished }
    pub fn is_truncated(&self) -> bool { self.status() == BattleStatus::Truncated }
    pub fn rounds_advanced(&self) -> usize { self.rounds_advanced }
    pub fn frames_emitted(&self) -> usize { self.frames_emitted }

    /// 推进经过空的 Runtime 回合，直至出现可见帧或终止结果。
    pub fn next_frame(&mut self) -> CliApiResult<Option<BattleReplayFrame>> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.is_done() {
            return Ok(None);
        }
        loop {
            // Runtime 的检查入口会在无效处理器配置时 panic；进入它之前先转换该校验失败。
            if let Err(error) = self.runner.validate_ready() {
                let error = CliApiError::Runtime(error.to_string());
                self.failure = Some(error.clone());
                return Err(error);
            }
            let round_index = self.rounds_advanced;
            let updates = self.runner.main_round();
            self.rounds_advanced += 1;
            let states = states_from_runner(&self.runner, self.options.include_icons, &mut self.icon_cache);
            let visible = !updates.updates.is_empty() || self.runner.have_winner();
            if visible {
                self.no_progress_rounds = 0;
            } else {
                self.no_progress_rounds += 1;
            }
            // 即使是允许的最后一回合或空更新，胜者也优先。
            self.stop_reason = if self.runner.have_winner() {
                Some(BattleStopReason::Winner)
            } else if self.rounds_advanced >= self.options.max_rounds {
                Some(BattleStopReason::MaxRounds)
            } else if self.no_progress_rounds >= states.len().max(1).saturating_mul(NO_PROGRESS_ROUNDS_PER_ENTITY) {
                Some(BattleStopReason::NoProgress)
            } else {
                None
            };
            let frame = visible.then(|| {
                let mut frame = build_frame(&updates, &self.current_states, &states, &self.runner);
                frame.frame_index = self.frames_emitted;
                frame.round_index = round_index;
                self.frames_emitted += 1;
                frame
            });
            self.current_states = states;
            if frame.is_some() || self.is_done() {
                return Ok(frame);
            }
        }
    }

    pub fn result(&self) -> Option<BattleResult> {
        Some(BattleResult {
            status: self.status(),
            stop_reason: self.stop_reason?,
            finished: self.is_finished(),
            truncated: self.is_truncated(),
            rounds_advanced: self.rounds_advanced,
            frames_emitted: self.frames_emitted,
            winner_ids: if self.is_finished() {
                self.runner.winner_ids()
            } else {
                Vec::new()
            },
            winner_team_indices: if self.is_finished() {
                self.runner.winner_team_indices()
            } else {
                Vec::new()
            },
            final_states: self.current_states.clone(),
        })
    }
}

pub(super) fn states_from_runner(
    runner: &RuntimeRunner,
    include_icons: bool,
    icon_cache: &mut HashMap<String, String>,
) -> Vec<BattlePlayerState> {
    runner
        .player_snapshots()
        .into_iter()
        .map(|snapshot| {
            let mut state = state_from_snapshot(snapshot, false);
            if include_icons {
                state.icon_png_base64 = Some(
                    icon_cache
                        .entry(state.icon_key.clone())
                        .or_insert_with(|| render_icon_b64_from_name(&state.icon_key))
                        .clone(),
                );
            }
            state
        })
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

pub(super) fn build_frame(
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
        frame_index: 0,
        round_index: 0,
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
    use crate::runtime::EntityIdx;

    fn session(max_rounds: usize) -> BattleSession {
        BattleSession::new(
            "left@red\n\nright@blue\n",
            BattleOptions {
                max_rounds,
                ..BattleOptions::default()
            },
        )
        .unwrap()
    }

    fn remove_round_actors(session: &mut BattleSession) {
        for id in session.runner.all_player_ids() {
            session.runner.runtime_mut().world.remove_round_actor(EntityIdx(id as u32));
        }
    }

    #[test]
    fn initial_and_terminal_queries_are_consistent_and_idempotent() {
        let mut session = session(20_000);
        assert_eq!(session.initial_states(), session.current_states());
        assert_eq!(session.status(), BattleStatus::Running);
        assert_eq!(session.stop_reason(), None);
        assert_eq!(session.result(), None);
        let initial = session.initial_states().to_vec();
        let mut count = 0;
        let mut previous_round = None;
        while let Some(frame) = session.next_frame().unwrap() {
            assert_eq!(frame.frame_index, count);
            assert!(previous_round.is_none_or(|previous| frame.round_index > previous));
            previous_round = Some(frame.round_index);
            assert_eq!(frame.states, session.current_states());
            count += 1;
        }
        assert_eq!(initial, session.initial_states());
        let result = session.result().unwrap();
        assert_eq!(result.status, BattleStatus::Finished);
        assert_eq!(result.stop_reason, BattleStopReason::Winner);
        assert!(result.finished && !result.truncated);
        assert_eq!(result.frames_emitted, count);
        assert!(result.rounds_advanced >= count);
        assert!(!result.winner_ids.is_empty());
        for _ in 0..10 {
            assert_eq!(session.next_frame().unwrap(), None);
            assert_eq!(session.result().unwrap(), result);
        }
    }

    #[test]
    fn empty_rounds_count_towards_max_rounds() {
        let mut session = session(3);
        remove_round_actors(&mut session);
        assert_eq!(session.next_frame().unwrap(), None);
        let result = session.result().unwrap();
        assert_eq!(result.rounds_advanced, 3);
        assert_eq!(result.frames_emitted, 0);
        assert_eq!(result.stop_reason, BattleStopReason::MaxRounds);
        assert!(result.truncated && !result.finished);
        assert!(result.winner_ids.is_empty() && result.winner_team_indices.is_empty());
    }

    #[test]
    fn no_progress_stops_at_entity_scaled_limit_and_max_rounds_has_priority() {
        for (max_rounds, expected) in [(100, BattleStopReason::NoProgress), (32, BattleStopReason::MaxRounds)] {
            let mut session = session(max_rounds);
            remove_round_actors(&mut session);
            assert_eq!(session.next_frame().unwrap(), None);
            assert_eq!(session.rounds_advanced(), 32);
            assert_eq!(session.stop_reason(), Some(expected));
            assert_eq!(session.status(), BattleStatus::Truncated);
        }
    }

    #[test]
    fn winner_on_empty_update_emits_frame_and_beats_round_limit() {
        let mut session = session(1);
        let runtime = session.runner.runtime_mut();
        let loser = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        loser.runtime.hp = 0;
        loser.runtime.alive = false;
        let frame = session.next_frame().unwrap().unwrap();
        assert!(frame.updates.is_empty());
        assert!(frame.finished);
        assert_eq!(frame.frame_index, 0);
        assert_eq!(frame.round_index, 0);
        assert_eq!(frame.winner_ids, vec![0]);
        assert!(frame.rows.iter().flat_map(|row| &row.clips).any(|clip| clip.winner));
        assert_eq!(session.stop_reason(), Some(BattleStopReason::Winner));
        assert_eq!(frame.states, session.result().unwrap().final_states);
    }

    #[test]
    fn visible_winner_on_last_round_beats_round_limit() {
        let mut full = session(20_000);
        while full.next_frame().unwrap().is_some() {}
        let mut limited = session(full.rounds_advanced());
        while limited.next_frame().unwrap().is_some() {}
        assert_eq!(limited.result(), full.result());
        assert!(limited.is_finished());
    }

    #[test]
    fn icons_are_cached_per_key_and_disabled_by_default() {
        let mut plain = session(3);
        assert!(plain.current_states().iter().all(|state| state.icon_png_base64.is_none()));
        plain.next_frame().unwrap();
        assert!(plain.icon_cache.is_empty());
        let mut icons = BattleSession::new(
            "left@red\n\nright@blue\n",
            BattleOptions {
                include_icons: true,
                max_rounds: 3,
                ..BattleOptions::default()
            },
        )
        .unwrap();
        let initial_cache = icons.icon_cache.clone();
        while icons.next_frame().unwrap().is_some() {}
        for state in icons.current_states() {
            assert_eq!(state.icon_png_base64.as_ref(), icons.icon_cache.get(&state.icon_key));
        }
        for (key, value) in initial_cache {
            assert_eq!(icons.icon_cache.get(&key), Some(&value));
        }
    }

    #[test]
    fn rejects_invalid_input_and_options_with_core_codes() {
        assert_eq!(
            BattleSession::new(" ", BattleOptions::default()).unwrap_err().code().as_str(),
            "INVALID_INPUT"
        );
        for options in [
            BattleOptions {
                max_rounds: 0,
                ..BattleOptions::default()
            },
            BattleOptions {
                eval_rq: f64::NAN,
                ..BattleOptions::default()
            },
        ] {
            assert_eq!(
                BattleSession::new("a\n\nb", options).unwrap_err().code().as_str(),
                "INVALID_ARGUMENT"
            );
        }
    }

    #[test]
    fn runtime_error_is_sticky_and_does_not_become_a_truncated_result() {
        let mut session = BattleSession::new(
            "alpha@red+bed2[3000]\n\nbeta@blue",
            BattleOptions {
                max_rounds: 1,
                ..BattleOptions::default()
            },
        )
        .unwrap();
        assert!(!session.is_failed());
        session.invalidate_runtime_for_test();
        // 同时安排一个胜者：无效 Runtime 配置必须具有优先级。
        session.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.alive = false;
        let mut previous_error = None;
        for _ in 0..2 {
            let error = session.next_frame().unwrap_err();
            assert!(session.is_failed());
            assert!(!session.is_done());
            assert_eq!(session.status(), BattleStatus::Running);
            assert!(!session.is_truncated());
            let current_error = (error.code().as_str(), error.to_string());
            if let Some(previous) = &previous_error {
                assert_eq!(&current_error, previous);
            }
            previous_error = Some(current_error);
            assert_eq!(error.code().as_str(), "RUNTIME_FAILED");
            assert_eq!(session.result(), None);
            assert_eq!(session.stop_reason(), None);
            assert_eq!(session.rounds_advanced(), 0);
        }
    }
}

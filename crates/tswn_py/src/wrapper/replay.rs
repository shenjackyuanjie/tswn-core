use std::collections::HashMap;

use pyo3::{
    IntoPyObjectExt, PyResult,
    types::{PyDict, PyDictMethods, PyList, PyListMethods},
};
use tswn_core::{
    RunUpdate, Runner,
    replay_view::{
        ReplayEventView, ReplayRow as CoreReplayRow, ReplayState, ReplayTextPart as CoreReplayTextPart,
        ReplayTextPartKind as CoreReplayTextPartKind, ReplayTone, ReplayViewFrame, WIN_UPDATE_DELAY0_MS, build_replay_view_frame,
        render_update_message as core_render_update_message,
    },
    runtime::PlrId,
    runtime::update::{RunUpdates, UpdateType},
    runtime::{BINDING_COMPLETION_MAX_ROUNDS, RuntimePlayerSnapshot as CorePlayerSnapshot},
};

#[derive(Clone)]
pub struct PlayerSnapshot {
    pub id: PlrId,
    pub team_index: Option<usize>,
    pub input_team_index: Option<usize>,
    pub owner_id: Option<PlrId>,
    pub source_id: Option<PlrId>,
    pub display_order: usize,
    pub id_name: String,
    pub id_key_name: String,
    pub icon_key: String,
    pub display_name: String,
    pub display_index: usize,
    pub base_name: String,
    pub player_type: String,
    pub minion_kind: Option<&'static str>,
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
    pub alive: bool,
    pub active: bool,
    pub frozen: bool,
}

impl ReplayState for PlayerSnapshot {
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

#[derive(Clone)]
pub struct EventDto {
    pub update_type: &'static str,
    pub tone: &'static str,
    pub message_template: String,
    pub message_rendered: String,
    pub caster_id: Option<PlrId>,
    pub target_id: Option<PlrId>,
    pub target_ids: Vec<PlrId>,
    pub param: Option<u32>,
    pub score: Option<u32>,
    pub delay0: i32,
    pub delay1: i32,
    pub is_win: bool,
    pub is_next_line: bool,
}

pub fn winner_names(runner: &Runner) -> Vec<String> {
    runner
        .winner_ids()
        .into_iter()
        .map(|id| {
            runner
                .player_snapshot(id)
                .map(|player| player.display_name)
                .unwrap_or_else(|| format!("#{id}"))
        })
        .collect()
}

pub fn snapshot_players(runner: &Runner) -> Vec<PlayerSnapshot> {
    runner
        .player_snapshots()
        .into_iter()
        .enumerate()
        .map(|(display_order, snapshot)| snapshot_from_core(snapshot, display_order))
        .collect()
}

fn snapshot_from_core(snapshot: CorePlayerSnapshot, display_order: usize) -> PlayerSnapshot {
    PlayerSnapshot {
        id: snapshot.id,
        team_index: Some(snapshot.team_index),
        input_team_index: snapshot.input_team_index,
        owner_id: snapshot.owner_id,
        source_id: snapshot.root_owner_id,
        display_order,
        id_name: snapshot.id_name,
        icon_key: snapshot.id_key_name.clone(),
        id_key_name: snapshot.id_key_name,
        display_name: snapshot.display_name,
        display_index: snapshot.display_index,
        base_name: snapshot.base_name,
        player_type: snapshot.player_type.to_owned(),
        minion_kind: snapshot.minion_kind.map(|kind| kind.as_str()),
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
        alive: snapshot.alive,
        active: snapshot.active,
        frozen: snapshot.frozen,
    }
}

fn update_type_str(update_type: UpdateType) -> &'static str {
    match update_type {
        UpdateType::Win => "win",
        UpdateType::None => "none",
        UpdateType::NextLine => "next_line",
    }
}

fn display_name_for_snapshot(state: &PlayerSnapshot) -> String {
    if state.minion_kind == Some("clone") && state.display_index > 0 {
        format!("{} #{}", state.display_name, state.display_index)
    } else {
        state.display_name.clone()
    }
}

pub fn player_names_from_snapshots(states: &[PlayerSnapshot]) -> HashMap<PlrId, String> {
    states.iter().map(|state| (state.id, display_name_for_snapshot(state))).collect()
}

pub fn render_update_message(update: &RunUpdate, names: &HashMap<PlrId, String>) -> String {
    core_render_update_message(update, names)
}

fn classify_message_tone(update: &RunUpdate, rendered: &str) -> &'static str {
    if matches!(update.update_type, UpdateType::Win) {
        return "win";
    }
    if matches!(update.update_type, UpdateType::NextLine) {
        return "line";
    }

    let template = update.message.as_ref();
    if template.contains("回复体力") || rendered.contains("回复体力") {
        "recover"
    } else if template.contains("被击倒")
        || rendered.contains("被击倒")
        || template.contains("消失了")
        || rendered.contains("消失了")
    {
        "knockout"
    } else if template.contains("回避") || rendered.contains("回避") {
        "dodge"
    } else if template.contains("点伤害") || rendered.contains("点伤害") {
        "damage"
    } else if is_status_exit_message(template) || is_status_exit_message(rendered) {
        "status_exit"
    } else if template.contains("状态")
        || rendered.contains("状态")
        || template.contains("解除")
        || rendered.contains("解除")
        || template.contains("中毒")
        || rendered.contains("中毒")
        || template.contains("冰冻")
        || rendered.contains("冰冻")
    {
        "status"
    } else if template.contains("使用")
        || rendered.contains("使用")
        || template.contains("发动")
        || rendered.contains("发动")
        || template.contains("发起攻击")
        || rendered.contains("发起攻击")
        || template.contains("召唤")
        || rendered.contains("召唤")
        || template.contains("出现")
        || rendered.contains("出现")
    {
        "action"
    } else {
        "normal"
    }
}

fn replay_tone_from_event_tone(tone: &str) -> ReplayTone {
    match tone {
        "damage" => ReplayTone::Damage,
        "recover" => ReplayTone::Recover,
        "knockout" | "win" => ReplayTone::Knockout,
        "status_exit" => ReplayTone::StatusExit,
        _ => ReplayTone::Normal,
    }
}

fn replay_tone_to_str(tone: ReplayTone) -> &'static str {
    match tone {
        ReplayTone::Normal => "normal",
        ReplayTone::Damage => "damage",
        ReplayTone::Recover => "recover",
        ReplayTone::Knockout => "knockout",
        ReplayTone::StatusExit => "status_exit",
    }
}

fn is_status_exit_message(message: &str) -> bool {
    has_between(message, "从[", "]中解除")
        || has_between(message, "从[", "]状态中解除")
        || has_between(message, "的[", "]被识破")
        || has_between(message, "的[", "]被中止了")
        || has_between(message, "的[", "]被中止")
        || has_between(message, "的[", "]被打消了")
        || has_between(message, "的[", "]被打消")
        || has_between(message, "的[", "]属性被打消")
}

fn has_between(text: &str, prefix: &str, suffix: &str) -> bool {
    let mut rest = text;
    while let Some(start) = rest.find(prefix) {
        let after_prefix = &rest[start + prefix.len()..];
        let Some(end) = after_prefix.find(suffix) else {
            return false;
        };
        if !after_prefix[..end].is_empty() {
            return true;
        }
        rest = &after_prefix[end + suffix.len()..];
    }
    false
}

fn replay_text_part_kind_to_str(kind: CoreReplayTextPartKind) -> &'static str {
    match kind {
        CoreReplayTextPartKind::Text => "text",
        CoreReplayTextPartKind::Highlight => "highlight",
        CoreReplayTextPartKind::Player => "player",
        CoreReplayTextPartKind::Data => "data",
    }
}

pub fn update_to_dto(update: &RunUpdate, names: &HashMap<PlrId, String>) -> EventDto {
    let message_rendered = render_update_message(update, names);
    EventDto {
        update_type: update_type_str(update.update_type),
        tone: classify_message_tone(update, &message_rendered),
        message_template: update.message.to_string(),
        message_rendered,
        caster_id: (!matches!(update.update_type, UpdateType::NextLine)).then_some(update.caster),
        target_id: (!matches!(update.update_type, UpdateType::NextLine)).then_some(update.target),
        target_ids: update.targets.iter().copied().collect(),
        param: update.param,
        score: Some(update.score),
        delay0: update.delay0,
        delay1: update.delay1,
        is_win: matches!(update.update_type, UpdateType::Win),
        is_next_line: matches!(update.update_type, UpdateType::NextLine),
    }
}

fn is_visible_event(event: &EventDto) -> bool { !event.is_next_line && !event.message_rendered.trim().is_empty() }

pub fn compute_event_delays(
    events: &[EventDto],
    scale_by_player_count: bool,
    player_count: usize,
    first_visible_immediate: bool,
) -> Vec<i32> {
    let mut next_wait = 1800;
    let mut seen_visible = false;
    let divisor = if scale_by_player_count {
        ((player_count as f64 / 2.0).sqrt().round() as i32).max(1)
    } else {
        1
    };
    events
        .iter()
        .map(|event| {
            if !is_visible_event(event) {
                return 0;
            }
            if first_visible_immediate && !seen_visible {
                seen_visible = true;
                next_wait = event.delay1;
                return 0;
            }
            seen_visible = true;
            let raw_wait = event.delay0.max(next_wait);
            next_wait = event.delay1;
            raw_wait / divisor
        })
        .collect()
}

pub fn playback_total_delay(events: &[EventDto]) -> i32 {
    let mut total_delay = 0;
    let mut next_wait = 1800;
    for event in events {
        if !is_visible_event(event) {
            continue;
        }
        let wait = event.delay0.max(next_wait);
        total_delay += wait;
        next_wait = event.delay1;
    }
    total_delay
}

pub fn py_none(py: pyo3::Python<'_>) -> PyResult<pyo3::Bound<'_, pyo3::PyAny>> { py.None().into_bound_py_any(py) }

fn set_optional_usize(dict: &pyo3::Bound<'_, PyDict>, key: &str, value: Option<usize>) -> PyResult<()> {
    if let Some(value) = value {
        dict.set_item(key, value)
    } else {
        dict.set_item(key, py_none(dict.py())?)
    }
}

fn set_optional_u32(dict: &pyo3::Bound<'_, PyDict>, key: &str, value: Option<u32>) -> PyResult<()> {
    if let Some(value) = value {
        dict.set_item(key, value)
    } else {
        dict.set_item(key, py_none(dict.py())?)
    }
}

pub fn snapshot_to_pydict<'py>(py: pyo3::Python<'py>, snapshot: &PlayerSnapshot) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("id", snapshot.id)?;
    set_optional_usize(&dict, "team_index", snapshot.team_index)?;
    set_optional_usize(&dict, "input_team_index", snapshot.input_team_index)?;
    set_optional_usize(&dict, "owner_id", snapshot.owner_id)?;
    set_optional_usize(&dict, "source_id", snapshot.source_id)?;
    dict.set_item("display_order", snapshot.display_order)?;
    dict.set_item("id_name", &snapshot.id_name)?;
    dict.set_item("id_key_name", &snapshot.id_key_name)?;
    dict.set_item("icon_key", &snapshot.icon_key)?;
    dict.set_item("display_name", &snapshot.display_name)?;
    dict.set_item("display_index", snapshot.display_index)?;
    dict.set_item("base_name", &snapshot.base_name)?;
    dict.set_item("player_type", &snapshot.player_type)?;
    if let Some(minion_kind) = snapshot.minion_kind {
        dict.set_item("minion_kind", minion_kind)?;
    } else {
        dict.set_item("minion_kind", py_none(py)?)?;
    }
    dict.set_item("hp", snapshot.hp)?;
    dict.set_item("max_hp", snapshot.max_hp)?;
    dict.set_item("magic_point", snapshot.magic_point)?;
    dict.set_item("move_point", snapshot.move_point)?;
    dict.set_item("attack", snapshot.attack)?;
    dict.set_item("defense", snapshot.defense)?;
    dict.set_item("speed", snapshot.speed)?;
    dict.set_item("agility", snapshot.agility)?;
    dict.set_item("magic", snapshot.magic)?;
    dict.set_item("resistance", snapshot.resistance)?;
    dict.set_item("wisdom", snapshot.wisdom)?;
    dict.set_item("alive", snapshot.alive)?;
    dict.set_item("active", snapshot.active)?;
    dict.set_item("frozen", snapshot.frozen)?;
    Ok(dict)
}

pub fn snapshots_to_pylist<'py>(py: pyo3::Python<'py>, snapshots: &[PlayerSnapshot]) -> PyResult<pyo3::Bound<'py, PyList>> {
    let list = PyList::empty(py);
    for snapshot in snapshots {
        list.append(snapshot_to_pydict(py, snapshot)?)?;
    }
    Ok(list)
}

pub fn event_to_pydict<'py>(py: pyo3::Python<'py>, event: &EventDto) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("type", event.update_type)?;
    dict.set_item("update_type", event.update_type)?;
    dict.set_item("tone", event.tone)?;
    dict.set_item("message_template", &event.message_template)?;
    dict.set_item("message_rendered", &event.message_rendered)?;
    set_optional_usize(&dict, "caster_id", event.caster_id)?;
    set_optional_usize(&dict, "target_id", event.target_id)?;
    dict.set_item("target_ids", &event.target_ids)?;
    set_optional_u32(&dict, "param", event.param)?;
    set_optional_u32(&dict, "score", event.score)?;
    dict.set_item("delay0", event.delay0)?;
    dict.set_item("delay1", event.delay1)?;
    dict.set_item("is_win", event.is_win)?;
    dict.set_item("is_next_line", event.is_next_line)?;
    Ok(dict)
}

pub fn replay_text_part_to_pydict<'py>(py: pyo3::Python<'py>, part: &CoreReplayTextPart) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    dict.set_item("kind", replay_text_part_kind_to_str(part.kind))?;
    dict.set_item("text", &part.text)?;
    set_optional_usize(&dict, "player_id", part.player_id)?;
    dict.set_item("show_hp", part.show_hp)?;
    dict.set_item("hp_before", part.hp_before)?;
    dict.set_item("hp_after", part.hp_after)?;
    dict.set_item("death_effect", part.death_effect)?;
    if let Some(emoji) = &part.emoji {
        dict.set_item("emoji", emoji)?;
    } else {
        dict.set_item("emoji", py_none(py)?)?;
    }
    Ok(dict)
}

pub fn replay_row_to_pydict<'py>(
    py: pyo3::Python<'py>,
    row: &CoreReplayRow<PlayerSnapshot>,
) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    let clips = PyList::empty(py);
    for clip in &row.clips {
        let clip_dict = PyDict::new(py);
        clip_dict.set_item("delay", clip.delay)?;
        clip_dict.set_item("color", &clip.color)?;
        clip_dict.set_item("tone", replay_tone_to_str(clip.tone))?;

        let parts = PyList::empty(py);
        for part in &clip.parts {
            parts.append(replay_text_part_to_pydict(py, part)?)?;
        }
        clip_dict.set_item("parts", parts)?;
        clip_dict.set_item("caster_ids", &clip.caster_ids)?;
        clip_dict.set_item("target_ids", &clip.target_ids)?;
        clip_dict.set_item("sidebar_states", snapshots_to_pylist(py, &clip.sidebar_states)?)?;
        clip_dict.set_item(
            "sidebar_previous_states",
            snapshots_to_pylist(py, &clip.sidebar_previous_states)?,
        )?;
        clip_dict.set_item("winner", clip.winner)?;
        clips.append(clip_dict)?;
    }
    dict.set_item("indent", row.indent)?;
    dict.set_item("clips", clips)?;
    Ok(dict)
}

pub fn replay_view_frame_to_pydict<'py>(
    py: pyo3::Python<'py>,
    view: &ReplayViewFrame<PlayerSnapshot>,
) -> PyResult<pyo3::Bound<'py, PyDict>> {
    let dict = PyDict::new(py);
    let rows = PyList::empty(py);
    for row in &view.rows {
        rows.append(replay_row_to_pydict(py, row)?)?;
    }
    dict.set_item("rows", rows)?;
    dict.set_item("total_delay", view.total_delay)?;
    Ok(dict)
}

pub fn updates_to_event_dtos(updates: &RunUpdates, names: &HashMap<PlrId, String>) -> Vec<EventDto> {
    updates.updates.iter().map(|update| update_to_dto(update, names)).collect()
}

pub fn build_replay(py: pyo3::Python<'_>, runner: &mut Runner, limit: Option<usize>) -> PyResult<pyo3::Py<pyo3::PyAny>> {
    let max_events = limit.unwrap_or(usize::MAX);
    let initial_states = snapshot_players(runner);
    let mut previous_states = initial_states.clone();
    let player_count = initial_states.len();
    let mut events_out = Vec::new();
    let mut frames_out = Vec::new();
    let mut idle_rounds = 0usize;
    let mut total_visible_events = 0usize;
    let mut any_visible_event_emitted = false;
    let mut rounds = 0usize;

    while !runner.have_winner() && total_visible_events < max_events && rounds < BINDING_COMPLETION_MAX_ROUNDS {
        rounds += 1;
        let before_states = previous_states.clone();
        let updates = runner.main_round();

        if !updates.had_updates() || updates.updates.is_empty() {
            idle_rounds += 1;
            if idle_rounds > 16usize.saturating_mul(runner.runtime.entities.len().max(1)) {
                break;
            }
            previous_states = snapshot_players(runner);
            continue;
        }
        idle_rounds = 0;

        let after_states = snapshot_players(runner);
        let names = player_names_from_snapshots(&after_states);
        let event_dtos = updates_to_event_dtos(&updates, &names);
        let replay_events = event_dtos
            .iter()
            .zip(updates.updates.iter())
            .map(|(event, update)| ReplayEventView {
                update,
                tone: replay_tone_from_event_tone(event.tone),
                message_rendered: event.message_rendered.as_str(),
            })
            .collect::<Vec<_>>();
        let replay_view = build_replay_view_frame(
            &replay_events,
            &before_states,
            &after_states,
            &names,
            runner.have_winner(),
            &runner.winner_ids(),
        );
        let frame = replay_view_frame_to_pydict(py, &replay_view)?;
        frame.set_item("finished", runner.have_winner())?;
        frame.set_item("winner_ids", runner.winner_ids())?;
        let frame_events = PyList::empty(py);
        for event in &event_dtos {
            frame_events.append(event_to_pydict(py, event)?)?;
        }
        frame.set_item("events", frame_events)?;
        frame.set_item("states", snapshots_to_pylist(py, &after_states)?)?;
        frames_out.push(frame);

        let first_visible_immediate = !any_visible_event_emitted;
        let delays = compute_event_delays(&event_dtos, true, player_count, first_visible_immediate);
        let raw_delays = compute_event_delays(&event_dtos, false, player_count, first_visible_immediate);
        let mut previous_was_break = false;

        for (idx, event) in event_dtos.into_iter().enumerate() {
            if is_visible_event(&event) {
                total_visible_events += 1;
                any_visible_event_emitted = true;
            }

            let item = PyDict::new(py);
            item.set_item("event", event_to_pydict(py, &event)?)?;
            item.set_item("before_states", snapshots_to_pylist(py, &before_states)?)?;
            item.set_item("after_states", snapshots_to_pylist(py, &after_states)?)?;
            item.set_item("delay_ms", *delays.get(idx).unwrap_or(&0))?;
            item.set_item("raw_delay_ms", *raw_delays.get(idx).unwrap_or(&0))?;
            item.set_item("row_break", event.is_next_line || previous_was_break)?;
            item.set_item("state_granularity", "tick")?;
            previous_was_break = event.is_next_line;
            events_out.push(item);

            if total_visible_events >= max_events {
                break;
            }
        }

        previous_states = after_states;
    }

    let final_states = snapshot_players(runner);
    let result = PyDict::new(py);
    result.set_item("initial_states", snapshots_to_pylist(py, &initial_states)?)?;
    result.set_item("events", PyList::new(py, events_out)?)?;
    result.set_item("frames", PyList::new(py, frames_out)?)?;
    result.set_item("final_states", snapshots_to_pylist(py, &final_states)?)?;
    set_optional_usize(&result, "winner_team_index", runner.winner_team_index())?;
    result.set_item("winner_team_indices", runner.winner_team_indices())?;
    result.set_item("winner_ids", runner.winner_ids())?;
    result.set_item("winner_names", winner_names(runner))?;
    result.set_item("state_granularity", "tick")?;
    result.set_item("win_delay_ms", if runner.have_winner() { WIN_UPDATE_DELAY0_MS } else { 0 })?;

    Ok(result.into_any().unbind())
}

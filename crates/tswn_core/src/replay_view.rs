use std::collections::{HashMap, HashSet};

use crate::engine::update::{RunUpdate, UpdateType};
use crate::player::PlrId;

pub const WIN_UPDATE_DELAY0_MS: i32 = 3000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayTone {
    Normal,
    Damage,
    Recover,
    Knockout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayTextPartKind {
    Text,
    Highlight,
    Player,
    Data,
}

#[derive(Debug, Clone)]
pub struct ReplayTextPart {
    pub kind: ReplayTextPartKind,
    pub text: String,
    pub player_id: Option<PlrId>,
    pub show_hp: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub death_effect: bool,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReplayClip<S> {
    pub delay: i32,
    pub text_template: String,
    pub color: ReplayTone,
    pub player_id: Option<PlrId>,
    pub data: Option<String>,
    pub show_hp: bool,
    pub hp_before: i32,
    pub hp_after: i32,
    pub death_effect: bool,
    pub emoji: Option<String>,
    pub parts: Vec<ReplayTextPart>,
    pub caster_ids: Vec<PlrId>,
    pub target_ids: Vec<PlrId>,
    pub sidebar_states: Vec<S>,
    pub sidebar_previous_states: Vec<S>,
    pub winner: bool,
}

#[derive(Debug, Clone)]
pub struct ReplayRow<S> {
    pub indent: bool,
    pub clips: Vec<ReplayClip<S>>,
}

#[derive(Debug, Clone)]
pub struct ReplayViewFrame<S> {
    pub rows: Vec<ReplayRow<S>>,
    pub total_delay: i32,
}

pub trait ReplayState: Clone {
    fn id(&self) -> PlrId;
    fn hp(&self) -> i32;
    fn max_hp(&self) -> i32;
    fn alive(&self) -> bool;
    fn with_hp_alive(&self, hp: i32, alive: bool) -> Self;
}

pub struct ReplayEventView<'a> {
    pub update: &'a RunUpdate,
    pub tone: ReplayTone,
    pub message_rendered: &'a str,
}

pub fn render_update_message(update: &RunUpdate, names: &HashMap<PlrId, String>) -> String {
    let mut message = update.message.to_string();
    message = message.replace("[0]", &render_name(update.caster, names));
    message = message.replace("[1]", &render_name(update.target, names));

    let param = if let Some(value) = update.param {
        value.to_string()
    } else if update.targets.is_empty() {
        update.score.to_string()
    } else {
        update
            .targets
            .iter()
            .map(|target| render_name(*target, names))
            .collect::<Vec<String>>()
            .join(",")
    };

    message.replace("[2]", &param)
}

fn render_name(id: PlrId, names: &HashMap<PlrId, String>) -> String { names.get(&id).cloned().unwrap_or_else(|| id.to_string()) }

pub fn hp_delta_for_tone(tone: ReplayTone, update: &RunUpdate) -> Option<i32> {
    let value = update.param.unwrap_or(update.score).min(i32::MAX as u32) as i32;
    match tone {
        ReplayTone::Damage => Some(-value),
        ReplayTone::Recover => Some(value),
        _ => None,
    }
}

pub fn build_replay_view_frame<S: ReplayState>(
    events: &[ReplayEventView<'_>],
    previous_states: &[S],
    frame_states: &[S],
    player_names: &HashMap<PlrId, String>,
    finished: bool,
    winner_ids: &[PlrId],
) -> ReplayViewFrame<S> {
    let mut rows: Vec<ReplayRow<S>> = Vec::new();
    let mut current_row = ReplayRow {
        indent: false,
        clips: Vec::new(),
    };
    let mut running = state_map(previous_states);
    let frame_state_map = state_map(frame_states);
    let state_order = frame_states.iter().map(ReplayState::id).collect::<Vec<_>>();
    let mut next_wait = 1800;
    let mut total_delay = 0;
    let mut quick_area_skill_active = false;

    for event in events {
        let update = event.update;
        if matches!(update.update_type, UpdateType::NextLine) {
            if !current_row.clips.is_empty() {
                rows.push(current_row);
            }
            current_row = ReplayRow {
                indent: !rows.is_empty(),
                clips: Vec::new(),
            };
            continue;
        }
        if event.message_rendered.trim().is_empty() {
            continue;
        }

        let raw_delay = update.delay0.max(next_wait);
        next_wait = update.delay1;
        if is_quick_area_skill_update(update, event.message_rendered) {
            quick_area_skill_active = true;
        }
        let delay = if quick_area_skill_active {
            raw_delay.min(300)
        } else {
            raw_delay
        };
        total_delay += delay;

        let before = running.clone();
        sync_reappeared_participants(update, &mut running, &frame_state_map);
        if let Some(hp_delta) = hp_delta_for_tone(event.tone, update) {
            apply_hp_delta(&mut running, update.target, hp_delta);
            for target_id in &update.targets {
                apply_hp_delta(&mut running, *target_id, hp_delta);
            }
        }
        let after = running.clone();
        let (text_template, parts, data, hp_before, hp_after, show_hp, death_effect) =
            build_clip_parts(update, event.tone, &before, &after, player_names);

        current_row.clips.push(ReplayClip {
            delay,
            text_template,
            color: event.tone,
            player_id: Some(update_player_id_hint(update, event.tone)),
            data,
            show_hp,
            hp_before,
            hp_after,
            death_effect,
            emoji: None,
            parts,
            caster_ids: vec![update.caster],
            target_ids: update_participant_ids(update).into_iter().filter(|id| *id != update.caster).collect(),
            sidebar_states: states_from_map(&after, &state_order),
            sidebar_previous_states: states_from_map(&before, &state_order),
            winner: false,
        });
    }

    if !current_row.clips.is_empty() {
        rows.push(current_row);
    }

    if finished {
        total_delay += WIN_UPDATE_DELAY0_MS;
        let names = winner_ids
            .iter()
            .map(|id| render_name(*id, player_names))
            .collect::<Vec<String>>()
            .join("、");
        rows.push(ReplayRow {
            indent: false,
            clips: vec![ReplayClip {
                delay: WIN_UPDATE_DELAY0_MS,
                text_template: "胜者：<data>".to_string(),
                color: ReplayTone::Knockout,
                player_id: None,
                data: None,
                show_hp: false,
                hp_before: 0,
                hp_after: 0,
                death_effect: false,
                emoji: None,
                parts: vec![
                    text_part(ReplayTextPartKind::Text, "胜者：".to_string()),
                    text_part(ReplayTextPartKind::Data, names),
                ],
                caster_ids: Vec::new(),
                target_ids: Vec::new(),
                sidebar_states: Vec::new(),
                sidebar_previous_states: Vec::new(),
                winner: true,
            }],
        });
    }

    ReplayViewFrame { rows, total_delay }
}

fn state_map<S: ReplayState>(states: &[S]) -> HashMap<PlrId, S> {
    states.iter().map(|state| (state.id(), state.clone())).collect()
}

fn states_from_map<S: ReplayState>(running: &HashMap<PlrId, S>, state_order: &[PlrId]) -> Vec<S> {
    let mut emitted = HashSet::new();
    let mut states = Vec::with_capacity(running.len());
    for id in state_order {
        if emitted.insert(*id)
            && let Some(state) = running.get(id)
        {
            states.push(state.clone());
        }
    }
    let mut rest = running
        .iter()
        .filter_map(|(id, state)| (!emitted.contains(id)).then_some(state.clone()))
        .collect::<Vec<S>>();
    rest.sort_by_key(ReplayState::id);
    states.extend(rest);
    states
}

fn update_participant_ids(update: &RunUpdate) -> Vec<PlrId> {
    let mut ids = Vec::new();
    push_unique_id(&mut ids, update.caster);
    push_unique_id(&mut ids, update.target);
    for id in &update.targets {
        push_unique_id(&mut ids, *id);
    }
    ids
}

fn push_unique_id(ids: &mut Vec<PlrId>, id: PlrId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

fn sync_reappeared_participants<S: ReplayState>(
    update: &RunUpdate,
    running: &mut HashMap<PlrId, S>,
    frame_states: &HashMap<PlrId, S>,
) {
    for id in update_participant_ids(update) {
        let Some(frame_state) = frame_states.get(&id) else {
            continue;
        };
        if !frame_state.alive() {
            continue;
        }
        if running.get(&id).is_some_and(ReplayState::alive) {
            continue;
        }
        running.insert(id, frame_state.clone());
    }
}

fn apply_hp_delta<S: ReplayState>(running: &mut HashMap<PlrId, S>, id: PlrId, hp_delta: i32) {
    let Some(state) = running.get(&id) else {
        return;
    };
    if state.max_hp() <= 0 {
        return;
    }
    let hp = if hp_delta < 0 {
        state.hp().saturating_add(hp_delta).max(0)
    } else if hp_delta > 0 {
        state.hp().saturating_add(hp_delta).min(state.max_hp())
    } else {
        state.hp()
    };
    running.insert(id, state.with_hp_alive(hp, hp > 0));
}

fn hp_pair<S: ReplayState>(player_id: PlrId, before: &HashMap<PlrId, S>, after: &HashMap<PlrId, S>) -> (i32, i32, bool) {
    let hp_before = before.get(&player_id).map(ReplayState::hp).unwrap_or(0);
    let hp_after = after.get(&player_id).map(ReplayState::hp).unwrap_or(hp_before);
    (hp_before, hp_after, hp_before != hp_after)
}

fn data_for_update(update: &RunUpdate, player_names: &HashMap<PlrId, String>) -> String {
    if let Some(param) = update.param {
        return param.to_string();
    }
    if update.targets.is_empty() {
        return update.score.to_string();
    }
    update
        .targets
        .iter()
        .map(|id| render_name(*id, player_names))
        .collect::<Vec<String>>()
        .join(",")
}

fn text_part(kind: ReplayTextPartKind, text: String) -> ReplayTextPart {
    ReplayTextPart {
        kind,
        text,
        player_id: None,
        show_hp: false,
        hp_before: 0,
        hp_after: 0,
        death_effect: false,
        emoji: None,
    }
}

fn push_plain_and_highlight_parts(parts: &mut Vec<ReplayTextPart>, template: &mut String, text: &str) {
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let before = &rest[..start];
        if !before.is_empty() {
            template.push_str(before);
            parts.push(text_part(ReplayTextPartKind::Text, before.to_string()));
        }
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find(']') else {
            template.push_str(&rest[start..]);
            parts.push(text_part(ReplayTextPartKind::Text, rest[start..].to_string()));
            return;
        };
        let token = &after_open[..end];
        template.push('[');
        template.push_str(token);
        template.push(']');
        parts.push(text_part(ReplayTextPartKind::Highlight, token.to_string()));
        rest = &after_open[end + 1..];
    }
    if !rest.is_empty() {
        template.push_str(rest);
        parts.push(text_part(ReplayTextPartKind::Text, rest.to_string()));
    }
}

fn push_player_part<S: ReplayState>(
    parts: &mut Vec<ReplayTextPart>,
    template: &mut String,
    player_id: PlrId,
    before: &HashMap<PlrId, S>,
    after: &HashMap<PlrId, S>,
    player_names: &HashMap<PlrId, String>,
    tone: ReplayTone,
) -> (i32, i32, bool, bool) {
    let (hp_before, hp_after, show_hp) = hp_pair(player_id, before, after);
    let death_effect = matches!(tone, ReplayTone::Knockout) || (hp_before > 0 && hp_after == 0);
    template.push_str("<player>");
    parts.push(ReplayTextPart {
        kind: ReplayTextPartKind::Player,
        text: render_name(player_id, player_names),
        player_id: Some(player_id),
        show_hp,
        hp_before,
        hp_after,
        death_effect,
        emoji: None,
    });
    (hp_before, hp_after, show_hp, death_effect)
}

fn push_data_part(parts: &mut Vec<ReplayTextPart>, template: &mut String, value: &str) {
    template.push_str("<data>");
    parts.push(text_part(ReplayTextPartKind::Data, value.to_string()));
}

fn update_player_id_hint(update: &RunUpdate, tone: ReplayTone) -> PlrId {
    if matches!(tone, ReplayTone::Damage | ReplayTone::Recover | ReplayTone::Knockout) {
        update.target
    } else {
        update.caster
    }
}

fn build_clip_parts<S: ReplayState>(
    update: &RunUpdate,
    tone: ReplayTone,
    before: &HashMap<PlrId, S>,
    after: &HashMap<PlrId, S>,
    player_names: &HashMap<PlrId, String>,
) -> (String, Vec<ReplayTextPart>, Option<String>, i32, i32, bool, bool) {
    let mut template = String::new();
    let mut parts = Vec::new();
    let data = data_for_update(update, player_names);
    let mut primary_hp_before = 0;
    let mut primary_hp_after = 0;
    let mut primary_show_hp = false;
    let mut primary_death_effect = false;

    let mut rest = update.message.as_ref();
    while let Some(start) = rest.find('[') {
        push_plain_and_highlight_parts(&mut parts, &mut template, &rest[..start]);
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find(']') else {
            push_plain_and_highlight_parts(&mut parts, &mut template, &rest[start..]);
            rest = "";
            break;
        };
        let token = &after_open[..end];
        match token {
            "0" => {
                let (hp_before, hp_after, show_hp, death_effect) =
                    push_player_part(&mut parts, &mut template, update.caster, before, after, player_names, tone);
                if !primary_show_hp && show_hp {
                    primary_hp_before = hp_before;
                    primary_hp_after = hp_after;
                    primary_show_hp = true;
                }
                primary_death_effect |= death_effect;
            }
            "1" => {
                let (hp_before, hp_after, show_hp, death_effect) =
                    push_player_part(&mut parts, &mut template, update.target, before, after, player_names, tone);
                if !primary_show_hp || update_player_id_hint(update, tone) == update.target {
                    primary_hp_before = hp_before;
                    primary_hp_after = hp_after;
                    primary_show_hp = show_hp;
                }
                primary_death_effect |= death_effect;
            }
            "2" => push_data_part(&mut parts, &mut template, &data),
            _ => {
                template.push('[');
                template.push_str(token);
                template.push(']');
                parts.push(text_part(ReplayTextPartKind::Highlight, token.to_string()));
            }
        }
        rest = &after_open[end + 1..];
    }
    push_plain_and_highlight_parts(&mut parts, &mut template, rest);

    (
        template,
        parts,
        update.param.map(|_| data),
        primary_hp_before,
        primary_hp_after,
        primary_show_hp,
        primary_death_effect,
    )
}

fn is_quick_area_skill_update(update: &RunUpdate, rendered: &str) -> bool {
    ["[雷击术]", "[地裂术]", "使用雷击术", "使用地裂术"]
        .iter()
        .any(|token| update.message.contains(token) || rendered.contains(token))
}

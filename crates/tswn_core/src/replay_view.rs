//! 展示投影：把已执行消息和帧前后快照拆成行、片段及玩家引用。
//!
//! [`build_replay_view_frame`] 纯粹读取当前事件和快照，保持战斗位置与 RNG 状态。
//! [`ReplayState`] 只暴露投影所需的 HP、护盾和存活信息；每个 [`ReplayClip`] 携带
//! 文本 parts、语气、播放延时及 sidebar 的前后值。
//! 部分中间值由消息与边界快照推导；完整机制检查点由 Runtime 边界数据表达。
//! delay 使用毫秒，服务于片段播放节奏。

use std::collections::{HashMap, HashSet};

use crate::runtime::PlrId;
use crate::runtime::update::{RunUpdate, UpdateType};

pub const WIN_UPDATE_DELAY0_MS: i32 = 3000;
pub const DEFAULT_TEXT_COLOR: &str = "0077BB";
pub const STATUS_EXIT_TEXT_COLOR: &str = "bb7700";
const FRAME_FIRST_DELAY_MS: i32 = 900;
const QUICK_AREA_ROW_FIRST_DELAY_MS: i32 = 150;
const HP_CHANGE_DELAY_MS: i32 = 600;
const DEFAULT_CLIP_DELAY_MS: i32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayTone {
    Normal,
    Damage,
    Recover,
    Knockout,
    StatusExit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayTextPartKind {
    Text,
    Highlight,
    Player,
    Data,
}

/// 一段拥有自身文本的展示内容；玩家片段可附带 HP 变化或死亡效果。
/// `player_id` 用于 UI 关联，普通文本与数值片段不要求对应玩家。
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

/// 一次播放片段，包含毫秒延时和该片段前后的 sidebar 状态副本。
/// HP 等中间值用于展示插值，不是可恢复战斗的机制检查点。
#[derive(Debug, Clone)]
pub struct ReplayClip<S> {
    pub delay: i32,
    pub color: String,
    pub tone: ReplayTone,
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

/// 展示投影所需的最小玩家快照接口。
///
/// 同一帧中的 `id` 应唯一；`with_hp_alive` 与 `with_shield` 返回保留身份及其他字段的新值，
/// 不应通过内部可变性改写原快照。默认实现不保存护盾；需要显示护盾的适配器应同时实现
/// `shield` 和 `with_shield`，否则片段间的护盾协调不会生效。
pub trait ReplayState: Clone {
    fn id(&self) -> PlrId;
    fn hp(&self) -> i32;
    fn max_hp(&self) -> i32;
    fn alive(&self) -> bool;
    fn shield(&self) -> i32 { 0 }
    fn with_shield(&self, _shield: i32) -> Self { self.clone() }
    fn with_hp_alive(&self, hp: i32, alive: bool) -> Self;
}

/// 对一条已执行更新的临时借用，语气由调用方提供。
///
/// `message_rendered` 用于可见性与快速群攻识别；结构化文本片段仍从 `update.message`
/// 的占位符构建，因此这两种文本应来自同一更新。结果帧不保留此借用。
pub struct ReplayEventView<'a> {
    pub update: &'a RunUpdate,
    pub tone: ReplayTone,
    pub message_rendered: &'a str,
}

/// 将更新中的玩家与数据占位符替换为可读文本，返回新字符串。
///
/// 缺失玩家名回退为十进制 ID；数据依次选择 `param`、目标列表或 `score`。
/// 保留技能等其他方括号标记，不对输出执行 HTML 转义，界面层仍需按文本渲染。
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

/// 根据伤害或恢复语气提取有符号 HP 变化，其他语气返回 `None`。
///
/// 百分比伤害消息使用实际 `score`，不把 `param` 中的百分比当伤害值；
/// 非百分比消息优先使用 `param`，并将绝对变化量限制在 `i32::MAX` 内。
pub fn hp_delta_for_tone(tone: ReplayTone, update: &RunUpdate) -> Option<i32> {
    let value = hp_delta_value(update).min(i32::MAX as u32) as i32;
    match tone {
        ReplayTone::Damage => Some(-value),
        ReplayTone::Recover => Some(value),
        _ => None,
    }
}

fn hp_delta_value(update: &RunUpdate) -> u32 {
    if is_percent_hp_damage_update(update) {
        update.score
    } else {
        update.param.unwrap_or(update.score)
    }
}

pub(crate) fn is_percent_hp_damage_update(update: &RunUpdate) -> bool {
    update.message.contains("体力减少") && update.message.contains("[2]%")
}

pub fn replay_tone_color(tone: ReplayTone) -> &'static str {
    match tone {
        ReplayTone::StatusExit => STATUS_EXIT_TEXT_COLOR,
        _ => DEFAULT_TEXT_COLOR,
    }
}

/// 由已执行事件和帧边界快照构造可独立持有的展示帧，不运行战斗或消耗 RNG。
///
/// 按事件源顺序分行，跳过空文本，并用消息推导 HP、死亡及复活的中间展示值；
/// 护盾缺少独立事件时在玩家首次可见处同步到帧末值。sidebar 先按 `frame_states`
/// 的顺序排列，仍只出现在旧快照中的玩家再按 ID 排列。
///
/// `finished=true` 会追加胜者行；`total_delay` 是所有片段毫秒延时之和。输出拥有文本和
/// 克隆的状态，内存随事件数与快照规模增长；它不是机制级状态回滚或恢复的输入。
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
    let mut total_delay = 0;
    let mut quick_area_skill_active = false;
    let mut frame_has_visible_clip = false;
    let mut current_row_has_visible_clip = false;

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
            current_row_has_visible_clip = false;
            continue;
        }
        if event.message_rendered.trim().is_empty() {
            continue;
        }

        if is_quick_area_skill_update(update, event.message_rendered) {
            quick_area_skill_active = true;
        }

        if is_revive_update(update) {
            apply_death_update(&mut running, update.target);
        }
        let before = running.clone();
        if should_sync_reappeared_participants(update, event.tone) {
            sync_reappeared_participants(update, &mut running, &frame_state_map);
        }
        if let Some(hp_delta) = hp_delta_for_tone(event.tone, update) {
            apply_hp_delta(&mut running, update.target, hp_delta);
            for target_id in &update.targets {
                apply_hp_delta(&mut running, *target_id, hp_delta);
            }
        }
        if is_hp_swap_update(update) {
            apply_hp_swap(&mut running, update.caster, update.target);
        }
        if is_death_effect_update(update) {
            apply_death_update(&mut running, update.target);
        }
        let parts = build_clip_parts(update, &before, &running, player_names);
        sync_visible_shields(&parts, &mut running, &frame_state_map);
        let after = running.clone();
        let show_hp = parts.iter().any(|part| part.show_hp);
        let delay = clip_delay(
            frame_has_visible_clip,
            current_row_has_visible_clip,
            quick_area_skill_active,
            show_hp,
        );
        total_delay += delay;
        frame_has_visible_clip = true;
        current_row_has_visible_clip = true;

        current_row.clips.push(ReplayClip {
            delay,
            color: replay_tone_color(event.tone).to_string(),
            tone: event.tone,
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
                color: replay_tone_color(ReplayTone::Knockout).to_string(),
                tone: ReplayTone::Knockout,
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
        let shield = running.get(&id).map_or(0, ReplayState::shield);
        running.insert(id, frame_state.with_shield(shield));
    }
}

fn sync_visible_shields<S: ReplayState>(
    parts: &[ReplayTextPart],
    running: &mut HashMap<PlrId, S>,
    frame_states: &HashMap<PlrId, S>,
) {
    // 护盾增减没有独立消息，只能按帧末净值协调；首次出现玩家文字时推进，后续片段沿用该值。
    for id in parts.iter().filter_map(|part| part.player_id) {
        let Some(state) = running.get(&id) else {
            continue;
        };
        let Some(frame_state) = frame_states.get(&id) else {
            continue;
        };
        let shield = if state.alive() { frame_state.shield() } else { 0 };
        if state.shield() != shield {
            running.insert(id, state.with_shield(shield));
        }
    }
}

fn should_sync_reappeared_participants(update: &RunUpdate, tone: ReplayTone) -> bool {
    !matches!(tone, ReplayTone::Knockout | ReplayTone::Recover) && !is_reraise_update(update) && !is_revive_update(update)
}

fn is_reraise_update(update: &RunUpdate) -> bool {
    update.message.contains("[护身符]") && update.message.contains("抵挡了一次死亡")
}

fn is_revive_update(update: &RunUpdate) -> bool { update.message.contains("复活") }

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

fn apply_hp_swap<S: ReplayState>(running: &mut HashMap<PlrId, S>, caster_id: PlrId, target_id: PlrId) {
    let Some(caster) = running.get(&caster_id).cloned() else {
        return;
    };
    let Some(target) = running.get(&target_id).cloned() else {
        return;
    };
    let caster_hp = caster.hp();
    let target_hp = target.hp();
    let caster_new_hp = if caster.max_hp() > 0 {
        target_hp.min(caster.max_hp())
    } else {
        target_hp
    };
    let target_new_hp = caster_hp;
    running.insert(caster_id, caster.with_hp_alive(caster_new_hp, caster_new_hp > 0));
    running.insert(target_id, target.with_hp_alive(target_new_hp, target_new_hp > 0));
}

fn apply_death_update<S: ReplayState>(running: &mut HashMap<PlrId, S>, id: PlrId) {
    let Some(state) = running.get(&id) else {
        return;
    };
    running.insert(id, state.with_hp_alive(0, false));
}

fn hp_pair<S: ReplayState>(player_id: PlrId, before: &HashMap<PlrId, S>, after: &HashMap<PlrId, S>) -> (i32, i32, bool) {
    let hp_after = after.get(&player_id).map(ReplayState::hp);
    let is_new_entity = !before.contains_key(&player_id) && hp_after.is_some();
    let hp_before = before.get(&player_id).map(ReplayState::hp).or(hp_after).unwrap_or(0);
    let hp_after = hp_after.unwrap_or(hp_before);
    (hp_before, hp_after, is_new_entity || hp_before != hp_after)
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

fn push_plain_and_highlight_parts(parts: &mut Vec<ReplayTextPart>, text: &str) {
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let before = &rest[..start];
        if !before.is_empty() {
            parts.push(text_part(ReplayTextPartKind::Text, before.to_string()));
        }
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find(']') else {
            parts.push(text_part(ReplayTextPartKind::Text, rest[start..].to_string()));
            return;
        };
        let token = &after_open[..end];
        parts.push(text_part(ReplayTextPartKind::Highlight, token.to_string()));
        rest = &after_open[end + 1..];
    }
    if !rest.is_empty() {
        parts.push(text_part(ReplayTextPartKind::Text, rest.to_string()));
    }
}

fn push_player_part<S: ReplayState>(
    parts: &mut Vec<ReplayTextPart>,
    player_id: PlrId,
    before: &HashMap<PlrId, S>,
    after: &HashMap<PlrId, S>,
    player_names: &HashMap<PlrId, String>,
    death_effect_allowed: bool,
    force_show_hp: bool,
) {
    let (hp_before, hp_after, hp_changed) = hp_pair(player_id, before, after);
    let death_effect = death_effect_allowed && hp_after == 0;
    let show_hp = (hp_changed || force_show_hp) && !death_effect;
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
}

fn push_data_part(parts: &mut Vec<ReplayTextPart>, value: &str) {
    parts.push(text_part(ReplayTextPartKind::Data, value.to_string()));
}

fn is_death_effect_update(update: &RunUpdate) -> bool {
    update.message.contains("\u{88ab}\u{51fb}\u{5012}") || update.message.contains("\u{6d88}\u{5931}")
}

fn is_hp_marker_update(update: &RunUpdate) -> bool { update.message == "[0]还剩[2]点血" }

fn is_hp_swap_update(update: &RunUpdate) -> bool {
    update.message.contains("\u{4f53}\u{529b}\u{503c}\u{4e0e}") && update.message.contains("\u{4e92}\u{6362}")
}

fn is_entity_conversion_update(update: &RunUpdate) -> bool { update.message == "[2]变成了[1]" && !update.targets.is_empty() }

fn build_clip_parts<S: ReplayState>(
    update: &RunUpdate,
    before: &HashMap<PlrId, S>,
    after: &HashMap<PlrId, S>,
    player_names: &HashMap<PlrId, String>,
) -> Vec<ReplayTextPart> {
    let mut parts = Vec::new();
    let data = data_for_update(update, player_names);
    let death_effect_allowed = is_death_effect_update(update);
    let force_hp_marker = is_hp_marker_update(update);
    let force_show_hp = is_hp_swap_update(update);
    let entity_conversion = is_entity_conversion_update(update);

    let mut rest = update.message.as_ref();
    while let Some(start) = rest.find('[') {
        push_plain_and_highlight_parts(&mut parts, &rest[..start]);
        let after_open = &rest[start + 1..];
        let Some(end) = after_open.find(']') else {
            push_plain_and_highlight_parts(&mut parts, &rest[start..]);
            rest = "";
            break;
        };
        let token = &after_open[..end];
        match token {
            "0" => {
                push_player_part(
                    &mut parts,
                    update.caster,
                    before,
                    after,
                    player_names,
                    death_effect_allowed,
                    force_hp_marker || force_show_hp,
                );
            }
            "1" => {
                push_player_part(
                    &mut parts,
                    update.target,
                    before,
                    after,
                    player_names,
                    death_effect_allowed,
                    force_show_hp,
                );
            }
            "2" if entity_conversion => {
                for (index, player_id) in update.targets.iter().enumerate() {
                    if index > 0 {
                        parts.push(text_part(ReplayTextPartKind::Text, ",".to_string()));
                    }
                    push_player_part(&mut parts, *player_id, before, after, player_names, false, false);
                }
            }
            "2" => push_data_part(&mut parts, &data),
            _ => {
                parts.push(text_part(ReplayTextPartKind::Highlight, token.to_string()));
            }
        }
        rest = &after_open[end + 1..];
    }
    push_plain_and_highlight_parts(&mut parts, rest);

    parts
}

fn is_quick_area_skill_update(update: &RunUpdate, rendered: &str) -> bool {
    ["[雷击术]", "[地裂术]", "使用雷击术", "使用地裂术"]
        .iter()
        .any(|token| update.message.contains(token) || rendered.contains(token))
}

fn clip_delay(
    frame_has_visible_clip: bool,
    current_row_has_visible_clip: bool,
    quick_area_skill_active: bool,
    show_hp: bool,
) -> i32 {
    if !frame_has_visible_clip {
        FRAME_FIRST_DELAY_MS
    } else if !current_row_has_visible_clip && quick_area_skill_active {
        QUICK_AREA_ROW_FIRST_DELAY_MS
    } else if show_hp {
        HP_CHANGE_DELAY_MS
    } else {
        DEFAULT_CLIP_DELAY_MS
    }
}

#[cfg(test)]
#[path = "replay_view_tests.rs"]
mod tests;

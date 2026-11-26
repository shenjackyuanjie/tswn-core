//! 展示投影：把已执行消息和帧前后快照拆成行、片段及玩家引用。
//!
//! [`build_replay_view_frame`] 纯粹读取当前事件和快照，保持战斗位置与 RNG 状态。
//! [`ReplayState`] 只暴露投影所需的 HP、护盾和存活信息；每个 [`ReplayClip`] 携带
//! 文本 parts、语气、播放延时及 sidebar 的前后值。
//! 部分中间值由消息与边界快照推导；完整机制检查点由 Runtime 边界数据表达。
//! delay 使用毫秒，服务于片段播放节奏。

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use crate::runtime::PlrId;
use crate::runtime::lang::{get_lang, keys};
use crate::runtime::update::{RunUpdate, RunUpdateHp, UpdateType};

/// 胜利更新的首段播放延时（毫秒）。
pub const WIN_UPDATE_DELAY0_MS: i32 = 3000;
/// 默认文本颜色。
pub const DEFAULT_TEXT_COLOR: &str = "0077BB";
/// 状态退出文本的颜色。
pub const STATUS_EXIT_TEXT_COLOR: &str = "bb7700";
/// 帧内首个可见片段的延迟（毫秒）。
const FRAME_FIRST_DELAY_MS: i32 = 900;
/// 瞬发区域技能所在行首个片段的延迟（毫秒）。
const QUICK_AREA_ROW_FIRST_DELAY_MS: i32 = 150;
/// 展示 HP 变化的片段延迟（毫秒）。
const HP_CHANGE_DELAY_MS: i32 = 600;
/// 其余片段的默认延迟（毫秒）。
const DEFAULT_CLIP_DELAY_MS: i32 = 500;

/// 片段语气：普通、伤害、恢复、击倒与状态退出。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayTone {
    /// 独立台词，仅影响文字背景。
    Dialogue,
    Normal,
    Damage,
    Recover,
    Knockout,
    StatusExit,
}

/// 文本片段类型：纯文本、高亮、玩家引用与数值。
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

/// 一行展示内容：是否缩进，以及该行按序播放的片段。
#[derive(Debug, Clone)]
pub struct ReplayRow<S> {
    pub indent: bool,
    pub clips: Vec<ReplayClip<S>>,
}

/// 一帧的展示投影：全部行与各片段延时之和。
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
    /// 玩家 id，同一帧内应唯一。
    fn id(&self) -> PlrId;
    /// 当前 HP。
    fn hp(&self) -> i32;
    /// 最大 HP；为 0 时跳过 HP 差值应用。
    fn max_hp(&self) -> i32;
    /// 是否存活。
    fn alive(&self) -> bool;
    /// 当前护盾；默认 0 表示不显示护盾。
    fn shield(&self) -> i32 { 0 }
    /// 返回设置护盾后的新快照；默认实现丢弃护盾。
    fn with_shield(&self, _shield: i32) -> Self { self.clone() }
    /// 返回设置 HP 与存活状态的新快照，保留其他字段。
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
    let mut message = Cow::Borrowed(update.message.as_ref());
    // 逐步检查替换后的文本：名字本身可以含后续占位符，保留 [0] → [1] → [2] 的顺序。
    for (token, id) in [("[0]", update.caster), ("[1]", update.target)] {
        if message.contains(token) {
            message = Cow::Owned(message.replace(token, &render_name(id, names)));
        }
    }
    if message.contains("[2]") {
        message.replace("[2]", &data_for_update(update, names))
    } else {
        message.into_owned()
    }
}

/// 按 id 取显示名；缺失时退化为 id 字符串。
fn render_name(id: PlrId, names: &HashMap<PlrId, String>) -> Cow<'_, str> {
    names
        .get(&id)
        .map_or_else(|| Cow::Owned(id.to_string()), |name| Cow::Borrowed(name.as_str()))
}

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

/// 取更新中的 HP 变化量：百分比伤害取 score，其余取 param 或 score。
fn hp_delta_value(update: &RunUpdate) -> u32 {
    if is_percent_hp_damage_update(update) {
        update.score
    } else {
        update.param.unwrap_or(update.score)
    }
}

/// 是否百分比体力减少消息。
pub fn is_percent_hp_damage_update(update: &RunUpdate) -> bool {
    update.message.contains("体力减少") && update.message.contains("[2]%")
}

/// 语气到文本颜色的映射；仅状态退出使用专用颜色，其余走默认色。
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
    let frame_state_map = frame_states.iter().map(|state| (state.id(), state)).collect::<HashMap<_, _>>();
    // 片段只能引入帧末已有的实体。统一排好两侧 ID，导出时只跳过当时尚不存在的实体。
    let mut state_order = frame_states.iter().map(ReplayState::id).collect::<Vec<_>>();
    if state_order.len() != frame_state_map.len() {
        let mut seen = HashSet::new();
        state_order.retain(|id| seen.insert(*id));
    }
    let frame_count = state_order.len();
    state_order.extend(running.keys().filter(|id| !frame_state_map.contains_key(id)).copied());
    state_order[frame_count..].sort_unstable();
    let mut total_delay = 0;
    let mut next_boss_delay = 0;
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

        if update.hp_changes.is_none() && is_revive_update(update) {
            apply_death_update(&mut running, update.target);
        }
        let mut before = running.clone();
        if let Some(changes) = &update.hp_changes {
            for &change in changes.iter() {
                if let Some(state) = before.get(&change.player) {
                    before.insert(change.player, state.with_hp_alive(change.before, change.before > 0));
                }
                // 分裂子体可能只存在于帧末快照，或在同一批中再次死亡；出场体力以事件为准。
                if let Some(state) = running.get(&change.player).or_else(|| frame_state_map.get(&change.player).copied()) {
                    running.insert(change.player, state.with_hp_alive(change.after, change.after > 0));
                }
            }
        } else {
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
        }
        let parts = build_clip_parts(update, &before, &running, player_names);
        sync_visible_shields(&parts, &mut running, &frame_state_map);
        let show_hp = parts.iter().any(|part| part.show_hp);
        let mut delay = clip_delay(
            frame_has_visible_clip,
            current_row_has_visible_clip,
            quick_area_skill_active,
            show_hp,
        )
        .max(next_boss_delay);
        // 只有原版的这两处 Boss 演出等待覆盖片段节奏：真相台词停在下一条之前，复活等待在本条之前。
        next_boss_delay = if update.message == get_lang(keys::SKL_CONAN_THINKING_FINISH2) {
            update.delay1
        } else {
            0
        };
        if update.message == get_lang(keys::BOSS_MARIO_REVIVE) && update.hp_changes.is_some() {
            delay = delay.max(update.delay0);
        }
        total_delay += delay;
        frame_has_visible_clip = true;
        current_row_has_visible_clip = true;

        let caster = update.caster_id();
        let mut target_ids = update_participant_ids(update);
        if let Some(animation_targets) = &update.animation_targets {
            for &id in animation_targets.iter() {
                push_unique_id(&mut target_ids, id);
            }
        }
        target_ids.retain(|id| Some(*id) != caster);
        // 前态是本片段持有的副本，可以直接移走；后态从继续推进的 running 只复制一次。
        let mut sidebar_previous_states = Vec::with_capacity(before.len());
        sidebar_previous_states.extend(state_order.iter().filter_map(|id| before.remove(id)));
        let mut sidebar_states = Vec::with_capacity(running.len());
        sidebar_states.extend(state_order.iter().filter_map(|id| running.get(id).cloned()));
        current_row.clips.push(ReplayClip {
            delay,
            color: replay_tone_color(event.tone).to_string(),
            tone: if update.dialogue { ReplayTone::Dialogue } else { event.tone },
            parts,
            caster_ids: caster.into_iter().collect(),
            target_ids,
            sidebar_states,
            sidebar_previous_states,
            winner: false,
        });
    }

    if !current_row.clips.is_empty() {
        rows.push(current_row);
    }

    if finished {
        total_delay += WIN_UPDATE_DELAY0_MS;
        let names = winner_ids.iter().map(|id| render_name(*id, player_names)).collect::<Vec<_>>().join("、");
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

/// 把状态列表按 id 建立映射。
fn state_map<S: ReplayState>(states: &[S]) -> HashMap<PlrId, S> {
    states.iter().map(|state| (state.id(), state.clone())).collect()
}

/// 收集更新的参与者 id（施法者、目标与目标列表），去重保序。
fn update_participant_ids(update: &RunUpdate) -> Vec<PlrId> {
    let mut ids = Vec::new();
    if let Some(caster) = update.caster_id() {
        push_unique_id(&mut ids, caster);
    }
    if let Some(target) = update.replay_target_id() {
        push_unique_id(&mut ids, target);
    }
    for id in &update.targets {
        push_unique_id(&mut ids, *id);
    }
    ids
}

/// 向量内不存在时才追加 id。
fn push_unique_id(ids: &mut Vec<PlrId>, id: PlrId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

/// 把帧内存活但运行态缺失的参与者补回运行态（复活 / 复苏场景）。
fn sync_reappeared_participants<S: ReplayState>(
    update: &RunUpdate,
    running: &mut HashMap<PlrId, S>,
    frame_states: &HashMap<PlrId, &S>,
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

/// 按帧末净值协调运行态护盾，仅在玩家文字片段上推进。
fn sync_visible_shields<S: ReplayState>(
    parts: &[ReplayTextPart],
    running: &mut HashMap<PlrId, S>,
    frame_states: &HashMap<PlrId, &S>,
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

/// 是否需要在击倒 / 恢复类更新之外补回重新出现的参与者。
fn should_sync_reappeared_participants(update: &RunUpdate, tone: ReplayTone) -> bool {
    !matches!(tone, ReplayTone::Knockout | ReplayTone::Recover)
        && !is_reraise_update(update)
        && !is_revive_update(update)
        && update.message != get_lang(keys::SKL_SLIME_SPAWN)
}

/// 是否护身符抵挡死亡的消息。
fn is_reraise_update(update: &RunUpdate) -> bool {
    update.message.contains("[护身符]") && update.message.contains("抵挡了一次死亡")
}

/// 是否复活消息。
fn is_revive_update(update: &RunUpdate) -> bool { update.message.contains("复活") }

/// 把 HP 变化应用到运行态：负值截到 0，正值不超过上限。
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

/// 交换施法者与目标的 HP，各自受自身上限约束。
fn apply_hp_swap<S: ReplayState>(running: &mut HashMap<PlrId, S>, caster_id: PlrId, target_id: PlrId) {
    let Some(caster) = running.get(&caster_id) else {
        return;
    };
    let Some(target) = running.get(&target_id) else {
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
    let caster_next = caster.with_hp_alive(caster_new_hp, caster_new_hp > 0);
    let target_next = target.with_hp_alive(target_new_hp, target_new_hp > 0);
    running.insert(caster_id, caster_next);
    running.insert(target_id, target_next);
}

/// 把运行态标记为死亡（HP 0 且非存活）。
fn apply_death_update<S: ReplayState>(running: &mut HashMap<PlrId, S>, id: PlrId) {
    let Some(state) = running.get(&id) else {
        return;
    };
    running.insert(id, state.with_hp_alive(0, false));
}

/// 取玩家更新前后的 HP，并返回是否发生变化或新出现。
fn hp_pair<S: ReplayState>(player_id: PlrId, before: &HashMap<PlrId, S>, after: &HashMap<PlrId, S>) -> (i32, i32, bool) {
    let hp_after = after.get(&player_id).map(ReplayState::hp);
    let is_new_entity = !before.contains_key(&player_id) && hp_after.is_some();
    let hp_before = before.get(&player_id).map(ReplayState::hp).or(hp_after).unwrap_or(0);
    let hp_after = hp_after.unwrap_or(hp_before);
    (hp_before, hp_after, is_new_entity || hp_before != hp_after)
}

/// 取更新的数据部分文本：param、score 或目标名列表。
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
        .collect::<Vec<_>>()
        .join(",")
}

/// 构造带默认值的文本片段。
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

/// 把文本按 `[...]` 拆分为普通与高亮片段。
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

/// 构造玩家文字片段：附带前后 HP、死亡特效，精确载荷优先。
fn build_player_part<S: ReplayState>(
    player_id: PlrId,
    before: &HashMap<PlrId, S>,
    after: &HashMap<PlrId, S>,
    player_names: &HashMap<PlrId, String>,
    death_effect_allowed: bool,
    force_show_hp: bool,
    exact: Option<&RunUpdateHp>,
) -> ReplayTextPart {
    // 刚出现的精确载荷可能没有旧 sidebar 状态；玩家文字仍使用实际记录的前后值。
    let (hp_before, hp_after, hp_changed) = match exact {
        Some(change) => (change.before, change.after, change.before != change.after),
        None => hp_pair(player_id, before, after),
    };
    let death_effect = death_effect_allowed && hp_after == 0;
    let show_hp = force_show_hp || exact.is_some() || (hp_changed && !death_effect);
    ReplayTextPart {
        kind: ReplayTextPartKind::Player,
        text: render_name(player_id, player_names).into_owned(),
        player_id: Some(player_id),
        show_hp,
        hp_before,
        hp_after,
        death_effect,
        emoji: None,
    }
}

/// 追加数据片段。
fn push_data_part(parts: &mut Vec<ReplayTextPart>, value: &str) {
    parts.push(text_part(ReplayTextPartKind::Data, value.to_string()));
}

/// 是否灭霸响指（化灰）消息。
pub fn is_thanos_dust_update(update: &RunUpdate) -> bool { update.message == get_lang(keys::SKL_THANOS_DUST) }

/// 是否触发死亡特效的消息（化灰 / 击倒 / 消失）。
fn is_death_effect_update(update: &RunUpdate) -> bool {
    is_thanos_dust_update(update)
        || update.message.contains("\u{88ab}\u{51fb}\u{5012}")
        || update.message.contains("\u{6d88}\u{5931}")
}

/// 是否“[0]还剩[2]点血”HP 标记消息。
fn is_hp_marker_update(update: &RunUpdate) -> bool { update.message == "[0]还剩[2]点血" }

/// 是否体力值互换消息。
fn is_hp_swap_update(update: &RunUpdate) -> bool {
    update.message.contains("\u{4f53}\u{529b}\u{503c}\u{4e0e}") && update.message.contains("\u{4e92}\u{6362}")
}

/// 是否实体转化消息（[2]变成了[1]）。
fn is_entity_conversion_update(update: &RunUpdate) -> bool { update.message == "[2]变成了[1]" && !update.targets.is_empty() }

/// 为单条更新构建全部文本片段。
fn build_clip_parts<S: ReplayState>(
    update: &RunUpdate,
    before: &HashMap<PlrId, S>,
    after: &HashMap<PlrId, S>,
    player_names: &HashMap<PlrId, String>,
) -> Vec<ReplayTextPart> {
    let mut parts = Vec::new();
    let mut data = None;
    let death_effect_allowed = is_death_effect_update(update);
    let force_hp_marker = is_hp_marker_update(update);
    let hp_swap = is_hp_swap_update(update);
    let exact_hp = |id| update.hp_changes.as_deref()?.iter().find(|change| change.player == id);
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
                parts.push(build_player_part(
                    update.caster,
                    before,
                    after,
                    player_names,
                    death_effect_allowed,
                    force_hp_marker || hp_swap,
                    exact_hp(update.caster),
                ));
            }
            "1" => {
                parts.push(build_player_part(
                    update.target,
                    before,
                    after,
                    player_names,
                    death_effect_allowed,
                    hp_swap,
                    exact_hp(update.target),
                ));
            }
            "2" if entity_conversion => {
                for (index, player_id) in update.targets.iter().enumerate() {
                    if index > 0 {
                        parts.push(text_part(ReplayTextPartKind::Text, ",".to_string()));
                    }
                    parts.push(build_player_part(
                        *player_id,
                        before,
                        after,
                        player_names,
                        false,
                        false,
                        exact_hp(*player_id),
                    ));
                }
            }
            "2" => push_data_part(&mut parts, data.get_or_insert_with(|| data_for_update(update, player_names))),
            _ => {
                parts.push(text_part(ReplayTextPartKind::Highlight, token.to_string()));
            }
        }
        rest = &after_open[end + 1..];
    }
    push_plain_and_highlight_parts(&mut parts, rest);
    // 生命之轮的精确换血带原版表情令牌，正文保持纯文本。
    if update.message == get_lang(keys::SKL_EXCHANGED)
        && update.hp_changes.is_some()
        && let Some(part) = parts.last_mut()
    {
        part.emoji = Some("s_exchange".to_string());
    }

    parts
}

/// 是否瞬发区域技能（雷击 / 地裂）相关的消息或渲染文本。
fn is_quick_area_skill_update(update: &RunUpdate, rendered: &str) -> bool {
    ["[雷击术]", "[地裂术]", "使用雷击术", "使用地裂术"]
        .iter()
        .any(|token| update.message.contains(token) || rendered.contains(token))
}

/// 按帧 / 行可见性、瞬发技能与 HP 展示计算片段延迟。
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
mod tests;

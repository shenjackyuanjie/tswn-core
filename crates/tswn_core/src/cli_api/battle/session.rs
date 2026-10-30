use std::collections::HashMap;

#[cfg(feature = "png_render")]
use crate::namerena::icon_render::render_icon_b64_from_name;
use crate::replay_view::{
    ReplayClip, ReplayEventView, ReplayRow, ReplayTextPart, ReplayTextPartKind, ReplayTone, build_replay_view_frame,
    hp_delta_for_tone, is_percent_hp_damage_update, render_update_message,
};
use crate::runtime::update::{RunUpdates, UpdateType};
use crate::runtime::{PlannedAction, PlrId, RuntimeMinionKind, RuntimePlayerSnapshot, RuntimeRunner};

use super::dto::*;
use crate::cli_api::CliApiResult;

#[cfg(not(feature = "png_render"))]
fn render_icon_b64_from_name(_name: &str) -> String {
    // 仅守卫内部契约；公开构造先返回 UnsupportedOption，不返回空图或占位图。
    unreachable!("无 PNG 构建不能调用内部图标渲染，公开入口应先拒绝 include_icons")
}

/// 拥有一场对局、边界快照和图标缓存的增量式展示会话。
///
/// 通过 [`Self::next_frame`] 同步推进，查询初始/当前快照不会消费随机流。
/// 一个可见帧可能跨过若干无消息回合，回合上限仍按实际推进次数计算。
/// 胜者、上限截断与长期无可见进展产生正常结果；Runtime 校验错误保存为粘性失败，
/// 由错误通道返回，不伪装成截断结果。
///
/// 初始快照保留至会话释放，当前快照随推进替换。启用图标时按完整身份键缓存 PNG data URL；
/// 不同实体可共用图标，但不以展示名合并身份。缓存随本会话释放，不跨对局共享。
#[derive(Debug)]
pub struct BattleSession {
    driver: super::driver::BattleDriver,
    options: BattleOptions,
    initial_states: Vec<BattlePlayerState>,
    current_states: Vec<BattlePlayerState>,
    icon_cache: HashMap<String, String>,
    /// 受控单位逼停回放时的等待者；非暂停边界为 `None`。
    paused: Option<PlrId>,
}

impl BattleSession {
    /// 解析并初始化对局，不推进回合；空输入、零上限或非有限 eval_rq 返回错误。
    ///
    /// 未启用 `png_render` 时，请求 `include_icons=true` 返回 `UnsupportedOption`，
    /// 不生成空图或占位图；默认的无图标对局不依赖 PNG 能力。
    ///
    /// `options.controlled_team` 只影响推进方式：该队伍的本体与分身缺指令时会暂停，
    /// 需要 LegacyStep 调度方式，否则返回 `UnsupportedOption`。
    pub fn new(raw: &str, options: BattleOptions) -> CliApiResult<Self> {
        #[cfg(not(feature = "png_render"))]
        if options.include_icons {
            return Err(crate::cli_api::CliApiError::UnsupportedOption(
                "战斗图标需要启用 png_render feature".to_owned(),
            ));
        }
        let mut driver = super::driver::BattleDriver::from_raw(raw, options)?;
        if let Some(team) = options.controlled_team {
            let runtime = driver.runner.runtime_mut();
            if !runtime.scheduler.uses_legacy_step_scheduler() {
                return Err(crate::cli_api::CliApiError::UnsupportedOption(
                    "controlled_team requires the LegacyStep action scheduler".to_owned(),
                ));
            }
            // 队伍编号写错时不能静默不暂停：先确认这场里确实有这个队伍。
            if !runtime.entities.iter().any(|(_, entity)| entity.runtime.team == team) {
                return Err(crate::cli_api::CliApiError::InvalidArgument(format!(
                    "controlled_team {team} matches no entity in this battle"
                )));
            }
            runtime.set_controlled_team(Some(team));
        }
        let mut icon_cache = HashMap::new();
        let initial_states = states_from_runner(&driver.runner, options.include_icons, options.include_skills, &mut icon_cache);
        Ok(Self {
            driver,
            options,
            current_states: initial_states.clone(),
            initial_states,
            icon_cache,
            paused: None,
        })
    }

    /// 创建会话并预置外部指令；指令按入队顺序消费，用尽后受控单位才会暂停。
    ///
    /// 指令里的 `actor` 必须是本场已存在的实体 ID，否则返回 `InvalidArgument`。
    /// 脚本可以为任意实体排队（包括敌方），不必属于 `controlled_team`。
    pub fn new_with_actions(raw: &str, options: BattleOptions, actions: &[PlannedAction]) -> CliApiResult<Self> {
        let mut session = Self::new(raw, options)?;
        session.queue_actions(actions)?;
        Ok(session)
    }

    /// 追加外部指令；已推进的会话同样可用，指令在下一次轮到该实体时生效。
    pub fn queue_actions(&mut self, actions: &[PlannedAction]) -> CliApiResult<()> {
        let entity_count = self.driver.runner.runtime().entities.len();
        for action in actions {
            if action.actor >= entity_count {
                return Err(crate::cli_api::CliApiError::InvalidArgument(format!(
                    "planned action actor {} does not exist",
                    action.actor
                )));
            }
        }
        self.driver.runner.runtime_mut().queue_planned_actions(actions.iter().copied());
        Ok(())
    }

    /// 受控单位逼停回放时返回等待指令的实体 ID；非暂停边界为 `None`。
    ///
    /// 暂停不是终局：[`Self::result`] 仍为 `None`，`status` 仍为 running。暂停边界已经
    /// 结束该回合，本次会话不再继续推进；调用方应把新指令并入脚本后重新创建会话
    /// （例如再调一次 `battle_replay_with_actions`），而不是继续 `next_frame`。
    pub fn paused_actor(&self) -> Option<PlrId> { self.paused }

    /// 仅用于测试：破坏 skill handlers，让下一次推进经过真实 Runtime validation 失败。
    #[cfg(any(test, feature = "battle-test-support"))]
    #[doc(hidden)]
    pub fn invalidate_runtime_for_test(&mut self) { self.driver.runner.runtime_mut().skill_handlers = Default::default(); }

    /// 读取当前机制状态，不消耗随机数，也不构造展示数据。
    pub fn model_state(
        &self,
    ) -> Result<crate::runtime::model_state::BattleModelState, crate::runtime::model_state::ModelStateError> {
        self.driver.model_state()
    }

    /// 本会话创建时保存的初始展示快照。
    pub fn initial_states(&self) -> &[BattlePlayerState] { &self.initial_states }
    /// 最近推进边界的展示快照，适合更新角色面板。
    pub fn current_states(&self) -> &[BattlePlayerState] { &self.current_states }
    /// 正常对局状态；失败另由 [`Self::is_failed`] 表示。
    pub fn status(&self) -> BattleStatus {
        match self.driver.stop_reason {
            None => BattleStatus::Running,
            Some(BattleStopReason::Winner) => BattleStatus::Finished,
            // Paused 只出现在回放的截断原因上，会话本身仍在运行。
            Some(BattleStopReason::Paused) => BattleStatus::Running,
            Some(_) => BattleStatus::Truncated,
        }
    }
    /// 正常终止原因；运行中及 Runtime 失败后均为 `None`。
    pub fn stop_reason(&self) -> Option<BattleStopReason> { self.driver.stop_reason }
    /// 是否已产生正常 terminal BattleResult；Runtime failure 不属于正常终止。
    pub fn is_done(&self) -> bool { self.driver.stop_reason.is_some() }
    /// Runtime error 后为 true；后续推进返回相同错误，调用方应停止推进并释放会话。
    pub fn is_failed(&self) -> bool { self.driver.failure.is_some() }
    pub fn is_finished(&self) -> bool { self.status() == BattleStatus::Finished }
    pub fn is_truncated(&self) -> bool { self.status() == BattleStatus::Truncated }
    /// 实际推进的 Runtime 回合数，包含没有可见帧的回合。
    pub fn rounds_advanced(&self) -> usize { self.driver.rounds_advanced }
    /// 已产出的可见帧数，不等于 Runtime 回合数。
    pub fn frames_emitted(&self) -> usize { self.driver.frames_emitted }

    /// 同步推进至下一个可见帧、正常终止、受控单位暂停或 Runtime 错误。
    ///
    /// 空回合会继续推进并更新当前快照，但不占可见帧编号；没有消息的胜利也会产生终止帧。
    /// 同一回合先判胜，再判回合上限，最后判长期无可见进展。正常终止后的调用返回
    /// `Ok(None)`；粘性失败后的调用返回第一次错误且不继续推进。
    /// 无可见帧的截断也可能返回 `Ok(None)`，用 [`Self::result`] 取得终止原因与最终快照。
    /// 受控单位暂停时返回该边界已有的帧（可能为 `None`），再用 [`Self::paused_actor`] 查询。
    pub fn next_frame(&mut self) -> CliApiResult<Option<BattleReplayFrame>> {
        if let Some(error) = &self.driver.failure {
            return Err(error.clone());
        }
        if self.is_done() || self.paused.is_some() {
            return Ok(None);
        }
        loop {
            let Some(step) = self.driver.advance_round()? else {
                return Ok(None);
            };
            let round_index = step.round_index;
            let updates = step.updates;
            let visible = step.frame_index.is_some();
            let states = states_from_runner(
                &self.driver.runner,
                self.options.include_icons,
                self.options.include_skills,
                &mut self.icon_cache,
            );
            let frame = visible.then(|| {
                let mut frame = build_frame(&updates, &self.current_states, &states, &self.driver.runner);
                frame.frame_index = step.frame_index.unwrap();
                frame.round_index = round_index;
                frame
            });
            self.current_states = states;
            // 脚本指令执行失败属于调用方输入错误；保留为粘性失败，不伪装成截断结果。
            if let Some(error) = self.driver.runner.runtime_mut().take_planned_action_error() {
                let error = crate::cli_api::CliApiError::InvalidArgument(error.to_string());
                self.driver.failure = Some(error.clone());
                return Err(error);
            }
            if let Some(actor) = self.driver.runner.runtime_mut().take_awaiting_actor() {
                self.paused = Some(actor);
            }
            if frame.is_some() || self.is_done() || self.paused.is_some() {
                return Ok(frame);
            }
        }
    }

    /// 正常终止后返回拥有最终快照的结果；运行中或粘性失败时为 `None`。
    ///
    /// 重复查询不推进战斗，但每次都会复制最终快照及其字符串。需要多次使用结果时
    /// 可保存首次返回值。截断结果不附胜者 ID 或获胜输入组，避免将未结束对局当成胜利。
    pub fn result(&self) -> Option<BattleResult> {
        Some(BattleResult {
            status: self.status(),
            stop_reason: self.driver.stop_reason?,
            finished: self.is_finished(),
            truncated: self.is_truncated(),
            rounds_advanced: self.driver.rounds_advanced,
            frames_emitted: self.driver.frames_emitted,
            winner_ids: if self.is_finished() {
                self.driver.runner.winner_ids()
            } else {
                Vec::new()
            },
            winner_team_indices: if self.is_finished() {
                self.driver.runner.winner_team_indices()
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
    include_skills: bool,
    icon_cache: &mut HashMap<String, String>,
) -> Vec<BattlePlayerState> {
    runner
        .player_snapshots()
        .into_iter()
        .map(|snapshot| {
            let mut state = state_from_snapshot(snapshot, false);
            if include_skills {
                state.skills = skill_snapshots(runner, state.id);
            }
            if include_icons {
                state.icon_png_base64 = Some(match icon_cache.get(&state.icon_key) {
                    Some(icon) => icon.clone(),
                    None => {
                        let icon = render_icon_b64_from_name(&state.icon_key);
                        icon_cache.insert(state.icon_key.clone(), icon.clone());
                        icon
                    }
                });
            }
            state
        })
        .collect()
}

/// 读取指定实体的技能熟练度投影。
///
/// 按固定槽位顺序遍历，跳过 0 级与注册表里查不到的技能；同名技能只保留一项并取最高等级。
fn skill_snapshots(runner: &RuntimeRunner, id: PlrId) -> Vec<SkillSnapshot> {
    let runtime = runner.runtime();
    let Some(entity) = runtime.entities.get(crate::runtime::entity::EntityIdx(id as u32)) else {
        return Vec::new();
    };
    let loadout = &entity.template.skills;
    let mut skills: Vec<SkillSnapshot> = Vec::new();
    for (lane, skill_id) in loadout.skills().iter().enumerate() {
        let level = loadout.level_at(lane).unwrap_or(0);
        if level == 0 {
            continue;
        }
        let Some(spec) = runtime.registry.skill(*skill_id) else {
            continue;
        };
        match skills.iter_mut().find(|skill| skill.export_name == spec.export_name) {
            Some(existing) => existing.level = existing.level.max(level),
            None => skills.push(SkillSnapshot {
                export_name: spec.export_name.clone(),
                level,
            }),
        }
    }
    skills
}

fn state_from_snapshot(snapshot: RuntimePlayerSnapshot, include_icons: bool) -> BattlePlayerState {
    let icon_key = snapshot.id_key_name.clone();
    let icon_png_base64 = include_icons.then(|| render_icon_b64_from_name(&icon_key));
    BattlePlayerState {
        id: snapshot.id,
        team_index: snapshot.team_index,
        input_team_index: snapshot.input_team_index,
        owner_id: snapshot.owner_id,
        source_id: snapshot.root_owner_id,
        id_name: snapshot.id_name,
        id_key_name: snapshot.id_key_name,
        icon_key,
        display_name: snapshot.display_name,
        display_index: snapshot.display_index,
        base_name: snapshot.base_name,
        player_type: snapshot.player_type.to_owned(),
        minion_kind: snapshot.minion_kind.map(RuntimeMinionKind::as_str),
        icon_png_base64,
        hp: snapshot.hp,
        max_hp: snapshot.max_hp,
        shield: snapshot.shield,
        max_shield: snapshot.max_shield,
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
        // 纯快照不含技能表；开启 include_skills 时由 states_from_runner 填充。
        skills: Vec::new(),
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
    } else if message.contains("点伤害") || is_percent_hp_damage_update(update) {
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
        for id in session.driver.runner.all_player_ids() {
            session.driver.runner.runtime_mut().world.remove_round_actor(EntityIdx(id as u32));
        }
    }

    #[test]
    fn shield_serializes_through_initial_frame_clip_and_final_states() {
        let mut session = session(1);
        let initial_json = serde_json::to_value(session.initial_states()).unwrap();
        assert!(initial_json.as_array().unwrap().iter().all(|state| state["shield"] == 0));
        let max_shield = session.initial_states()[1].max_shield;
        assert_eq!(initial_json[1]["max_shield"], max_shield);
        session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.shield = 40;
        let previous = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
        session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.shield = 25;
        let states = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
        let mut updates = RunUpdates::new();
        updates.add(crate::RunUpdate::new("[0]发起攻击", 0, 1, 0));
        updates.add(crate::RunUpdate::new("[1]受到[2]点伤害", 0, 1, 0));
        updates.add(crate::RunUpdate::new("[1]挡住攻击", 0, 1, 0));
        let frame = build_frame(&updates, &previous, &states, &session.driver.runner);
        let frame_json = serde_json::to_value(&frame).unwrap();
        assert_eq!(frame_json["states"][1]["shield"], 25);
        assert_eq!(frame_json["states"][1]["max_shield"], max_shield);
        let clips = &frame_json["rows"][0]["clips"];
        assert_eq!(clips[0]["sidebar_previous_states"][1]["shield"], 40);
        assert_eq!(clips[0]["sidebar_states"][1]["shield"], 40);
        assert_eq!(clips[1]["sidebar_previous_states"][1]["shield"], 40);
        assert_eq!(clips[1]["sidebar_states"][1]["shield"], 25);
        assert_eq!(clips[2]["sidebar_previous_states"][1]["shield"], 25);
        assert_eq!(clips[2]["sidebar_states"][1]["shield"], 25);
        for clip in clips.as_array().unwrap() {
            assert_eq!(clip["sidebar_previous_states"][1]["max_shield"], max_shield);
            assert_eq!(clip["sidebar_states"][1]["max_shield"], max_shield);
        }
        assert_eq!(
            session.driver.runner.runtime().entities.get(EntityIdx(1)).unwrap().runtime.shield,
            25
        );

        remove_round_actors(&mut session);
        assert_eq!(session.next_frame().unwrap(), None);
        let result_json = serde_json::to_value(session.result().unwrap()).unwrap();
        assert_eq!(result_json["final_states"][1]["shield"], 25);
        assert_eq!(result_json["final_states"][1]["max_shield"], max_shield);
    }

    #[test]
    fn half_skill_percent_message_classifies_as_damage_and_preserves_actual_hp_pair() {
        let mut session = session(1);
        let runtime = session.driver.runner.runtime_mut();
        let caster = runtime.entities.get_mut(EntityIdx(0)).unwrap();
        caster.runtime.wisdom = 512;
        caster.runtime.magic = 0;
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 20;
        target.runtime.resistance = 0;
        target.runtime.agility = 0;
        target.template.skills = Default::default();
        let previous = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
        let mut updates = RunUpdates::new();
        session
            .driver
            .runner
            .runtime_mut()
            .drain_plain_half_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);
        let states = states_from_runner(&session.driver.runner, false, false, &mut session.icon_cache);
        let frame = build_frame(&updates, &previous, &states, &session.driver.runner);
        assert_eq!(frame.updates[0].message_template, "[0]使用[瘟疫]");
        assert_eq!(frame.updates[1].message_template, "[1]体力减少[2]%");
        assert_eq!(frame.updates[1].tone, "damage");
        assert_eq!(frame.updates[1].param, Some(47));
        assert_eq!(frame.updates[1].score, 9);
        assert_eq!(frame.updates[1].hp_delta, Some(-9));
        let clips = &frame.rows[0].clips;
        assert!(clips[0].parts.iter().all(|part| !part.show_hp));
        let part = clips[1].parts.iter().find(|part| part.player_id == Some(1)).unwrap();
        assert!(part.show_hp);
        assert_eq!((part.hp_before, part.hp_after), (20, 11));
        let json = serde_json::to_value(&frame).unwrap();
        assert_eq!(json["updates"][1]["hp_delta"], -9);
        assert_eq!(json["rows"][0]["clips"][1]["parts"][0]["show_hp"], true);
    }

    #[test]
    fn stress_half_fixture_exposes_percent_hp_parts() {
        let raw =
            "My_Generation lrSP5W8@芒萁\nUnbound v7mEDujiDSLN@芒萁\n\n潮汐-糖藷愠颏@涵虚\n风驰-xtxxgwcb@涵虚\nseed:33554538@!";
        let mut session = BattleSession::new(raw, BattleOptions::default()).unwrap();
        let mut saw_half = false;
        while let Some(frame) = session.next_frame().unwrap() {
            for update in &frame.updates {
                if update.message_template == "[1]体力减少[2]%" && update.score > 0 {
                    if !saw_half {
                        println!(
                            "瘟疫百分比句首次出现：frame={} round={} {}",
                            frame.frame_index, frame.round_index, update.message_rendered
                        );
                    }
                    saw_half = true;
                    assert_eq!(update.tone, "damage");
                    assert_eq!(update.hp_delta, Some(-(update.score as i32)));
                    assert!(frame.rows.iter().flat_map(|row| &row.clips).any(|clip| {
                        clip.tone == "damage"
                            && clip
                                .parts
                                .iter()
                                .any(|part| part.player_id == update.target_id && part.show_hp && part.hp_before > part.hp_after)
                    }));
                }
            }
        }
        assert!(saw_half);
    }

    #[test]
    fn shield_overlay_fixture_exposes_positive_snapshot_and_clip_delta() {
        let raw = r#"left@red+ol:{"skills":{"shield":80}}

right@blue+ol:{"skills":{}}
seed:7@!"#;
        let mut session = BattleSession::new(raw, BattleOptions::default()).unwrap();
        let mut saw_shield = false;
        let mut saw_consumption = false;
        while let Some(frame) = session.next_frame().unwrap() {
            if !saw_shield && frame.states.iter().any(|state| state.shield > 0) {
                saw_shield = true;
                println!("护盾首次出现：frame={} round={}", frame.frame_index, frame.round_index);
            }
            for clip in frame.rows.iter().flat_map(|row| &row.clips) {
                for (before, after) in clip.sidebar_previous_states.iter().zip(&clip.sidebar_states) {
                    if before.shield > after.shield {
                        saw_consumption = true;
                        assert!(clip.parts.iter().any(|part| part.player_id == Some(after.id)));
                    }
                }
            }
        }
        assert!(saw_shield);
        assert!(saw_consumption);
    }

    #[test]
    fn skills_are_only_exported_when_requested() {
        let raw = r#"left@red+ol:{"skills":{"shield":80}}

right@blue+ol:{"skills":{}}
seed:7@!"#;

        // 默认关闭：技能表为空，序列化时连键都不出现，既有载荷不变。
        let plain = BattleSession::new(raw, BattleOptions::default()).unwrap();
        assert!(plain.initial_states().iter().all(|state| state.skills.is_empty()));
        let json = serde_json::to_value(plain.initial_states()).unwrap();
        assert!(json[0].get("skills").is_none(), "默认不应序列化 skills: {json}");

        let options = BattleOptions {
            include_skills: true,
            ..BattleOptions::default()
        };
        let mut session = BattleSession::new(raw, options).unwrap();
        let left = &session.initial_states()[0];
        assert_eq!(left.display_name, "left");
        assert_eq!(left.skills.len(), 1, "overlay 只声明 shield 一项熟练度：{:?}", left.skills);
        assert_eq!(left.skills[0].level, 80);
        assert!(!left.skills[0].export_name.is_empty());

        // 帧内快照同样带熟练度，且不影响战斗结果。
        let mut saw_skills = false;
        while let Some(frame) = session.next_frame().unwrap() {
            saw_skills |= frame.states.iter().any(|state| !state.skills.is_empty());
        }
        assert!(saw_skills);
        assert!(session.result().unwrap().finished);
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
        let runtime = session.driver.runner.runtime_mut();
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

    #[cfg(feature = "png_render")]
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

    #[cfg(feature = "png_render")]
    #[test]
    fn icon_projection_preserves_full_keys_cache_values_and_rng() {
        let runner = RuntimeRunner::new_from_namerena_raw("同名@红队\n\n同名@蓝队\nseed:cache@!".to_owned()).unwrap();
        let snapshots = runner.player_snapshots();
        assert_eq!(snapshots[0].id_name, snapshots[1].id_name);
        assert_ne!(snapshots[0].id_key_name, snapshots[1].id_key_name);
        let before = runner.model_state().unwrap();
        let rng_before = runner.runtime().rng.clone();
        let cached_key = snapshots[0].id_key_name.clone();
        let missing_key = snapshots[1].id_key_name.clone();
        let mut cache = HashMap::from([(cached_key.clone(), "预填图标".to_owned())]);
        let original_cache = cache.clone();

        let plain = states_from_runner(&runner, false, false, &mut cache);
        assert!(plain.iter().all(|state| state.icon_png_base64.is_none()));
        assert_eq!(cache, original_cache);

        let with_icons = states_from_runner(&runner, true, false, &mut cache);
        assert_eq!(cache.len(), 2);
        assert_eq!(with_icons[0].icon_key, cached_key);
        assert_eq!(with_icons[0].icon_png_base64.as_deref(), Some("预填图标"));
        assert_eq!(with_icons[1].icon_key, missing_key);
        assert_eq!(cache[&missing_key], render_icon_b64_from_name(&missing_key));
        for ((state, plain), snapshot) in with_icons.iter().zip(&plain).zip(&snapshots) {
            assert_eq!(state.id, snapshot.id);
            assert_eq!(state.id_key_name, snapshot.id_key_name);
            assert_eq!(state.icon_png_base64.as_ref(), cache.get(&state.icon_key));
            let mut without_icon = state.clone();
            without_icon.icon_png_base64 = None;
            assert_eq!(&without_icon, plain);
        }
        assert_eq!(state_from_snapshot(snapshots[1].clone(), true), with_icons[1],);
        let filled_cache = cache.clone();
        assert_eq!(states_from_runner(&runner, true, false, &mut cache), with_icons);
        assert_eq!(cache, filled_cache);
        assert_eq!(runner.model_state().unwrap(), before);
        let rng_after = &runner.runtime().rng;
        assert_eq!((rng_after.i, rng_after.j), (rng_before.i, rng_before.j));
        assert_eq!(rng_after.main_val, rng_before.main_val);
        #[cfg(not(feature = "no_debug"))]
        assert_eq!(rng_after.byte_count, rng_before.byte_count);
    }

    #[cfg(not(feature = "png_render"))]
    #[test]
    fn missing_png_feature_rejects_icon_requests() {
        let options = BattleOptions {
            include_icons: true,
            ..BattleOptions::default()
        };
        let raw = "left@red\n\nright@blue";
        for error in [
            BattleSession::new(raw, options).unwrap_err(),
            super::super::battle_replay(raw, options).unwrap_err(),
        ] {
            assert_eq!(error.code().as_str(), "UNSUPPORTED_OPTION");
            assert!(error.to_string().contains("png_render"));
        }
    }

    #[cfg(not(feature = "png_render"))]
    #[test]
    fn missing_png_feature_keeps_plain_and_model_sessions_usable() {
        let raw = "left@red\n\nright@blue";
        let options = BattleOptions {
            max_rounds: 2,
            ..BattleOptions::default()
        };
        let mut plain = BattleSession::new(raw, options).unwrap();
        let mut model = super::super::BattleModelSession::new(
            raw,
            BattleOptions {
                include_icons: true,
                ..options
            },
        )
        .unwrap();
        assert!(!plain.initial_states().is_empty());
        assert!(plain.initial_states().iter().all(|state| state.icon_png_base64.is_none()));
        assert_eq!(plain.model_state().unwrap(), model.model_state().unwrap());
        loop {
            let display = plain.next_frame().unwrap();
            let boundary = model.next_frame().unwrap();
            assert_eq!(display.is_some(), boundary.is_some());
            if let (Some(display), Some(boundary)) = (&display, boundary) {
                assert_eq!(
                    (display.frame_index, display.round_index),
                    (boundary.frame_index, boundary.round_index),
                );
                assert!(display.states.iter().all(|state| state.icon_png_base64.is_none()));
            }
            assert_eq!(plain.model_state().unwrap(), model.model_state().unwrap());
            if display.is_none() {
                break;
            }
        }
        let result = plain.result().unwrap();
        let model_result = model.result().unwrap();
        assert_eq!(result.stop_reason, model_result.stop_reason);
        assert_eq!(result.rounds_advanced, model_result.rounds_advanced);
        assert!(result.final_states.iter().all(|state| state.icon_png_base64.is_none()));
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
        session.driver.runner.runtime_mut().entities.get_mut(EntityIdx(1)).unwrap().runtime.alive = false;
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

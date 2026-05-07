use std::collections::HashMap;

use tswn_core::player::PlrId;
use tswn_core::player::skill::{
    act::{
        berserk::BerserkState, charm::CharmState, curse::CurseState, haste::HasteState, ice::IceState, iron::IronState,
        poison::PoisonState, slow::SlowState,
    },
    skl::{protect::ProtectState, upgrade::UpgradeState},
};
use tswn_core::{RunUpdates, Runner};
use wasm_bindgen::prelude::*;

use crate::error::{WasmResult, internal_error, invalid_input, parse_options, runner_init_failed, to_js_value};
use crate::model::{FightOptions, FightReplay, FightSummary, PlayerMeta, PlayerState, RoundFrame, UpdateView};
use crate::render::{classify_message_tone, render_update_message};

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
        let Some(player) = runner.storage.get_player(player_id) else {
            return Err(internal_error(format!("player {player_id} missing from storage")));
        };
        let display_name = player.display_name();
        let team_index = runner.world.team_index_of(*player_id).unwrap_or(0);
        let icon_png_base64 = if include_icons {
            Some(tswn_core::player::icon_render::render_icon_b64_from_name(&player.id_key_name()))
        } else {
            None
        };
        names.insert(*player_id, display_name.clone());
        players.push(PlayerMeta {
            id: *player_id,
            team_index,
            id_name: player.id_name(),
            display_name,
            icon_png_base64,
        });
    }

    Ok((players, names))
}

/// 追溯幻影/分身/使魔的根本体 ID。如果玩家本身不是 minion，返回 None。
fn root_owner_id(storage: &tswn_core::engine::storage::Storage, start_id: PlrId) -> Option<PlrId> {
    use tswn_core::player::skill::act::minion::MinionRuntimeState;
    let mut current = start_id;
    // 先检查自己是不是 minion
    let first = storage.get_player_or_pending(&current)?;
    first.get_state::<MinionRuntimeState>()?;
    // 追溯 owner 链直到非 minion
    loop {
        let plr = storage.get_player_or_pending(&current)?;
        let Some(minion) = plr.get_state::<MinionRuntimeState>() else {
            return Some(current);
        };
        let owner = minion.owner?;
        current = owner;
    }
}

fn push_status_label(labels: &mut Vec<String>, label: &str) {
    if labels.iter().any(|existing| existing == label) {
        return;
    }
    labels.push(label.to_string());
}

fn has_active_skill<F>(player: &tswn_core::player::Player, runtime_kind_suffix: &str, active: F) -> bool
where
    F: Fn(&tswn_core::player::skill::Skill) -> bool,
{
    player
        .skill_storage()
        .store
        .values()
        .any(|skill| skill.debug_skill_type_name().ends_with(runtime_kind_suffix) && active(skill))
}

fn collect_states(runner: &Runner, player_order: &[PlrId]) -> WasmResult<Vec<PlayerState>> {
    // 收集所有当前玩家（含召唤单位），保持初始顺序 + 新单位追加
    let mut seen: std::collections::HashSet<PlrId> = std::collections::HashSet::new();
    let mut all_ids: Vec<PlrId> = Vec::new();
    for id in player_order {
        if seen.insert(*id) {
            all_ids.push(*id);
        }
    }
    for id in runner.storage.all_player_ids() {
        if seen.insert(id) {
            all_ids.push(id);
        }
    }

    let mut states = Vec::with_capacity(all_ids.len());
    for player_id in &all_ids {
        let Some(player) = runner.storage.get_player(player_id) else {
            continue;
        };
        let owner_id = root_owner_id(&runner.storage, *player_id);
        let minion_kind = player
            .get_state::<tswn_core::player::skill::act::minion::MinionRuntimeState>()
            .map(|state| state.kind.into());
        let status = player.get_status();
        let mut status_labels = Vec::new();

        if has_active_skill(player, "::AccumulateSkill", |skill| skill.dynamic_update_state_enabled()) {
            push_status_label(&mut status_labels, "聚气");
        }
        if has_active_skill(player, "::ChargeSkill", |skill| skill.charge_runtime_active()) {
            let step = player
                .skill_storage()
                .store
                .values()
                .find_map(|s| {
                    if s.debug_skill_type_name().ends_with("::ChargeSkill") {
                        Some(s.charge_step())
                    } else {
                        None
                    }
                })
                .unwrap_or(0);
            let suffix = if step > 0 {
                format!(" ({})", step)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("蓄力{}", suffix));
        }
        if has_active_skill(player, "::HideSkill", |skill| skill.dynamic_update_state_enabled()) {
            push_status_label(&mut status_labels, "隐匿");
        }
        if has_active_skill(player, "::AssassinateSkill", |skill| skill.dynamic_pre_action_enabled()) {
            let target = player
                .skill_storage()
                .store
                .values()
                .find_map(|s| {
                    if s.debug_skill_type_name().ends_with("::AssassinateSkill") {
                        s.assassinate_target()
                    } else {
                        None
                    }
                });
            if let Some(target_id) = target {
                push_status_label(&mut status_labels, &format!("潜行至 #{}", target_id));
            } else {
                push_status_label(&mut status_labels, "潜行");
            }
        }

        if let Some(state) = player.get_state::<BerserkState>() {
            let suffix = if state.step > 0 {
                format!(" ({})", state.step)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("狂暴{}", suffix));
        }
        if let Some(state) = player.get_state::<CharmState>() {
            let suffix = if state.step > 0 {
                format!(" ({})", state.step)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("魅惑{}", suffix));
        }
        if let Some(state) = player.get_state::<CurseState>() {
            let suffix = if state.multiply > 0 {
                format!(" x{}", state.multiply)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("诅咒{}", suffix));
        }
        if let Some(state) = player.get_state::<HasteState>() {
            let suffix = if state.faster > 0 {
                format!(" +{}", state.faster)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("疾走{}", suffix));
        }
        if player.get_state::<IceState>().is_some() {
            push_status_label(&mut status_labels, "冰冻");
        }
        if let Some(state) = player.get_state::<IronState>() {
            let suffix = if state.protect > 0 {
                format!(" +{}", state.protect)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("铁壁{}", suffix));
        }
        if let Some(state) = player.get_state::<PoisonState>() {
            push_status_label(&mut status_labels, &format!("中毒 {}层", state.count));
        }
        if let Some(protect_state) = player.get_state::<ProtectState>()
            && let Some(link) = protect_state.protect_from.first()
        {
            let protector_id = link.owner;
            push_status_label(&mut status_labels, &format!("被 #{} 守护", protector_id));
        }
        // 检查当前玩家是否正在守护他人（拥有 ProtectSkill 且 protect_to 有值）
        let protect_target = player.skill_storage().store.values().find_map(|s| s.protect_to_id());
        if let Some(target_id) = protect_target
            && target_id != *player_id
        {
            push_status_label(&mut status_labels, &format!("守护 #{} 中", target_id));
        }
        if let Some(state) = player.get_state::<SlowState>() {
            let suffix = if state.step > 0 {
                format!(" ({})", state.step)
            } else {
                String::new()
            };
            push_status_label(&mut status_labels, &format!("迟缓{}", suffix));
        }
        if player.get_state::<UpgradeState>().is_some() {
            push_status_label(&mut status_labels, "垂死");
        }
        if status.frozen {
            push_status_label(&mut status_labels, "冰冻");
        }
        states.push(PlayerState {
            id: *player_id,
            team_index: runner.world.team_index_of(*player_id).unwrap_or(0),
            id_name: player.id_name(),
            display_name: player.display_name(),
            owner_id,
            minion_kind,
            hp: status.hp,
            max_hp: status.max_hp,
            magic_point: status.magic_point,
            move_point: status.move_point,
            attack: status.attack,
            defense: status.defense,
            speed: status.speed,
            agility: status.agility,
            magic: status.magic,
            resistance: status.resistance,
            wisdom: status.wisdom,
            point: status.point,
            all_sum: status.all_sum,
            name_factor: player.get_name_factor(),
            at_boost: status.at_boost,
            attract: status.attract,
            frozen: status.frozen,
            alive: player.alive(),
            status_labels,
        });
    }
    Ok(states)
}

fn player_names_from_states(states: &[PlayerState]) -> HashMap<PlrId, String> {
    states.iter().map(|state| (state.id, state.display_name.clone())).collect()
}

fn convert_updates(updates: RunUpdates, player_names: &HashMap<PlrId, String>) -> Vec<UpdateView> {
    updates
        .updates
        .into_iter()
        .map(|update| {
            let tone = classify_message_tone(&update.message);
            let message_rendered = render_update_message(&update, player_names);
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
                tone,
            }
        })
        .collect()
}

fn winner_ids(runner: &Runner) -> Vec<usize> { runner.world.winner.clone().unwrap_or_default() }

#[wasm_bindgen]
pub struct FightSession {
    runner: Runner,
    player_order: Vec<PlrId>,
    players: Vec<PlayerMeta>,
    capture_replay: bool,
}

impl FightSession {
    pub(crate) fn new_internal(raw_input: String, options: FightOptions) -> WasmResult<Self> {
        let runner = build_runner(raw_input, options.resolved_eval_rq())?;
        let player_order = runner.all_plrs();
        let (players, _player_names) = collect_players(&runner, &player_order, options.include_icons())?;
        Ok(Self {
            runner,
            player_order,
            players,
            capture_replay: options.capture_replay(),
        })
    }

    fn build_frame(&self, updates: RunUpdates) -> WasmResult<RoundFrame> {
        let states = collect_states(&self.runner, &self.player_order)?;
        let converted = convert_updates(updates, &player_names_from_states(&states));
        let total_delay: i32 = converted.iter().map(|u| if u.delay1 != 0 { u.delay1 } else { u.delay0 }).sum();
        Ok(RoundFrame {
            finished: self.runner.have_winner(),
            winner_ids: winner_ids(&self.runner),
            updates: converted,
            states,
            total_delay,
        })
    }

    pub(crate) fn run_to_end_internal(&mut self, limit: Option<usize>) -> WasmResult<FightReplay> {
        let max_frames = limit.unwrap_or(usize::MAX);
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
            final_states: collect_states(&self.runner, &self.player_order)?,
        })
    }
}

#[wasm_bindgen]
impl FightSession {
    #[wasm_bindgen(constructor)]
    pub fn new(raw_input: String, options: Option<JsValue>) -> WasmResult<FightSession> {
        crate::install_panic_hook();
        let options = parse_options(options)?;
        Self::new_internal(raw_input, options)
    }

    pub fn players(&self) -> WasmResult<JsValue> { to_js_value(&self.players) }

    pub fn state(&self) -> WasmResult<JsValue> {
        let states = collect_states(&self.runner, &self.player_order)?;
        to_js_value(&states)
    }

    pub fn is_finished(&self) -> bool { self.runner.have_winner() }

    pub fn winner_ids(&self) -> WasmResult<JsValue> { to_js_value(&winner_ids(&self.runner)) }

    pub fn step(&mut self) -> WasmResult<JsValue> {
        let frame = if self.runner.have_winner() {
            self.build_frame(RunUpdates::new())?
        } else {
            let updates = self.runner.main_round();
            self.build_frame(updates)?
        };
        to_js_value(&frame)
    }

    pub fn run_to_end(&mut self, limit: Option<usize>) -> WasmResult<JsValue> {
        let replay = self.run_to_end_internal(limit)?;
        to_js_value(&replay)
    }
}

pub(crate) fn fight_impl(raw_input: String, options: FightOptions) -> WasmResult<JsValue> {
    let mut session = FightSession::new_internal(raw_input, options)?;
    let replay = session.run_to_end_internal(None)?;
    to_js_value(&replay)
}

pub(crate) fn fight_summary_impl(raw_input: String, options: FightOptions) -> WasmResult<JsValue> {
    let mut session = FightSession::new_internal(raw_input, options)?;
    let replay = session.run_to_end_internal(None)?;
    let summary = FightSummary {
        finished: session.runner.have_winner(),
        players: replay.players,
        winner_ids: replay.winner_ids,
        final_states: replay.final_states,
    };
    to_js_value(&summary)
}

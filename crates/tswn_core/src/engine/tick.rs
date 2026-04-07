//! # 单 Tick 行动流程 (tick)
//!
//! 本模块包含每个 tick 内战斗解算的各个步骤函数，由 [`EngineCore::tick`](crate::engine::engine_core::EngineCore::tick) 按序调用。
//!
//! ## Tick 执行流程
//!
//! ```text
//! 1. next_actor()         — 从 WorldState.players 中轮流取出下一个行动角色
//! 2. choose_action()      — 决定该角色本 tick 是否执行行动（存活且未冻结）
//! 3. select_targets()     — 根据魅惑状态等确定友方/敌方/全场存活目标集合
//! 4. resolve_combat()     — 驱动 pre_damage 钩子 → Player::step() → post_damage 钩子
//! 5. run_update_end()     — 处理 on_update_end 回调队列（持续效果结算）
//! 6. check_winner()       — 检查是否只剩一支队伍存活，若是则写入 winner
//! ```
//!
//! ## 可见性说明
//!
//! 本模块中的大部分函数标记为 `pub(super)`，意味着只有 `engine` 模块内部
//!（主要是 `engine_core`）可以调用它们，外部包不直接调用单独的 tick 步骤。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::engine::{hooks::HookPipeline, rules::RuleRegistry, world_state::WorldState};
use crate::player::{ActionTargets, PlrId, action_targets::PlrVec};
use crate::rc4::RC4;

/// Tick 行动决策枚举，由 [`choose_action`] 返回。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionDecision {
    StepDriver,
    Skip,
}

pub fn next_actor(world: &mut WorldState, _storage: &Arc<Storage>) -> Option<PlrId> {
    if world.players.is_empty() {
        return None;
    }
    let idx = world.next_round_index(world.players.len());
    Some(world.players[idx])
}

pub fn choose_action(
    actor: PlrId,
    world: &WorldState,
    storage: &Arc<Storage>,
    _randomer: &mut RC4,
    _rules: &RuleRegistry,
) -> ActionDecision {
    if world.have_winner() {
        return ActionDecision::Skip;
    }
    if storage.get_player(&actor).map(|x| x.get_status().alive()).unwrap_or(false) {
        ActionDecision::StepDriver
    } else {
        ActionDecision::Skip
    }
}

pub(super) fn select_targets(actor: PlrId, world: &WorldState, storage: &Arc<Storage>) -> ActionTargets {
    use crate::player::skill::charm::CharmState;

    let is_alive_now = |id: &PlrId| storage.get_player(id).map(|player| player.alive()).unwrap_or(false);

    let Some(team_idx) = world.team_index_of(actor) else {
        return ActionTargets::default();
    };
    let effective_team = storage
        .get_player(&actor)
        .and_then(|player| player.get_state::<CharmState>())
        .and_then(|charm| charm.effective_team_idx.or_else(|| world.team_index_of(charm.group_id)))
        .unwrap_or(team_idx);
    let Some(team_roster) = world.team_roster(effective_team).map(PlrVec::from_slice) else {
        return ActionTargets::default();
    };

    let ally_alive: PlrVec = world
        .team_alive(effective_team)
        .map(|team| team.iter().copied().filter(is_alive_now).collect())
        .unwrap_or_default();
    let ally_all = team_roster.clone();
    let ally_dead: PlrVec = team_roster.iter().copied().filter(|id| !ally_alive.contains(id)).collect();
    // 使用 world.flat_alive 而非从 teams 重建，以保持与 JS Engine.e 相同的全局存活顺序。
    // JS 的 pickSkipRange 依赖 all_alive 中各实体的精确位置，当复活/召唤导致
    // 实体追加到末尾时，按 team 迭代重建会把它们放回 team 槽位，与 JS 顺序不同。
    let mut all_alive: PlrVec = PlrVec::new();
    let mut enemy_alive: PlrVec = PlrVec::new();
    let mut enemy_skip_indices = smallvec::SmallVec::<[usize; 4]>::new();
    for id in world.flat_alive.iter().copied() {
        if !is_alive_now(&id) {
            continue;
        }
        let idx = all_alive.len();
        all_alive.push(id);
        if ally_alive.contains(&id) {
            enemy_skip_indices.push(idx);
        } else {
            enemy_alive.push(id);
        }
    }

    ActionTargets {
        enemy_alive,
        ally_alive,
        ally_all,
        ally_dead,
        all_alive,
        enemy_skip_indices,
    }
}

pub struct TickContext<'a> {
    pub storage: &'a Arc<Storage>,
    pub randomer: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
}

pub fn resolve_combat(
    actor: PlrId,
    decision: ActionDecision,
    targets: &ActionTargets,
    ctx: &mut TickContext<'_>,
    hooks: &HookPipeline,
) {
    match decision {
        ActionDecision::StepDriver => {
            hooks.run_pre_damage(actor, ctx.storage, ctx.randomer, ctx.updates);
            if let Some(plr) = ctx.storage.just_get_player_mut(actor) {
                plr.step(ctx.randomer, ctx.updates, ctx.storage, targets);
            }
            hooks.run_post_damage(actor, ctx.storage, ctx.randomer, ctx.updates);
        }
        ActionDecision::Skip => {}
    }
}

pub fn check_winner(world: &mut WorldState, _storage: &Arc<Storage>) {
    let mut alive_count = 0u32;
    let mut last_alive_idx = 0usize;
    for (idx, team) in world.teams.iter().enumerate() {
        if !team.alive.is_empty() {
            alive_count += 1;
            last_alive_idx = idx;
            if alive_count > 1 {
                break;
            }
        }
    }
    world.winner = if alive_count == 1 {
        world.winner_roster(last_alive_idx)
    } else {
        None
    };
}

pub(super) fn has_updates(updates: &RunUpdates) -> bool { updates.had_updates() }

pub(super) fn run_update_end(storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
    let mut guard = 0usize;
    while guard < 64 && !updates.on_update_end.is_empty() {
        let pending = std::mem::take(&mut updates.on_update_end);
        for actor in pending {
            if let Some(plr) = storage.just_get_player_mut(actor) {
                let _ = plr.on_update_end(randomer, updates, storage);
            }
        }
        guard += 1;
    }
}

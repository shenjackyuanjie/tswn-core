use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::engine::{world_state::WorldState, hooks::HookPipeline, rules::RuleRegistry};
use crate::player::{ActionTargets, PlrId};
use crate::rc4::RC4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionDecision {
    StepDriver,
    Skip,
}

pub(super) fn next_actor(world: &mut WorldState, _storage: &Arc<Storage>) -> Option<PlrId> {
    if world.players.is_empty() {
        return None;
    }
    let idx = world.next_round_index(world.players.len());
    Some(world.players[idx])
}

pub(super) fn choose_action(
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

    let Some(team_idx) = world.team_index_of(actor) else {
        return ActionTargets::default();
    };
    let effective_team = storage
        .get_player(&actor)
        .and_then(|player| player.get_state::<CharmState>())
        .and_then(|charm| world.team_index_of(charm.group_id))
        .unwrap_or(team_idx);
    let Some(team_roster) = world.team_roster(effective_team).map(|team| team.to_vec()) else {
        return ActionTargets::default();
    };
    
    let ally_alive = world.team_alive(effective_team).map(|team| team.to_vec()).unwrap_or_default();
    let ally_all = team_roster.clone();
    let ally_dead = team_roster.iter().copied().filter(|id| !ally_alive.contains(id)).collect::<Vec<PlrId>>();
    let all_alive = world.alives_flat(storage);
    let enemy_alive = world
        .teams
        .iter()
        .enumerate()
        .filter(|(idx, _)| *idx != effective_team)
        .flat_map(|(_, team)| team.alive.iter().copied())
        .collect::<Vec<PlrId>>();

    ActionTargets {
        enemy_alive,
        ally_alive,
        ally_all,
        ally_dead,
        all_alive,
    }
}

pub struct TickContext<'a> {
    pub storage: &'a Arc<Storage>,
    pub randomer: &'a mut RC4,
    pub updates: &'a mut RunUpdates,
}

pub(super) fn resolve_combat(
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

pub(super) fn check_winner(world: &mut WorldState, _storage: &Arc<Storage>) {
    let mut alive_team_indices = world
        .teams
        .iter()
        .enumerate()
        .filter_map(|(idx, team)| (!team.alive.is_empty()).then_some(idx))
        .collect::<Vec<usize>>();
    world.winner = if alive_team_indices.len() == 1 {
        world.winner_roster(alive_team_indices.remove(0))
    } else {
        None
    };
}

pub(super) fn has_updates(updates: &RunUpdates) -> bool {
    !updates.updates.is_empty()
}

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

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::engine::{world_state::WorldState, hooks::HookPipeline, rules::RuleRegistry};
use crate::player::PlrId;
use crate::rc4::RC4;

#[derive(Default)]
pub struct EngineCore {
    pub hooks: HookPipeline,
    pub rules: RuleRegistry,
}

impl EngineCore {
    pub fn register_pre_action_hook(&mut self, hook: crate::engine::hooks::ActorHook) {
        self.hooks.register_pre_action(hook);
    }

    pub fn register_post_action_hook(&mut self, hook: crate::engine::hooks::ActorHook) {
        self.hooks.register_post_action(hook);
    }

    pub fn debug_world_state(tag: &str, world: &WorldState, storage: &Arc<Storage>) {
        if std::env::var_os("TSWN_DEBUG_WORLD").is_none() {
            return;
        }
        let players = world
            .players
            .iter()
            .map(|id| storage.get_player(id).map(|p| p.id_name()).unwrap_or_else(|| format!("#{id}")))
            .collect::<Vec<String>>()
            .join(" -> ");
        eprintln!("[world:{}] round_pos={} players=[{}]", tag, world.round_pos, players);
    }

    pub fn sync_runtime_entities(&self, world: &mut WorldState, storage: &Arc<Storage>) {
        Self::debug_world_state("pre_sync", world, storage);

        let pending_remove_players = storage.take_pending_remove_players();
        let death_queue = storage.take_death_queue();
        for id in &death_queue {
            if world.contains_alive(*id) && !storage.get_player(id).map(|p| p.alive()).unwrap_or(false) {
                world.remove_player(*id);
                Self::debug_world_state("after_dead_remove", world, storage);
            }
        }

        for ptr in &pending_remove_players {
            if !death_queue.contains(ptr) {
                world.remove_player(*ptr);
                Self::debug_world_state("after_pending_remove_only", world, storage);
            }
        }

        let remaining_dead: Vec<PlrId> = world
            .alives_flat(storage)
            .into_iter()
            .filter(|id| !storage.get_player(id).map(|p| p.alive()).unwrap_or(false))
            .collect();
        for id in remaining_dead {
            world.remove_player(id);
            Self::debug_world_state("after_dead_remove_fallback", world, storage);
        }

        let mut revived_ids: Vec<PlrId> = Vec::new();
        for team in &world.teams {
            for id in &team.roster {
                if world.contains_alive(*id) {
                    continue;
                }
                let revived = storage.get_player(id).map(|p| p.alive()).unwrap_or(false);
                if revived && !revived_ids.contains(id) {
                    revived_ids.push(*id);
                }
            }
        }
        for id in revived_ids {
            world.revive_player(id, id);
            Self::debug_world_state("after_revive_sync", world, storage);
        }

        let pending_spawns = storage.take_pending_spawns();
        for pending in pending_spawns {
            let owner = pending.owner;
            let plr_id = storage.just_insert_player(pending.player);
            world.add_new_player(plr_id, owner);
        }

        storage.sync_groups(&world.groups);
        storage.sync_alive_groups(&world.alives_by_group(storage));
        Self::debug_world_state("post_sync", world, storage);
    }

    pub fn tick(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4, updates: &mut RunUpdates) {
        self.sync_runtime_entities(world, storage);
        if world.have_winner() {
            return;
        }

        let Some(actor) = crate::engine::tick::next_actor(world, storage) else {
            crate::engine::tick::check_winner(world, storage);
            return;
        };

        let debug_tick = std::env::var_os("TSWN_DEBUG_TICK").is_some();
        let rc4_before = if debug_tick { (randomer.i, randomer.j) } else { (0, 0) };
        if debug_tick && let Some(plr) = storage.get_player(&actor) {
            eprintln!(
                "[tick] actor={} mp={} hp={} rc4=({}, {})",
                plr.id_name(),
                plr.move_point(),
                plr.get_status().hp,
                randomer.i,
                randomer.j
            );
        }

        self.hooks.run_pre_action(actor, storage, randomer, updates);
        let decision = crate::engine::tick::choose_action(actor, world, storage, randomer, &self.rules);
        let targets = crate::engine::tick::select_targets(actor, world, storage);
        let mut ctx = crate::engine::tick::TickContext {
            storage,
            randomer,
            updates,
        };
        crate::engine::tick::resolve_combat(actor, decision, &targets, &mut ctx, &self.hooks);
        crate::engine::tick::run_update_end(storage, ctx.randomer, ctx.updates);
        if debug_tick && (ctx.randomer.i != rc4_before.0 || ctx.randomer.j != rc4_before.1)
            && let Some(plr) = storage.get_player(&actor) {
                let bytes = (ctx.randomer.i as i32 - rc4_before.0 as i32).rem_euclid(256);
                eprintln!(
                    "[tick_end] actor={} rc4=({},{})->({},{}) bytes={}",
                    plr.id_name(),
                    rc4_before.0,
                    rc4_before.1,
                    ctx.randomer.i,
                    ctx.randomer.j,
                    bytes
                );
            }
        self.sync_runtime_entities(world, storage);
        self.hooks.run_post_action(actor, storage, ctx.randomer, ctx.updates);
        crate::engine::tick::check_winner(world, storage);
    }

    pub fn main_round(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4) -> RunUpdates {
        let mut updates = RunUpdates::new();
        let max_ticks = world.all_plr_len().max(1) * 4;
        let mut ticks = 0;

        while ticks < max_ticks && !world.have_winner() && !crate::engine::tick::has_updates(&updates) {
            self.tick(world, storage, randomer, &mut updates);
            ticks += 1;
        }
        updates
    }
}

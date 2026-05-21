//! # 引擎核心调度器 (engine_core)
//!
//! 本模块提供 [`EngineCore`]，是整个战斗循环的主调度器，负责：
//!
//! 1. **实体同步** (`sync_runtime_entities`) — 每个 tick 开始前，将上一 tick 中
//!    发生的死亡/复活/召唤操作同步到 [`WorldState`]。
//! 2. **单 tick 驱动** (`tick`) — 从 [`WorldState`] 中选出本 tick 的行动角色，
//!    执行完整的「行动前钩子 → 行动决策 → 目标选取 → 战斗解算 → 钩子后处理」流程。
//! 3. **回合驱动** (`main_round`) — 循环调用 `tick`，直到产生第一条有效的 [`RunUpdates`]
//!    或战斗结束。
//!
//! ## 关系图
//!
//! ```text
//! Runner::main_round()
//!   └── EngineCore::tick()
//!         ├── sync_runtime_entities()   ← 实体同步
//!         ├── tick::next_actor()         ← 轮次推进
//!         ├── hooks::run_pre_action()    ← 前置钩子
//!         ├── tick::choose_action()      ← 行动决策
//!         ├── tick::select_targets()     ← 目标选取
//!         ├── tick::resolve_combat()     ← 战斗解算（含 Player::step()）
//!         ├── tick::run_update_end()     ← 持续效果结算
//!         ├── sync_runtime_entities()   ← 二次同步
//!         ├── tick::check_winner()       ← 胜负检查（若已决出胜负则本 tick 结束）
//!         ├── hooks::run_post_action()   ← 后置钩子
//!         └── tick::check_winner()       ← 兜底胜负检查
//! ```

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::engine::{hooks::HookPipeline, rules::RuleRegistry, world_state::WorldState};
use crate::player::PlrId;
use crate::rc4::RC4;

/// 战斗引擎核心，持有 [`HookPipeline`] 和 [`RuleRegistry`]，
/// 是调用 `tick` / `main_round` 的入口。
#[derive(Default)]
pub struct EngineCore {
    pub hooks: HookPipeline,
    pub rules: RuleRegistry,
}

impl EngineCore {
    pub fn register_pre_action_hook(&mut self, hook: crate::engine::hooks::ActorHook) { self.hooks.register_pre_action(hook); }

    pub fn register_pre_action_hook_dyn<H: crate::engine::hooks::ActorHookDyn + 'static>(&mut self, hook: H) {
        self.hooks.register_pre_action_dyn(hook);
    }

    pub fn register_post_action_hook(&mut self, hook: crate::engine::hooks::ActorHook) { self.hooks.register_post_action(hook); }

    pub fn register_post_action_hook_dyn<H: crate::engine::hooks::ActorHookDyn + 'static>(&mut self, hook: H) {
        self.hooks.register_post_action_dyn(hook);
    }

    pub fn register_pre_damage_hook(&mut self, hook: crate::engine::hooks::ActorHook) { self.hooks.register_pre_damage(hook); }

    pub fn register_pre_damage_hook_dyn<H: crate::engine::hooks::ActorHookDyn + 'static>(&mut self, hook: H) {
        self.hooks.register_pre_damage_dyn(hook);
    }

    pub fn register_post_damage_hook(&mut self, hook: crate::engine::hooks::ActorHook) { self.hooks.register_post_damage(hook); }

    pub fn register_post_damage_hook_dyn<H: crate::engine::hooks::ActorHookDyn + 'static>(&mut self, hook: H) {
        self.hooks.register_post_damage_dyn(hook);
    }

    #[cfg(not(feature = "no_debug"))]
    pub fn debug_world_state(tag: &str, world: &WorldState, storage: &Arc<Storage>) {
        if std::env::var_os("TSWN_DEBUG_WORLD").is_none() {
            return;
        }
        let players = world
            .players
            .iter()
            .map(|id| {
                storage
                    .get_player(id)
                    .map(|p| format!("{}:{}", id, p.id_name()))
                    .unwrap_or_else(|| format!("#{id}"))
            })
            .collect::<Vec<String>>()
            .join(" -> ");
        eprintln!("[world:{}] round_pos={} players=[{}]", tag, world.round_pos, players);
    }

    pub fn sync_runtime_entities(&self, world: &mut WorldState, storage: &Arc<Storage>) {
        if !storage.needs_sync() {
            return;
        }

        #[cfg(not(feature = "no_debug"))]
        Self::debug_world_state("pre_sync", world, storage);

        // 复活需要先于 spawn / death 落地：
        // - JS 中 revive 会立刻把旧实体重新挂回当前 roster / alive 视图；
        // - 如果同一 tick 里稍后又发生了 spawn（例如尸体转亡灵），spawn 应该排在 revive 之后；
        // - 如果同一 tick 里稍后又发生死亡移除，也必须保持“先 revive / spawn，再 remove”。
        //
        // 这里先处理显式 queue_revival，再处理 fallback 扫描出的已复活成员，最后才处理 pending spawn。
        let revivals = storage.take_pending_revivals();
        for id in revivals {
            if !world.contains_alive(id) && storage.get_player(&id).map(|p| p.alive()).unwrap_or(false) {
                world.revive_player(id, id);
                #[cfg(not(feature = "no_debug"))]
                Self::debug_world_state("after_revive_sync", world, storage);
            }
        }
        let mut revived_ids: smallvec::SmallVec<[PlrId; 4]> = smallvec::SmallVec::new();
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
            #[cfg(not(feature = "no_debug"))]
            Self::debug_world_state("after_revive_sync", world, storage);
        }

        let pending_spawns = storage.take_pending_spawns();
        // JS `addNew` 会在当前行动尚未结束前把新召唤物挂进 roster/alive。
        // 只要仍然保证 spawn 先于同一批 death/remove 落地，owner 稍后死亡时的 round_pos 语义就不会变。
        for pending in pending_spawns {
            let owner = pending.owner;
            let plr_id = storage.just_insert_player(pending.player);
            world.add_new_player(plr_id, owner);
        }

        let pending_remove_players = storage.take_pending_remove_players();
        let death_queue = storage.take_death_queue();
        for id in &death_queue {
            if world.contains_alive(*id) && !storage.get_player(id).map(|p| p.alive()).unwrap_or(false) {
                world.remove_player(*id);
                #[cfg(not(feature = "no_debug"))]
                Self::debug_world_state("after_dead_remove", world, storage);
            }
        }

        for ptr in &pending_remove_players {
            if !death_queue.contains(ptr) && !storage.get_player(ptr).map(|p| p.alive()).unwrap_or(false) {
                world.remove_player(*ptr);
                #[cfg(not(feature = "no_debug"))]
                Self::debug_world_state("after_pending_remove_only", world, storage);
            }
        }

        let remaining_dead: smallvec::SmallVec<[PlrId; 4]> = world
            .teams
            .iter()
            .flat_map(|team| team.alive.iter().copied())
            .filter(|id| !storage.get_player(id).map(|p| p.alive()).unwrap_or(false))
            .collect();
        for id in remaining_dead {
            world.remove_player(id);
            #[cfg(not(feature = "no_debug"))]
            Self::debug_world_state("after_dead_remove_fallback", world, storage);
        }
        storage.sync_groups(&world.groups);
        storage.sync_alive_groups_owned_with_count(world.alives_by_group(storage), world.alive_group_count());
        storage.clear_sync_flag();
        #[cfg(not(feature = "no_debug"))]
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

        #[cfg(not(feature = "no_debug"))]
        let debug_tick = std::env::var_os("TSWN_DEBUG_TICK").is_some();
        #[cfg(not(feature = "no_debug"))]
        let rc4_before = if debug_tick { (randomer.i, randomer.j) } else { (0, 0) };
        #[cfg(not(feature = "no_debug"))]
        if debug_tick && let Some(plr) = storage.get_player(&actor) {
            eprintln!(
                "[tick] actor={} id={} mv={} hp={} rc4=({}, {})",
                plr.id_name(),
                actor,
                plr.move_point(),
                plr.get_status().hp,
                randomer.i,
                randomer.j
            );
        }

        self.hooks.run_pre_action(actor, storage, randomer, updates);
        let decision = crate::engine::tick::choose_action(actor, world, storage, randomer, &self.rules);
        let preselected_targets = self
            .hooks
            .has_pre_damage_hooks()
            .then(|| crate::engine::tick::select_targets(actor, world, storage));
        let mut ctx = crate::engine::tick::TickContext {
            storage,
            randomer,
            updates,
        };
        crate::engine::tick::resolve_combat(actor, decision, preselected_targets.as_ref(), world, &mut ctx, &self.hooks);
        crate::engine::tick::run_update_end(storage, ctx.randomer, ctx.updates);
        #[cfg(not(feature = "no_debug"))]
        if debug_tick
            && (ctx.randomer.i != rc4_before.0 || ctx.randomer.j != rc4_before.1)
            && let Some(plr) = storage.get_player(&actor)
        {
            let bytes = (ctx.randomer.i as i32 - rc4_before.0 as i32).rem_euclid(256);
            eprintln!(
                "[tick_end] actor={} id={} mp_after={} hp_after={} rc4=({},{})->({},{}) bytes={}",
                plr.id_name(),
                actor,
                plr.move_point(),
                plr.get_status().hp,
                rc4_before.0,
                rc4_before.1,
                ctx.randomer.i,
                ctx.randomer.j,
                bytes
            );
        }
        self.sync_runtime_entities(world, storage);
        // 对齐 JS：当本次行动在执行过程中已经决出胜负时，会中断当前流程，
        // 不再继续执行当前 actor 的 post_action 链路（例如避免额外的“从铁壁中解除”尾日志）。
        crate::engine::tick::check_winner(world, storage);
        if world.have_winner() {
            return;
        }
        self.hooks.run_post_action(actor, storage, ctx.randomer, ctx.updates);
        crate::engine::tick::check_winner(world, storage);
    }

    pub fn main_round(&mut self, world: &mut WorldState, storage: &Arc<Storage>, randomer: &mut RC4) -> RunUpdates {
        let mut updates = RunUpdates::new();
        self.main_round_into(world, storage, randomer, &mut updates);
        updates
    }

    /// 与 `main_round` 逻辑相同，但复用调用方传入的 `RunUpdates`。
    /// 调用前需确保已调用 `updates.reset()`。
    pub fn main_round_into(
        &mut self,
        world: &mut WorldState,
        storage: &Arc<Storage>,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
    ) {
        let max_ticks = world.all_plr_len().max(1) * 4;
        let mut ticks = 0;

        while ticks < max_ticks && !world.have_winner() && !crate::engine::tick::has_updates(updates) {
            self.tick(world, storage, randomer, updates);
            ticks += 1;
        }
    }
}

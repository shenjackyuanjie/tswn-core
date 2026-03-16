//! # Boss 系统 (boss)
//!
//! 本模块定义 Boss 专用初始化及专属行为。
//!
//! ## Boss 类型
//!
//! | Boss 名称 | 说明                          |
//! |-----------|-------------------------------|
//! | `Covid`   | 新冠病毒 Boss（感染传播、变异机制）|
//! | `Lazy`    | 懒癌 Boss                     |
//! | `Saitama` | 一拳超人 Boss                 |
//! | `Generic` | 通用 Boss（无特殊行为）       |
//!
//! ## Boss 状态
//!
//! 每个 Boss 都有专属的状态结构：
//! - [`CovidBossState`] — 新冠病毒 Boss 状态
//! - [`LazyBossState`] — 懒癌 Boss 状态
//! - [`SaitamaState`] — 一拳超人 Boss 状态
//!
//! ## Boss 行为
//!
//! - **初始化** — `init_boss_state()` 根据 Boss 名称初始化状态
//! - **行动概率** — `boss_action_prob_count()` 返回 Boss 行动概率计数
//! - **免疫阈值** — `boss_immune_threshold()` 返回 Boss 对特定技能的免疫阈值
//! - **默认行动** — `boss_default_action()` 执行 Boss 的默认行动
//!
//! ## Boss 免疫
//!
//! 不同 Boss 对不同技能有不同的免疫阈值：
//! - **一拳超人** — 对 `half`、`exchange` 免疫阈值 240，对 `berserk`、`slow`、`ice` 免疫阈值 192
//! - **新冠病毒** — 对 `charm`、`berserk`、`exchange` 免疫阈值 192
//! - **懒癌** — 对 `assassinate`、`half`、`curse`、`exchange` 免疫阈值 192
//! - **通用 Boss** — 对 `assassinate`、`charm`、`berserk`、`half`、`curse`、`exchange`、`slow`、`ice` 免疫阈值 192
//!
//! ## 子模块
//!
//! - **`covid`** — 新冠病毒 Boss 实现
//! - **`lazy`** — 懒癌 Boss 实现
//! - **`saitama`** — 一拳超人 Boss 实现
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::boss::{boss_kind, init_boss_state};
//!
//! let kind = boss_kind("covid");
//! init_boss_state(&mut player);
//! ```

use std::sync::{Arc, OnceLock, RwLock};

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{ActionTargets, Player};
use crate::rc4::RC4;
use foldhash::HashMap as FastHashMap;

mod covid;
mod lazy;
mod saitama;

pub use covid::{CovidBossState, CovidEntry, CovidInfection};
pub use lazy::{LazyBossState, LazyInfection};
pub use saitama::SaitamaState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BossKind {
    Covid,
    Lazy,
    Saitama,
    Generic,
}

type BossInitFn = fn(&mut Player);
type BossActionProbFn = fn() -> usize;
type BossImmuneFn = fn(&str) -> i32;
type BossActionFn = fn(&mut Player, bool, &mut RC4, &mut RunUpdates, &Arc<Storage>, &ActionTargets);

#[derive(Clone, Copy)]
pub struct BossHandler {
    pub init_state: BossInitFn,
    pub action_prob_count: BossActionProbFn,
    pub immune_threshold: BossImmuneFn,
    pub default_action: BossActionFn,
}

impl BossHandler {
    pub const fn new(
        init_state: BossInitFn,
        action_prob_count: BossActionProbFn,
        immune_threshold: BossImmuneFn,
        default_action: BossActionFn,
    ) -> Self {
        Self {
            init_state,
            action_prob_count,
            immune_threshold,
            default_action,
        }
    }
}

struct BossRegistry {
    handlers: FastHashMap<&'static str, BossHandler>,
    fallback: BossHandler,
}

impl BossRegistry {
    fn with_builtins() -> Self {
        let fallback = BossHandler::new(init_generic_state, generic_action_prob_count, generic_immune_threshold, generic_boss_action);
        let mut handlers = FastHashMap::default();
        handlers.insert(
            "covid",
            BossHandler::new(init_covid_state, covid_action_prob_count, covid_immune_threshold, covid::covid_boss_action),
        );
        handlers.insert(
            "lazy",
            BossHandler::new(init_lazy_state, lazy_action_prob_count, lazy_immune_threshold, lazy::lazy_boss_action),
        );
        handlers.insert(
            "saitama",
            BossHandler::new(
                init_saitama_state,
                saitama_action_prob_count,
                saitama_immune_threshold,
                saitama::saitama_boss_action,
            ),
        );
        Self {
            handlers,
            fallback,
        }
    }

    #[inline]
    fn handler_for(&self, name: &str) -> BossHandler { self.handlers.get(name).copied().unwrap_or(self.fallback) }

    #[inline]
    fn register(&mut self, name: &'static str, handler: BossHandler) -> Option<BossHandler> { self.handlers.insert(name, handler) }
}

fn global_boss_registry() -> &'static RwLock<BossRegistry> {
    static REGISTRY: OnceLock<RwLock<BossRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| RwLock::new(BossRegistry::with_builtins()))
}

/// 注册（或覆盖）一个 Boss 行为处理器。
/// 默认内置 `covid/lazy/saitama`，其余名称会回退到通用 Boss 逻辑。
pub fn register_boss_handler(name: &'static str, handler: BossHandler) -> Option<BossHandler> {
    let mut registry = global_boss_registry().write().expect("boss registry poisoned");
    registry.register(name, handler)
}

pub fn boss_kind(name: &str) -> BossKind {
    match name {
        "covid" => BossKind::Covid,
        "lazy" => BossKind::Lazy,
        "saitama" => BossKind::Saitama,
        _ => BossKind::Generic,
    }
}

pub fn init_boss_state(player: &mut Player) {
    let name = player.id_name();
    let registry = global_boss_registry().read().expect("boss registry poisoned");
    let handler = registry.handler_for(name.as_str());
    (handler.init_state)(player);
}

pub fn boss_action_prob_count(name: &str) -> usize {
    let registry = global_boss_registry().read().expect("boss registry poisoned");
    let handler = registry.handler_for(name);
    (handler.action_prob_count)()
}

pub fn boss_immune_threshold(boss_name: &str, key: &str) -> i32 {
    let registry = global_boss_registry().read().expect("boss registry poisoned");
    let handler = registry.handler_for(boss_name);
    (handler.immune_threshold)(key)
}

pub fn boss_default_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let name = player.id_name();
    let registry = global_boss_registry().read().expect("boss registry poisoned");
    let handler = registry.handler_for(name.as_str());
    (handler.default_action)(player, smart, randomer, updates, storage, targets);
}

#[inline]
fn init_covid_state(player: &mut Player) { player.set_state(CovidBossState { mutation: 40 }); }

#[inline]
fn init_lazy_state(player: &mut Player) { player.set_state(LazyBossState { at_boost: 1.0 }); }

#[inline]
fn init_saitama_state(player: &mut Player) {
    player.set_state(SaitamaState {
        turns: 0,
        damages: 0,
        hitters: std::collections::HashSet::new(),
        minions: std::collections::HashSet::new(),
    });
}

#[inline]
fn init_generic_state(_player: &mut Player) {}

#[inline]
fn covid_action_prob_count() -> usize { 0 }

#[inline]
fn lazy_action_prob_count() -> usize { 0 }

#[inline]
fn saitama_action_prob_count() -> usize { 1 }

#[inline]
fn generic_action_prob_count() -> usize { 1 }

#[inline]
fn saitama_immune_threshold(key: &str) -> i32 {
    match key {
        "half" | "exchange" => 240,
        "berserk" | "slow" | "ice" => 192,
        _ => 84,
    }
}

#[inline]
fn covid_immune_threshold(key: &str) -> i32 {
    match key {
        "charm" | "berserk" | "exchange" => 192,
        _ => 84,
    }
}

#[inline]
fn lazy_immune_threshold(key: &str) -> i32 {
    match key {
        "assassinate" | "half" | "curse" | "exchange" => 192,
        _ => 84,
    }
}

#[inline]
fn generic_immune_threshold(key: &str) -> i32 {
    match key {
        "assassinate" | "charm" | "berserk" | "half" | "curse" | "exchange" | "slow" | "ice" => 192,
        _ => 84,
    }
}

fn generic_boss_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };
    let atp = player.get_at(false, randomer);
    updates.add(RunUpdate::new("[0]发起攻击", player.as_ptr(), target_id, 0));
    storage.just_get_player_mut(target_id).expect("generic_boss_action target").attacked(
        atp,
        false,
        player.as_ptr(),
        crate::player::noop_on_damage,
        randomer,
        updates,
        storage,
    );
}

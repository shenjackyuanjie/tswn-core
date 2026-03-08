use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{ActionTargets, Player};
use crate::rc4::RC4;

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
    match boss_kind(&name) {
        BossKind::Covid => {
            player.set_state(CovidBossState { mutation: 40 });
        }
        BossKind::Lazy => {
            player.set_state(LazyBossState { at_boost: 1.0 });
        }
        BossKind::Saitama => {
            player.set_state(SaitamaState {
                turns: 0,
                damages: 0,
                hitters: std::collections::HashSet::new(),
                minions: std::collections::HashSet::new(),
            });
        }
        BossKind::Generic => {}
    }
}

pub fn boss_action_prob_count(name: &str) -> usize {
    match boss_kind(name) {
        BossKind::Covid | BossKind::Lazy => 0,
        BossKind::Saitama | BossKind::Generic => 1,
    }
}

pub fn boss_immune_threshold(boss_name: &str, key: &str) -> i32 {
    match boss_kind(boss_name) {
        BossKind::Saitama => match key {
            "half" | "exchange" => 240,
            "berserk" | "slow" | "ice" => 192,
            _ => 84,
        },
        BossKind::Covid => match key {
            "charm" | "berserk" | "exchange" => 192,
            _ => 84,
        },
        BossKind::Lazy => match key {
            "assassinate" | "half" | "curse" | "exchange" => 192,
            _ => 84,
        },
        BossKind::Generic => match key {
            "assassinate" | "charm" | "berserk" | "half" | "curse" | "exchange" | "slow" | "ice" => 192,
            _ => 84,
        },
    }
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
    match boss_kind(&name) {
        BossKind::Covid => covid::covid_boss_action(player, smart, randomer, updates, storage, targets),
        BossKind::Lazy => lazy::lazy_boss_action(player, smart, randomer, updates, storage, targets),
        BossKind::Saitama => saitama::saitama_boss_action(player, smart, randomer, updates, storage, targets),
        BossKind::Generic => generic_boss_action(player, smart, randomer, updates, storage, targets),
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

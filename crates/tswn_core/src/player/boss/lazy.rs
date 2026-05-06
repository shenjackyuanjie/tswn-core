use std::cell::Cell;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{ActionTargets, Player, PlrId, StateTrait, noop_on_damage};
use crate::rc4::RC4;

thread_local! {
    static LAZY_ON_DAMAGE_CTX: Cell<Option<PlrId>> = const { Cell::new(None) };
}

#[derive(Clone, Debug)]
pub struct LazyBossState {
    pub at_boost: f64,
}

impl StateTrait for LazyBossState {
    fn meta_type(&self) -> i32 { 0 }
    fn post_damage_priority(&self) -> i32 { 1000 }

    fn on_post_damage(
        &mut self,
        owner: PlrId,
        _dmg: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) {
        lazy_infect(owner, caster, randomer, updates, storage);
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

#[derive(Clone, Debug)]
pub struct LazyInfection {
    pub boss_id: PlrId,
}

impl StateTrait for LazyInfection {
    fn meta_type(&self) -> i32 { -1 }

    fn update_state_priority(&self) -> i32 { 1000 }
    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) { status.speed /= 2; }

    fn pre_action_priority(&self) -> i32 { 1000 }
    fn on_pre_action(
        &mut self,
        owner: PlrId,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        _targets: &ActionTargets,
    ) -> bool {
        let roll = randomer.next_u8();
        if roll < 128 {
            lazy_select_consume_bytes(self.boss_id, smart, randomer, storage);
            be_lazy(owner, randomer, updates, storage);
            return true;
        }
        false
    }

    fn post_action_priority(&self) -> i32 { 1000 }
    fn on_post_action(
        &mut self,
        owner: PlrId,
        _alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        if !storage.get_player(&self.boss_id).map(|p| p.alive()).unwrap_or(false) {
            return false;
        }
        lazy_post_action_damage(owner, self.boss_id, randomer, updates, storage);
        false
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

pub fn lazy_boss_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let boss_id = player.as_ptr();

    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };

    let target_infected = storage.get_player(&target_id).map(|p| p.has_state::<LazyInfection>()).unwrap_or(false);
    if target_infected && randomer.next_u8() < 128 {
        be_lazy(boss_id, randomer, updates, storage);
        if let Some(boss_state) = player.get_state_mut::<LazyBossState>() {
            boss_state.at_boost += 0.5;
        }
        return;
    }

    let at_boost = player.get_state::<LazyBossState>().map(|s| s.at_boost).unwrap_or(1.0);
    let atp = player.get_at(false, randomer) * at_boost;
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));

    LAZY_ON_DAMAGE_CTX.set(Some(boss_id));
    let actual_dmg = {
        let core = {
            let target = storage.just_get_player_mut(target_id).expect("lazy_boss_action target");
            target.attacked_core(atp, false, boss_id, lazy_attack_on_damage, randomer, updates, storage)
        };
        if core.hit {
            lazy_attack_on_damage(boss_id, core.target, core.dmg, randomer, updates, storage);
            let target = storage.just_get_player_mut(core.target).expect("lazy_boss_action target");
            target.finish_damage(core.dmg, core.old_hp, boss_id, randomer, updates, storage)
        } else {
            0
        }
    };
    LAZY_ON_DAMAGE_CTX.set(None);

    if actual_dmg > 0
        && let Some(boss_state) = player.get_state_mut::<LazyBossState>()
    {
        boss_state.at_boost = 1.0;
    }
}

fn lazy_attack_on_damage(
    _caster: PlrId,
    target: PlrId,
    _dmg: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let Some(boss_id) = LAZY_ON_DAMAGE_CTX.get() else {
        return;
    };
    lazy_infect(boss_id, target, randomer, updates, storage);
}

fn lazy_select_consume_bytes(boss_id: PlrId, smart: bool, randomer: &mut RC4, storage: &Arc<Storage>) {
    let n = if smart { 3usize } else { 2usize };
    let all_alive = storage.all_alive_ids();
    let boss_group = storage.alive_group_at_team_of(boss_id).cloned().unwrap_or_default();

    let skip_indices: Vec<usize> = boss_group.iter().filter_map(|id| all_alive.iter().position(|a| a == id)).collect();

    let mut selected = Vec::new();
    let mut dup = 0usize;
    let n_i32 = n as i32;
    let invalid = -n_i32;

    while (dup as i32) <= n_i32 && invalid <= n_i32 {
        let picked = if skip_indices.is_empty() {
            randomer.pick(&all_alive)
        } else if all_alive.len() > skip_indices.len() {
            randomer.pick_skip_range(&all_alive, &skip_indices)
        } else {
            None
        };
        let Some(picked_idx) = picked else {
            break;
        };
        if !selected.contains(&picked_idx) {
            selected.push(picked_idx);
            if selected.len() >= n {
                break;
            }
        } else {
            dup += 1;
        }
    }

    for _ in &selected {
        if !smart {
            let _ = randomer.rFFFF();
        }
    }
}

fn lazy_infect(boss_id: PlrId, target: PlrId, _randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if target_plr.as_ptr() == boss_id {
        return;
    }
    if target_plr.has_state::<LazyInfection>() {
        return;
    }

    let boss_display = storage.get_player(&boss_id).map(|p| p.display_name()).unwrap_or_default();

    target_plr.set_state(LazyInfection { boss_id });
    updates.add(RunUpdate::new(format!("[1]感染了{boss_display}"), boss_id, target, 0));
}

fn be_lazy(owner: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    let roll = randomer.next_u8();
    let activity = if roll < 50 {
        "Steam"
    } else if roll < 100 {
        "守望先锋"
    } else if roll < 150 {
        "文明6"
    } else if roll < 190 {
        "英雄联盟"
    } else if roll < 230 {
        "微博"
    } else {
        "朋友圈"
    };
    let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
    updates.add(RunUpdate::new(
        format!("{owner_name}打开了{activity}, 这回合什么也没做"),
        owner,
        owner,
        0,
    ));
}

fn lazy_post_action_damage(owner: PlrId, boss_id: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    let Some(owner_plr) = storage.get_player(&owner) else {
        return;
    };
    if !owner_plr.alive() {
        return;
    }

    let boss_at = {
        let boss_plr = storage.get_player(&boss_id).expect("lazy_post_action boss");
        boss_plr.get_at(true, randomer)
    };
    let target_df = {
        let plr = storage.get_player(&owner).unwrap();
        plr.get_df(true)
    };
    let dmg = (boss_at / target_df as f64).ceil() as i32;
    if dmg <= 0 {
        return;
    }

    let boss_display = storage.get_player(&boss_id).map(|p| p.display_name()).unwrap_or_default();
    let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();

    updates.add(RunUpdate::new(format!(" {owner_name}{boss_display}发作"), boss_id, owner, 0));

    let owner_plr = storage.just_get_player_mut(owner).expect("lazy_post_action owner");
    owner_plr.damage(dmg, boss_id, noop_on_damage, randomer, updates, storage);
}

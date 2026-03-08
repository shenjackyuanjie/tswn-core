use std::cell::Cell;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    ActionTargets, Player, PlayerStatus, PlrId, StateTrait,
    noop_on_damage,
};
use crate::rc4::RC4;

// ─── Thread-local context for COVID on_damage callback ────────────────────
// OnDamageFunc is a bare fn pointer that cannot capture state.
// We use thread-local storage to pass (boss_id, mutation) into the callback.

thread_local! {
    static COVID_ON_DAMAGE_CTX: Cell<Option<(PlrId, i32)>> = const { Cell::new(None) };
}

fn covid_spread_on_damage(
    _caster: PlrId,
    target: PlrId,
    dmg: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let Some((boss_id, mutation)) = COVID_ON_DAMAGE_CTX.get() else {
        return;
    };
    // JS tB: if target not already infected AND (rc4.n() & 63) + 1 < dmg → infect
    let already_infected = storage
        .get_player(&target)
        .map(|p| p.has_state::<CovidInfection>())
        .unwrap_or(false);
    if already_infected {
        return;
    }
    let roll = (randomer.next_u8() & 63) as i32 + 1;
    if roll < dmg {
        covid_infect(boss_id, target, mutation, randomer, updates, storage);
    }
}

// ─── Thread-local context for Lazy on_damage callback ─────────────────────

thread_local! {
    static LAZY_ON_DAMAGE_CTX: Cell<Option<PlrId>> = const { Cell::new(None) };
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

// ─── Boss kind ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BossKind {
    Covid,
    Lazy,
    Saitama,
    /// Bosses that only need default attack (mario, sonic, etc.)
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

// ─── init_boss_state: called after build() ────────────────────────────────

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
                hitters: 0,
                minions: 0,
            });
        }
        BossKind::Generic => {}
    }
}

// ─── boss_default_action: called from default_attack for Boss players ─────

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
        BossKind::Covid => covid_boss_action(player, smart, randomer, updates, storage, targets),
        BossKind::Lazy => lazy_boss_action(player, smart, randomer, updates, storage, targets),
        BossKind::Saitama => saitama_boss_action(player, smart, randomer, updates, storage, targets),
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
    storage
        .just_get_player_mut(target_id)
        .expect("generic_boss_action target")
        .attacked(atp, false, player.as_ptr(), noop_on_damage, randomer, updates, storage);
}

// ═══════════════════════════════════════════════════════════════════════════
//  COVID
// ═══════════════════════════════════════════════════════════════════════════

/// Boss-side state: SklCovidDefend — infects attackers (PostDamage on boss).
#[derive(Clone, Debug)]
pub struct CovidBossState {
    pub mutation: i32,
}

impl StateTrait for CovidBossState {
    fn meta_type(&self) -> i32 { 0 }
    fn post_damage_priority(&self) -> i32 { 1000 }

    /// SklCovidDefend.aD (PostDamage): infect whoever attacked boss.
    fn on_post_damage(
        &mut self,
        _owner: PlrId,
        _dmg: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) {
        let boss_id = _owner;
        covid_infect(boss_id, caster, self.mutation, randomer, updates, storage);
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

/// Infection state on a non-boss player.
#[derive(Clone, Debug)]
pub struct CovidInfection {
    pub boss_id: PlrId,
    pub mutation: i32,
    /// JS 'days' — incremented in act (v()), checked in postAction (at())
    pub days: i32,
}

impl StateTrait for CovidInfection {
    fn meta_type(&self) -> i32 { -1 }

    // ── PreAction hook: ALWAYS hijacks the player's action ──
    // This combines JS preAction (aN) + act (v)
    fn pre_action_priority(&self) -> i32 { 1000 }
    fn on_pre_action(
        &mut self,
        owner: PlrId,
        _smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> bool {
        // === preAction (aN): mutation check ===
        let pre_byte = randomer.next_u8();
        if pre_byte < 64 {
            self.mutation = randomer.r127() as i32;
        }

        // === act (v): spreading / ICU / home isolation ===
        let owner_wisdom = storage
            .get_player(&owner)
            .map(|p| p.get_status().wisdom)
            .unwrap_or(0);
        let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();

        let condition = self.days == 0 || (randomer.next_u8() as i32) > owner_wisdom;
        eprintln!("[COVID_ACT] owner={} days={} condition={} pre_byte={} rc4=({},{})", 
            owner_name, self.days, condition, pre_byte, randomer.i, randomer.j);
        if condition {
            self.days += (randomer.next_u8() & 3) as i32; // 1st increment (r3)
            // Try spreading: 5 attempts
            let all_alive = targets.all_alive.clone();
            eprintln!("[COVID_SPREAD] all_alive={:?} rc4=({},{})", 
                all_alive.iter().map(|id| storage.get_player(id).map(|p| p.display_name()).unwrap_or_default()).collect::<Vec<_>>(),
                randomer.i, randomer.j);
            for attempt in 0..5 {
                if all_alive.is_empty() {
                    break;
                }
                let Some(pick_idx) = randomer.pick(&all_alive) else {
                    break;
                };
                let candidate = all_alive[pick_idx];
                let cand_name = storage.get_player(&candidate).map(|p| p.display_name()).unwrap_or_default();
                eprintln!("[COVID_PICK] attempt={} pick_idx={} candidate={} is_owner={} is_boss={}", 
                    attempt, pick_idx, cand_name, candidate == owner, candidate == self.boss_id);
                if candidate == owner || candidate == self.boss_id {
                    continue;
                }
                let candidate_alive = storage
                    .get_player(&candidate)
                    .map(|p| p.alive())
                    .unwrap_or(false);
                if !candidate_alive {
                    continue;
                }

                // Check if candidate already has this mutation (immune)
                let already_has_mutation = storage
                    .get_player(&candidate)
                    .and_then(|p| p.get_state::<CovidInfection>())
                    .map(|inf| inf.mutation == self.mutation)
                    .unwrap_or(false);
                if already_has_mutation {
                    continue;
                }

                // Determine if same team as owner
                let owner_group = storage.group_containing(owner);
                let candidate_in_owner_group = owner_group
                    .map(|g| g.contains(&candidate))
                    .unwrap_or(false);

                if candidate_in_owner_group {
                    covid_contact_spread(owner, candidate, self.boss_id, self.mutation, randomer, updates, storage);
                } else {
                    covid_attack_spread(owner, candidate, self.boss_id, self.mutation, randomer, updates, storage);
                }
                return true; // spread succeeded → early return, skip ICU/home
            }
            // Fall through: no spread target found
        }

        // 2nd increment (always reaches here if condition was false OR no spread target)
        self.days += (randomer.next_u8() & 3) as i32;

        if self.days > 2 {
            updates.add(RunUpdate::new(
                "[1]在重症监护室无法行动",
                self.boss_id,
                owner,
                0,
            ));
        } else {
            updates.add(RunUpdate::new(
                "[1]在家中自我隔离",
                self.boss_id,
                owner,
                0,
            ));
        }

        true // always hijack
    }

    // ── PostAction hook: pneumonia + auto-cure ──
    fn post_action_priority(&self) -> i32 { 1000 }
    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        // Pneumonia: only if alive and days > 1
        if alive && self.days > 1 {
            covid_pneumonia(owner, self.boss_id, self.mutation, randomer, updates, storage);
        }
        // Auto-cure: days > 6
        if self.days > 6 {
            // Remove infection state
            if let Some(plr) = storage.just_get_player_mut(owner) {
                plr.update_states();
            }
            return true; // request removal
        }
        false
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

/// COVID boss 自己的行动: 正常普攻 + on_damage infect
fn covid_boss_action(
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
    let boss_id = player.as_ptr();
    let mutation = player
        .get_state::<CovidBossState>()
        .map(|s| s.mutation)
        .unwrap_or(40);

    let atp = player.get_at(false, randomer);
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));

    // Set thread-local context for on_damage callback
    COVID_ON_DAMAGE_CTX.set(Some((boss_id, mutation)));
    storage
        .just_get_player_mut(target_id)
        .expect("covid_boss_action target")
        .attacked(atp, false, boss_id, covid_spread_on_damage, randomer, updates, storage);
    COVID_ON_DAMAGE_CTX.set(None);
}

/// j7: infect a player with COVID
fn covid_infect(
    boss_id: PlrId,
    target: PlrId,
    mutation: i32,
    _randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if target_plr.as_ptr() == boss_id {
        return;
    }
    if target_plr.has_state::<CovidInfection>() {
        return;
    }

    let _target_name = target_plr.display_name();
    let boss_display = storage
        .get_player(&boss_id)
        .map(|p| p.display_name())
        .unwrap_or_default();

    target_plr.set_state(CovidInfection {
        boss_id,
        mutation,
        days: 0,
    });
    updates.add(RunUpdate::new(
        format!("[1]感染了{boss_display}"),
        boss_id,
        target,
        0,
    ));

    // spsum adjustments: iterate ALL alive players (JS: caster.group.f.alives)
    // infected target += 2048, everyone else -= 256
    let all_alive = storage.all_alive_ids();
    for &pid in &all_alive {
        if pid == target {
            if let Some(p) = storage.just_get_player_mut(pid) {
                p.add_move_point(2048);
            }
        } else {
            if let Some(p) = storage.just_get_player_mut(pid) {
                p.add_move_point(-256);
            }
        }
    }
}

/// fH: contact spread — resistance check
fn covid_contact_spread(
    owner: PlrId,
    candidate: PlrId,
    boss_id: PlrId,
    mutation: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
    let candidate_name = storage.get_player(&candidate).map(|p| p.display_name()).unwrap_or_default();

    updates.add(RunUpdate::new(
        format!("{owner_name}和{candidate_name}近距离接触"),
        owner,
        candidate,
        0,
    ));

    // Check if already infected
    let already_infected = storage
        .get_player(&candidate)
        .map(|p| p.has_state::<CovidInfection>())
        .unwrap_or(false);

    if already_infected {
        return;
    }

    // Resistance check: rc4.n() < (oq ? smart+192 : smart>>1)
    // oq = candidate has CovidInfection already (it's a re-infection check)
    // Since already_infected=false here, we use smart>>1
    let candidate_smart = storage
        .get_player(&candidate)
        .map(|p| p.get_status().wisdom)
        .unwrap_or(0);
    let threshold = candidate_smart >> 1;
    let roll = randomer.next_u8() as i32;
    if roll < threshold {
        // Resisted
        updates.add(RunUpdate::new(
            format!("但{candidate_name}没被感染"),
            owner,
            candidate,
            0,
        ));
        return;
    }
    covid_infect(boss_id, candidate, mutation, randomer, updates, storage);
}

/// Attack spread: boss attacks the candidate through the infected player
fn covid_attack_spread(
    owner: PlrId,
    candidate: PlrId,
    boss_id: PlrId,
    mutation: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    // JS: l = getAt(target/*=infected owner*/, false, rc4)
    let atp = {
        let owner_plr = storage.get_player(&owner).expect("covid_attack_spread owner");
        owner_plr.get_at(false, randomer)
    };

    let _owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
    let _candidate_name = storage.get_player(&candidate).map(|p| p.display_name()).unwrap_or_default();

    updates.add(RunUpdate::new("[0]发起攻击", owner, candidate, 0));

    // Set thread-local for on_damage callback
    COVID_ON_DAMAGE_CTX.set(Some((boss_id, mutation)));
    storage
        .just_get_player_mut(candidate)
        .expect("covid_attack_spread candidate")
        .attacked(atp, false, owner, covid_spread_on_damage, randomer, updates, storage);
    COVID_ON_DAMAGE_CTX.set(None);
}

/// at(): pneumonia damage
fn covid_pneumonia(
    owner: PlrId,
    boss_id: PlrId,
    mutation: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let Some(owner_plr) = storage.get_player(&owner) else {
        return;
    };
    if !owner_plr.alive() {
        return;
    }

    let owner_name = owner_plr.display_name();

    // JS: floor((getAt(target, true, rc4) + mutation*80) / getDf(target, true))
    let at_val = owner_plr.get_at(true, randomer);
    let df_val = owner_plr.get_df(true);
    let dmg = ((at_val + (mutation * 80) as f64) / df_val as f64).floor() as i32;

    if dmg <= 0 {
        return;
    }

    updates.add(RunUpdate::new(
        format!(" {owner_name}肺炎发作"),
        boss_id,
        owner,
        0,
    ));

    // Apply damage to target (raw, through `damage` for proper death handling)
    let _old_hp = {
        let plr = storage.get_player(&owner).unwrap();
        plr.get_status().hp
    };
    let actual_dmg = {
        let plr = storage.just_get_player_mut(owner).expect("covid_pneumonia owner");
        plr.damage(dmg, boss_id, noop_on_damage, randomer, updates, storage)
    };

    // Boss heals: min(dmg>>1, actualDmg) or min((dmg>>2)+1, actualDmg) if full HP
    let boss_hp_full = storage
        .get_player(&boss_id)
        .map(|p| {
            let s = p.get_status();
            s.hp >= s.max_hp
        })
        .unwrap_or(false);

    let heal_amount = if boss_hp_full {
        std::cmp::min((dmg >> 2) + 1, actual_dmg)
    } else {
        std::cmp::min(dmg >> 1, actual_dmg)
    };

    if heal_amount > 0 {
        if let Some(boss_plr) = storage.just_get_player_mut(boss_id) {
            let boss_display = boss_plr.display_name();
            boss_plr.heal(heal_amount);
            updates.add(RunUpdate::new(
                format!("{boss_display}回复体力{heal_amount}点"),
                boss_id,
                boss_id,
                0,
            ));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  LAZY
// ═══════════════════════════════════════════════════════════════════════════

/// Boss-side state: SklLazyDefend — infects attackers (PostDamage on boss).
#[derive(Clone, Debug)]
pub struct LazyBossState {
    pub at_boost: f64,
}

impl StateTrait for LazyBossState {
    fn meta_type(&self) -> i32 { 0 }
    fn post_damage_priority(&self) -> i32 { 1000 }

    /// SklLazyDefend.aD (PostDamage): infect whoever attacked boss.
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

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

/// Infection state on a non-boss player.
#[derive(Clone, Debug)]
pub struct LazyInfection {
    pub boss_id: PlrId,
}

impl StateTrait for LazyInfection {
    fn meta_type(&self) -> i32 { -1 }

    // ── UpdateState: speed /= 2 ──
    fn update_state_priority(&self) -> i32 { 1000 }
    fn apply_update_state(&self, status: &mut PlayerStatus) {
        status.speed /= 2;
    }

    // ── PreAction: 50% chance to skip turn ──
    fn pre_action_priority(&self) -> i32 { 1000 }
    fn on_pre_action(
        &mut self,
        owner: PlrId,
        _smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        _targets: &ActionTargets,
    ) -> bool {
        let roll = randomer.next_u8();
        if roll < 128 {
            // Skip turn: be lazy
            be_lazy(owner, randomer, updates, storage);
            // PostAction: lazy damage
            lazy_post_action_damage(owner, self.boss_id, randomer, updates, storage);
            return true;
        }
        false // don't hijack, let normal action proceed
    }

    // ── PostAction: lazy damage ──
    fn post_action_priority(&self) -> i32 { 1000 }
    fn on_post_action(
        &mut self,
        owner: PlrId,
        _alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        lazy_post_action_damage(owner, self.boss_id, randomer, updates, storage);
        false
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

/// Lazy boss action
fn lazy_boss_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let boss_id = player.as_ptr();

    // JS SklLazyAttack.v(): 50% chance + target infected → be lazy
    let lazy_roll = randomer.next_u8();
    if lazy_roll < 128 {
        // Check if intended target is infected
        let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
            return;
        };
        let target_infected = storage
            .get_player(&target_id)
            .map(|p| p.has_state::<LazyInfection>())
            .unwrap_or(false);
        if target_infected {
            // Boss is lazy this turn, atboost += 0.5
            if let Some(boss_state) = player.get_state_mut::<LazyBossState>() {
                boss_state.at_boost += 0.5;
            }
            be_lazy(boss_id, randomer, updates, storage);
            return;
        }
        // Target not infected, attack normally with atboost
        let at_boost = player.get_state::<LazyBossState>().map(|s| s.at_boost).unwrap_or(1.0);
        let atp = player.get_at(false, randomer) * at_boost;
        updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));

        // Set thread-local for on_damage callback
        LAZY_ON_DAMAGE_CTX.set(Some(boss_id));
        let actual_dmg = storage
            .just_get_player_mut(target_id)
            .expect("lazy_boss_action target")
            .attacked(atp, false, boss_id, lazy_attack_on_damage, randomer, updates, storage);
        LAZY_ON_DAMAGE_CTX.set(None);

        // Reset atboost on hit
        if actual_dmg > 0 {
            if let Some(boss_state) = player.get_state_mut::<LazyBossState>() {
                boss_state.at_boost = 1.0;
            }
        }
    } else {
        // Normal attack with atboost
        let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
            return;
        };
        let at_boost = player.get_state::<LazyBossState>().map(|s| s.at_boost).unwrap_or(1.0);
        let atp = player.get_at(false, randomer) * at_boost;
        updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));

        LAZY_ON_DAMAGE_CTX.set(Some(boss_id));
        let actual_dmg = storage
            .just_get_player_mut(target_id)
            .expect("lazy_boss_action target")
            .attacked(atp, false, boss_id, lazy_attack_on_damage, randomer, updates, storage);
        LAZY_ON_DAMAGE_CTX.set(None);

        if actual_dmg > 0 {
            if let Some(boss_state) = player.get_state_mut::<LazyBossState>() {
                boss_state.at_boost = 1.0;
            }
        }
    }
}

/// Infect target with lazy
fn lazy_infect(
    boss_id: PlrId,
    target: PlrId,
    _randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if target_plr.as_ptr() == boss_id {
        return;
    }
    if target_plr.has_state::<LazyInfection>() {
        return;
    }

    let boss_display = storage
        .get_player(&boss_id)
        .map(|p| p.display_name())
        .unwrap_or_default();

    target_plr.set_state(LazyInfection { boss_id });
    updates.add(RunUpdate::new(
        format!("[1]感染了{boss_display}"),
        boss_id,
        target,
        0,
    ));
}

/// be_lazy: display a lazy message
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

/// PostAction damage: ceil(getAt(boss, true, rc4) / getDf(target, true))
fn lazy_post_action_damage(
    owner: PlrId,
    boss_id: PlrId,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let Some(owner_plr) = storage.get_player(&owner) else {
        return;
    };
    if !owner_plr.alive() {
        return;
    }

    // JS: ceil(getAt(boss, true, rc4) / getDf(target, true))
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

    updates.add(RunUpdate::new(
        format!(" {owner_name}{boss_display}发作"),
        boss_id,
        owner,
        0,
    ));

    let owner_plr = storage.just_get_player_mut(owner).expect("lazy_post_action owner");
    owner_plr.damage(dmg, boss_id, noop_on_damage, randomer, updates, storage);
}

// ═══════════════════════════════════════════════════════════════════════════
//  SAITAMA
// ═══════════════════════════════════════════════════════════════════════════

/// Saitama state: PostDefend dmg/100, charge counter, hunger check
#[derive(Clone, Debug)]
pub struct SaitamaState {
    pub turns: i32,
    pub damages: i32,
    pub hitters: i32,
    pub minions: i32,
}

impl StateTrait for SaitamaState {
    fn meta_type(&self) -> i32 { 0 }

    // ── PostDefend: dmg / 100 ──
    fn post_defend_priority(&self) -> i32 { i32::MAX } // JS: priority = Infinity
    fn on_post_defend(
        &mut self,
        _owner: PlrId,
        dmg: &mut i32,
        _caster: PlrId,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
    ) {
        self.damages += *dmg;
        self.hitters += 1;
        *dmg /= 100;
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

fn saitama_boss_action(
    player: &mut Player,
    smart: bool,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
    targets: &ActionTargets,
) {
    let boss_id = player.as_ptr();

    // Hunger check: damages / (hitters + minions/3 + 1) > 255
    let (damages, hitters, minions) = player
        .get_state::<SaitamaState>()
        .map(|s| (s.damages, s.hitters, s.minions))
        .unwrap_or((0, 0, 0));

    let hunger_denominator = hitters + minions / 3 + 1;
    if damages / hunger_denominator.max(1) > 255 {
        // 觉得有点饿 → 离开了战场
        let boss_display = player.display_name();
        updates.add(RunUpdate::new(
            format!("{boss_display}觉得有点饿"),
            boss_id,
            boss_id,
            0,
        ));
        updates.add(RunUpdate::new_newline());
        updates.add(RunUpdate::new(
            format!(" {boss_display}离开了战场"),
            boss_id,
            boss_id,
            0,
        ));
        // Self-death
        let old_hp = player.get_status().hp;
        player.apply_raw_damage(old_hp);
        player.on_die(old_hp, boss_id, randomer, updates, storage);
        return;
    }

    // Turn counter
    let turns = player
        .get_state::<SaitamaState>()
        .map(|s| s.turns)
        .unwrap_or(0);

    if turns < 10 {
        // Increment and do nothing (no attack)
        if let Some(state) = player.get_state_mut::<SaitamaState>() {
            state.turns += 1;
        }
        return;
    }

    // turns >= 10: attack with getAt × 12
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };
    let atp = player.get_at(false, randomer) * 12.0;
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));
    storage
        .just_get_player_mut(target_id)
        .expect("saitama attack target")
        .attacked(atp, false, boss_id, noop_on_damage, randomer, updates, storage);

    // After attack: all allies spsum=0, self spsum=1700
    // For boss@!, boss is the only team member
    if let Some(boss_group) = storage.group_containing(boss_id) {
        let group = boss_group.clone();
        for &member_id in &group {
            if let Some(member) = storage.just_get_player_mut(member_id) {
                member.set_move_point(0);
            }
        }
    }
    player.set_move_point(1700);
}

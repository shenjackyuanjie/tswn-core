/// Boss-specific mechanics: COVID, Lazy, Saitama, etc.
///
/// Each boss overrides createSkills (ac) with custom attack/defense skills.
/// PlrBoss.bs() only adds k1 defense skills (instanceof ActionSkill) to k4,
/// so COVID/Lazy bosses have empty k4 (no normal skill prob consumption).

use std::any::Any;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates, UpdateType};
use crate::player::{PlrId, PlayerType, StateTrait, PlayerStatus, state_tag, OnDamageFunc};
use crate::rc4::RC4;

/// Boss 类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossKind {
    Covid,
    Lazy,
    Saitama,
    Mario,
    Sonic,
    Mosquito,
    Yuri,
    Slime,
    Ikaruga,
    Conan,
    Aokiji,
}

impl BossKind {
    pub fn from_name(name: &str) -> Option<BossKind> {
        match name {
            "covid" => Some(BossKind::Covid),
            "lazy" => Some(BossKind::Lazy),
            "saitama" => Some(BossKind::Saitama),
            "mario" => Some(BossKind::Mario),
            "sonic" => Some(BossKind::Sonic),
            "mosquito" => Some(BossKind::Mosquito),
            "yuri" => Some(BossKind::Yuri),
            "slime" => Some(BossKind::Slime),
            "ikaruga" => Some(BossKind::Ikaruga),
            "conan" => Some(BossKind::Conan),
            "aokiji" => Some(BossKind::Aokiji),
            _ => None,
        }
    }
}

// =====================================================================
// COVID Infection State
// =====================================================================

/// 新冠感染状态，附加在被感染的玩家身上。
/// JS: CovidState { fr(boss), fx(infected), fy(pandemic_stage), go(mutation) }
#[derive(Debug, Clone)]
pub struct CovidInfection {
    /// Boss 玩家 ID
    pub boss_id: PlrId,
    /// 当前疫情阶段 (pandemic_stage), 每轮增加 0-3
    pub pandemic_stage: i32,
    /// 变异值 (mutation), 影响肺炎伤害
    pub mutation: i32,
}

impl CovidInfection {
    pub fn new(boss_id: PlrId, mutation: i32) -> Self {
        CovidInfection {
            boss_id,
            pandemic_stage: 0,
            mutation,
        }
    }
}

impl StateTrait for CovidInfection {
    fn post_action_priority(&self) -> i32 { 500 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        if !alive {
            return false;
        }
        // v(): spreading phase
        covid_spreading(owner, self, randomer, updates, storage);

        // at(): pneumonia phase
        if self.pandemic_stage > 1 {
            covid_pneumonia(owner, self, randomer, updates, storage);
        }

        // ICU: if pandemic_stage > 6 ($.a4()), remove infection
        if self.pandemic_stage > 6 {
            return true; // signal to remove this state
        }

        false
    }

    fn on_action_mode(&self, _smart: bool, forced_attack: &mut Option<super::ForcedAttackConfig>) {
        // ICU: pandemic_stage > 6 → skip turn
        // This is handled differently - through pre_action in JS
        // For now, we handle ICU in forced_action
        if self.pandemic_stage > 6 {
            // We'll handle ICU messages separately
        }
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

// =====================================================================
// COVID Boss Attack On-Damage Callback
// =====================================================================

/// on_damage callback for COVID boss attack: infects the target.
/// JS: SklCovidAttack passes T.v8() metadata which calls j7() on hit.
pub fn covid_on_damage(
    caster: PlrId,
    target: PlrId,
    dmg: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    if dmg <= 0 {
        return;
    }
    // Infect the target
    covid_infect(caster, target, 40, randomer, updates, storage);
}

/// JS j7(): Infect a player with COVID.
/// mutation: initial mutation value ($.bg() = 40 for boss attack)
pub fn covid_infect(
    boss_id: PlrId,
    target: PlrId,
    mutation: i32,
    _randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    // Check if already infected with same boss's COVID
    {
        let target_plr = storage.get_player(&target).expect("target not found");
        if target_plr.get_state::<CovidInfection>().is_some() {
            return;
        }
    }

    // Create infection state
    let state = CovidInfection::new(boss_id, mutation);
    {
        let target_plr = storage.just_get_player_mut(target).expect("target not found");
        target_plr.set_state(state);
    }

    // Output infection message: "[1]感染了新冠病毒"
    let update = RunUpdate::new("[1]感染了[0]", boss_id, target, 0);
    updates.add(update);
}

// =====================================================================
// COVID Spreading (v() phase)
// =====================================================================

/// JS CovidState.v(): Spreading phase for an infected player.
/// Called during post_action of the infected player.
fn covid_spreading(
    owner: PlrId,
    state: &mut CovidInfection,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    // Increase pandemic stage: += (rc4 & 3)
    let stage_inc = (randomer.next_u8() & 3) as i32;
    state.pandemic_stage += stage_inc;

    // Mutation update: ~25% chance (rc4 < 64)
    if randomer.next_u8() < 64 {
        state.mutation = (randomer.next_u8() & 127) as i32;
    }

    let boss_id = state.boss_id;

    // Get all alive non-boss players as potential spread targets
    let all_alive: Vec<PlrId> = storage.all_player_ids().into_iter().filter(|id| {
        let Some(plr) = storage.get_player(id) else { return false };
        plr.alive() && *id != owner && plr.player_type() != PlayerType::Boss
    }).collect();

    if all_alive.is_empty() {
        return;
    }

    // Try to spread to one player (simplified from JS v() which tries up to 5)
    // Pick a random alive non-boss player
    let Some(pick_idx) = randomer.pick(&all_alive) else { return };
    let contact_target = all_alive[pick_idx];

    let already_infected = storage.get_player(&contact_target)
        .map(|p| p.get_state::<CovidInfection>().is_some())
        .unwrap_or(true);

    // Contact message
    updates.add(RunUpdate::new_newline());
    let contact_update = RunUpdate::new("[0]和[1]近距离接触", owner, contact_target, 0);
    updates.add(contact_update);

    if !already_infected {
        // Spread check: random roll
        let spread_roll = randomer.next_u8();
        let distance = 128; // simplified distance check
        if spread_roll >= distance / 2 {
            // Infect!
            covid_infect(boss_id, contact_target, state.mutation, randomer, updates, storage);
        } else {
            // Failed: "但[1]没被感染"
            let fail_update = RunUpdate::new("但[1]没被感染", boss_id, contact_target, 0);
            updates.add(fail_update);
        }
    }
    // If already infected: just show the contact message (no new infection)
}

// =====================================================================
// COVID Pneumonia (at() phase)
// =====================================================================

/// JS CovidState.at(): Pneumonia damage phase.
/// Called after spreading if pandemic_stage > 1.
fn covid_pneumonia(
    owner: PlrId,
    state: &CovidInfection,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let boss_id = state.boss_id;

    // Check owner is alive
    let owner_alive = storage.get_player(&owner).map(|p| p.alive()).unwrap_or(false);
    if !owner_alive {
        return;
    }

    // Calculate pneumonia damage:
    // dmg = floor(getAt(target, offense, rc4) + mutation * 80) / getDf(target, defense, rc4))
    let (atp, dfp) = {
        let target_plr = storage.get_player(&owner).expect("owner not found");
        let atp = target_plr.get_at(true, randomer);
        let dfp = target_plr.get_df(true);
        (atp, dfp)
    };
    let pneumonia_dmg = ((atp + state.mutation as f64 * 80.0) / dfp as f64).floor() as i32;
    let pneumonia_dmg = pneumonia_dmg.max(1);

    // Apply pneumonia damage to the infected player
    updates.add(RunUpdate::new_newline());
    let pneumonia_update = RunUpdate::new(" [1][肺炎]发作", boss_id, owner, 0);
    updates.add(pneumonia_update);

    // Apply damage directly
    {
        let target_plr = storage.just_get_player_mut(owner).expect("owner not found");
        let old_hp = target_plr.get_status().hp;
        target_plr.apply_raw_damage(pneumonia_dmg);

        let dmg_update = RunUpdate::new("[1]受到[2]点伤害", boss_id, owner, pneumonia_dmg as u32);
        updates.add(dmg_update);

        // Boss heals
        let actual_dmg = old_hp - target_plr.get_status().hp;
        let heal_amount = if actual_dmg > 0 {
            let boss_alive = storage.get_player(&boss_id).map(|p| p.alive()).unwrap_or(false);
            if boss_alive {
                let base_heal = actual_dmg / 2;
                let boss_hp = storage.get_player(&boss_id).map(|p| p.get_status().hp).unwrap_or(0);
                let boss_max_hp = storage.get_player(&boss_id).map(|p| p.get_status().max_hp).unwrap_or(0);
                let heal = if boss_hp >= boss_max_hp {
                    actual_dmg / 4 + 1
                } else {
                    base_heal
                };
                heal.min(actual_dmg)
            } else {
                0
            }
        } else {
            0
        };

        if heal_amount > 0 {
            if let Some(boss) = storage.just_get_player_mut(boss_id) {
                boss.heal(heal_amount);
            }
            let heal_update = RunUpdate::new("[1]回复体力[2]点", boss_id, boss_id, heal_amount as u32);
            updates.add(heal_update);
        }

        // Check if the infected player died
        if target_plr.get_status().hp <= 0 {
            target_plr.mark_dead(boss_id, updates, storage);
        }
    }
}

// =====================================================================
// ICU (重症监护室)
// =====================================================================

/// Check if a player is in ICU and should skip their turn.
/// Returns true if the player must skip.
pub fn covid_check_icu(owner: PlrId, storage: &Arc<Storage>, updates: &mut RunUpdates) -> bool {
    let icu = storage.get_player(&owner)
        .and_then(|p| p.get_state::<CovidInfection>())
        .map(|c| c.pandemic_stage > 6)
        .unwrap_or(false);

    if icu {
        updates.add(RunUpdate::new("[0]在重症监护室无法行动", owner, owner, 0));
    }

    icu
}

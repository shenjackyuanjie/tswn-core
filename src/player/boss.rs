use std::cell::Cell;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{ActionTargets, Player, PlayerStatus, PlrId, StateTrait, noop_on_damage};
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
    // JS tB: if target not already infected AND (rc4.n() & 63) < dmg → infect
    let already_infected = storage.get_player(&target).map(|p| p.has_state::<CovidInfection>()).unwrap_or(false);
    if already_infected {
        return;
    }
    let roll = (randomer.next_u8() & 63) as i32;
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
                hitters: std::collections::HashSet::new(),
                minions: std::collections::HashSet::new(),
            });
        }
        BossKind::Generic => {}
    }
}

// ─── boss_default_action: called from default_attack for Boss players ─────

/// Number of ActionSkl prob() bytes to consume for a boss.
///
/// In Dart, PlrBoss.addSkillsToProc() adds ActionSkls from `skills` to `actions`.
/// During the normal action loop, each ActionSkl's prob() consumes 1 RC4 byte (r127).
/// - Saitama, Generic: dftAct IS in skills → actions=[dftAct] → 1 prob byte
/// - COVID, Lazy: dftAct NOT in skills → actions=[] → 0 prob bytes
pub fn boss_action_prob_count(name: &str) -> usize {
    match boss_kind(name) {
        BossKind::Covid | BossKind::Lazy => 0,
        BossKind::Saitama | BossKind::Generic => 1,
    }
}

/// Per-skill immunity threshold for a boss (matches Dart PlrBoss.immune()).
///
/// Dart has three tiers:
/// - immunedx skills → r.c94 (94/127)
/// - immuned skills → r.c75 (75/127)
/// - default → r.c33 (33/127)
/// Returns the raw-byte threshold for boss immunity checks.
/// Dart: `c94` → `nextByte() < 240`, `c75` → `nextByte() < 192`, `c33` → `nextByte() < 84`.
/// The caller must compare with `next_u8() as i32`, NOT `r127()`.
pub fn boss_immune_threshold(boss_name: &str, key: &str) -> i32 {
    match boss_kind(boss_name) {
        BossKind::Saitama => match key {
            "half" | "exchange" => 240,        // c94
            "berserk" | "slow" | "ice" => 192, // c75
            _ => 84,                           // c33
        },
        BossKind::Covid => match key {
            "charm" | "berserk" | "exchange" => 192, // c75
            _ => 84,                                 // c33
        },
        BossKind::Lazy => match key {
            "assassinate" | "half" | "curse" | "exchange" => 192, // c75
            _ => 84,                                              // c33
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
    storage.just_get_player_mut(target_id).expect("generic_boss_action target").attacked(
        atp,
        false,
        player.as_ptr(),
        noop_on_damage,
        randomer,
        updates,
        storage,
    );
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
        // Dart: if (caster.meta[Dt.covid] == null) { ... }
        // Any existing CovidMeta (including recovered) blocks infection.
        let has_any_covid = storage.get_player(&caster).map(|p| p.has_state::<CovidInfection>()).unwrap_or(false);
        if has_any_covid {
            return;
        }
        // Dart: if (_hasMask(caster) && r.c75) return;
        let has_mask = storage
            .get_player(&caster)
            .and_then(|p| p.get_weapon_name().map(|n| n.ends_with("mask") || n.ends_with("口罩")))
            .unwrap_or(false);
        if has_mask && randomer.c75() {
            return;
        }
        covid_infect(boss_id, caster, self.mutation, randomer, updates, storage);
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

/// A single COVID infection entry (corresponds to one JS CovidState).
#[derive(Clone, Debug)]
pub struct CovidEntry {
    pub boss_id: PlrId,
    pub mutation: i32,
    /// JS 'days' — incremented in act (v()), checked in postAction (at())
    pub days: i32,
}

/// Infection state on a non-boss player.
/// In JS, a player can have multiple CovidState objects (one per different mutation).
/// CovidMeta tracks a mutation set for immunity.
/// We unify all of this into a single state with multiple entries.
#[derive(Clone, Debug)]
pub struct CovidInfection {
    pub entries: Vec<CovidEntry>,
    /// All mutations this player has ever been infected with (JS CovidMeta.c set).
    pub mutation_set: Vec<i32>,
    /// Dart CovidMeta.recovered — true once all entries have been cured (days > 6).
    /// The state is NOT removed; it persists with recovered=true so new infection
    /// of a different mutation can still happen, but same-mutation re-infection is blocked.
    pub recovered: bool,
}

impl StateTrait for CovidInfection {
    // JS CovidMeta.gT() returns 0, NOT negative. Must be 0 so clear_negative_states() doesn't remove it.
    fn meta_type(&self) -> i32 { 0 }

    // ── PreAction hook: ALWAYS hijacks the player's action (while actively infected) ──
    // JS: ALL CovidState.aN() fire (mutation check), then LAST CovidState's aa()+v() fires.
    fn pre_action_priority(&self) -> i32 { 1000 }
    fn on_pre_action(
        &mut self,
        owner: PlrId,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> bool {
        // Recovered (no active entries) → don't hijack
        if self.entries.is_empty() {
            return false;
        }
        // === Step 1: aN() for ALL entries (mutation check) ===
        // In JS, each CovidState's aN() fires. It consumes 1 byte (possibly 2 if mutation changes).
        let owner_name_pre = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
        eprintln!(
            "[COVID_PREACT] owner={} entries={} rc4=({},{})",
            owner_name_pre,
            self.entries.len(),
            randomer.i,
            randomer.j
        );
        for (ei, entry) in self.entries.iter_mut().enumerate() {
            let pre_byte = randomer.next_u8();
            if pre_byte < 64 {
                let new_mutation = randomer.r127() as i32;
                eprintln!(
                    "[COVID_aN] owner={} entry={} mutation {} -> {} rc4=({},{})",
                    owner_name_pre, ei, entry.mutation, new_mutation, randomer.i, randomer.j
                );
                entry.mutation = new_mutation;
                if !self.mutation_set.contains(&new_mutation) {
                    self.mutation_set.push(new_mutation);
                }
            } else {
                eprintln!(
                    "[COVID_aN] owner={} entry={} mutation={} no_change rc4=({},{})",
                    owner_name_pre, ei, entry.mutation, randomer.i, randomer.j
                );
            }
        }
        eprintln!("[COVID_aN] owner={} mutation_set={:?}", owner_name_pre, self.mutation_set);

        // === Step 2: aa() + v() for LAST entry only ===
        let last_idx = self.entries.len() - 1;
        let boss_id = self.entries[last_idx].boss_id;
        let mutation = self.entries[last_idx].mutation;
        let days = self.entries[last_idx].days;

        // === JS aa(): dummy target picking (results discarded but RC4 must be consumed) ===
        // JS gap() returns the boss player, so aa() picks from boss's enemies (all non-boss alive).
        // Skip indices = boss's team members in all_alive.
        {
            let select_count: usize = if smart { 3 } else { 2 };
            // Boss group = group containing boss_id
            let boss_group_vec;
            let boss_group: &[PlrId] = match storage.group_containing(boss_id) {
                Some(g) => {
                    boss_group_vec = g;
                    boss_group_vec.as_slice()
                }
                None => &[],
            };
            let skip_indices: Vec<usize> = targets
                .all_alive
                .iter()
                .enumerate()
                .filter(|(_, id)| boss_group.contains(id))
                .map(|(i, _)| i)
                .collect();
            let mut selected = Vec::new();
            let mut dup = 0usize;
            let mut invalid = -(select_count as i32);
            while dup <= select_count && invalid <= select_count as i32 {
                let picked = if skip_indices.is_empty() {
                    randomer.pick(&targets.all_alive)
                } else {
                    randomer.pick_skip_range(&targets.all_alive, skip_indices.clone())
                };
                let Some(pick_idx) = picked else { break };
                let target_id = targets.all_alive[pick_idx];
                if selected.contains(&target_id) {
                    dup += 1;
                    continue;
                }
                selected.push(target_id);
                if selected.len() >= select_count {
                    break;
                }
            }
            if !smart {
                for _ in &selected {
                    let _ = randomer.rFFFF();
                }
            }
        }

        // === act (v): spreading / ICU / home isolation ===
        let owner_wisdom = storage.get_player(&owner).map(|p| p.get_status().wisdom).unwrap_or(0);
        let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();

        let condition = days == 0 || (randomer.next_u8() as i32) > owner_wisdom;
        eprintln!(
            "[COVID_ACT] owner={} days={} condition={} mutation={} rc4=({},{})",
            owner_name, days, condition, mutation, randomer.i, randomer.j
        );
        if condition {
            self.entries[last_idx].days += (randomer.next_u8() & 3) as i32;
            let all_alive = targets.all_alive.clone();
            eprintln!(
                "[COVID_SPREAD] all_alive={:?} rc4=({},{})",
                all_alive
                    .iter()
                    .map(|id| storage.get_player(id).map(|p| p.display_name()).unwrap_or_default())
                    .collect::<Vec<_>>(),
                randomer.i,
                randomer.j
            );
            for attempt in 0..5 {
                if all_alive.is_empty() {
                    break;
                }
                let Some(pick_idx) = randomer.pick(&all_alive) else {
                    break;
                };
                let candidate = all_alive[pick_idx];
                let cand_name = storage.get_player(&candidate).map(|p| p.display_name()).unwrap_or_default();
                eprintln!(
                    "[COVID_PICK] attempt={} pick_idx={} candidate={} is_owner={} is_boss={}",
                    attempt,
                    pick_idx,
                    cand_name,
                    candidate == owner,
                    candidate == boss_id
                );
                if candidate == owner || candidate == boss_id {
                    continue;
                }
                let candidate_alive = storage.get_player(&candidate).map(|p| p.alive()).unwrap_or(false);
                if !candidate_alive {
                    continue;
                }

                // Check if candidate already has this mutation (immune)
                // JS: n = CovidMeta of candidate; if n != null: m = !n.c.w(0, mutation)
                let candidate_set = storage
                    .get_player(&candidate)
                    .and_then(|p| p.get_state::<CovidInfection>())
                    .map(|inf| inf.mutation_set.clone());
                let already_has_mutation = candidate_set.as_ref().map(|s| s.contains(&mutation)).unwrap_or(false);
                eprintln!(
                    "[COVID_MUTCHECK] candidate={} mutation={} candidate_set={:?} already={}",
                    cand_name, mutation, candidate_set, already_has_mutation
                );
                if already_has_mutation {
                    continue;
                }

                let owner_group = storage.group_containing(owner);
                let candidate_in_owner_group = owner_group.map(|g| g.contains(&candidate)).unwrap_or(false);

                if candidate_in_owner_group {
                    covid_contact_spread(owner, candidate, boss_id, mutation, randomer, updates, storage);
                } else {
                    covid_attack_spread(owner, candidate, boss_id, mutation, randomer, updates, storage);
                }
                return true;
            }
        }

        // 2nd increment
        self.entries[last_idx].days += (randomer.next_u8() & 3) as i32;

        if self.entries[last_idx].days > 2 {
            updates.add(RunUpdate::new("[1]在重症监护室无法行动", boss_id, owner, 0));
        } else {
            updates.add(RunUpdate::new("[1]在家中自我隔离", boss_id, owner, 0));
        }

        true
    }

    // ── PostAction hook: pneumonia + auto-cure ──
    // JS: ALL CovidState.at() fire (pneumonia for each).
    fn post_action_priority(&self) -> i32 { 1000 }
    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        if self.entries.is_empty() {
            return false;
        }
        let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
        eprintln!(
            "[COVID_POSTACT] owner={} alive={} entries={} days={:?}",
            owner_name,
            alive,
            self.entries.len(),
            self.entries.iter().map(|e| e.days).collect::<Vec<_>>()
        );
        // Pneumonia for EACH entry
        for entry in &self.entries {
            if alive && entry.days > 1 {
                covid_pneumonia(owner, entry.boss_id, entry.mutation, randomer, updates, storage);
            }
        }
        // Auto-cure: remove entries with days > 6 (Dart: CovidState.destroy sets recovered=true)
        self.entries.retain(|e| e.days <= 6);
        if self.entries.is_empty() && !self.recovered {
            self.recovered = true;
            // Dart: destroy() calls target.updateStates()
            if let Some(plr) = storage.just_get_player_mut(owner) {
                plr.update_states();
            }
        }
        false // never remove the state; keep it with recovered=true for immunity tracking
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
    let mutation = player.get_state::<CovidBossState>().map(|s| s.mutation).unwrap_or(40);

    let atp = player.get_at(false, randomer);
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));

    // Set thread-local context for on_damage callback
    COVID_ON_DAMAGE_CTX.set(Some((boss_id, mutation)));
    storage.just_get_player_mut(target_id).expect("covid_boss_action target").attacked(
        atp,
        false,
        boss_id,
        covid_spread_on_damage,
        randomer,
        updates,
        storage,
    );
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

    let _target_name = target_plr.display_name();
    let boss_display = storage.get_player(&boss_id).map(|p| p.display_name()).unwrap_or_default();

    // Dart newState: if (covidMeta == null || (covidMeta.recovered && !covidMeta.hasMutation(mutation)))
    if let Some(inf) = target_plr.get_state::<CovidInfection>() {
        // CovidMeta exists — only allow re-infection if recovered AND new mutation
        if !inf.recovered || inf.mutation_set.contains(&mutation) {
            return;
        }
    }

    // Add entry to existing CovidInfection, or create new one
    if let Some(inf) = target_plr.get_state_mut::<CovidInfection>() {
        inf.recovered = false; // re-activated
        inf.entries.push(CovidEntry {
            boss_id,
            mutation,
            days: 0,
        });
        if !inf.mutation_set.contains(&mutation) {
            inf.mutation_set.push(mutation);
        }
    } else {
        target_plr.set_state(CovidInfection {
            entries: vec![CovidEntry {
                boss_id,
                mutation,
                days: 0,
            }],
            mutation_set: vec![mutation],
            recovered: false,
        });
    }
    updates.add(RunUpdate::new(format!("[1]感染了{boss_display}"), boss_id, target, 0));

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

    // JS talk: itlMask = _hasMask(candidate) ? itl + 192 : itl >> 1; if (r.r255 < itlMask) → resisted
    let candidate_wisdom = storage.get_player(&candidate).map(|p| p.get_status().wisdom).unwrap_or(0);
    let has_mask = storage
        .get_player(&candidate)
        .and_then(|p| p.get_weapon_name().map(|n| n.ends_with("mask") || n.ends_with("口罩")))
        .unwrap_or(false);
    let threshold = if has_mask {
        candidate_wisdom + 192
    } else {
        candidate_wisdom >> 1
    };
    let roll = randomer.next_u8() as i32;
    if roll < threshold {
        updates.add(RunUpdate::new(format!("但{candidate_name}没被感染"), owner, candidate, 0));
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
    storage.just_get_player_mut(candidate).expect("covid_attack_spread candidate").attacked(
        atp,
        false,
        owner,
        covid_spread_on_damage,
        randomer,
        updates,
        storage,
    );
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
    eprintln!(
        "[COVID_PNEUMONIA] owner={} rc4=({},{}) before get_at",
        owner_name, randomer.i, randomer.j
    );
    let at_val = owner_plr.get_at(true, randomer);
    let df_val = owner_plr.get_df(true);
    let dmg = ((at_val + (mutation * 80) as f64) / df_val as f64).ceil() as i32;
    eprintln!("[COVID_PNEUMONIA] at_val={at_val} df_val={df_val} mutation={mutation} dmg={dmg}");

    if dmg <= 0 {
        return;
    }

    updates.add(RunUpdate::new(format!(" {owner_name}肺炎发作"), boss_id, owner, 0));

    // Apply damage to target (raw, through `damage` for proper death handling)
    let _old_hp = {
        let plr = storage.get_player(&owner).unwrap();
        plr.get_status().hp
    };
    let actual_dmg = {
        let plr = storage.just_get_player_mut(owner).expect("covid_pneumonia owner");
        plr.damage(dmg, boss_id, noop_on_damage, randomer, updates, storage)
    };
    eprintln!("[COVID_PNEUMONIA] actual_dmg={actual_dmg} dmg={dmg}");

    // Boss heals: min(dmg>>1, actualDmg) or min((dmg>>2)+1, actualDmg) if full HP
    let boss_hp_full = storage
        .get_player(&boss_id)
        .map(|p| {
            let s = p.get_status();
            eprintln!("[COVID_PNEUMONIA] boss hp={} max_hp={}", s.hp, s.max_hp);
            s.hp >= s.max_hp
        })
        .unwrap_or(false);

    // JS: n = s >> 1; if full: n = (n >> 2) + 1 = (s >> 3) + 1
    let heal_amount = if boss_hp_full {
        std::cmp::min((dmg >> 3) + 1, actual_dmg)
    } else {
        std::cmp::min(dmg >> 1, actual_dmg)
    };
    eprintln!("[COVID_PNEUMONIA] boss_hp_full={boss_hp_full} heal_amount={heal_amount}");
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
    fn apply_update_state(&self, status: &mut PlayerStatus) { status.speed /= 2; }

    // ── PreAction: 50% chance to skip turn ──
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
            // Dart: preAction returns LazyState → normal action flow runs select → act.
            // LazyState inherits Skill.select() using boss as owner → boss.selectEnemy()
            // We must consume the same RC4 bytes as LazyState.select() would.
            lazy_select_consume_bytes(self.boss_id, smart, randomer, storage);
            // Skip turn: be lazy (damage handled by on_post_action after NewLine)
            be_lazy(owner, randomer, updates, storage);
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
        // Dart throws an exception when the boss's group is eliminated, aborting postAction.
        // Replicate by skipping when the boss is dead (fight would end).
        if !storage.get_player(&self.boss_id).map(|p| p.alive()).unwrap_or(false) {
            return false;
        }
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

    // Dart flow: dftAct.select() runs BEFORE dftAct.act(), so select target first
    let Some(target_id) = player.select_default_attack_target(smart, randomer, storage, targets) else {
        return;
    };

    // Dart: `if (lazyState != null && r.c50)` — short-circuit: byte only consumed if infected
    let target_infected = storage.get_player(&target_id).map(|p| p.has_state::<LazyInfection>()).unwrap_or(false);
    if target_infected && randomer.next_u8() < 128 {
        // Boss is lazy this turn
        be_lazy(boss_id, randomer, updates, storage);
        if let Some(boss_state) = player.get_state_mut::<LazyBossState>() {
            boss_state.at_boost += 0.5;
        }
        return;
    }

    // Normal attack with atboost
    let at_boost = player.get_state::<LazyBossState>().map(|s| s.at_boost).unwrap_or(1.0);
    let atp = player.get_at(false, randomer) * at_boost;
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));

    LAZY_ON_DAMAGE_CTX.set(Some(boss_id));
    let actual_dmg = storage.just_get_player_mut(target_id).expect("lazy_boss_action target").attacked(
        atp,
        false,
        boss_id,
        lazy_attack_on_damage,
        randomer,
        updates,
        storage,
    );
    LAZY_ON_DAMAGE_CTX.set(None);

    // Reset atboost on hit
    if actual_dmg > 0 {
        if let Some(boss_state) = player.get_state_mut::<LazyBossState>() {
            boss_state.at_boost = 1.0;
        }
    }
}

/// Simulate the bytes consumed by Dart's LazyState.select() when lazy preAction fires.
/// LazyState inherits Skill.select() with selCount=2, selCountSmart=3.
/// selectOneTarget() = boss.selectEnemy(r) = r.pickSkipRange(all_alives, boss_alives).
/// validTarget() always returns true. scoreTarget: non-smart → r.rFFFF (2 bytes), smart → 0 bytes.
fn lazy_select_consume_bytes(boss_id: PlrId, smart: bool, randomer: &mut RC4, storage: &Arc<Storage>) {
    let n = if smart { 3usize } else { 2usize };
    let all_alive = storage.all_alive_ids();
    let boss_group = storage.alive_group_at_team_of(boss_id).cloned().unwrap_or_default();

    // Build skip indices (positions of boss group members in all_alive)
    let skip_indices: Vec<usize> = boss_group.iter().filter_map(|id| all_alive.iter().position(|a| a == id)).collect();

    let mut selected = Vec::new();
    let mut dup = 0usize;
    let n_i32 = n as i32;
    let mut invalid = -n_i32;

    while (dup as i32) <= n_i32 && invalid <= n_i32 {
        let picked = if skip_indices.is_empty() {
            randomer.pick(&all_alive)
        } else if all_alive.len() > skip_indices.len() {
            randomer.pick_skip_range(&all_alive, skip_indices.clone())
        } else {
            None
        };
        let Some(picked_idx) = picked else {
            break;
        };
        // validTarget always true; no invalid increments
        if !selected.contains(&picked_idx) {
            selected.push(picked_idx);
            if selected.len() >= n {
                break;
            }
        } else {
            dup += 1;
        }
    }

    // scoreTarget for each selected target
    for _ in &selected {
        if !smart {
            let _ = randomer.rFFFF();
        }
    }
}

/// Infect target with lazy
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
fn lazy_post_action_damage(owner: PlrId, boss_id: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
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

    updates.add(RunUpdate::new(format!(" {owner_name}{boss_display}发作"), boss_id, owner, 0));

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
    /// Unique hitters (Dart: Set<Plr>). For IAddPlr, stores owner; for normal, stores caster.
    pub hitters: std::collections::HashSet<PlrId>,
    /// Unique minions (Dart: Set<Plr>). Only IAddPlr casters themselves.
    pub minions: std::collections::HashSet<PlrId>,
}

impl StateTrait for SaitamaState {
    fn meta_type(&self) -> i32 { 0 }

    // ── PostDefend: dmg / 100 ──
    fn post_defend_priority(&self) -> i32 { i32::MAX } // JS: priority = Infinity
    fn on_post_defend(
        &mut self,
        _owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        _randomer: &mut RC4,
        _updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) {
        self.damages += *dmg;
        // Check if caster is a minion/clone (IAddPlr in Dart)
        if let Some(caster_plr) = storage.get_player(&caster) {
            if let Some(minion_state) = caster_plr.get_state::<crate::player::skill::act::minion::MinionRuntimeState>() {
                if let Some(owner_id) = minion_state.owner {
                    self.hitters.insert(owner_id);
                    self.minions.insert(caster);
                } else {
                    self.hitters.insert(caster);
                }
            } else {
                self.hitters.insert(caster);
            }
        } else {
            self.hitters.insert(caster);
        }
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

    // Dart: dftAct.select() is called BEFORE act(), even when turns < 10.
    // We must always select a target to consume the same RC4 bytes as Dart.
    let selected_target = player.select_default_attack_target(smart, randomer, storage, targets);

    // Hunger check: damages / (hitters.len + minions.len/3 + 1) > 255
    let (damages, hitters_len, minions_len) = player
        .get_state::<SaitamaState>()
        .map(|s| (s.damages, s.hitters.len() as i32, s.minions.len() as i32))
        .unwrap_or((0, 0, 0));

    let hunger_denominator = hitters_len + minions_len / 3 + 1;
    if damages / hunger_denominator.max(1) > 255 {
        // 觉得有点饿 → 离开了战场
        let boss_display = player.display_name();
        updates.add(RunUpdate::new(format!("{boss_display}觉得有点饿"), boss_id, boss_id, 0));
        updates.add(RunUpdate::new_newline());
        updates.add(RunUpdate::new(format!(" {boss_display}离开了战场"), boss_id, boss_id, 0));
        // Dart: owner.group.die(owner) — just removes from alive, no die event
        let old_hp = player.get_status().hp;
        player.apply_raw_damage(old_hp);
        player.status.set_alive(false);
        storage.record_death(boss_id);
        return;
    }

    // Turn counter
    let turns = player.get_state::<SaitamaState>().map(|s| s.turns).unwrap_or(0);

    if turns < 10 {
        // Increment and do nothing (no attack); target selected but unused
        if let Some(state) = player.get_state_mut::<SaitamaState>() {
            state.turns += 1;
        }
        return;
    }

    // turns >= 10: attack with getAt × 12
    let Some(target_id) = selected_target else {
        return;
    };
    let atp = player.get_at(false, randomer) * 12.0;
    updates.add(RunUpdate::new("[0]发起攻击", boss_id, target_id, 0));
    storage.just_get_player_mut(target_id).expect("saitama attack target").attacked(
        atp,
        false,
        boss_id,
        noop_on_damage,
        randomer,
        updates,
        storage,
    );

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

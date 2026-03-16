use std::cell::Cell;
use std::sync::Arc;

#[allow(unused_imports)]
use crate::debug::debug_println;
use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{ActionTargets, Player, PlrId, StateTrait, noop_on_damage};
use crate::rc4::RC4;

thread_local! {
    static COVID_ON_DAMAGE_CTX: Cell<Option<(PlrId, i32)>> = const { Cell::new(None) };
}

#[derive(Clone, Debug)]
pub struct CovidBossState {
    pub mutation: i32,
}

impl StateTrait for CovidBossState {
    fn meta_type(&self) -> i32 { 0 }
    fn post_damage_priority(&self) -> i32 { 1000 }

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
        let has_any_covid = storage.get_player(&caster).map(|p| p.has_state::<CovidInfection>()).unwrap_or(false);
        if has_any_covid {
            return;
        }
        let has_mask = storage
            .get_player(&caster)
            .and_then(|p| p.get_weapon_name().map(|n| n.ends_with("mask") || n.ends_with("口罩")))
            .unwrap_or(false);
        if has_mask && randomer.c75() {
            return;
        }
        covid_infect(boss_id, caster, self.mutation, randomer, updates, storage);
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

#[derive(Clone, Debug)]
pub struct CovidEntry {
    pub boss_id: PlrId,
    pub mutation: i32,
    pub days: i32,
}

#[derive(Clone, Debug)]
pub struct CovidInfection {
    pub entries: Vec<CovidEntry>,
    pub mutation_set: Vec<i32>,
    pub recovered: bool,
}

impl StateTrait for CovidInfection {
    fn meta_type(&self) -> i32 { 0 }

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
        if self.entries.is_empty() {
            return false;
        }
        #[cfg(not(feature = "no_debug"))]
        let debug_covid = crate::debug::debug_covid();
        #[cfg(not(feature = "no_debug"))]
        let owner_name_pre = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
        #[cfg(not(feature = "no_debug"))]
        debug_println!(
            debug_covid,
            "[COVID_PREACT] owner={} entries={} rc4=({},{})",
            owner_name_pre,
            self.entries.len(),
            randomer.i,
            randomer.j
        );
        for (_ei, entry) in self.entries.iter_mut().enumerate() {
            let pre_byte = randomer.next_u8();
            if pre_byte < 64 {
                let new_mutation = randomer.r127() as i32;
                debug_println!(
                    debug_covid,
                    "[COVID_aN] owner={} entry={} mutation {} -> {} rc4=({},{})",
                    owner_name_pre,
                    _ei,
                    entry.mutation,
                    new_mutation,
                    randomer.i,
                    randomer.j
                );
                entry.mutation = new_mutation;
                if !self.mutation_set.contains(&new_mutation) {
                    self.mutation_set.push(new_mutation);
                }
            } else {
                debug_println!(
                    debug_covid,
                    "[COVID_aN] owner={} entry={} mutation={} no_change rc4=({},{})",
                    owner_name_pre,
                    _ei,
                    entry.mutation,
                    randomer.i,
                    randomer.j
                );
            }
        }
        debug_println!(
            debug_covid,
            "[COVID_aN] owner={} mutation_set={:?}",
            owner_name_pre,
            self.mutation_set
        );

        let last_idx = self.entries.len() - 1;
        let boss_id = self.entries[last_idx].boss_id;
        let mutation = self.entries[last_idx].mutation;
        let days = self.entries[last_idx].days;

        {
            let select_count: usize = if smart { 3 } else { 2 };
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
            let invalid = -(select_count as i32);
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

        let owner_wisdom = storage.get_player(&owner).map(|p| p.get_status().wisdom).unwrap_or(0);
        #[cfg(not(feature = "no_debug"))]
        let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();

        let condition = days == 0 || (randomer.next_u8() as i32) > owner_wisdom;
        debug_println!(
            debug_covid,
            "[COVID_ACT] owner={} days={} condition={} mutation={} rc4=({},{})",
            owner_name,
            days,
            condition,
            mutation,
            randomer.i,
            randomer.j
        );
        if condition {
            self.entries[last_idx].days += (randomer.next_u8() & 3) as i32;
            let all_alive = targets.all_alive.clone();
            debug_println!(
                debug_covid,
                "[COVID_SPREAD] all_alive={:?} rc4=({},{})",
                all_alive
                    .iter()
                    .map(|id| storage.get_player(id).map(|p| p.display_name()).unwrap_or_default())
                    .collect::<Vec<_>>(),
                randomer.i,
                randomer.j
            );
            for _attempt in 0..5 {
                if all_alive.is_empty() {
                    break;
                }
                let Some(pick_idx) = randomer.pick(&all_alive) else {
                    break;
                };
                let candidate = all_alive[pick_idx];
                #[cfg(not(feature = "no_debug"))]
                let cand_name = storage.get_player(&candidate).map(|p| p.display_name()).unwrap_or_default();
                debug_println!(
                    debug_covid,
                    "[COVID_PICK] attempt={} pick_idx={} candidate={} is_owner={} is_boss={}",
                    _attempt,
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

                let candidate_set = storage
                    .get_player(&candidate)
                    .and_then(|p| p.get_state::<CovidInfection>())
                    .map(|inf| inf.mutation_set.clone());
                let already_has_mutation = candidate_set.as_ref().map(|s| s.contains(&mutation)).unwrap_or(false);
                debug_println!(
                    debug_covid,
                    "[COVID_MUTCHECK] candidate={} mutation={} candidate_set={:?} already={}",
                    cand_name,
                    mutation,
                    candidate_set,
                    already_has_mutation
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

        self.entries[last_idx].days += (randomer.next_u8() & 3) as i32;

        if self.entries[last_idx].days > 2 {
            updates.add(RunUpdate::new("[1]在重症监护室无法行动", boss_id, owner, 0));
        } else {
            updates.add(RunUpdate::new("[1]在家中自我隔离", boss_id, owner, 0));
        }

        true
    }

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
        #[cfg(not(feature = "no_debug"))]
        let debug_covid = crate::debug::debug_covid();
        #[cfg(not(feature = "no_debug"))]
        let owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
        #[cfg(not(feature = "no_debug"))]
        debug_println!(
            debug_covid,
            "[COVID_POSTACT] owner={} alive={} entries={} days={:?}",
            owner_name,
            alive,
            self.entries.len(),
            self.entries.iter().map(|e| e.days).collect::<Vec<_>>()
        );
        for entry in &self.entries {
            if alive && entry.days > 1 {
                covid_pneumonia(owner, entry.boss_id, entry.mutation, randomer, updates, storage);
            }
        }
        self.entries.retain(|e| e.days <= 6);
        if self.entries.is_empty() && !self.recovered {
            self.recovered = true;
            if let Some(plr) = storage.just_get_player_mut(owner) {
                plr.update_states();
            }
        }
        false
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(self.clone()) }
}

pub fn covid_boss_action(
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
    let already_infected = storage.get_player(&target).map(|p| p.has_state::<CovidInfection>()).unwrap_or(false);
    if already_infected {
        return;
    }
    let roll = (randomer.next_u8() & 63) as i32;
    if roll < dmg {
        covid_infect(boss_id, target, mutation, randomer, updates, storage);
    }
}

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

    if let Some(inf) = target_plr.get_state::<CovidInfection>()
        && (!inf.recovered || inf.mutation_set.contains(&mutation))
    {
        return;
    }

    if let Some(inf) = target_plr.get_state_mut::<CovidInfection>() {
        inf.recovered = false;
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

fn covid_attack_spread(
    owner: PlrId,
    candidate: PlrId,
    boss_id: PlrId,
    mutation: i32,
    randomer: &mut RC4,
    updates: &mut RunUpdates,
    storage: &Arc<Storage>,
) {
    let atp = {
        let owner_plr = storage.get_player(&owner).expect("covid_attack_spread owner");
        owner_plr.get_at(false, randomer)
    };

    let _owner_name = storage.get_player(&owner).map(|p| p.display_name()).unwrap_or_default();
    let _candidate_name = storage.get_player(&candidate).map(|p| p.display_name()).unwrap_or_default();

    updates.add(RunUpdate::new("[0]发起攻击", owner, candidate, 0));

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

    #[cfg(not(feature = "no_debug"))]
    let debug_covid = crate::debug::debug_covid();
    let owner_name = owner_plr.display_name();

    #[cfg(not(feature = "no_debug"))]
    debug_println!(
        debug_covid,
        "[COVID_PNEUMONIA] owner={} rc4=({},{}) before get_at",
        owner_name,
        randomer.i,
        randomer.j
    );
    let at_val = owner_plr.get_at(true, randomer);
    let df_val = owner_plr.get_df(true);
    let dmg = ((at_val + (mutation * 80) as f64) / df_val as f64).ceil() as i32;
    debug_println!(
        debug_covid,
        "[COVID_PNEUMONIA] at_val={at_val} df_val={df_val} mutation={mutation} dmg={dmg}"
    );

    if dmg <= 0 {
        return;
    }

    updates.add(RunUpdate::new(format!(" {owner_name}肺炎发作"), boss_id, owner, 0));

    let _old_hp = {
        let plr = storage.get_player(&owner).unwrap();
        plr.get_status().hp
    };
    let actual_dmg = {
        let plr = storage.just_get_player_mut(owner).expect("covid_pneumonia owner");
        plr.damage(dmg, boss_id, noop_on_damage, randomer, updates, storage)
    };
    debug_println!(debug_covid, "[COVID_PNEUMONIA] actual_dmg={actual_dmg} dmg={dmg}");

    let boss_hp_full = storage
        .get_player(&boss_id)
        .map(|p| {
            let s = p.get_status();
            debug_println!(debug_covid, "[COVID_PNEUMONIA] boss hp={} max_hp={}", s.hp, s.max_hp);
            s.hp >= s.max_hp
        })
        .unwrap_or(false);

    let heal_amount = if boss_hp_full {
        std::cmp::min((dmg >> 3) + 1, actual_dmg)
    } else {
        std::cmp::min(dmg >> 1, actual_dmg)
    };
    debug_println!(
        debug_covid,
        "[COVID_PNEUMONIA] boss_hp_full={boss_hp_full} heal_amount={heal_amount}"
    );
    if heal_amount > 0
        && let Some(boss_plr) = storage.just_get_player_mut(boss_id)
    {
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

use super::*;

impl CombatRuntime {
    pub fn clear_plain_hide_before_action(&mut self, actor: EntityIdx) {
        let Some(hide) = self.entities.get_mut(actor).and_then(|entity| entity.runtime.hide.take()) else {
            return;
        };
        let actor = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime_v2 hide owner disappeared while clearing: {}", actor.0));
        actor.runtime.attract_bits = hide.attract_bits;
        actor.runtime.agility = hide.agility;
        actor.runtime.defense = hide.defense;
        actor.runtime.resistance = hide.resistance;
    }

    pub fn drain_plain_post_damage_skill_chain_into(
        &mut self,
        target: EntityIdx,
        damage: i32,
        caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        #[derive(Debug, Clone, Copy)]
        enum PlainPostDamageSkill {
            Upgrade,
            Hide,
            Counter,
            Assassinate,
            SummonShareDamage,
        }

        let mut plan = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 post-damage target: {}", target.0))
            .template
            .skills
            .skills()
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(fixed_lane, skill_id)| {
                let export_name = self.registry.skill(skill_id)?.export_name.as_str();
                let skill = match export_name {
                    DEFAULT_CORE_UPGRADE_SKILL_EXPORT => PlainPostDamageSkill::Upgrade,
                    DEFAULT_CORE_HIDE_SKILL_EXPORT => PlainPostDamageSkill::Hide,
                    DEFAULT_CORE_COUNTER_SKILL_EXPORT => PlainPostDamageSkill::Counter,
                    export_name if export_name == BuiltinActiveSkill::Assassinate.export_name() => {
                        PlainPostDamageSkill::Assassinate
                    }
                    DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT => PlainPostDamageSkill::SummonShareDamage,
                    _ => return None,
                };
                let level = self.entities.get(target)?.template.skills.level_at(fixed_lane)?;
                if level == 0 {
                    return None;
                }
                Some((skill, level))
            })
            .collect::<Vec<_>>();
        plan.sort_by_key(|(skill, _)| matches!(skill, PlainPostDamageSkill::Assassinate));
        #[cfg(not(feature = "no_debug"))]
        let debug_counter = std::env::var_os("TSWN_PROBE_COUNTER").is_some();
        #[cfg(not(feature = "no_debug"))]
        if debug_counter {
            eprintln!(
                "[counter_probe:v2:plan] target={} caster={} damage={} updates_id={} plan={:?} rc4=({}, {})",
                target.0, caster.0, damage, updates.id, plan, self.rng.i, self.rng.j,
            );
        }

        for (skill, level) in plan {
            #[cfg(not(feature = "no_debug"))]
            let rng_before = (self.rng.i, self.rng.j);
            match skill {
                PlainPostDamageSkill::Upgrade => {
                    self.run_plain_upgrade_post_damage_into(target, level, damage, caster, updates);
                }
                PlainPostDamageSkill::Hide => {
                    self.run_plain_hide_post_damage_into(target, level, damage, caster, updates);
                }
                PlainPostDamageSkill::Counter => {
                    self.run_plain_counter_post_damage_into(target, level, damage, caster, updates);
                }
                PlainPostDamageSkill::Assassinate => {
                    self.run_plain_assassinate_post_damage_into(target, damage, updates);
                }
                PlainPostDamageSkill::SummonShareDamage => {
                    self.drain_plain_summon_share_damage_into(target, level, damage, caster, updates);
                }
            }
            #[cfg(not(feature = "no_debug"))]
            if debug_counter {
                eprintln!(
                    "[counter_probe:v2:skill] target={} caster={} skill={:?} level={} updates_id={} rc4=({}, {}) -> ({}, {})",
                    target.0, caster.0, skill, level, updates.id, rng_before.0, rng_before.1, self.rng.i, self.rng.j,
                );
            }
        }
        if damage > 0 && self.lazy_boss_at_boost(target).is_some() {
            self.infect_with_lazy_into(target, caster, updates);
        }
    }

    pub fn run_plain_counter_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        if level == 0 {
            return;
        }
        let owner_wisdom = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 counter owner: {}", target.0))
            .runtime
            .wisdom
            .clamp(0, 127) as u32;
        let owner_ally_team = self.plain_effective_team(target);
        let caster_team = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime_v2 counter caster: {}", caster.0))
            .runtime
            .team;
        if owner_ally_team == caster_team && self.rng.r63() < owner_wisdom {
            return;
        }

        let counter = &mut self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 counter owner disappeared: {}", target.0))
            .runtime
            .counter;
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_COUNTER").is_some() {
            eprintln!(
                "[counter_probe:v2:state] target={} caster={} updates_id={} last_updates_id={:?} pending={} last_target={:?}",
                target.0,
                caster.0,
                updates.id,
                counter.last_updates_id,
                counter.pending,
                counter.last_target.map(|idx| idx.0),
            );
        }
        if counter.last_updates_id == Some(updates.id) {
            if counter.pending && Some(caster) != counter.last_target && self.rng.r127() < level {
                counter.last_target = Some(caster);
            }
            return;
        }

        counter.last_updates_id = Some(updates.id);
        if self.rng.r255() < level {
            counter.last_target = Some(caster);
            counter.pending = true;
            updates.on_update_end.push(target.0 as usize);
        } else {
            counter.pending = false;
            counter.last_target = None;
        }
    }

    pub fn drain_plain_update_end_into(&mut self, updates: &mut RunUpdates) {
        let mut guard = 0usize;
        while guard < 64 && !updates.on_update_end.is_empty() {
            let pending = std::mem::take(&mut updates.on_update_end);
            for actor in pending {
                let Ok(actor) = u32::try_from(actor) else {
                    continue;
                };
                self.run_plain_counter_update_end_into(EntityIdx(actor), updates);
            }
            guard += 1;
        }
    }

    pub fn run_plain_counter_update_end_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let counter_levels = self
            .entities
            .get(owner)
            .map(|entity| {
                entity
                    .template
                    .skills
                    .skills()
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(fixed_lane, skill_id)| {
                        (self.registry.skill(skill_id)?.export_name == DEFAULT_CORE_COUNTER_SKILL_EXPORT)
                            .then(|| entity.template.skills.level_at(fixed_lane))
                            .flatten()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for _level in counter_levels {
            let target = {
                let Some(owner_entity) = self.entities.get_mut(owner) else {
                    return;
                };
                let counter = &mut owner_entity.runtime.counter;
                if !counter.pending || counter.last_updates_id != Some(updates.id) {
                    continue;
                }
                counter.pending = false;
                counter.last_updates_id = None;
                counter.last_target.take()
            };
            let Some(target) = target else {
                continue;
            };
            if !self.entities.get(target).is_some_and(EntityRecord::is_active) {
                continue;
            }

            let atp = {
                let owner_runtime = &mut self
                    .entities
                    .get_mut(owner)
                    .unwrap_or_else(|| panic!("runtime_v2 counter owner disappeared: {}", owner.0))
                    .runtime;
                if !owner_runtime.mp_ready(&mut self.rng) {
                    continue;
                }
                owner_runtime.get_at(false, &mut self.rng)
            };
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                "[0]发起[反击][s_counter]",
                owner.0 as usize,
                target.0 as usize,
                1,
            ));
            self.drain_plain_attack_with_atp_into(owner, target, false, atp, updates);
        }
    }

    pub fn run_plain_upgrade_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        _caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let (already_active, alive, hp) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.upgrade_active, entity.runtime.alive, entity.runtime.hp))
            .unwrap_or_else(|| panic!("unknown runtime_v2 upgrade target: {}", target.0));
        if level == 0 || already_active || !alive || hp <= 0 {
            return;
        }
        let min_hp = 16 + level.saturating_sub(63) as i32;
        if hp >= min_hp + self.rng.r63() as i32 {
            return;
        }
        if self.rng.r63() >= level {
            return;
        }

        updates.add_newline();
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]做出[垂死]抗争",
            target.0 as usize,
            target.0 as usize,
            60,
        ));
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]所有属性上升",
            target.0 as usize,
            target.0 as usize,
            0,
        ));
        let target = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 upgrade target disappeared: {}", target.0));
        assert!(target.activate_upgrade_runtime(), "runtime_v2 upgrade activated twice");
    }

    pub fn run_plain_hide_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        _caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let (already_active, owner_active) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hide.is_some(), entity.runtime.alive && entity.runtime.hp > 0))
            .unwrap_or_else(|| panic!("unknown runtime_v2 hide target: {}", target.0));
        if level == 0 || already_active || !owner_active {
            return;
        }
        let effective_team = self.plain_effective_team(target);
        let alive_allies = self.world.team_alive(effective_team).map_or(0, |team| {
            team.iter()
                .filter(|ally| {
                    self.entities
                        .get(**ally)
                        .is_some_and(|entity| entity.runtime.alive && entity.runtime.hp > 0)
                })
                .count()
        });
        if alive_allies <= 1 || self.rng.r63() >= level {
            return;
        }

        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 hide target disappeared: {}", target.0));
        target_entity.runtime.hide = Some(HideRuntime {
            level,
            attract_bits: target_entity.runtime.attract_bits,
            agility: target_entity.runtime.agility,
            defense: target_entity.runtime.defense,
            resistance: target_entity.runtime.resistance,
        });
        target_entity.runtime.attract_bits = (target_entity.runtime.attract() / 10.0).to_bits();
        if level > 63 {
            let boost = (level - 63) as i32;
            target_entity.runtime.agility += boost;
            target_entity.runtime.defense += boost;
            target_entity.runtime.resistance += boost;
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发动[隐匿]",
            target.0 as usize,
            target.0 as usize,
            10,
        ));
    }
}

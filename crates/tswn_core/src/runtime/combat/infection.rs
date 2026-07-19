use super::*;

impl CombatRuntime {
    pub fn entity_mp_ready(&mut self, owner: EntityIdx) -> bool {
        let Some(entity) = self.entities.get(owner) else {
            return false;
        };
        if !entity.is_active() {
            return false;
        }
        self.entities
            .get_mut(owner)
            .expect("runtime entity disappeared during mp_ready")
            .runtime
            .mp_ready(&mut self.rng)
    }

    pub fn apply_poison_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 4 {
            return;
        }
        let target_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime poison target: {}", target.0))
            .runtime
            .hp;
        if target_hp <= 0 || self.status_immune(target, "poison") {
            return;
        }

        let poison_atp = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime poison caster: {}", caster.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 1.2000000476837158;
        let poison_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_POISON_STATE_EXPORT)
            .expect("default runtime profile must register poison state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime poison target: {}", target.0));
        let existing = target_entity.states.entry(PLAIN_POISON_STATE_KEY).map(|entry| match entry.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                ..
            } => (caster, target, f64::from_bits(atp_bits)),
            _ => panic!("runtime poison state key is occupied by another payload"),
        });
        if let Some((_, existing_target, existing_atp)) = existing {
            assert!(
                target_entity.states.set_payload(
                    PLAIN_POISON_STATE_KEY,
                    StatePayload::Poison {
                        caster: Some(caster.0),
                        target: existing_target.or(Some(target.0)),
                        atp_bits: (existing_atp + poison_atp).to_bits(),
                        count: 4,
                    },
                ),
                "runtime poison state disappeared while stacking"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::poison(
                    PLAIN_POISON_STATE_KEY,
                    poison_state,
                    Some(caster.0),
                    Some(target.0),
                    poison_atp,
                    4,
                    SkillPriority(150),
                )),
                "runtime poison state key should be vacant"
            );
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[1][中毒]",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    pub fn covid_boss_mutation(&self, boss: EntityIdx) -> Option<i32> {
        self.entities.get(boss)?.states.entries().iter().find_map(|entry| {
            let StatePayload::CovidBoss { mutation } = &entry.payload else {
                return None;
            };
            Some(*mutation)
        })
    }

    pub fn has_covid_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(&entry.payload, StatePayload::CovidInfection { .. }))
        })
    }

    pub fn lazy_boss_at_boost(&self, boss: EntityIdx) -> Option<f64> {
        self.entities.get(boss)?.states.entries().iter().find_map(|entry| {
            let StatePayload::LazyBoss { at_boost_bits } = &entry.payload else {
                return None;
            };
            Some(f64::from_bits(*at_boost_bits))
        })
    }

    pub fn set_lazy_boss_at_boost(&mut self, boss: EntityIdx, at_boost: f64) {
        let boss_entity = self
            .entities
            .get_mut(boss)
            .unwrap_or_else(|| panic!("unknown runtime lazy boss entity: {}", boss.0));
        assert!(
            boss_entity.states.set_payload(
                PLAIN_LAZY_BOSS_STATE_KEY,
                StatePayload::LazyBoss {
                    at_boost_bits: at_boost.to_bits(),
                },
            ),
            "runtime lazy boss state disappeared"
        );
    }

    pub fn has_lazy_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(&entry.payload, StatePayload::LazyInfection { .. }))
        })
    }

    pub fn infect_with_lazy_into(&mut self, boss: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        if target == boss || self.has_lazy_infection(target) {
            return false;
        }
        let state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT)
            .expect("default runtime profile must register lazy infection state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime lazy infection target: {}", target.0));
        if !target_entity.states.add_entry(StateEntry::lazy_infection(
            PLAIN_LAZY_INFECTION_STATE_KEY,
            state_id,
            boss,
            SkillPriority(1000),
        )) {
            return false;
        }
        let boss_display = self
            .entities
            .get(boss)
            .unwrap_or_else(|| panic!("unknown runtime lazy boss entity: {}", boss.0))
            .template
            .display_name
            .clone();
        updates.add(crate::runtime::update::RunUpdate::new(
            format!("[1]感染了{boss_display}"),
            boss.0 as usize,
            target.0 as usize,
            0,
        ));
        true
    }

    pub fn emit_lazy_activity_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let activity = match self.rng.next_u8() {
            0..=49 => "Steam",
            50..=99 => "守望先锋",
            100..=149 => "文明6",
            150..=189 => "英雄联盟",
            190..=229 => "微博",
            _ => "朋友圈",
        };
        let owner_name = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime lazy activity owner: {}", owner.0))
            .template
            .display_name
            .clone();
        updates.add(crate::runtime::update::RunUpdate::new(
            format!("{owner_name}打开了{activity}, 这回合什么也没做"),
            owner.0 as usize,
            owner.0 as usize,
            0,
        ));
    }

    pub fn try_covid_spread_on_damage_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        damage: i32,
        updates: &mut RunUpdates,
    ) {
        if self.has_covid_infection(target) {
            return;
        }
        if i32::from(self.rng.next_u8() & 63) < damage {
            self.infect_with_covid_into(boss, target, mutation, updates);
        }
    }

    pub fn infect_with_covid_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        if target == boss {
            return false;
        }
        let boss_display = self
            .entities
            .get(boss)
            .unwrap_or_else(|| panic!("unknown runtime covid boss entity: {}", boss.0))
            .template
            .display_name
            .clone();
        let infection_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT)
            .expect("default runtime profile must register covid infection state");

        let infected = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime covid target entity: {}", target.0));
            if let Some(entry) = target_entity.states.entry_mut(PLAIN_COVID_INFECTION_STATE_KEY) {
                let StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                } = &mut entry.payload
                else {
                    panic!("runtime covid infection key is occupied by a different state");
                };
                if !*recovered || mutation_set.contains(&mutation) {
                    false
                } else {
                    *recovered = false;
                    entries.push(CovidInfectionEntry { boss, mutation, days: 0 });
                    mutation_set.push(mutation);
                    true
                }
            } else {
                target_entity.states.add_entry(StateEntry::covid_infection(
                    PLAIN_COVID_INFECTION_STATE_KEY,
                    infection_state,
                    boss,
                    mutation,
                    SkillPriority(1000),
                ))
            }
        };
        if !infected {
            return false;
        }

        updates.add(crate::runtime::update::RunUpdate::new(
            format!("[1]感染了{boss_display}"),
            boss.0 as usize,
            target.0 as usize,
            0,
        ));
        let all_alive = self.world.flat_alive().to_vec();
        for entity_idx in all_alive {
            let delta = if entity_idx == target { 2048 } else { -256 };
            self.entities
                .get_mut(entity_idx)
                .unwrap_or_else(|| panic!("runtime covid alive entity disappeared: {}", entity_idx.0))
                .runtime
                .move_state
                .speed_points += delta;
        }
        true
    }

    pub fn drain_covid_pneumonia_into(&mut self, owner: EntityIdx, boss: EntityIdx, mutation: i32, updates: &mut RunUpdates) {
        if !self.entities.get(owner).is_some_and(|entity| entity.runtime.alive) {
            return;
        }
        let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
        let atp = self.entities.get(owner).unwrap().runtime.get_at(true, &mut self.rng);
        let defense = self.entities.get(owner).unwrap().runtime.magic_defense();
        let damage = ((atp + f64::from(mutation * 80)) / f64::from(defense)).ceil() as i32;
        if damage <= 0 {
            return;
        }

        updates.add(crate::runtime::update::RunUpdate::new(
            format!(" {owner_name}肺炎发作"),
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        let old_hp = self.entities.get(owner).unwrap().runtime.hp;
        let killed = self.apply_plain_attack_damage_with_covid_into(boss, owner, damage, None, updates);
        let actual_damage = if killed { old_hp } else { damage };
        if killed {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
        }

        let boss_hp_full = {
            let boss_entity = self.entities.get(boss).unwrap();
            boss_entity.runtime.hp >= boss_entity.template.max_hp
        };
        let heal_amount = if boss_hp_full {
            ((damage >> 3) + 1).min(actual_damage)
        } else {
            (damage >> 1).min(actual_damage)
        };
        if heal_amount <= 0 {
            return;
        }
        let boss_entity = self.entities.get_mut(boss).unwrap();
        boss_entity.runtime.hp = (boss_entity.runtime.hp + heal_amount).min(boss_entity.template.max_hp);
        let boss_display = boss_entity.template.display_name.clone();
        updates.add(crate::runtime::update::RunUpdate::new(
            format!("{boss_display}回复体力{heal_amount}点"),
            boss.0 as usize,
            boss.0 as usize,
            0,
        ));
    }

    pub fn drain_lazy_flare_into(&mut self, owner: EntityIdx, boss: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(owner).is_some_and(|entity| entity.runtime.alive)
            || !self.entities.get(boss).is_some_and(|entity| entity.runtime.alive)
        {
            return;
        }
        let boss_atp = self.entities.get(boss).unwrap().runtime.get_at(true, &mut self.rng);
        let target_defense = self.entities.get(owner).unwrap().runtime.magic_defense();
        let damage = (boss_atp / f64::from(target_defense)).ceil() as i32;
        if damage <= 0 {
            return;
        }
        let boss_display = self.entities.get(boss).unwrap().template.display_name.clone();
        let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
        updates.add(crate::runtime::update::RunUpdate::new(
            format!(" {owner_name}{boss_display}发作"),
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        if self.apply_plain_attack_damage_with_covid_into(boss, owner, damage, None, updates) {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
        }
    }

    pub fn drain_plain_lethal_damage_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.emit_plain_lethal_replay_into(caster, target, updates);
        self.drain_die_hooks_into(target, updates);

        let (hp, team) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hp, entity.runtime.team))
            .unwrap_or_else(|| panic!("runtime lethal target disappeared: {}", target.0));
        if hp > 0 {
            return;
        }

        self.entities.get_mut(target).unwrap().runtime.alive = false;
        self.mark_dead_with_linked_minions_into(target, team, updates);
        if self.should_run_kill_hooks(caster, target) {
            self.drain_kill_hooks_into(caster, target, updates);
        }
    }

    pub fn emit_plain_lethal_replay_into(&self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime lethal replay target: {}", target.0));
        let is_combat_minion = target_entity.runtime.is_combat_minion();
        if is_combat_minion
            && !self
                .entities
                .iter()
                .any(|(_, entity)| entity.runtime.alive && entity.runtime.team != target_entity.runtime.team)
        {
            // legacy 在战斗已结束时抑制战斗召唤物自身的死亡日志；典型路径是使魔自爆击倒最后敌人。
            return;
        }
        let die_message = if is_combat_minion { "[1]消失了" } else { "[1]被击倒了" };
        updates.add_newline();
        updates.add(crate::runtime::update::RunUpdate::new(
            die_message,
            caster.0 as usize,
            target.0 as usize,
            50,
        ));
    }

    pub fn recover_plain_actor_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let recover_threshold = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime recovery actor: {}", actor.0))
            .runtime
            .wisdom
            + 64;
        if (self.rng.r127() as i32) < recover_threshold {
            self.entities.get_mut(actor).unwrap().runtime.magic_point += 16;
        }
        updates.add_newline();
    }
}

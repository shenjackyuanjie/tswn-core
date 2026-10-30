use super::*;

/// `on_update_end` 排空时要处理的反击寄存器；玄月奖杯与普通反击分开保存。
#[derive(Debug, Clone, Copy)]
enum PlainUpdateEndCounter {
    Counter,
    XuanYue,
}

impl CombatRuntime {
    pub fn clear_plain_hide_before_action(&mut self, actor: EntityIdx) {
        let Some(actor) = self.entities.get_mut(actor) else {
            return;
        };
        if actor.runtime.hide.take().is_none() {
            return;
        }
        // legacy 的 Hide.pre_action 会在清除隐匿标记后调用 update_states，同时同步疾走等待生效状态。
        actor.refresh_runtime_stats_from_template();
    }

    pub fn drain_plain_post_damage_skill_chain_into(
        &mut self,
        target: EntityIdx,
        damage: i32,
        caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        if damage <= 0 {
            return;
        }

        #[derive(Debug, Clone, Copy)]
        enum PlainPostDamageSkill {
            Upgrade,
            Hide,
            Counter,
            Assassinate,
            SummonShareDamage,
            WeaponDeathNote,
            WeaponXuanYue,
        }

        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime post-damage target: {}", target.0));
        let mut plan = target_entity
            .template
            .skills
            .post_damage_order()
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(registration_order, fixed_lane)| {
                let skill_id = *target_entity.template.skills.skills().get(fixed_lane)?;
                let export_name = self.registry.skill(skill_id)?.export_name.as_str();
                let skill = match export_name {
                    DEFAULT_CORE_UPGRADE_SKILL_EXPORT => PlainPostDamageSkill::Upgrade,
                    DEFAULT_CORE_HIDE_SKILL_EXPORT => PlainPostDamageSkill::Hide,
                    DEFAULT_CORE_COUNTER_SKILL_EXPORT => PlainPostDamageSkill::Counter,
                    export_name if export_name == BuiltinActiveSkill::Assassinate.export_name() => {
                        PlainPostDamageSkill::Assassinate
                    }
                    DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT => PlainPostDamageSkill::SummonShareDamage,
                    DEFAULT_CORE_WEAPON_DEATH_NOTE_SKILL_EXPORT => PlainPostDamageSkill::WeaponDeathNote,
                    DEFAULT_CORE_WEAPON_XUAN_YUE_COUNTER_SKILL_EXPORT => PlainPostDamageSkill::WeaponXuanYue,
                    _ => return None,
                };
                let level = target_entity.template.skills.level_at(fixed_lane)?;
                if level == 0 {
                    return None;
                }
                let priority = match skill {
                    // 死亡笔记包在 PostDamageImpl 里，产物按 Infinity 排到静态钩子之后；
                    // 影袭是运行期动态注册的 Infinity 钩子，仍排在死亡笔记之后。
                    PlainPostDamageSkill::WeaponDeathNote => i32::MAX - 1,
                    PlainPostDamageSkill::Assassinate => i32::MAX,
                    _ => 0,
                };
                Some((skill, level, priority, registration_order))
            })
            .collect::<smallvec::SmallVec<[_; 4]>>();
        // `post_damage_order` 保存钩子的实际注册顺序。Merge 可能在运行期启用
        // 原本为零级的槽位，因此固定槽位顺序和主动行动顺序都不足以复现该事件列表。
        plan.sort_by_key(|(_, _, priority, registration_order)| (*priority, *registration_order));
        #[cfg(not(feature = "no_debug"))]
        let debug_counter = crate::debug::probe_counter().is_some();
        #[cfg(not(feature = "no_debug"))]
        if debug_counter {
            eprintln!(
                "[counter_probe:runtime:plan] target={} caster={} damage={} updates_id={} plan={:?} rc4=({}, {})",
                target.0, caster.0, damage, updates.id, plan, self.rng.i, self.rng.j,
            );
        }

        for (skill, level, _, _) in plan {
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
                PlainPostDamageSkill::WeaponDeathNote => {
                    self.run_plain_weapon_death_note_post_damage_into(target, damage, caster);
                }
                PlainPostDamageSkill::WeaponXuanYue => {
                    self.run_plain_weapon_xuan_yue_post_damage_into(target, caster, updates);
                }
            }
            #[cfg(not(feature = "no_debug"))]
            if debug_counter {
                eprintln!(
                    "[counter_probe:runtime:skill] target={} caster={} skill={:?} level={} updates_id={} rc4=({}, {}) -> ({}, {})",
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
            .unwrap_or_else(|| panic!("unknown runtime counter owner: {}", target.0))
            .runtime
            .wisdom
            .clamp(0, 127) as u32;
        let owner_ally_team = self.plain_effective_team(target);
        let caster_team = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime counter caster: {}", caster.0))
            .runtime
            .team;
        if owner_ally_team == caster_team && self.rng.r63() < owner_wisdom {
            return;
        }

        let counter = &mut self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime counter owner disappeared: {}", target.0))
            .runtime
            .counter;
        #[cfg(not(feature = "no_debug"))]
        if crate::debug::probe_counter().is_some() {
            eprintln!(
                "[counter_probe:runtime:state] target={} caster={} updates_id={} last_updates_id={:?} pending={} last_target={:?}",
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

    /// 0.5.5 `SklDeathNote.aD`：被命中后按闪避判定把攻击者写进死亡笔记。
    pub fn run_plain_weapon_death_note_post_damage_into(&mut self, owner: EntityIdx, damage: i32, caster: EntityIdx) {
        if damage <= 0 || caster == owner {
            return;
        }
        let accuracy = {
            let caster_entity = self
                .entities
                .get(caster)
                .unwrap_or_else(|| panic!("unknown runtime death note caster: {}", caster.0));
            caster_entity.runtime.wisdom + caster_entity.runtime.resistance
        };
        let dodge_value = {
            let owner_entity = self
                .entities
                .get(owner)
                .unwrap_or_else(|| panic!("unknown runtime death note owner: {}", owner.0));
            owner_entity.runtime.wisdom + owner_entity.runtime.magic
        };
        if PlayerRuntime::dodge(accuracy, dodge_value, &mut self.rng) {
            self.entities
                .get_mut(owner)
                .unwrap_or_else(|| panic!("runtime death note owner disappeared: {}", owner.0))
                .runtime
                .death_note_mark = Some(caster);
        }
    }

    /// 0.5.5 `SklCounter.hc.aD`：玄月奖杯只登记本批次的攻击者，不掷骰。
    pub fn run_plain_weapon_xuan_yue_post_damage_into(&mut self, owner: EntityIdx, caster: EntityIdx, updates: &mut RunUpdates) {
        let owner_ally_team = self.plain_effective_team(owner);
        let caster_team = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime xuan yue caster: {}", caster.0))
            .runtime
            .team;
        if owner_ally_team == caster_team {
            return;
        }
        let counter = &mut self
            .entities
            .get_mut(owner)
            .unwrap_or_else(|| panic!("runtime xuan yue owner disappeared: {}", owner.0))
            .runtime
            .counter_xuan_yue;
        if counter.last_updates_id == Some(updates.id) {
            // 同一批次内后来的攻击者会覆盖待执行的反击目标。
            if counter.pending && counter.last_target != Some(caster) {
                counter.last_target = Some(caster);
            }
            return;
        }
        counter.last_updates_id = Some(updates.id);
        counter.last_target = Some(caster);
        counter.pending = true;
        updates.on_update_end.push(owner.0 as usize);
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
        for _level in self.plain_update_end_counter_levels(owner, DEFAULT_CORE_COUNTER_SKILL_EXPORT) {
            self.run_plain_update_end_counter_attack_into(owner, PlainUpdateEndCounter::Counter, updates);
        }
        // 普通反击可能间接登记玄月，因此在它结算后重新读取；没有本批次待发反击时无需扫技能表。
        let xuan_yue_pending = self.entities.get(owner).is_some_and(|entity| {
            let counter = &entity.runtime.counter_xuan_yue;
            counter.pending && counter.last_updates_id == Some(updates.id)
        });
        if xuan_yue_pending {
            for _level in self.plain_update_end_counter_levels(owner, DEFAULT_CORE_WEAPON_XUAN_YUE_COUNTER_SKILL_EXPORT) {
                self.run_plain_update_end_counter_attack_into(owner, PlainUpdateEndCounter::XuanYue, updates);
            }
        }
    }

    fn plain_update_end_counter_levels(&self, owner: EntityIdx, export_name: &str) -> Vec<u32> {
        self.entities
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
                        (self.registry.skill(skill_id)?.export_name == export_name)
                            .then(|| entity.template.skills.level_at(fixed_lane))
                            .flatten()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    fn run_plain_update_end_counter_attack_into(
        &mut self,
        owner: EntityIdx,
        kind: PlainUpdateEndCounter,
        updates: &mut RunUpdates,
    ) {
        let target = {
            let Some(owner_entity) = self.entities.get_mut(owner) else {
                return;
            };
            let counter = match kind {
                PlainUpdateEndCounter::Counter => &mut owner_entity.runtime.counter,
                PlainUpdateEndCounter::XuanYue => &mut owner_entity.runtime.counter_xuan_yue,
            };
            if !counter.pending || counter.last_updates_id != Some(updates.id) {
                return;
            }
            counter.pending = false;
            counter.last_updates_id = None;
            counter.last_target.take()
        };
        let Some(target) = target else {
            return;
        };
        // legacy 这里只检查反击目标的 alive 标记；冰冻目标仍然可以被反击。
        if !self.entities.get(target).is_some_and(|entity| entity.runtime.alive) {
            return;
        }
        if !self.entities.get(owner).is_some_and(EntityRecord::is_active) {
            return;
        }

        if !self.entity_mp_ready(owner) {
            return;
        }
        let atp = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("runtime counter owner disappeared: {}", owner.0))
            .runtime
            .get_at(false, &mut self.rng);
        updates.add_newline();
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]发起[反击][s_counter]",
            owner.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_into(owner, target, false, atp, updates);
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
            .unwrap_or_else(|| panic!("unknown runtime upgrade target: {}", target.0));
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
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]做出[垂死]抗争",
            target.0 as usize,
            target.0 as usize,
            60,
        ));
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]所有属性上升",
            target.0 as usize,
            target.0 as usize,
            0,
        ));
        let target = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime upgrade target disappeared: {}", target.0));
        assert!(target.activate_upgrade_runtime(), "runtime upgrade activated twice");
    }

    pub fn run_plain_hide_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        _caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        #[cfg(not(feature = "no_debug"))]
        let probe_hide = crate::debug::probe_hide().is_some();
        let (already_active, owner_active) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hide.is_some(), entity.is_active()))
            .unwrap_or_else(|| panic!("unknown runtime hide target: {}", target.0));
        if level == 0 || already_active || !owner_active {
            #[cfg(not(feature = "no_debug"))]
            if probe_hide {
                eprintln!(
                    "[hide_probe:runtime:skip] target={} level={} already_active={} owner_active={} rc4=({}, {})",
                    target.0, level, already_active, owner_active, self.rng.i, self.rng.j,
                );
            }
            return;
        }
        let effective_team = self.plain_effective_team(target);
        let alive_allies = self.world.team_alive(effective_team).map_or(0, |team| {
            team.iter()
                .filter(|ally| self.entities.get(**ally).is_some_and(|entity| entity.runtime.alive))
                .count()
        });
        if alive_allies <= 1 {
            #[cfg(not(feature = "no_debug"))]
            if probe_hide {
                eprintln!(
                    "[hide_probe:runtime:alone] target={} team={} level={} alive_allies={} rc4=({}, {})",
                    target.0, effective_team, level, alive_allies, self.rng.i, self.rng.j,
                );
            }
            return;
        }
        #[cfg(not(feature = "no_debug"))]
        let rng_before = (self.rng.i, self.rng.j);
        let roll = self.rng.r63();
        #[cfg(not(feature = "no_debug"))]
        if probe_hide {
            eprintln!(
                "[hide_probe:runtime:roll] target={} team={} level={} alive_allies={} roll={} rc4=({}, {}) -> ({}, {})",
                target.0, effective_team, level, alive_allies, roll, rng_before.0, rng_before.1, self.rng.i, self.rng.j,
            );
        }
        if roll >= level {
            return;
        }

        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime hide target disappeared: {}", target.0));
        target_entity.runtime.hide = Some(HideRuntime {
            level,
            attract_bits: target_entity.runtime.attract_bits,
            agility: target_entity.runtime.agility,
            defense: target_entity.runtime.defense,
            resistance: target_entity.runtime.resistance,
        });
        // legacy 激活隐匿后会调用 update_states，因此必须连同疾走等状态一起重算，而不是只局部改隐匿属性。
        target_entity.refresh_runtime_stats_from_template();
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]发动[隐匿]",
            target.0 as usize,
            target.0 as usize,
            10,
        ));
    }
}

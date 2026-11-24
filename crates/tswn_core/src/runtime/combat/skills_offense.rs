use super::*;

impl CombatRuntime {
    /// 按选中的内置主动技能分派到对应技能实现；能力作用域限定于实际施法者。
    pub fn drain_plain_builtin_skill_into(
        &mut self,
        actor: EntityIdx,
        prepared: PreparedBuiltinSkillAction,
        updates: &mut RunUpdates,
    ) {
        // 效果倍率限定于实际施法者的能力作用域。
        #[cfg(feature = "cards")]
        let _ability = self.ability_scope(actor, prepared.selected.skill.export_name());
        match prepared.selected.skill {
            BuiltinActiveSkill::Fire => {
                let target = prepared.targets[0];
                self.drain_plain_fire_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Thunder => {
                let target = prepared.targets[0];
                self.drain_plain_thunder_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Quake => {
                self.drain_plain_quake_skill_into(actor, prepared.targets, updates);
            }
            BuiltinActiveSkill::Absorb => {
                let target = prepared.targets[0];
                self.drain_plain_absorb_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Poison => {
                let target = prepared.targets[0];
                self.drain_plain_poison_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Critical => {
                let target = prepared.targets[0];
                self.drain_plain_critical_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Berserk => {
                let target = prepared.targets[0];
                self.drain_plain_berserk_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Ice => {
                let target = prepared.targets[0];
                self.drain_plain_ice_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Rapid => {
                self.drain_plain_rapid_skill_into(actor, prepared.targets, updates);
            }
            BuiltinActiveSkill::Half => {
                let target = prepared.targets[0];
                self.drain_plain_half_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Curse => {
                let target = prepared.targets[0];
                self.drain_plain_curse_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Haste => {
                let target = prepared.targets[0];
                self.drain_plain_haste_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Heal => {
                let target = prepared.targets[0];
                self.drain_plain_heal_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Shadow => {
                self.drain_plain_shadow_skill_into(actor, prepared.selected.fixed_lane, updates);
            }
            BuiltinActiveSkill::Charm => {
                let target = prepared.targets[0];
                self.drain_plain_charm_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Slow => {
                let target = prepared.targets[0];
                self.drain_plain_slow_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Exchange => {
                let target = prepared.targets[0];
                self.drain_plain_exchange_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Revive => {
                let target = prepared.targets[0];
                self.drain_plain_revive_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Disperse => {
                let target = prepared.targets[0];
                self.drain_plain_disperse_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Iron => {
                self.drain_plain_iron_skill_into(actor, updates);
            }
            BuiltinActiveSkill::Clone => {
                self.drain_plain_clone_skill_into(actor, prepared.selected.fixed_lane, updates);
            }
            BuiltinActiveSkill::Charge => {
                self.drain_plain_charge_skill_into(actor, updates);
            }
            BuiltinActiveSkill::Accumulate => {
                self.drain_plain_accumulate_skill_into(actor, updates);
            }
            BuiltinActiveSkill::Assassinate => {
                let target = prepared.targets.first().copied().or_else(|| {
                    self.entities
                        .get(actor)
                        .and_then(|entity| entity.runtime.assassinate)
                        .map(|pending| pending.target)
                });
                let target = target.expect("runtime assassinate action is missing its selected or pending target");
                self.drain_plain_assassinate_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Summon => {
                self.drain_plain_summon_skill_into(actor, updates);
            }
            BuiltinActiveSkill::SummonExplode => {
                let target = prepared.targets[0];
                self.drain_plain_summon_explode_into(actor, target, updates);
            }
            BuiltinActiveSkill::Possess => {
                let target = prepared.targets[0];
                self.drain_plain_possess_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::WeaponS11 => {
                self.drain_plain_weapon_s11_skill_into(actor, updates);
            }
            BuiltinActiveSkill::WeaponDeathNote => {
                self.drain_plain_weapon_death_note_skill_into(actor, updates);
            }
            BuiltinActiveSkill::RinickIceAge => {
                self.drain_plain_boss_ice_age_into(actor, prepared.targets, updates);
            }
            BuiltinActiveSkill::RinickYuriControl => {
                let target = prepared.targets[0];
                self.drain_plain_yuri_control_into(actor, target, updates);
            }
            BuiltinActiveSkill::WeaponXuanYue => {
                unreachable!("runtime weapon xuan-yue skill never enters the action scan");
            }
        }
    }

    /// 默认选敌：按聪明与否的配额在存活敌人中抽取并评分排序。
    pub fn select_plain_default_enemy_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 3 } else { 2 })
    }

    /// 默认选敌并显式指定抽取配额；配额为 0 时返回空列表。
    pub fn select_plain_default_enemy_targets_with_count(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        select_count: usize,
    ) -> PreparedTargetList {
        if select_count == 0 {
            return PreparedTargetList::new();
        }
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        if all_alive.is_empty() {
            return PreparedTargetList::new();
        }
        // 被子有存活召唤物时不可被选中：单独挑目标，避免破坏原版连续跳过区间。
        if all_alive.iter().any(|idx| self.quilt_untargetable(*idx)) {
            let candidates: Vec<EntityIdx> = all_alive
                .iter()
                .copied()
                .filter(|idx| self.plain_effective_team(*idx) != actor_team && !self.quilt_untargetable(*idx))
                .collect();
            return candidates.into_iter().take(select_count).collect();
        }
        let ally_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                self.entities
                    .get(*candidate)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<crate::rc4::PickSkipRange>();
        #[cfg(feature = "no_debug")]
        if select_count <= 6 {
            if let Some(index) = ally_skip_indices.single_remaining_index(all_alive.len()) {
                // 配额为一时首次抽中即结束。否则抽中一次后会再重复配额加一次。
                if all_alive.len() > 1 {
                    match select_count {
                        1 => self.rng.discard_bytes::<2>(),
                        2 => self.rng.discard_bytes::<8>(),
                        3 => self.rng.discard_bytes::<10>(),
                        4 => self.rng.discard_bytes::<12>(),
                        5 => self.rng.discard_bytes::<14>(),
                        6 => self.rng.discard_bytes::<16>(),
                        _ => unreachable!("零目标配额应提前返回"),
                    }
                }
                let target = all_alive[index];
                self.entities.get(target).unwrap();
                if !smart {
                    self.rng.discard_bytes::<2>();
                }
                return smallvec::smallvec![target];
            }
            if select_count >= 2
                && self.world.alive_group_count() <= 2
                && let Some([left, right]) = ally_skip_indices.two_remaining_indices(all_alive.len())
                && all_alive[left] != all_alive[right]
            {
                let (first, second) = self.rank_two_plain_enemy_targets([all_alive[left], all_alive[right]], smart, select_count);
                let mut targets = smallvec::smallvec![first];
                targets.extend(second);
                return targets;
            }
        }
        let mut selected = smallvec::SmallVec::<[EntityIdx; 8]>::new();
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = self.rng.pick_skip_span(all_alive, ally_skip_indices);
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return PreparedTargetList::new();
        }
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_default_enemy_target(target, smart)))
            .collect::<smallvec::SmallVec<[(EntityIdx, f64); 8]>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    /// 默认选敌评分：聪明按血量与吸引力，普通掷随机加吸引力。
    pub fn score_plain_default_enemy_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime default enemy target: {}", target.0));
        if smart {
            Self::plain_smart_attack_score(&self.world, target, &entity.runtime)
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    /// 火球术：队列为空时直接结算火焰攻击，否则入队后统一结算。
    pub fn drain_plain_fire_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        if self.effects.is_empty() {
            self.drain_fire_attack_effect_into(actor, target, PLAIN_FIRE_STATE_KEY, updates);
            self.drain_effects_into(updates);
            return;
        }
        self.effects.push(QueuedEffect::FireAttack {
            caster: actor,
            target,
            fire_state_key: PLAIN_FIRE_STATE_KEY,
        });
        self.drain_effects_into(updates);
    }

    /// 雷击术：3 到 6 段攻击，每段命中递减 10，威力按 0.36 折算。
    pub fn drain_plain_thunder_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[雷击术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let mut accuracy = 100
            + self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime thunder actor: {}", actor.0))
                .runtime
                .agility;
        let count = 3 + self.rng.r3() as usize;
        self.control.begin_thunder(actor.0 as PlrId, target.0 as PlrId);
        let mut previous_level = None;
        for hit in 0..count {
            let actor_active = self.entities.get(actor).is_some_and(EntityRecord::is_active);
            let target_alive = self.entities.get(target).is_some_and(|entity| entity.runtime.alive);
            if !actor_active || !target_alive {
                continue;
            }

            updates.add_newline();
            let (target_active, target_dodge) = {
                let target_entity = self
                    .entities
                    .get(target)
                    .unwrap_or_else(|| panic!("unknown runtime thunder target: {}", target.0));
                (
                    target_entity.is_active(),
                    target_entity.runtime.agility + target_entity.runtime.resistance,
                )
            };
            self.control.set_thunder_hit(hit as u32 + 1, true, previous_level);
            let dodged =
                target_active && self.roll_plain_dodge_with_updates(target, accuracy, target_dodge, false, Some(updates));
            previous_level = Some(u32::from(Self::plain_dodge_chance(accuracy, target_dodge)));
            self.control.set_thunder_hit(hit as u32 + 1, false, previous_level);
            if dodged {
                updates.add(RuntimeFrame::replay_update(
                    target.0 as usize,
                    actor.0 as usize,
                    "[0][回避]了攻击",
                    0,
                ));
                break;
            }

            accuracy -= 10;
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime thunder actor: {}", actor.0))
                .runtime
                .get_at(true, &mut self.rng)
                * 0.36000001430511475;
            let update_pos = updates.updates.len();
            self.drain_plain_attack_from_defense_into(actor, target, true, atp, updates);
            if let Some(update) = updates.updates.get_mut(update_pos) {
                update.delay0 = 300;
            }
        }
        self.control.finish_thunder();
    }

    /// 地裂术：随机取 4 或 5 个目标，威力按目标数分摊。
    pub fn drain_plain_quake_skill_into(&mut self, actor: EntityIdx, mut targets: PreparedTargetList, updates: &mut RunUpdates) {
        if targets.is_empty() {
            return;
        }
        let round = if self.rng.c50() { 5 } else { 4 };
        targets.truncate(round.min(targets.len()));
        if targets.is_empty() {
            return;
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[地裂术]",
            actor.0 as usize,
            targets[0].0 as usize,
            1,
        ));
        let divisor = targets.len() as f64 + 0.6000000238418579;
        for target in targets {
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime quake actor: {}", actor.0))
                .runtime
                .get_at(true, &mut self.rng)
                * 2.440000057220459
                / divisor;
            let target_alive = self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0);
            if !target_alive {
                continue;
            }

            updates.add_newline();
            self.drain_plain_attack_with_atp_into(actor, target, true, atp, updates);
            if self.world.sync_winner(&self.entities).is_some() {
                break;
            }
        }
    }

    /// 吸血攻击：1.3 倍魔法威力并附加吸血效果。
    pub fn drain_plain_absorb_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime absorb actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 1.2999999523162842;
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]发起[吸血攻击]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(
            actor,
            target,
            true,
            atp,
            DamageKind::Default,
            PlainAttackOnDamage::Absorb,
            updates,
        );
    }

    /// 投毒：按魔法威力攻击并附加中毒效果。
    pub fn drain_plain_poison_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime poison actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0][投毒]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(
            actor,
            target,
            true,
            atp,
            DamageKind::Default,
            PlainAttackOnDamage::Poison,
            updates,
        );
    }

    /// 会心一击：三段物理取样取最高值攻击。
    pub fn drain_plain_critical_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let actor_runtime = &mut self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime critical actor: {}", actor.0))
            .runtime;
        let atp0 = actor_runtime.get_at(false, &mut self.rng) * 1.149999976158142;
        let atp1 = actor_runtime.get_at(false, &mut self.rng) * 1.2000000476837158;
        let atp2 = actor_runtime.get_at(false, &mut self.rng) * 1.25;
        let atp = atp0.max(atp1).max(atp2);
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]发动[会心一击]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
    }

    /// 狂暴选目标：聪明时排除已狂暴目标与战斗使魔。
    pub fn select_plain_berserk_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        self.select_plain_filtered_enemy_targets(
            actor,
            smart,
            |target, smart| {
                target.is_some_and(|entity| {
                    !smart
                        || (!entity
                            .states
                            .entries()
                            .iter()
                            .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
                            && !entity.runtime.is_combat_minion())
                })
            },
            Self::score_plain_berserk_target,
        )
    }

    /// 狂暴选目标评分：已狂暴或已魅惑的目标评分打折。
    pub fn score_plain_berserk_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let mut score = self.score_plain_default_enemy_target(target, smart);
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime berserk target: {}", target.0));
        if target_entity
            .states
            .entries()
            .iter()
            .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. } | StatePayload::Charm { .. }))
        {
            score /= 1.2000000476837158;
        }
        score
    }

    /// 狂暴术：魔法威力攻击并附加狂暴效果。
    pub fn drain_plain_berserk_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime berserk actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[狂暴术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(
            actor,
            target,
            true,
            atp,
            DamageKind::Default,
            PlainAttackOnDamage::Berserk,
            updates,
        );
    }

    /// 加速选目标：同队存活成员，聪明时有体力下限与状态重叠限制。
    pub fn select_plain_haste_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let candidates = self.world.team_alive(actor_team).unwrap_or_default();
        if candidates.is_empty() {
            return PreparedTargetList::new();
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut duplicate_count = 0usize;
        let mut invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let Some(picked) = self.rng.pick(candidates) else {
                return PreparedTargetList::new();
            };
            let target = candidates[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                if !smart {
                    return true;
                }
                entity.runtime.hp >= 60
                    && entity
                        .states
                        .entry(PLAIN_HASTE_STATE_KEY)
                        .and_then(StateEntry::haste_value)
                        .is_none_or(|(_, step)| (step + 1) * 60 <= entity.runtime.hp)
                    && !entity.runtime.is_combat_minion()
            });
            if !valid {
                invalid_count += 1;
                continue;
            }
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_haste_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    /// 加速选目标评分：已疾走的目标评分减为四分之一。
    pub fn score_plain_haste_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if !smart {
            return self.rng.rFFFF() as f64;
        }
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime haste target: {}", target.0));
        let hp = entity.runtime.hp;
        let rate_hi_hp = if hp < 20 {
            30.0
        } else if hp > 300 {
            300.0
        } else {
            hp as f64
        };
        let mut score = rate_hi_hp * entity.runtime.attr_sum as f64;
        if entity.states.entry(PLAIN_HASTE_STATE_KEY).is_some() {
            score /= 4.0;
        }
        score
    }

    /// 加速术：推进施法者行动点，并给目标附加或延长疾走状态。
    pub fn drain_plain_haste_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[加速术]",
            actor.0 as usize,
            target.0 as usize,
            60,
        ));
        let (charge_active, owner_speed) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime haste actor: {}", actor.0));
            (owner.runtime.charge.active, owner.effective_speed())
        };
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime haste actor: {}", actor.0))
            .runtime
            .move_state
            .speed_points += owner_speed;

        let haste_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT)
            .expect("default runtime profile must register core haste state");
        let haste_priority = self
            .registry
            .state(haste_state_id)
            .expect("default runtime core haste state disappeared")
            .priority;
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime haste target: {}", target.0));
        if let Some((mut faster, effective_faster, mut step)) = target_entity
            .states
            .entry(PLAIN_HASTE_STATE_KEY)
            .and_then(StateEntry::haste_runtime_value)
        {
            step += 2;
            if charge_active {
                faster += 2;
                step += 2;
            }
            assert!(
                target_entity.states.set_payload(
                    PLAIN_HASTE_STATE_KEY,
                    StatePayload::Haste {
                        faster,
                        effective_faster,
                        step,
                    },
                ),
                "runtime haste state disappeared during extension"
            );
        } else {
            target_entity.refresh_runtime_stats_from_template();
            assert!(
                target_entity.states.add_entry(StateEntry::haste_with_effective_faster(
                    PLAIN_HASTE_STATE_KEY,
                    haste_state_id,
                    if charge_active { 4 } else { 2 },
                    2,
                    if charge_active { 5 } else { 3 },
                    haste_priority,
                )),
                "runtime haste state should be inserted"
            );
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[1]进入[疾走]状态",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    /// 铁壁：按魔法与蓄力计算保护值与步数，并扣除 256 行动点。
    pub fn drain_plain_iron_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime iron actor: {}", actor.0));
            (owner.runtime.magic, owner.runtime.charge.active)
        };
        let step = 3 + if charge_active { 4 } else { 0 };
        let protect = 110 + magic + if charge_active { 240 + magic * 4 } else { 0 };
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]发动[铁壁]",
            actor.0 as usize,
            actor.0 as usize,
            60,
        ));

        let iron_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_IRON_STATE_EXPORT)
            .expect("default runtime profile must register core iron state");
        let iron_priority = self
            .registry
            .state(iron_state_id)
            .expect("default runtime core iron state disappeared")
            .priority;
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime iron actor: {}", actor.0));
        if owner.states.entry(PLAIN_IRON_STATE_KEY).is_some() {
            assert!(
                owner.states.set_payload(PLAIN_IRON_STATE_KEY, StatePayload::Iron { protect, step }),
                "runtime iron state disappeared during replacement"
            );
        } else {
            assert!(
                owner.states.add_entry(StateEntry::iron(
                    PLAIN_IRON_STATE_KEY,
                    iron_state_id,
                    protect,
                    step,
                    iron_priority,
                )),
                "runtime iron state should be inserted"
            );
        }
        owner.runtime.move_state.speed_points -= 256;
        owner.refresh_runtime_stats_from_template();
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]防御力大幅上升",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    /// 连击选目标：聪明抽 5 个、普通抽 3 个敌人并按评分排序。
    pub fn select_plain_rapid_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        if all_alive.is_empty() {
            return PreparedTargetList::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<crate::rc4::PickSkipRange>();
        let select_count = if smart { 5 } else { 3 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 5]>::new();
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = self.rng.pick_skip_span(all_alive, enemy_skip_indices);
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return PreparedTargetList::new();
        }

        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 5]>::new();
        for target in selected {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("runtime rapid target disappeared: {}", target.0));
            let score = if smart {
                let hp = if target_entity.runtime.hp < 20 {
                    30
                } else if target_entity.runtime.hp > 300 {
                    300
                } else {
                    target_entity.runtime.hp
                };
                if self.world.alive_group_count() > 2 {
                    hp as f64 * self.world.alive_group_len_containing(target) as f64 * target_entity.runtime.attract()
                } else {
                    (1.0 / hp as f64) * target_entity.runtime.atk_sum as f64 * target_entity.runtime.attract()
                }
            } else {
                self.rng.rFFFF() as f64 + target_entity.runtime.attract()
            };
            scored.push((target, score));
        }
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    /// 连击：2 或 3 轮攻击，同一目标再次命中时威力衰减。
    pub fn drain_plain_rapid_skill_into(&mut self, actor: EntityIdx, mut targets: PreparedTargetList, updates: &mut RunUpdates) {
        if targets.is_empty() {
            return;
        }
        let rounds = if self.rng.c50() { 3.0 } else { 2.0 };
        targets.truncate(3);
        let mut hit_scores = vec![0.0f64; targets.len()];
        let mut position = 0usize;
        let mut round = 0.0f64;
        while round < rounds {
            let actor_active = self.entities.get(actor).is_some_and(|entity| entity.is_active());
            if !actor_active {
                return;
            }

            let target = targets[position];
            let target_dead = self.entities.get(target).map(|entity| !entity.runtime.alive).unwrap_or(true);
            if target_dead {
                round -= 0.5;
            } else {
                let atp = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("runtime rapid actor disappeared: {}", actor.0))
                    .runtime
                    .get_at(false, &mut self.rng)
                    * (0.75 - hit_scores[position] * 0.15000000596046448);
                hit_scores[position] += 1.0;
                updates.add(crate::runtime::update::RunUpdate::new(
                    if round == 0.0 { "[0]发起攻击" } else { "[0][连击]" },
                    actor.0 as usize,
                    target.0 as usize,
                    if round == 0.0 { 0 } else { 1 },
                ));
                let damage = self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
                // 原版 group.die 判胜时直接退出连击，不追加换行、也不抽取下一目标。
                if damage <= 0 || self.plain_battle_has_ended() {
                    return;
                }
                updates.add_newline();
            }
            position = (position + self.rng.r3() as usize) % targets.len();
            round += 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试辅助：推进至多 128 步并跳过纯空帧，返回下一可见批次及其胜者编号。
    fn next_visible_batch(runner: &mut RuntimeRunner) -> (RuntimeFrame, Option<usize>) {
        (0..128)
            .filter_map(|_| {
                let outcome = runner.run_round();
                outcome.frame.map(|frame| (frame, outcome.winner_team))
            })
            .find(|(frame, _)| frame.updates.updates.iter().any(|update| !update.message.trim().is_empty()))
            .expect("应在跳过纯空帧后产生下一可见批次")
    }

    /// 测试辅助：抽取帧中非空消息的 `(message, score)` 列表。
    fn visible_events(frame: &RuntimeFrame) -> Vec<(&str, u32)> {
        frame
            .updates
            .updates
            .iter()
            .filter(|update| !update.message.trim().is_empty())
            .map(|update| (update.message.as_ref(), update.score))
            .collect()
    }

    /// 测试辅助：断言 RC4 游标与 `main_val` 的 FNV-1a 散列和期望值一致。
    fn assert_rapid_rng(runner: &RuntimeRunner, cursor: (u32, u32), expected_permutation: u64, context: &str) {
        let rng = &runner.runtime().rng;
        assert_eq!((rng.i, rng.j), cursor, "{context}");
        let permutation = rng.main_val.iter().fold(0xcbf2_9ce4_8422_2325_u64, |digest, &byte| {
            (digest ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        });
        assert_eq!(permutation, expected_permutation, "{context}");
    }

    #[test]
    fn rapid_stops_on_the_winning_hit_before_drawing_next_target() {
        // 原版 group.die 判胜时直接离开 Rapid.act，连带使魔消失后也一样。
        for (raw, batch, damage, cursor, permutation, sonic_state, linked_summon) in [
            (
                "sonic@!\n\nalice",
                2,
                115,
                (108, 17),
                0x1371_d5ff_5e3a_a133,
                (256, 977, 14),
                false,
            ),
            (
                "sonic@!\n\nBob",
                4,
                94,
                (137, 72),
                0x389e_6c08_1d26_0b4f,
                (299, 4295, 33),
                false,
            ),
            (
                "saitama@!\n\nsonic@!\n\nalice",
                22,
                90,
                (225, 42),
                0xa6c2_0d32_1117_7029,
                (299, 3645, 29),
                true,
            ),
        ] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
            for _ in 1..batch {
                let (_, winner) = next_visible_batch(&mut runner);
                assert_eq!(winner, None, "{raw}");
            }
            let (frame, winner) = next_visible_batch(&mut runner);
            let players = runner.player_snapshots();
            let sonic = players.iter().find(|player| player.id_key_name == "sonic@!").unwrap();
            assert_eq!(winner, Some(sonic.team_index), "{raw}");
            assert_eq!(runner.alive_player_ids(), [sonic.id], "{raw}");
            assert_eq!((sonic.hp, sonic.move_point, sonic.magic_point), sonic_state, "{raw}");

            let mut events = vec![("[0]发起攻击", 0), ("[1]受到[2]点伤害", damage), ("[1]被击倒了", 50)];
            if linked_summon {
                events.push(("[1]消失了", 50));
                let summon = players.iter().find(|player| player.id_key_name == "alice?0@alice").unwrap();
                assert_eq!((summon.alive, summon.hp), (false, 0), "{raw}");
            }
            assert_eq!(visible_events(&frame), events, "{raw}");
            assert_rapid_rng(&runner, cursor, permutation, raw);
            assert_eq!(
                frame.updates.updates.last().unwrap().message.as_ref(),
                events.last().unwrap().0,
                "{raw}"
            );
        }
    }

    #[test]
    fn rapid_continues_after_a_nonterminal_knockout_or_revive() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("sonic@!\n\nAlice\n\nmario@!".to_owned()).unwrap();
        for batch in 1..=19 {
            let (frame, winner) = next_visible_batch(&mut runner);
            assert_eq!(winner, None);
            let (damage, hp, cursor, permutation) = match batch {
                12 => (30, 356, (177, 119), 0x5aa7_4049_efc3_9355),
                19 => (17, 369, (124, 235), 0x5efc_7fa0_31e9_aa5d),
                _ => continue,
            };
            let events = visible_events(&frame);
            let last_hit = [("[0][连击]", 1), ("[1]受到[2]点伤害", damage)];
            let (earlier, last) = events.split_at(events.len() - last_hit.len());
            assert!(earlier.contains(&("[1]被击倒了", 50)));
            assert_eq!(earlier.contains(&("[0]满血复活", 0)), batch == 19);
            assert_eq!(last, last_hit);
            let mario = runner
                .player_snapshots()
                .into_iter()
                .find(|player| player.id_key_name == "mario@!")
                .unwrap();
            assert_eq!((mario.alive, mario.hp), (true, hp));
            assert_rapid_rng(&runner, cursor, permutation, "非终局连击");
        }
    }
}

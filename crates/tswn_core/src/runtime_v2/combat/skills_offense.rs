use super::*;

impl CombatRuntime {
    pub fn drain_plain_builtin_skill_into(
        &mut self,
        actor: EntityIdx,
        prepared: PreparedBuiltinSkillAction,
        updates: &mut RunUpdates,
    ) {
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
                let target = target.expect("runtime_v2 assassinate action is missing its selected or pending target");
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
        }
    }

    pub fn select_plain_default_enemy_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 3 } else { 2 })
    }

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
        let ally_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                self.entities
                    .get(*candidate)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<smallvec::SmallVec<[usize; 8]>>();
        let mut selected = smallvec::SmallVec::<[EntityIdx; 8]>::new();
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if ally_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &ally_skip_indices)
            };
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

    pub fn score_plain_default_enemy_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default enemy target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / rate_hi_hp(entity.runtime.hp)) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    pub fn drain_plain_fire_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.effects.push(QueuedEffect::FireAttack {
            caster: actor,
            target,
            fire_state_key: PLAIN_FIRE_STATE_KEY,
        });
        self.drain_effects_into(updates);
    }

    pub fn drain_plain_thunder_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[雷击术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let mut accuracy = 100
            + self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 thunder actor: {}", actor.0))
                .runtime
                .agility;
        let count = 3 + self.rng.r3() as usize;
        for _ in 0..count {
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
                    .unwrap_or_else(|| panic!("unknown runtime_v2 thunder target: {}", target.0));
                (
                    target_entity.is_active(),
                    target_entity.runtime.agility + target_entity.runtime.resistance,
                )
            };
            if target_active && PlayerRuntime::dodge(accuracy, target_dodge, &mut self.rng) {
                updates.add(RuntimeFrame::replay_update(
                    target.0 as usize,
                    actor.0 as usize,
                    "[0][回避]了攻击",
                    0,
                ));
                return;
            }

            accuracy -= 10;
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 thunder actor: {}", actor.0))
                .runtime
                .get_at(true, &mut self.rng)
                * 0.36000001430511475;
            let update_pos = updates.updates.len();
            self.drain_plain_attack_from_defense_into(actor, target, true, atp, updates);
            if let Some(update) = updates.updates.get_mut(update_pos) {
                update.delay0 = 300;
            }
        }
    }

    pub fn drain_plain_quake_skill_into(&mut self, actor: EntityIdx, mut targets: PreparedTargetList, updates: &mut RunUpdates) {
        if targets.is_empty() {
            return;
        }
        let round = if self.rng.c50() { 5 } else { 4 };
        targets.truncate(round.min(targets.len()));
        if targets.is_empty() {
            return;
        }
        updates.add(crate::engine::update::RunUpdate::new(
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
                .unwrap_or_else(|| panic!("unknown runtime_v2 quake actor: {}", actor.0))
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

    pub fn drain_plain_absorb_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 absorb actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 1.2999999523162842;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发起[吸血攻击]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Absorb, updates);
    }

    pub fn drain_plain_poison_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 poison actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0][投毒]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Poison, updates);
    }

    pub fn drain_plain_critical_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let actor_runtime = &mut self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 critical actor: {}", actor.0))
            .runtime;
        let atp0 = actor_runtime.get_at(false, &mut self.rng) * 1.149999976158142;
        let atp1 = actor_runtime.get_at(false, &mut self.rng) * 1.2000000476837158;
        let atp2 = actor_runtime.get_at(false, &mut self.rng) * 1.25;
        let atp = atp0.max(atp1).max(atp2);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发动[会心一击]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
    }

    pub fn select_plain_berserk_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        if all_alive.is_empty() {
            return PreparedTargetList::new();
        }
        let ally_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<smallvec::SmallVec<[usize; 8]>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut duplicate_count = 0usize;
        let mut invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if ally_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &ally_skip_indices)
            };
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                !smart
                    || (!entity
                        .states
                        .entries()
                        .iter()
                        .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
                        && !entity.runtime.is_combat_minion())
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
            scored.push((target, self.score_plain_berserk_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_berserk_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let mut score = self.score_plain_default_enemy_target(target, smart);
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 berserk target: {}", target.0));
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

    pub fn drain_plain_berserk_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 berserk actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[狂暴术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Berserk, updates);
    }

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

    pub fn score_plain_haste_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if !smart {
            return self.rng.rFFFF() as f64;
        }
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 haste target: {}", target.0));
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

    pub fn drain_plain_haste_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[加速术]",
            actor.0 as usize,
            target.0 as usize,
            60,
        ));
        let (charge_active, owner_speed) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 haste actor: {}", actor.0));
            (owner.runtime.charge.active, owner.effective_speed())
        };
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 haste actor: {}", actor.0))
            .runtime
            .move_state
            .speed_points += owner_speed;

        let haste_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT)
            .expect("default runtime v2 profile must register core haste state");
        let haste_priority = self
            .registry
            .state(haste_state_id)
            .expect("default runtime v2 core haste state disappeared")
            .priority;
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 haste target: {}", target.0));
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
                "runtime_v2 haste state disappeared during extension"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::haste_with_effective_faster(
                    PLAIN_HASTE_STATE_KEY,
                    haste_state_id,
                    if charge_active { 4 } else { 2 },
                    2,
                    if charge_active { 5 } else { 3 },
                    haste_priority,
                )),
                "runtime_v2 haste state should be inserted"
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]进入[疾走]状态",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    pub fn drain_plain_iron_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 iron actor: {}", actor.0));
            (owner.runtime.magic, owner.runtime.at_boost_millionths >= 3_000_000)
        };
        let step = 3 + if charge_active { 4 } else { 0 };
        let protect = 110 + magic + if charge_active { 240 + magic * 4 } else { 0 };
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发动[铁壁]",
            actor.0 as usize,
            actor.0 as usize,
            60,
        ));

        let iron_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_IRON_STATE_EXPORT)
            .expect("default runtime v2 profile must register core iron state");
        let iron_priority = self
            .registry
            .state(iron_state_id)
            .expect("default runtime v2 core iron state disappeared")
            .priority;
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 iron actor: {}", actor.0));
        if owner.states.entry(PLAIN_IRON_STATE_KEY).is_some() {
            assert!(
                owner.states.set_payload(PLAIN_IRON_STATE_KEY, StatePayload::Iron { protect, step }),
                "runtime_v2 iron state disappeared during replacement"
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
                "runtime_v2 iron state should be inserted"
            );
        }
        owner.runtime.move_state.speed_points -= 256;
        owner.refresh_runtime_stats_from_template();
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]防御力大幅上升",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

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
            .collect::<smallvec::SmallVec<[usize; 8]>>();
        let select_count = if smart { 5 } else { 3 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 5]>::new();
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &enemy_skip_indices)
            };
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
                .unwrap_or_else(|| panic!("runtime_v2 rapid target disappeared: {}", target.0));
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
                    .unwrap_or_else(|| panic!("runtime_v2 rapid actor disappeared: {}", actor.0))
                    .runtime
                    .get_at(false, &mut self.rng)
                    * (0.75 - hit_scores[position] * 0.15000000596046448);
                hit_scores[position] += 1.0;
                updates.add(crate::engine::update::RunUpdate::new(
                    if round == 0.0 { "[0]发起攻击" } else { "[0][连击]" },
                    actor.0 as usize,
                    target.0 as usize,
                    if round == 0.0 { 0 } else { 1 },
                ));
                let damage = self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
                if damage <= 0 {
                    return;
                }
                updates.add_newline();
            }
            position = (position + self.rng.r3() as usize) % targets.len();
            round += 1.0;
        }
    }
}

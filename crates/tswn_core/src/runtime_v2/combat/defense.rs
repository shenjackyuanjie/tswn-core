use super::*;

impl CombatRuntime {
    pub fn apply_disperse_hit_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 disperse target entity: {}", target.0);
        };
        let clear_messages = target_entity.clear_positive_messages();
        let mp = target_entity.runtime.magic_point;
        target_entity.runtime.magic_point = if mp > 64 {
            mp - 64
        } else if mp > 32 {
            0
        } else {
            mp - 32
        };
        for (_, message) in clear_messages {
            updates.add_newline();
            updates.add(RuntimeFrame::replay_update(caster.0 as usize, target.0 as usize, message, 0));
        }
    }

    pub fn drain_lethal_damage_hooks_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.drain_die_hooks_into(target, updates);
        if self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0) {
            return;
        }
        if self.should_run_kill_hooks(caster, target) {
            self.drain_kill_hooks_into(caster, target, updates);
        }
    }

    pub fn has_alive_enemy_or_pending_spawn(&self, caster: EntityIdx) -> bool {
        let caster_team = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("runtime_v2 kill caster disappeared: {}", caster.0))
            .runtime
            .team;
        if self
            .world
            .flat_alive()
            .iter()
            .copied()
            .any(|target| self.entities.get(target).is_some_and(|entity| entity.runtime.team != caster_team))
        {
            return true;
        }

        self.effects.iter().any(|effect| {
            let owner = match effect {
                QueuedEffect::Spawn { caster, .. }
                | QueuedEffect::SpawnSilent { caster, .. }
                | QueuedEffect::SpawnWithMessage { caster, .. } => *caster,
                _ => return false,
            };
            self.entities.get(owner).is_some_and(|entity| entity.runtime.team != caster_team)
        })
    }

    pub fn should_run_kill_hooks(&self, caster: EntityIdx, killed_target: EntityIdx) -> bool {
        caster != killed_target
            && self.entities.get(caster).is_some_and(|entity| entity.runtime.hp > 0)
            && self.has_alive_enemy_or_pending_spawn(caster)
    }

    pub fn drain_pre_defend_hooks_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        self.drain_pre_defend_hooks_with_on_damage_into(target, updates, defend_value, PlainAttackOnDamage::None);
    }

    pub fn drain_pre_defend_hooks_with_on_damage_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
        on_damage: PlainAttackOnDamage,
    ) {
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, target, ProcMask::PRE_DEFEND);
        let protect_split = self.entities.get(target).and_then(|entity| {
            (!entity.runtime.protect_from.is_empty())
                .then_some(entity.runtime.protect_pre_defend_skill_count)
                .flatten()
        });
        if let Some(protect_split) = protect_split {
            let started_zero = defend_value.atp() == Some(0.0);
            let protect_split = protect_split.min(skill_plan.entries.len());
            let before_protect = SkillHookPlan {
                owner: skill_plan.owner,
                hook: skill_plan.hook,
                loadout_len: skill_plan.loadout_len,
                entries: skill_plan.entries[..protect_split].to_vec(),
            };
            self.drain_skill_hook_plan_with_defend_value_into(&before_protect, updates, defend_value);
            if defend_value.atp() == Some(0.0) && (!started_zero || protect_split > 0) {
                return;
            }
            if self.drain_plain_protect_pre_defend_into(target, updates, defend_value, on_damage) {
                return;
            }
            let after_protect = SkillHookPlan {
                owner: skill_plan.owner,
                hook: skill_plan.hook,
                loadout_len: skill_plan.loadout_len,
                entries: skill_plan.entries[protect_split..].to_vec(),
            };
            self.drain_skill_hook_plan_with_defend_value_into(&after_protect, updates, defend_value);
            if defend_value.atp() == Some(0.0) {
                return;
            }
            let state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::PRE_DEFEND);
            self.drain_state_hook_plan_with_defend_value_into(&state_plan, updates, defend_value);
            return;
        }
        self.drain_skill_hook_plan_with_defend_value_into(&skill_plan, updates, defend_value);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::PRE_DEFEND);
        self.drain_state_hook_plan_with_defend_value_into(&state_plan, updates, defend_value);
    }

    pub fn plain_protect_level(&self, owner: EntityIdx, fallback: u32) -> u32 {
        let Some(owner) = self.entities.get(owner) else {
            return fallback;
        };
        owner
            .template
            .skills
            .skills()
            .iter()
            .copied()
            .enumerate()
            .find_map(|(fixed_lane, skill_id)| {
                (self.registry.skill(skill_id)?.export_name == DEFAULT_CORE_PROTECT_SKILL_EXPORT)
                    .then(|| owner.template.skills.level_at(fixed_lane))
                    .flatten()
            })
            .unwrap_or(fallback)
    }

    pub fn drain_plain_protect_post_action_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let mut plan =
            self.scheduler
                .skill_post_action_hook_plan(&self.entities, &self.registry, owner, SkillPostActionPhase::Early);
        plan.entries.retain(|entry| {
            self.registry
                .skill(entry.skill_id)
                .is_some_and(|skill| skill.export_name == DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        });
        if !plan.entries.is_empty() {
            self.drain_skill_hook_plan_into(&plan, updates);
        }
    }

    pub fn drain_plain_protect_pre_defend_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
        on_damage: PlainAttackOnDamage,
    ) -> bool {
        let Some(incoming_atp) = defend_value.atp() else {
            return false;
        };
        let is_magic = defend_value
            .is_magic()
            .expect("runtime_v2 protect PRE_DEFEND value must carry attack type");
        let caster = defend_value.caster();
        let target_team = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("runtime_v2 protect target disappeared: {}", target.0))
            .runtime
            .team;

        loop {
            let link_count = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("runtime_v2 protect target disappeared: {}", target.0))
                .runtime
                .protect_from
                .len();
            let link_index = match link_count {
                0 => return false,
                1 => 0,
                count => self.rng.next_i32(count as i32) as usize,
            };
            let link = self
                .entities
                .get(target)
                .unwrap()
                .runtime
                .protect_from
                .get(link_index)
                .cloned()
                .unwrap_or_else(|| panic!("runtime_v2 protect link index disappeared: {link_index}"));
            let level = self.plain_protect_level(link.owner, link.level);
            let same_group = self
                .entities
                .get(link.owner)
                .is_some_and(|_| self.plain_effective_team(link.owner) == target_team);
            let trigger_ok = same_group && self.rng.r127() < level;
            let protector_ready = trigger_ok
                && self
                    .entities
                    .get_mut(link.owner)
                    .is_some_and(|protector| protector.runtime.mp_ready(&mut self.rng));

            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_PROBE_PROTECT").is_some() {
                eprintln!(
                    "[protect_probe:v2] target={} protector={} link_index={} links={} same_group={} level={} trigger_ok={} protector_ready={} rc4=({}, {})",
                    target.0,
                    link.owner.0,
                    link_index,
                    link_count,
                    same_group,
                    level,
                    trigger_ok,
                    protector_ready,
                    self.rng.i,
                    self.rng.j,
                );
            }

            if trigger_ok && protector_ready {
                self.drain_plain_protect_post_action_into(link.owner, updates);
                updates.add(crate::engine::update::RunUpdate::new(
                    "[0][守护][1]",
                    link.owner.0 as usize,
                    target.0 as usize,
                    40,
                ));

                let mut redirected_atp = RuntimeDefendValue::Atp {
                    value: incoming_atp,
                    caster,
                    target: link.owner,
                    is_magic,
                };
                self.drain_pre_defend_hooks_into(link.owner, updates, &mut redirected_atp);
                let redirected_atp = redirected_atp.atp().expect("runtime_v2 protect pre-defend hooks must leave an atp value");
                if redirected_atp == 0.0 {
                    defend_value.set_atp(0.0);
                    return true;
                }

                let defense = {
                    let protector = self
                        .entities
                        .get(link.owner)
                        .unwrap_or_else(|| panic!("runtime_v2 protector disappeared: {}", link.owner.0));
                    if is_magic {
                        protector.runtime.resistance + 64
                    } else {
                        protector.runtime.defense + 64
                    }
                };
                let redirected_damage = (redirected_atp * 0.5 / defense as f64).floor() as i32;
                let mut redirected_damage_value = RuntimeDefendValue::Damage {
                    value: redirected_damage,
                    caster,
                    target: link.owner,
                };
                self.drain_post_defend_hooks_into(link.owner, updates, &mut redirected_damage_value);
                let redirected_damage = redirected_damage_value
                    .damage()
                    .expect("runtime_v2 protect post-defend hooks must leave a damage value");
                if self.apply_plain_attack_damage_with_covid_and_on_damage_into(
                    caster,
                    link.owner,
                    redirected_damage,
                    None,
                    on_damage,
                    updates,
                ) {
                    self.drain_plain_lethal_damage_into(caster, link.owner, updates);
                }
                defend_value.set_atp(0.0);
                return true;
            }

            let target_runtime = &mut self.entities.get_mut(target).unwrap().runtime;
            target_runtime.protect_from.remove(link_index);
            if target_runtime.protect_from.is_empty() {
                target_runtime.protect_pre_defend_skill_count = None;
            }
            if let Some(protector) = self.entities.get_mut(link.owner)
                && protector.runtime.protect_to == Some(target)
            {
                protector.runtime.protect_to = None;
            }
        }
    }

    pub fn drain_post_defend_hooks_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, target, ProcMask::POST_DEFEND);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::POST_DEFEND);

        #[derive(Clone, Copy)]
        enum DefendHookPlanEntry {
            Skill(SkillHookPlanEntry),
            State(StateHookPlanEntry),
        }

        let mut entries = Vec::with_capacity(skill_plan.entries.len() + state_plan.entries.len());
        entries.extend(skill_plan.entries.iter().copied().map(|entry| {
            (
                entry.priority,
                0_u8,
                entry.active_order,
                entry.registration_order,
                DefendHookPlanEntry::Skill(entry),
            )
        }));
        entries.extend(state_plan.entries.iter().copied().map(|entry| {
            (
                entry.priority,
                1_u8,
                usize::MAX,
                entry.registration_order,
                DefendHookPlanEntry::State(entry),
            )
        }));
        entries.sort_by_key(|(priority, kind_order, active_order, registration_order, _)| {
            (*priority, *kind_order, *active_order, *registration_order)
        });

        for (_, _, _, _, entry) in entries {
            match entry {
                DefendHookPlanEntry::Skill(entry) => {
                    let plan = SkillHookPlan {
                        owner: skill_plan.owner,
                        hook: skill_plan.hook,
                        loadout_len: skill_plan.loadout_len,
                        entries: vec![entry],
                    };
                    self.drain_skill_hook_plan_with_defend_value_into(&plan, updates, defend_value);
                }
                DefendHookPlanEntry::State(entry) => {
                    let plan = StateHookPlan {
                        hook: state_plan.hook,
                        store_generation: state_plan.store_generation,
                        entries: vec![entry],
                    };
                    self.drain_state_hook_plan_with_defend_value_into(&plan, updates, defend_value);
                }
            }
        }
        self.apply_runtime_shield_post_defend(target, defend_value);
    }

    pub fn apply_runtime_shield_post_defend(&mut self, target: EntityIdx, defend_value: &mut RuntimeDefendValue) {
        let Some(damage) = defend_value.damage() else {
            return;
        };
        if damage <= 0 {
            return;
        }
        let target = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 shield target disappeared: {}", target.0));
        if target.runtime.shield <= 0 {
            return;
        }
        if damage > target.runtime.shield {
            target.runtime.shield = 0;
        } else {
            target.runtime.shield -= damage;
            defend_value.set_damage(0);
        }
    }

    pub fn drain_die_hooks_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let die_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, target, ProcMask::DIE);
        self.drain_skill_hook_plan_into(&die_skill_plan, updates);
        if self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0) {
            return;
        }
        let die_state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::DIE);
        self.drain_state_hook_plan_into(&die_state_plan, updates);
    }

    pub fn drain_kill_hooks_into(&mut self, caster: EntityIdx, killed_target: EntityIdx, updates: &mut RunUpdates) {
        let kill_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, caster, ProcMask::KILL);
        self.drain_plain_kill_skill_plan_into(&kill_skill_plan, killed_target, updates);
        let kill_state_plan = self.scheduler.state_hook_plan(&self.entities, caster, ProcMask::KILL);
        self.drain_state_hook_plan_into(&kill_state_plan, updates);
    }

    pub fn apply_damage_into(&mut self, caster: EntityIdx, target: EntityIdx, amount: i32, updates: &mut RunUpdates) -> bool {
        self.apply_damage_with_replay_into(caster, target, amount, updates, RuntimeFrame::damage_update)
    }

    pub fn apply_legacy_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        self.apply_damage_with_replay_into(caster, target, amount, updates, RuntimeFrame::legacy_damage_update)
    }

    pub fn apply_plain_legacy_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 plain legacy damage target entity: {}", target.0));
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        killed
    }

    pub fn apply_damage_with_replay_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
        replay: fn(usize, usize, i32) -> crate::engine::update::RunUpdate,
    ) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 damage target entity: {}", target.0);
        };
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        if killed {
            target_entity.runtime.alive = false;
        }
        let team = target_entity.runtime.team;
        updates.add(replay(caster.0 as usize, target.0 as usize, amount));
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        if killed {
            self.mark_dead_with_linked_minions_into(target, team, updates);
        }
        killed
    }

    pub fn apply_poison_tick_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 poison target entity: {}", target.0));
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        killed
    }

    pub fn apply_disperse_attack_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 disperse damage target entity: {}", target.0);
        };
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        if amount > 0 {
            self.apply_disperse_hit_into(caster, target, updates);
        }
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        killed
    }

    pub fn emit_poison_release_if_cleared(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 poison release target entity: {}", target.0);
        };
        if target_entity
            .states
            .entries()
            .iter()
            .any(|entry| matches!(entry.payload, StatePayload::Poison { .. }))
        {
            return;
        }
        updates.add_newline();
        updates.add(RuntimeFrame::replay_update(
            target.0 as usize,
            target.0 as usize,
            "[1]从[中毒]中解除",
            0,
        ));
    }

    pub fn magic_attack_dodged(&mut self, caster: EntityIdx, target: EntityIdx) -> bool {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 magic attack dodge target entity: {}", target.0);
        };
        if !target_entity.is_active() {
            return false;
        }

        let accuracy = self.entities.get(caster).unwrap().runtime.magic_accuracy();
        let dodge_value = target_entity.runtime.magic_dodge();
        PlayerRuntime::dodge(accuracy, dodge_value, &mut self.rng)
    }

    pub fn apply_fire_on_damage(&mut self, target: EntityIdx, fire_state_key: u32) {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 fire target entity: {}", target.0);
        };
        if target_entity.runtime.hp <= 0 || self.fire_immune(target) {
            return;
        }

        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 fire target entity: {}", target.0);
        };
        target_entity.states.add_fire_mag_half_step(fire_state_key);
    }

    pub fn apply_ice_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 ice target entity: {}", target.0);
        };
        if target_entity.runtime.hp <= 0 || !target_entity.runtime.alive || self.ice_immune(target) {
            return;
        }
        let charge_active = self
            .entities
            .get(caster)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);
        let frozen_step = 1024 + if charge_active { 2048 } else { 0 };
        self.entities
            .get_mut(target)
            .unwrap()
            .states
            .add_ice_frozen_step(PLAIN_ICE_STATE_KEY, frozen_step);
        updates.add(RuntimeFrame::replay_update(
            caster.0 as usize,
            target.0 as usize,
            "[1]被[冰冻]了",
            40,
        ));
    }

    pub fn status_immune(&mut self, target: EntityIdx, status: &'static str) -> bool {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 {status} immune target entity: {}", target.0);
        };
        if target_entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_entity.template.name, status);
            return (self.rng.next_u8() as i32) < threshold;
        }
        if target_entity.runtime.flags.contains(PlayerKindFlags::BOOST) {
            return self.rng.r127() < crate::player::boost_value(&target_entity.template.name);
        }
        false
    }

    pub fn ice_immune(&mut self, target: EntityIdx) -> bool { self.status_immune(target, "ice") }

    pub fn fire_immune(&mut self, target: EntityIdx) -> bool { self.status_immune(target, "fire") }

    pub fn kill_entity_without_damage_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let killed = self.kill_entity_without_damage_mark_only_into(target);
        if killed {
            let team = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 self-death target entity: {}", target.0))
                .runtime
                .team;
            self.entities.get_mut(target).unwrap().runtime.alive = false;
            self.mark_dead_with_linked_minions_into(target, team, updates);
        }
        killed
    }

    pub fn kill_entity_without_damage_mark_only_into(&mut self, target: EntityIdx) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 self-death target entity: {}", target.0);
        };
        let killed = target_entity.runtime.alive;
        target_entity.runtime.hp = 0;
        killed
    }

    pub fn mark_dead_with_linked_minions_into(&mut self, owner: EntityIdx, team: usize, updates: &mut RunUpdates) {
        self.cleanup_linked_minions_for_owner(owner, updates);
        self.world.mark_dead(owner, team);
    }

    pub fn cleanup_linked_minions_for_owner(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let linked_minions = self
            .entities
            .iter()
            .filter_map(|(idx, entity)| {
                (idx != owner && entity.runtime.alive && entity.runtime.owner == owner && entity.runtime.is_combat_minion())
                    .then_some(idx)
            })
            .collect::<Vec<_>>();

        for minion in linked_minions {
            let Some(minion_entity) = self.entities.get_mut(minion) else {
                panic!("unknown runtime_v2 linked minion entity: {}", minion.0);
            };
            minion_entity.runtime.hp = 0;
            minion_entity.runtime.alive = false;
            let team = minion_entity.runtime.team;
            self.world.mark_dead(minion, team);
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                "[1]消失了",
                owner.0 as usize,
                minion.0 as usize,
                50,
            ));
        }
    }

    pub fn resolve_damage_target(&self, target: EntityIdx) -> EntityIdx {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 damage target entity: {}", target.0));
        match target_entity.runtime.policies.owner_resolution {
            OwnerResolutionPolicy::SelfEntity => target,
            OwnerResolutionPolicy::RootOwner => target_entity.runtime.root_owner,
        }
    }

    pub fn resolve_damage_share_targets(&self, target: EntityIdx, resolved_target: EntityIdx) -> Vec<EntityIdx> {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 damage target entity: {}", target.0));
        match target_entity.runtime.policies.damage_share {
            DamageSharePolicy::None => Vec::new(),
            DamageSharePolicy::ShareToOwner => {
                let owner = target_entity.runtime.owner;
                (owner != resolved_target).then_some(owner).into_iter().collect()
            }
            DamageSharePolicy::ShareToSummons => {
                if resolved_target != target {
                    return Vec::new();
                }
                self.entities
                    .iter()
                    .filter_map(|(idx, entity)| {
                        (idx != target && entity.runtime.alive && entity.runtime.owner == target).then_some(idx)
                    })
                    .collect()
            }
        }
    }

    pub fn ensure_effect_entity(&self, effect: &'static str, role: &'static str, entity: EntityIdx) {
        if self.entities.get(entity).is_none() {
            panic!("unknown runtime_v2 {effect} {role} entity: {}", entity.0);
        }
    }
}

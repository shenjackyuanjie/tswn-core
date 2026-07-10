use super::*;

impl CombatRuntime {
    pub fn drain_plain_kill_skill_plan_into(&mut self, plan: &SkillHookPlan, killed_target: EntityIdx, updates: &mut RunUpdates) {
        for entry in &plan.entries {
            let is_zombie = self
                .registry
                .skill(entry.skill_id)
                .is_some_and(|skill| skill.export_name == DEFAULT_CORE_ZOMBIE_SKILL_EXPORT);
            if is_zombie {
                self.drain_plain_zombie_kill_skill_into(entry.owner, entry.fixed_lane, killed_target, updates);
                continue;
            }
            let entry_plan = SkillHookPlan {
                owner: plan.owner,
                hook: plan.hook,
                loadout_len: plan.loadout_len,
                entries: vec![*entry],
            };
            self.drain_skill_hook_plan_with_selected_target_into(&entry_plan, updates, Some(killed_target));
        }
    }

    pub fn drain_plain_zombie_kill_skill_into(
        &mut self,
        caster: EntityIdx,
        fixed_lane: usize,
        killed_target: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let target_is_minion = self
            .entities
            .get(killed_target)
            .unwrap_or_else(|| panic!("runtime_v2 zombie target disappeared: {}", killed_target.0))
            .runtime
            .flags
            .contains(PlayerKindFlags::MINION);
        if target_is_minion {
            return;
        }

        let level = self
            .entities
            .get(caster)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 zombie level missing for fixed lane {fixed_lane}"));
        if self.rng.r63() >= level {
            return;
        }

        let blueprint_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core zombie blueprint slot");
        let blueprint = match self.entities.get(caster).and_then(|entity| entity.slots.get(blueprint_slot)) {
            Some(SlotValue::PlayerTemplate(template)) => Some(template.as_ref().clone()),
            Some(_) => panic!("runtime_v2 core zombie blueprint slot has invalid value"),
            None => None,
        };
        if blueprint.is_none() {
            self.mark_zombie_corpse(killed_target);
            return;
        }
        if !self
            .entities
            .get_mut(caster)
            .unwrap_or_else(|| panic!("runtime_v2 zombie caster disappeared: {}", caster.0))
            .runtime
            .mp_ready(&mut self.rng)
        {
            return;
        }

        self.mark_zombie_corpse(killed_target);
        let mut template = blueprint.unwrap();
        template.name = self.allocate_plain_minion_name(caster);
        template.team = self.entities.get(caster).unwrap().runtime.team;
        template.move_state.speed_points = self.rng.r255() as i32 * 4;
        let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
        let zombie = self
            .entities
            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
        let team = self.entities.get(zombie).unwrap().runtime.team;
        self.world.add_spawned_alive(zombie, team);

        updates.add_newline();
        let mut summon_update =
            crate::engine::update::RunUpdate::new("[0][召唤亡灵]", caster.0 as usize, killed_target.0 as usize, 60);
        summon_update.delay0 = 1500;
        updates.add(summon_update);
        let mut zombied = crate::engine::update::RunUpdate::new("[2]变成了[1]", caster.0 as usize, zombie.0 as usize, 0);
        zombied.targets.push(killed_target.0 as usize);
        updates.add(zombied);
    }

    pub fn mark_zombie_corpse(&mut self, target: EntityIdx) {
        self.entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 zombie corpse target disappeared: {}", target.0))
            .runtime
            .corpse = RuntimeCorpseKind::Zombie;
    }
}

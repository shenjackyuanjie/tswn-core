use super::*;

impl CombatRuntime {
    pub(super) fn plain_summon_probability_allowed(&self, actor: EntityIdx, smart: bool) -> bool {
        let owner = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 summon owner: {}", actor.0));
        if smart && owner.runtime.hp < 80 {
            return false;
        }
        let remembered_slot = self.plain_summon_entity_slot();
        match owner.slots.get(remembered_slot) {
            Some(SlotValue::U64(raw)) => {
                let summoned = EntityIdx((*raw).try_into().expect("runtime_v2 remembered summon index overflow"));
                !self
                    .entities
                    .get(summoned)
                    .unwrap_or_else(|| panic!("runtime_v2 remembered summon disappeared: {}", summoned.0))
                    .runtime
                    .alive
            }
            Some(_) => panic!("runtime_v2 core summoned entity slot has invalid value"),
            None => true,
        }
    }

    pub(super) fn drain_plain_summon_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[血祭]",
            actor.0 as usize,
            actor.0 as usize,
            60,
        ));
        let charge_active = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 summon owner: {}", actor.0))
            .runtime
            .at_boost()
            >= 3.0;
        let random_move_points = self.rng.r255() as i32 * 4;
        let move_points = if charge_active { 2048 } else { random_move_points };
        let blueprint = self.plain_summon_blueprint(actor);
        let remembered_slot = self.plain_summon_entity_slot();
        let remembered = match self.entities.get(actor).unwrap().slots.get(remembered_slot) {
            Some(SlotValue::U64(raw)) => Some(EntityIdx(
                (*raw).try_into().expect("runtime_v2 remembered summon index overflow"),
            )),
            Some(_) => panic!("runtime_v2 core summoned entity slot has invalid value"),
            None => None,
        };

        if let Some(summoned) = remembered {
            if self
                .entities
                .get(summoned)
                .unwrap_or_else(|| panic!("runtime_v2 remembered summon disappeared: {}", summoned.0))
                .runtime
                .alive
            {
                return;
            }
            self.reset_plain_summon(actor, summoned, blueprint, move_points, !charge_active);
            updates.add(RuntimeFrame::replay_update(
                actor.0 as usize,
                summoned.0 as usize,
                "召唤出[1]",
                0,
            ));
            return;
        }

        let mut template = blueprint;
        let summoned = self.entities.next_spawn_idx(&template);
        template.name = self.allocate_plain_minion_name(actor);
        template.team = self.entities.get(actor).unwrap().runtime.team;
        template.move_state.speed_points = move_points;
        self.set_plain_summon_share_level(&mut template.skills, !charge_active);
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template,
            message: "召唤出[1]".to_owned(),
        });
        self.drain_effects_into(updates);
        assert!(
            self.entities.get(summoned).is_some(),
            "runtime_v2 summon spawn did not create expected entity {}",
            summoned.0
        );
        self.entities
            .get_mut(actor)
            .expect("runtime_v2 summon owner disappeared after spawn")
            .slots
            .set(remembered_slot, SlotValue::U64(u64::from(summoned.0)))
            .expect("runtime_v2 core summoned entity slot must exist");
    }

    pub(super) fn drain_plain_summon_explode_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.effects.push(QueuedEffect::SummonExplode {
            caster: actor,
            target,
            fire_state_key: PLAIN_FIRE_STATE_KEY,
        });
        self.drain_effects_into(updates);
    }

    pub(super) fn drain_plain_summon_share_damage_into(
        &mut self,
        summoned: EntityIdx,
        level: u32,
        damage: i32,
        caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        if level == 0 {
            return;
        }
        let owner = self
            .entities
            .get(summoned)
            .unwrap_or_else(|| panic!("unknown runtime_v2 summon share source: {}", summoned.0))
            .runtime
            .owner;
        if owner == summoned || self.entities.get(owner).is_none() {
            return;
        }
        let shared_damage = damage / 2;
        let (killed, team) = {
            let owner_entity = self
                .entities
                .get_mut(owner)
                .unwrap_or_else(|| panic!("runtime_v2 summon share owner disappeared: {}", owner.0));
            owner_entity.runtime.hp = (owner_entity.runtime.hp - shared_damage).max(0);
            let killed = owner_entity.runtime.hp == 0 && owner_entity.runtime.alive;
            if killed {
                owner_entity.runtime.alive = false;
            }
            (killed, owner_entity.runtime.team)
        };
        updates.add(RuntimeFrame::legacy_damage_update(
            caster.0 as usize,
            owner.0 as usize,
            shared_damage,
        ));
        self.drain_plain_post_damage_skill_chain_into(owner, shared_damage, caster, updates);
        if !killed {
            return;
        }
        self.emit_plain_lethal_replay_into(caster, owner, updates);
        self.world.mark_dead(owner, team);
        self.cleanup_plain_summon_owner_minions_except(owner, summoned, updates);
        self.drain_lethal_damage_hooks_into(caster, owner, updates);
    }

    fn plain_summon_blueprint(&self, actor: EntityIdx) -> PlayerTemplate {
        let slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
            .expect("default runtime v2 profile must reserve core summon blueprint slot");
        let owner = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 summon owner: {}", actor.0));
        let mut template = match owner.slots.get(slot) {
            Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
            Some(_) => panic!("runtime_v2 core summon blueprint slot has invalid value"),
            None => panic!("runtime_v2 summon owner {} is missing core summon blueprint", actor.0),
        };
        if let (Some(owner_build), Some(summon_build)) = (owner.template.clone_build.as_ref(), template.clone_build.as_mut()) {
            summon_build.refresh_summon_owner_attrs(owner_build);
            let stats = summon_build.derive_stats();
            template.apply_derived_stats(stats);
            template.magic_point = (template.wisdom >> 1).max(0);
        }
        template
    }

    fn plain_summon_entity_slot(&self) -> EntitySlotId {
        self.registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_ENTITY_EXPORT)
            .expect("default runtime v2 profile must reserve core summoned entity slot")
    }

    fn set_plain_summon_share_level(&self, skills: &mut SkillLoadout, enabled: bool) {
        let lane = (0..skills.len()).find(|lane| {
            skills.fixed_lane_key_at(*lane) == Some(crate::player::skill::act::summon::SUMMON_SHARE_DAMAGE_SKILL_KEY)
        });
        let Some(lane) = lane else {
            panic!("runtime_v2 summon blueprint is missing share-damage fixed lane");
        };
        assert!(
            skills.set_level_at(lane, u32::from(enabled)),
            "runtime_v2 summon share-damage fixed lane disappeared"
        );
    }

    fn reset_plain_summon(
        &mut self,
        actor: EntityIdx,
        summoned: EntityIdx,
        mut template: PlayerTemplate,
        move_points: i32,
        share_damage: bool,
    ) {
        let (id, name, display_name, root_owner) = {
            let existing = self
                .entities
                .get(summoned)
                .unwrap_or_else(|| panic!("runtime_v2 remembered summon disappeared: {}", summoned.0));
            (
                existing.template.id,
                existing.template.name.clone(),
                existing.template.display_name.clone(),
                existing.runtime.root_owner,
            )
        };
        template.id = id;
        template.name = name;
        template.display_name = display_name;
        template.team = self.entities.get(actor).unwrap().runtime.team;
        template.move_state.speed_points = move_points;
        self.set_plain_summon_share_level(&mut template.skills, share_damage);
        let runtime = PlayerRuntime::from_template(&template, &self.registry, actor, root_owner);
        *self
            .entities
            .get_mut(summoned)
            .unwrap_or_else(|| panic!("runtime_v2 remembered summon disappeared: {}", summoned.0)) = EntityRecord {
            template,
            runtime,
            states: StateStore::default(),
            slots: EntitySlotStorage::from_registry(&self.registry),
        };
        let team = self.entities.get(summoned).unwrap().runtime.team;
        self.world.revive_round_actor(summoned);
        self.world.revive_alive(summoned, team);
    }

    pub fn allocate_plain_minion_name(&mut self, actor: EntityIdx) -> String {
        let root = self.plain_minion_name_root(actor);
        let counter_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core minion counter slot");
        let root_entity = self
            .entities
            .get(root)
            .unwrap_or_else(|| panic!("unknown runtime_v2 minion name root: {}", root.0));
        let root_name = root_entity.template.name.clone();
        let next = match root_entity.slots.get(counter_slot) {
            Some(SlotValue::U64(next)) => *next,
            Some(_) => panic!("runtime_v2 core minion counter slot has invalid value"),
            None => 0,
        };
        self.entities
            .get_mut(root)
            .expect("runtime_v2 minion name root disappeared")
            .slots
            .set(counter_slot, SlotValue::U64(next + 1))
            .expect("runtime_v2 core minion counter slot must exist");
        format!("{root_name}?{next}")
    }

    fn plain_minion_name_root(&self, start: EntityIdx) -> EntityIdx {
        let mut current = start;
        loop {
            let entity = self
                .entities
                .get(current)
                .unwrap_or_else(|| panic!("unknown runtime_v2 minion name owner: {}", current.0));
            if entity.runtime.flags.contains(PlayerKindFlags::MINION) {
                return current;
            }
            if entity.runtime.owner == current {
                return current;
            }
            current = entity.runtime.owner;
        }
    }

    fn cleanup_plain_summon_owner_minions_except(&mut self, owner: EntityIdx, excluded: EntityIdx, updates: &mut RunUpdates) {
        let linked = self
            .entities
            .iter()
            .filter_map(|(idx, entity)| {
                (idx != excluded
                    && idx != owner
                    && entity.runtime.alive
                    && entity.runtime.owner == owner
                    && entity.runtime.flags.contains(PlayerKindFlags::MINION))
                .then_some(idx)
            })
            .collect::<Vec<_>>();
        for minion in linked {
            let entity = self
                .entities
                .get_mut(minion)
                .unwrap_or_else(|| panic!("unknown runtime_v2 linked minion entity: {}", minion.0));
            entity.runtime.hp = 0;
            entity.runtime.alive = false;
            let team = entity.runtime.team;
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
}

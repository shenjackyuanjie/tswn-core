use super::*;
use crate::runtime_v2::combat::PlainAttackOnDamage;

pub struct SkillContext<'a> {
    entities: &'a mut EntityArena,
    world: &'a mut WorldArena,
    registry: &'a ExtensionRegistry,
    template_slots: &'a TemplateSlotStorage,
    slots: &'a mut BattleSlotStorage,
    queue: &'a mut EffectQueue,
    updates: &'a mut RunUpdates,
    rng: &'a mut RC4,
    defend_value: Option<&'a mut RuntimeDefendValue>,
    defend_on_damage: PlainAttackOnDamage,
    selected_target: Option<EntityIdx>,
    owner: EntityIdx,
    capabilities: &'a [ExtensionCapability],
}

impl<'a> SkillContext<'a> {
    pub fn new(
        entities: &'a mut EntityArena,
        world: &'a mut WorldArena,
        registry: &'a ExtensionRegistry,
        template_slots: &'a TemplateSlotStorage,
        slots: &'a mut BattleSlotStorage,
        queue: &'a mut EffectQueue,
        updates: &'a mut RunUpdates,
        rng: &'a mut RC4,
        entry: SkillHookPlanEntry,
        capabilities: &'a [ExtensionCapability],
    ) -> Self {
        Self {
            entities,
            world,
            registry,
            template_slots,
            slots,
            queue,
            updates,
            rng,
            defend_value: None,
            defend_on_damage: PlainAttackOnDamage::None,
            selected_target: None,
            owner: entry.owner,
            capabilities,
        }
    }

    pub fn with_defend_value(mut self, defend_value: &'a mut RuntimeDefendValue) -> Self {
        self.defend_value = Some(defend_value);
        self
    }

    pub fn with_defend_on_damage(mut self, on_damage: PlainAttackOnDamage) -> Self {
        self.defend_on_damage = on_damage;
        self
    }

    pub fn with_selected_target(mut self, selected_target: EntityIdx) -> Self {
        self.selected_target = Some(selected_target);
        self
    }

    pub fn owner_idx(&self) -> EntityIdx { self.owner }

    pub fn selected_target(&self) -> Option<EntityIdx> { self.selected_target }

    pub fn owner(&self) -> Option<&EntityRecord> { self.entities.get(self.owner) }

    pub fn skill_level(&self, entry: &SkillHookPlanEntry) -> u32 {
        assert_eq!(
            entry.owner, self.owner,
            "runtime_v2 skill hook entry owner must match skill context owner"
        );
        self.owner()
            .and_then(|entity| entity.template.skills.level_at(entry.fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 skill level missing for fixed lane {}", entry.fixed_lane))
    }

    pub fn owner_charge_runtime(&self) -> Option<ChargeRuntime> { self.owner().map(|entity| entity.runtime.charge) }

    pub fn owner_accumulate_runtime(&self) -> Option<AccumulateRuntime> { self.owner().map(|entity| entity.runtime.accumulate) }

    pub fn owner_shield(&self) -> Option<i32> { self.owner().map(|entity| entity.runtime.shield) }

    pub fn set_owner_shield(&mut self, shield: i32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if shield > 0 {
            owner.states.register_compressed_legacy_state(CompressedLegacyState::Shield);
        }
        owner.runtime.shield = shield.max(0);
        Ok(())
    }

    pub fn activate_owner_charge_runtime(&mut self) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        owner.activate_charge_runtime();
        Ok(())
    }

    pub fn tick_owner_charge_post_action(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.tick_charge_post_action())
    }

    pub fn clear_owner_charge_runtime(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_charge_runtime())
    }

    pub fn activate_owner_accumulate_runtime(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.activate_accumulate_runtime())
    }

    pub fn clear_owner_accumulate_runtime(&mut self) -> Result<bool, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_accumulate_runtime())
    }

    pub fn clear_owner_positive_runtime_messages(&mut self) -> Result<Vec<(i32, &'static str)>, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_positive_runtime_messages())
    }

    pub fn clear_owner_positive_state_messages(&mut self) -> Result<Vec<(i32, &'static str)>, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_positive_state_messages())
    }

    pub fn clear_owner_positive_messages(&mut self) -> Result<Vec<(i32, &'static str)>, EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        Ok(owner.clear_positive_messages())
    }

    pub fn entity_count(&self) -> usize { self.entities.len() }

    pub fn entity(&self, entity: EntityIdx) -> Result<&EntityRecord, EffectContextError> {
        let observed = self.entities.get(entity).ok_or(EffectContextError::UnknownEntity(entity))?;
        if entity == self.owner {
            return Ok(observed);
        }

        let owner = self.entities.get(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let capability = if observed.runtime.team == owner.runtime.team {
            ExtensionCapability::ReadAllies
        } else {
            ExtensionCapability::ReadEnemies
        };
        self.require(capability)?;
        Ok(observed)
    }

    pub fn battle_slot(&self, id: crate::runtime_v2::BattleSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
        self.require(ExtensionCapability::ReadBattleSlots)?;
        Ok(self.slots.get(id))
    }

    pub fn template_slot(&self, id: TemplateSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
        self.require(ExtensionCapability::ReadTemplateSlots)?;
        Ok(self.template_slots.get(id))
    }

    pub fn set_entity_slot(&mut self, entity: EntityIdx, slot: EntitySlotId, value: SlotValue) -> Result<(), EffectContextError> {
        self.require(ExtensionCapability::MutateEntitySlots)?;
        let Some(entity) = self.entities.get_mut(entity) else {
            return Err(EffectContextError::UnknownEntity(entity));
        };
        entity.slots.set(slot, value)?;
        Ok(())
    }

    pub fn push_nested(&mut self, effect: QueuedEffect) { self.queue.push_nested(effect); }

    pub fn add_update(&mut self, update: RunUpdate) { self.updates.add(update); }

    pub fn add_newline(&mut self) { self.updates.add_newline(); }

    pub fn last_non_newline_update(&self) -> Option<&RunUpdate> {
        self.updates.last_non_newline_update()
    }

    pub fn rng_next_u8(&mut self) -> u8 { self.rng.next_u8() }

    pub fn rng_next_i32(&mut self, max: i32) -> i32 { self.rng.next_i32(max) }

    pub fn rng_c50(&mut self) -> bool { self.rng.c50() }

    pub fn rng_r255(&mut self) -> u32 { self.rng.r255() }

    pub fn rng_r127(&mut self) -> u32 { self.rng.r127() }

    pub fn rng_r63(&mut self) -> u32 { self.rng.r63() }

    pub fn rng_r16(&mut self) -> u32 { self.rng.r16() }

    pub fn reraise_owner(&mut self, entry: &SkillHookPlanEntry, hp: i32) -> Result<(), EffectContextError> {
        assert_eq!(
            entry.owner, self.owner,
            "runtime_v2 reraise hook entry owner must match skill context owner"
        );
        let owner = self.entities.get_mut(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let current_level = owner
            .template
            .skills
            .level_at(entry.fixed_lane)
            .unwrap_or_else(|| panic!("runtime_v2 reraise level missing for fixed lane {}", entry.fixed_lane));
        assert!(
            owner.template.skills.set_level_at(entry.fixed_lane, current_level.div_ceil(2)),
            "runtime_v2 reraise fixed lane disappeared: {}",
            entry.fixed_lane
        );
        owner.runtime.hp = hp.max(1).min(owner.template.max_hp);
        owner.runtime.alive = true;
        Ok(())
    }

    pub fn owner_mp_ready(&mut self) -> Result<bool, EffectContextError> {
        let active = self
            .entities
            .get(self.owner)
            .ok_or(EffectContextError::UnknownEntity(self.owner))
            .map(EntityRecord::is_active)?;
        if !active {
            return Ok(false);
        }
        let required_mp = self.rng.r3x3() as i32;
        let owner = self.entities.get_mut(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        if owner.runtime.magic_point < required_mp {
            return Ok(false);
        }
        owner.runtime.magic_point -= required_mp;
        Ok(true)
    }

    pub fn owner_attack_power(&mut self, use_magic: bool) -> Result<f64, EffectContextError> {
        let owner = self.entities.get(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        Ok(owner.runtime.get_at(use_magic, self.rng))
    }

    pub fn defend_caster_active(&self) -> Result<bool, EffectContextError> {
        let caster = self.defend_caster().ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let caster = self.entities.get(caster).ok_or(EffectContextError::UnknownEntity(caster))?;
        Ok(caster.runtime.active())
    }

    pub fn refresh_owner_protect_target(&mut self, level: u32) -> Result<Option<EntityIdx>, EffectContextError> {
        self.require(ExtensionCapability::ReadAllies)?;
        let owner = self.entities.get(self.owner).ok_or(EffectContextError::UnknownEntity(self.owner))?;
        let wisdom = owner.runtime.wisdom.max(0) as u32;
        let effective_team = owner
            .states
            .entries()
            .iter()
            .find_map(|entry| match &entry.payload {
                StatePayload::Charm {
                    group_id,
                    effective_team_idx,
                    ..
                } => (*effective_team_idx).or_else(|| {
                    u32::try_from(*group_id)
                        .ok()
                        .and_then(|group_entity| self.entities.get(EntityIdx(group_entity)))
                        .map(|entity| entity.runtime.team)
                }),
                _ => None,
            })
            .unwrap_or(owner.runtime.team);
        let candidates = self
            .world
            .team_alive(effective_team)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|candidate| {
                self.entities
                    .get(*candidate)
                    .is_some_and(|entity| entity.runtime.alive && entity.runtime.hp > 0)
            })
            .collect::<Vec<_>>();

        // legacy Protect 会先消耗 smart 判定，再处理有效友军列表为空的情况。
        let smart = self.rng.r127() < wisdom;
        let next_target = if candidates.is_empty() {
            None
        } else {
            let owner_pos = candidates.iter().position(|candidate| *candidate == self.owner);
            let select_count = if smart { 3 } else { 2 };
            let mut selected = Vec::with_capacity(select_count);
            let mut dup = 0usize;
            let mut invalid = -(select_count as i32);
            while dup <= select_count && invalid <= select_count as i32 {
                let picked = if let Some(owner_pos) = owner_pos {
                    self.rng.pick_skip(&candidates, owner_pos)
                } else {
                    self.rng.pick(&candidates)
                };
                let Some(picked) = picked else {
                    break;
                };
                let target = candidates[picked];
                let valid = self.entities.get(target).is_some_and(|entity| !entity.runtime.is_combat_minion());
                if !valid {
                    invalid += 1;
                    continue;
                }
                if selected.contains(&target) {
                    dup += 1;
                    continue;
                }
                selected.push(target);
                if selected.len() >= select_count {
                    break;
                }
            }
            let mut scored = selected
                .into_iter()
                .map(|target| {
                    let entity = self
                        .entities
                        .get(target)
                        .unwrap_or_else(|| panic!("runtime_v2 protect candidate disappeared: {}", target.0));
                    let score = if smart {
                        let hp = entity.runtime.hp;
                        let rate_hi_hp = if hp < 20 {
                            30.0
                        } else if hp > 300 {
                            300.0
                        } else {
                            hp as f64
                        };
                        (1.0 / rate_hi_hp) * entity.runtime.atk_sum as f64 / (entity.runtime.protect_from.len() + 1) as f64
                    } else {
                        self.rng.rFFFF() as f64
                    };
                    (target, score)
                })
                .collect::<Vec<_>>();
            scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
            scored.first().map(|(target, _)| *target)
        };

        let old_target = self
            .entities
            .get(self.owner)
            .ok_or(EffectContextError::UnknownEntity(self.owner))?
            .runtime
            .protect_to;
        if old_target != next_target {
            if let Some(old_target) = old_target
                && let Some(target) = self.entities.get_mut(old_target)
            {
                target.runtime.protect_from.retain(|link| link.owner != self.owner);
                if target.runtime.protect_from.is_empty() {
                    target.runtime.protect_pre_defend_skill_count = None;
                    target.states.clear_compressed_legacy_state(CompressedLegacyState::Protect);
                }
            }
            self.entities
                .get_mut(self.owner)
                .ok_or(EffectContextError::UnknownEntity(self.owner))?
                .runtime
                .protect_to = next_target;
        }
        if let Some(next_target) = next_target {
            let pre_defend_skill_count = {
                let target = self.entities.get(next_target).ok_or(EffectContextError::UnknownEntity(next_target))?;
                target
                    .template
                    .skills
                    .active_order()
                    .iter()
                    .filter(|fixed_lane| {
                        target.template.skills.level_at(**fixed_lane).is_some_and(|level| level > 0)
                            && target
                                .template
                                .skills
                                .skills()
                                .get(**fixed_lane)
                                .and_then(|skill_id| self.registry.skill(*skill_id))
                                .is_some_and(|skill| skill.hook_mask.intersects(ProcMask::PRE_DEFEND))
                    })
                    .count()
            };
            let target = self.entities.get_mut(next_target).ok_or(EffectContextError::UnknownEntity(next_target))?;
            if target.runtime.protect_from.is_empty() {
                target.runtime.protect_pre_defend_skill_count = Some(pre_defend_skill_count);
                target.states.register_compressed_legacy_state(CompressedLegacyState::Protect);
            }
            if let Some(link) = target.runtime.protect_from.iter_mut().find(|link| link.owner == self.owner) {
                link.level = level;
            } else {
                target.runtime.protect_from.push(ProtectLinkRuntime {
                    owner: self.owner,
                    level,
                });
            }
        }
        Ok(next_target)
    }

    pub fn sync_winner(&mut self) -> Option<usize> { self.world.sync_winner(self.entities) }

    pub fn defend_atp(&self) -> Option<f64> { self.defend_value.as_ref().and_then(|value| value.atp()) }

    pub fn defend_is_magic(&self) -> Option<bool> { self.defend_value.as_ref().and_then(|value| value.is_magic()) }

    pub fn defend_on_damage(&self) -> PlainAttackOnDamage { self.defend_on_damage }

    pub fn set_defend_atp(&mut self, atp: f64) {
        let Some(value) = self.defend_value.as_deref_mut() else {
            panic!("runtime_v2 defend atp is only available during PRE_DEFEND hooks");
        };
        value.set_atp(atp);
    }

    pub fn defend_damage(&self) -> Option<i32> { self.defend_value.as_ref().and_then(|value| value.damage()) }

    pub fn set_defend_damage(&mut self, damage: i32) {
        let Some(value) = self.defend_value.as_deref_mut() else {
            panic!("runtime_v2 defend damage is only available during POST_DEFEND hooks");
        };
        value.set_damage(damage);
    }

    pub fn defend_caster(&self) -> Option<EntityIdx> { self.defend_value.as_ref().map(|value| value.caster()) }

    pub fn defend_target(&self) -> Option<EntityIdx> { self.defend_value.as_ref().map(|value| value.target()) }

    pub fn owner_state_payload(&self, legacy_order_key: u32) -> Option<StatePayload> {
        self.owner()?.states.entry(legacy_order_key).map(|entry| entry.payload.clone())
    }

    pub fn set_owner_state_payload(&mut self, legacy_order_key: u32, payload: StatePayload) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if !owner.states.set_payload(legacy_order_key, payload) {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        owner.refresh_runtime_stats_from_template();
        Ok(())
    }

    fn require(&self, capability: ExtensionCapability) -> Result<(), EffectContextError> {
        if self.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(EffectContextError::MissingCapability(capability))
        }
    }
}

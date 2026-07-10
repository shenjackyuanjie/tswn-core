use super::*;

pub struct StateContext<'a> {
    entities: &'a mut EntityArena,
    world: &'a mut WorldArena,
    template_slots: &'a TemplateSlotStorage,
    slots: &'a mut BattleSlotStorage,
    queue: &'a mut EffectQueue,
    updates: &'a mut RunUpdates,
    rng: &'a mut RC4,
    defend_value: Option<&'a mut RuntimeDefendValue>,
    owner: EntityIdx,
    hook: ProcMask,
    capabilities: &'a [ExtensionCapability],
    action_intercepted: bool,
    action_smart: Option<bool>,
}

impl<'a> StateContext<'a> {
    pub fn new(
        entities: &'a mut EntityArena,
        world: &'a mut WorldArena,
        template_slots: &'a TemplateSlotStorage,
        slots: &'a mut BattleSlotStorage,
        queue: &'a mut EffectQueue,
        updates: &'a mut RunUpdates,
        rng: &'a mut RC4,
        entry: StateHookPlanEntry,
        hook: ProcMask,
        capabilities: &'a [ExtensionCapability],
    ) -> Self {
        Self {
            entities,
            world,
            template_slots,
            slots,
            queue,
            updates,
            rng,
            defend_value: None,
            owner: entry.owner,
            hook,
            capabilities,
            action_intercepted: false,
            action_smart: None,
        }
    }

    pub fn with_defend_value(mut self, defend_value: &'a mut RuntimeDefendValue) -> Self {
        self.defend_value = Some(defend_value);
        self
    }

    pub fn with_action_smart(mut self, smart: bool) -> Self {
        self.action_smart = Some(smart);
        self
    }

    pub fn owner_idx(&self) -> EntityIdx { self.owner }

    pub fn hook(&self) -> ProcMask { self.hook }

    pub fn owner(&self) -> Option<&EntityRecord> { self.entities.get(self.owner) }

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

    pub fn push(&mut self, effect: QueuedEffect) { self.queue.push(effect); }

    pub fn add_update(&mut self, update: RunUpdate) { self.updates.add(update); }

    pub fn add_newline(&mut self) { self.updates.add_newline(); }

    pub fn intercept_action(&mut self) { self.action_intercepted = true; }

    pub fn action_intercepted(&self) -> bool { self.action_intercepted }

    pub fn action_smart(&self) -> Option<bool> { self.action_smart }

    pub fn flat_alive(&self) -> Result<Vec<EntityIdx>, EffectContextError> {
        self.require(ExtensionCapability::ReadAllies)?;
        self.require(ExtensionCapability::ReadEnemies)?;
        Ok(self.world.flat_alive().to_vec())
    }

    pub fn last_non_newline_update(&self) -> Option<&RunUpdate> {
        self.updates
            .updates
            .iter()
            .rev()
            .find(|update| !matches!(update.update_type, UpdateType::NextLine))
    }

    pub fn rng_next_u8(&mut self) -> u8 { self.rng.next_u8() }

    pub fn rng_next_i32(&mut self, max: i32) -> i32 { self.rng.next_i32(max) }

    pub fn rng_r127(&mut self) -> u32 { self.rng.r127() }

    pub fn rng_r_ffff(&mut self) -> u32 { self.rng.rFFFF() }

    pub fn rng_pick_entity(&mut self, candidates: &[EntityIdx]) -> Option<usize> { self.rng.pick(candidates) }

    pub fn rng_pick_skip_range_entity(&mut self, candidates: &[EntityIdx], skip_indices: &[usize]) -> Option<usize> {
        self.rng.pick_skip_range(candidates, skip_indices)
    }

    pub fn sync_winner(&mut self) -> Option<usize> { self.world.sync_winner(self.entities) }

    pub fn defend_atp(&self) -> Option<f64> { self.defend_value.as_ref().and_then(|value| value.atp()) }

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
        Ok(())
    }

    pub fn clear_owner_state(&mut self, legacy_order_key: u32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if !owner.states.clear_legacy_key(legacy_order_key) {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        Ok(())
    }

    pub fn adjust_owner_speed_points(&mut self, delta: i32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        owner.runtime.move_state.speed_points += delta;
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

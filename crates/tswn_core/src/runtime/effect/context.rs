use super::*;

pub struct EffectContext<'a> {
    entities: &'a mut EntityArena,
    world: &'a mut WorldArena,
    template_slots: &'a TemplateSlotStorage,
    slots: &'a mut BattleSlotStorage,
    queue: &'a mut EffectQueue,
    updates: &'a mut RunUpdates,
    rng: &'a mut RC4,
    caster: EntityIdx,
    target: Option<EntityIdx>,
    capabilities: &'a [ExtensionCapability],
}

impl<'a> EffectContext<'a> {
    pub fn new(
        entities: &'a mut EntityArena,
        world: &'a mut WorldArena,
        template_slots: &'a TemplateSlotStorage,
        slots: &'a mut BattleSlotStorage,
        queue: &'a mut EffectQueue,
        updates: &'a mut RunUpdates,
        rng: &'a mut RC4,
        effect: &CustomEffect,
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
            caster: effect.caster,
            target: effect.target,
            capabilities,
        }
    }

    pub fn caster(&self) -> Option<&EntityRecord> { self.entities.get(self.caster) }

    pub fn target(&self) -> Option<&EntityRecord> { self.target.and_then(|target| self.entities.get(target)) }

    pub fn entity(&self, entity: EntityIdx) -> Result<&EntityRecord, EffectContextError> {
        if entity != self.caster && Some(entity) != self.target {
            self.require(ExtensionCapability::ReadEnemies)?;
        }
        self.entities.get(entity).ok_or(EffectContextError::UnknownEntity(entity))
    }

    pub fn battle_slot(&self, id: crate::runtime::BattleSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
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

    pub fn last_non_newline_update(&self) -> Option<&RunUpdate> { self.updates.last_non_newline_update() }

    pub fn rng_next_u8(&mut self) -> u8 { self.rng.next_u8() }

    pub fn rng_next_i32(&mut self, max: i32) -> i32 { self.rng.next_i32(max) }

    pub fn sync_winner(&mut self) -> Option<usize> { self.world.sync_winner(self.entities) }

    fn require(&self, capability: ExtensionCapability) -> Result<(), EffectContextError> {
        if self.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(EffectContextError::MissingCapability(capability))
        }
    }
}

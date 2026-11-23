use super::*;

/// 自定义效果 handler 的执行上下文：实体、世界、槽、队列、更新与 RNG 的受控视图。
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
    /// 从一个待处理的自定义效果构造上下文。
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

    /// 施法者实体记录；实体已不存在时返回 None。
    pub fn caster(&self) -> Option<&EntityRecord> { self.entities.get(self.caster) }

    /// 目标实体记录；效果无目标或目标已不存在时返回 None。
    pub fn target(&self) -> Option<&EntityRecord> { self.target.and_then(|target| self.entities.get(target)) }

    /// 读取任意实体；访问施法者与目标以外的实体需要 ReadEnemies 能力。
    pub fn entity(&self, entity: EntityIdx) -> Result<&EntityRecord, EffectContextError> {
        if entity != self.caster && Some(entity) != self.target {
            self.require(ExtensionCapability::ReadEnemies)?;
        }
        self.entities.get(entity).ok_or(EffectContextError::UnknownEntity(entity))
    }

    /// 读取对局级槽；需要 ReadBattleSlots 能力。
    pub fn battle_slot(&self, id: crate::runtime::BattleSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
        self.require(ExtensionCapability::ReadBattleSlots)?;
        Ok(self.slots.get(id))
    }

    /// 读取模板级槽；需要 ReadTemplateSlots 能力。
    pub fn template_slot(&self, id: TemplateSlotId) -> Result<Option<&SlotValue>, EffectContextError> {
        self.require(ExtensionCapability::ReadTemplateSlots)?;
        Ok(self.template_slots.get(id))
    }

    /// 写入实体槽；需要 MutateEntitySlots 能力，实体不存在返回错误。
    pub fn set_entity_slot(&mut self, entity: EntityIdx, slot: EntitySlotId, value: SlotValue) -> Result<(), EffectContextError> {
        self.require(ExtensionCapability::MutateEntitySlots)?;
        let Some(entity) = self.entities.get_mut(entity) else {
            return Err(EffectContextError::UnknownEntity(entity));
        };
        entity.slots.set(slot, value)?;
        Ok(())
    }

    /// 入队一个优先执行的效果（插到队首）。
    pub fn push_nested(&mut self, effect: QueuedEffect) { self.queue.push_nested(effect); }

    /// 入队一个后续执行的效果（追加到队尾）。
    pub fn push(&mut self, effect: QueuedEffect) { self.queue.push(effect); }

    /// 追加一条展示更新。
    pub fn add_update(&mut self, update: RunUpdate) { self.updates.add(update); }

    /// 追加一个换行分隔。
    pub fn add_newline(&mut self) { self.updates.add_newline(); }

    /// 最近一条非换行更新，用于在既有消息上续写。
    pub fn last_non_newline_update(&self) -> Option<&RunUpdate> { self.updates.last_non_newline_update() }

    /// 从战斗随机流取一个字节。
    pub fn rng_next_u8(&mut self) -> u8 { self.rng.next_u8() }

    /// 从战斗随机流取 [0, max) 内的整数。
    pub fn rng_next_i32(&mut self, max: i32) -> i32 { self.rng.next_i32(max) }

    /// 同步胜负视图，返回已产生的胜者队伍（如有）。
    pub fn sync_winner(&mut self) -> Option<usize> { self.world.sync_winner(self.entities) }

    /// 校验上下文是否持有指定扩展能力；缺失时返回 [`EffectContextError::MissingCapability`]。
    fn require(&self, capability: ExtensionCapability) -> Result<(), EffectContextError> {
        if self.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(EffectContextError::MissingCapability(capability))
        }
    }
}

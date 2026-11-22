//! 状态 hook 上下文，包含状态 owner、当前 hook、受击值及延后清理列表。
//!
//! PRE_ACTION 状态可接管本次行动，并继续适用的恢复与行动后逻辑。
//! POST_ACTION 中有些状态先登记待清理，等状态与穿插技能执行完后统一移除。
//! 状态修改通过 store 与对应效果入口维护条目、游标和计划缓存。

use super::*;

/// 一次状态 handler 调用的受控借用视图；能力与阶段约束同技能上下文。
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
    deferred_owner_state_clears: Option<&'a mut Vec<u32>>,
}

impl<'a> StateContext<'a> {
    /// 从状态 hook 条目与当前 hook 构造上下文。
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
            deferred_owner_state_clears: None,
        }
    }

    /// 附加受击值视图，供 PRE_DEFEND / POST_DEFEND hook 读写。
    pub fn with_defend_value(mut self, defend_value: &'a mut RuntimeDefendValue) -> Self {
        self.defend_value = Some(defend_value);
        self
    }

    /// 记录本次行动的聪明判定结果。
    pub fn with_action_smart(mut self, smart: bool) -> Self {
        self.action_smart = Some(smart);
        self
    }

    /// 附加延后清理列表，POST_ACTION 状态统一到批次结束时移除。
    pub fn with_deferred_owner_state_clears(mut self, clears: &'a mut Vec<u32>) -> Self {
        self.deferred_owner_state_clears = Some(clears);
        self
    }

    /// 状态 owner 的实体下标。
    pub fn owner_idx(&self) -> EntityIdx { self.owner }

    /// 当前执行的 hook 掩码。
    pub fn hook(&self) -> ProcMask { self.hook }

    /// owner 实体记录；实体已不存在时返回 None。
    pub fn owner(&self) -> Option<&EntityRecord> { self.entities.get(self.owner) }

    pub(crate) fn owner_has_boss_infection(&self, key: u32) -> bool {
        self.owner()
            .and_then(|owner| owner.states.entry(key))
            .is_some_and(|entry| entry.is_boss_infection(self.entities))
    }

    /// 读取任意实体；读取 owner 以外的实体时按队友/敌方检查能力。
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

    /// 选敌按魅惑指定的队伍取样，被魅惑者自身仍属于原队。
    pub fn effective_team(&self, entity: EntityIdx) -> Result<usize, EffectContextError> {
        let entity = self.entity(entity)?;
        Ok(entity
            .states
            .entries()
            .iter()
            .find_map(|entry| {
                let (group_id, effective_team, _, _, _) = entry.charm_value()?;
                effective_team.or_else(|| {
                    let group_entity = EntityIdx(u32::try_from(group_id).ok()?);
                    self.entities.get(group_entity).map(|member| member.runtime.team)
                })
            })
            .unwrap_or(entity.runtime.team))
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

    /// 写入实体槽；需要 MutateEntitySlots 能力。
    pub fn set_entity_slot(&mut self, entity: EntityIdx, slot: EntitySlotId, value: SlotValue) -> Result<(), EffectContextError> {
        self.require(ExtensionCapability::MutateEntitySlots)?;
        let Some(entity) = self.entities.get_mut(entity) else {
            return Err(EffectContextError::UnknownEntity(entity));
        };
        entity.slots.set(slot, value)?;
        Ok(())
    }

    /// 入队一个优先执行的效果。
    pub fn push_nested(&mut self, effect: QueuedEffect) { self.queue.push_nested(effect); }

    /// 入队一个后续执行的效果。
    pub fn push(&mut self, effect: QueuedEffect) { self.queue.push(effect); }

    /// 追加一条展示更新。
    pub fn add_update(&mut self, update: RunUpdate) { self.updates.add(update); }

    /// 追加换行分隔。
    pub fn add_newline(&mut self) { self.updates.add_newline(); }

    /// 标记本次行动被当前状态接管。
    pub fn intercept_action(&mut self) { self.action_intercepted = true; }

    /// 本次行动是否已被状态接管。
    pub fn action_intercepted(&self) -> bool { self.action_intercepted }

    /// 本次行动的聪明判定结果（未提供时为 None）。
    pub fn action_smart(&self) -> Option<bool> { self.action_smart }

    /// 全场存活者列表；需要同时具备 ReadAllies 与 ReadEnemies 能力。
    pub fn flat_alive(&self) -> Result<Vec<EntityIdx>, EffectContextError> {
        self.require(ExtensionCapability::ReadAllies)?;
        self.require(ExtensionCapability::ReadEnemies)?;
        Ok(self.world.flat_alive().to_vec())
    }

    /// 上一条更新是否是指定双方的普通防御消息。
    pub fn last_update_was_plain_defense(&self, caster: EntityIdx, target: EntityIdx) -> bool {
        self.updates.last_update_was_plain_defense(caster.0 as usize, target.0 as usize)
    }

    /// 取一个随机字节。
    pub fn rng_next_u8(&mut self) -> u8 { self.rng.next_u8() }

    /// 取 [0, max) 内的随机整数。
    pub fn rng_next_i32(&mut self, max: i32) -> i32 { self.rng.next_i32(max) }

    /// 取 [0, 127] 内的随机值。
    pub fn rng_r127(&mut self) -> u32 { self.rng.r127() }

    /// 取 16 位随机值。
    pub fn rng_r_ffff(&mut self) -> u32 { self.rng.rFFFF() }

    /// 在候选实体中等概率抽取一个下标；候选为空返回 None。
    pub fn rng_pick_entity(&mut self, candidates: &[EntityIdx]) -> Option<usize> { self.rng.pick(candidates) }

    /// 在候选实体中跳过指定下标集合后抽取。
    pub fn rng_pick_skip_range_entity(&mut self, candidates: &[EntityIdx], skip_indices: &[usize]) -> Option<usize> {
        self.rng.pick_skip_range(candidates, skip_indices)
    }

    /// 同步胜负视图，返回已产生的胜者队伍（如有）。
    pub fn sync_winner(&mut self) -> Option<usize> { self.world.sync_winner(self.entities) }

    /// 受击值中的攻击威力（PRE_DEFEND 阶段为浮点 ATP）。
    pub fn defend_atp(&self) -> Option<f64> { self.defend_value.as_ref().and_then(|value| value.atp()) }

    /// 改写攻击威力；仅在 PRE_DEFEND 阶段可用，其他阶段调用会 panic。
    pub fn set_defend_atp(&mut self, atp: f64) {
        let Some(value) = self.defend_value.as_deref_mut() else {
            panic!("runtime defend atp is only available during PRE_DEFEND hooks");
        };
        value.set_atp(atp);
    }

    /// 受击值中的整数伤害（POST_DEFEND 阶段）。
    pub fn defend_damage(&self) -> Option<i32> { self.defend_value.as_ref().and_then(|value| value.damage()) }

    /// 改写整数伤害；仅在 POST_DEFEND 阶段可用，其他阶段调用会 panic。
    pub fn set_defend_damage(&mut self, damage: i32) {
        let Some(value) = self.defend_value.as_deref_mut() else {
            panic!("runtime defend damage is only available during POST_DEFEND hooks");
        };
        value.set_damage(damage);
    }

    /// 受击值中的攻击者下标。
    pub fn defend_caster(&self) -> Option<EntityIdx> { self.defend_value.as_ref().map(|value| value.caster()) }

    /// 受击值中的目标下标。
    pub fn defend_target(&self) -> Option<EntityIdx> { self.defend_value.as_ref().map(|value| value.target()) }

    /// 克隆读取 owner 指定顺序键的状态载荷；状态不存在返回 None。
    pub fn owner_state_payload(&self, legacy_order_key: u32) -> Option<StatePayload> {
        self.owner()?.states.entry(legacy_order_key).map(|entry| entry.payload.clone())
    }

    /// 写入 owner 的状态载荷并刷新派生数值；顺序键不存在返回错误。
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

    /// 写入 owner 的状态载荷但不刷新派生数值，供批量修改后统一刷新。
    pub fn set_owner_state_payload_without_refresh(
        &mut self,
        legacy_order_key: u32,
        payload: StatePayload,
    ) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if !owner.states.set_payload(legacy_order_key, payload) {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        Ok(())
    }

    /// 延后清理 owner 状态；没有延后列表时立即清理。
    pub fn defer_owner_state_clear(&mut self, legacy_order_key: u32) -> Result<(), EffectContextError> {
        if self
            .entities
            .get(self.owner)
            .and_then(|owner| owner.states.entry(legacy_order_key))
            .is_none()
        {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        let Some(clears) = self.deferred_owner_state_clears.as_deref_mut() else {
            return self.clear_owner_state(legacy_order_key);
        };
        if !clears.contains(&legacy_order_key) {
            clears.push(legacy_order_key);
        }
        Ok(())
    }

    /// 立即清除 owner 状态并刷新派生数值；状态不存在返回错误。
    pub fn clear_owner_state(&mut self, legacy_order_key: u32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if !owner.states.clear_legacy_key(legacy_order_key) {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        owner.refresh_runtime_stats_from_template();
        Ok(())
    }

    /// 清除 owner 状态但不刷新派生数值；状态不存在返回错误。
    pub fn clear_owner_state_without_refresh(&mut self, legacy_order_key: u32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        if !owner.states.clear_legacy_key(legacy_order_key) {
            return Err(EffectContextError::UnknownEntity(self.owner));
        }
        Ok(())
    }

    /// 按增量调整 owner 的行动点，可为负。
    pub fn adjust_owner_speed_points(&mut self, delta: i32) -> Result<(), EffectContextError> {
        let Some(owner) = self.entities.get_mut(self.owner) else {
            return Err(EffectContextError::UnknownEntity(self.owner));
        };
        owner.runtime.move_state.speed_points += delta;
        Ok(())
    }

    /// 校验上下文是否持有指定扩展能力；缺失时返回 [`EffectContextError::MissingCapability`]。
    fn require(&self, capability: ExtensionCapability) -> Result<(), EffectContextError> {
        if self.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(EffectContextError::MissingCapability(capability))
        }
    }
}

use crate::engine::update::{RunUpdate, RunUpdates};
use crate::rc4::RC4;
use crate::runtime_v2::entity::{EntityIdx, PlayerTemplate, StateEntry};
use crate::runtime_v2::extension::{
    EffectHandlerId, ExtensionCapability, ExtensionRegistry, ReplayRendererId, ShowRendererId, SkillId, StateId,
};
use crate::runtime_v2::scheduler::{SkillHookPlanEntry, StateHookPlanEntry};
use crate::runtime_v2::{
    BattleSlotStorage, EntityArena, EntityRecord, EntitySlotId, SlotError, SlotValue, TemplateSlotId, TemplateSlotStorage,
    WorldArena,
};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueuedEffect {
    Damage {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
    },
    SummonExplode {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
    },
    Heal {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
    },
    Spawn {
        caster: EntityIdx,
        template: PlayerTemplate,
    },
    SpawnSilent {
        caster: EntityIdx,
        template: PlayerTemplate,
    },
    SpawnWithMessage {
        caster: EntityIdx,
        template: PlayerTemplate,
        message: String,
    },
    AddState {
        target: EntityIdx,
        state: StateEntry,
    },
    ClearState {
        target: EntityIdx,
        legacy_order_key: u32,
    },
    Revive {
        caster: EntityIdx,
        target: EntityIdx,
        hp: i32,
    },
    ReviveWithMessage {
        caster: EntityIdx,
        target: EntityIdx,
        hp: i32,
        message: String,
    },
    Remove {
        caster: EntityIdx,
        target: EntityIdx,
    },
    Merge {
        caster: EntityIdx,
        target: EntityIdx,
    },
    Replay {
        caster: EntityIdx,
        target: EntityIdx,
        message: String,
        score: u32,
    },
    Custom(CustomEffect),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEffect {
    pub handler: EffectHandlerId,
    pub caster: EntityIdx,
    pub target: Option<EntityIdx>,
    pub payload: CustomEffectPayload,
}

impl CustomEffect {
    pub fn new(handler: EffectHandlerId, caster: EntityIdx, target: Option<EntityIdx>, payload: CustomEffectPayload) -> Self {
        Self {
            handler,
            caster,
            target,
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomEffectPayload {
    None,
    Int(i32),
    Text(String),
}

pub type EffectHandlerFn = fn(&mut EffectContext<'_>, &CustomEffect);
pub type SkillHandlerFn = fn(&mut SkillContext<'_>, &SkillHookPlanEntry);
pub type StateHandlerFn = fn(&mut StateContext<'_>, &StateHookPlanEntry);
pub type ReplayRendererFn = fn(&RuntimeFrame) -> Option<RenderedReplay>;
pub type ShowRendererFn = fn(&RuntimeFrame) -> Option<RenderedShow>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContextError {
    MissingCapability(ExtensionCapability),
    UnknownEntity(EntityIdx),
    Slot(SlotError),
}

impl From<SlotError> for EffectContextError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectHandlers {
    handlers: Vec<Option<EffectHandlerFn>>,
    capabilities: Vec<Vec<ExtensionCapability>>,
}

impl EffectHandlers {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.effect_handlers().len()],
            capabilities: vec![Vec::new(); registry.effect_handlers().len()],
        }
    }

    pub fn set(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.set_with_capabilities(id, handler, &[]); }

    pub fn set_with_capabilities(&mut self, id: EffectHandlerId, handler: EffectHandlerFn, capabilities: &[ExtensionCapability]) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime_v2 effect handler id: {}", id.0);
        };
        *slot = Some(handler);
        self.capabilities[id.0 as usize] = capabilities.to_vec();
    }

    pub fn get(&self, id: EffectHandlerId) -> Option<EffectHandlerFn> {
        self.handlers.get(id.0 as usize).and_then(|handler| *handler)
    }

    pub fn capabilities(&self, id: EffectHandlerId) -> Option<&[ExtensionCapability]> {
        self.capabilities.get(id.0 as usize).map(Vec::as_slice)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkillHandlers {
    handlers: Vec<Option<SkillHandlerFn>>,
    capabilities: Vec<Vec<ExtensionCapability>>,
}

impl SkillHandlers {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.skills().len()],
            capabilities: vec![Vec::new(); registry.skills().len()],
        }
    }

    pub fn set(&mut self, id: SkillId, handler: SkillHandlerFn) { self.set_with_capabilities(id, handler, &[]); }

    pub fn set_with_capabilities(&mut self, id: SkillId, handler: SkillHandlerFn, capabilities: &[ExtensionCapability]) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime_v2 skill handler id: {}", id.0);
        };
        *slot = Some(handler);
        self.capabilities[id.0 as usize] = capabilities.to_vec();
    }

    pub fn get(&self, id: SkillId) -> Option<SkillHandlerFn> { self.handlers.get(id.0 as usize).and_then(|handler| *handler) }

    pub fn capabilities(&self, id: SkillId) -> Option<&[ExtensionCapability]> {
        self.capabilities.get(id.0 as usize).map(Vec::as_slice)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateHandlers {
    handlers: Vec<Option<StateHandlerFn>>,
    capabilities: Vec<Vec<ExtensionCapability>>,
}

impl StateHandlers {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.states().len()],
            capabilities: vec![Vec::new(); registry.states().len()],
        }
    }

    pub fn set(&mut self, id: StateId, handler: StateHandlerFn) { self.set_with_capabilities(id, handler, &[]); }

    pub fn set_with_capabilities(&mut self, id: StateId, handler: StateHandlerFn, capabilities: &[ExtensionCapability]) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime_v2 state handler id: {}", id.0);
        };
        *slot = Some(handler);
        self.capabilities[id.0 as usize] = capabilities.to_vec();
    }

    pub fn get(&self, id: StateId) -> Option<StateHandlerFn> { self.handlers.get(id.0 as usize).and_then(|handler| *handler) }

    pub fn capabilities(&self, id: StateId) -> Option<&[ExtensionCapability]> {
        self.capabilities.get(id.0 as usize).map(Vec::as_slice)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedReplay {
    pub renderer: ReplayRendererId,
    pub payload: String,
}

impl RenderedReplay {
    pub fn new(renderer: ReplayRendererId, payload: impl Into<String>) -> Self {
        Self {
            renderer,
            payload: payload.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedShow {
    pub renderer: ShowRendererId,
    pub payload: String,
}

impl RenderedShow {
    pub fn new(renderer: ShowRendererId, payload: impl Into<String>) -> Self {
        Self {
            renderer,
            payload: payload.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreReplayEvent {
    pub message: String,
    pub caster: usize,
    pub target: usize,
    pub targets: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreShowEvent {
    pub text: String,
    pub score: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReplayRenderers {
    renderers: Vec<Option<ReplayRendererFn>>,
}

impl ReplayRenderers {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            renderers: vec![None; registry.replay_renderers().len()],
        }
    }

    pub fn set(&mut self, id: ReplayRendererId, renderer: ReplayRendererFn) {
        let Some(slot) = self.renderers.get_mut(id.0 as usize) else {
            panic!("unknown runtime_v2 replay renderer id: {}", id.0);
        };
        *slot = Some(renderer);
    }

    pub fn get(&self, id: ReplayRendererId) -> Option<ReplayRendererFn> {
        self.renderers.get(id.0 as usize).and_then(|renderer| *renderer)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShowRenderers {
    renderers: Vec<Option<ShowRendererFn>>,
}

impl ShowRenderers {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            renderers: vec![None; registry.show_renderers().len()],
        }
    }

    pub fn set(&mut self, id: ShowRendererId, renderer: ShowRendererFn) {
        let Some(slot) = self.renderers.get_mut(id.0 as usize) else {
            panic!("unknown runtime_v2 show renderer id: {}", id.0);
        };
        *slot = Some(renderer);
    }

    pub fn get(&self, id: ShowRendererId) -> Option<ShowRendererFn> {
        self.renderers.get(id.0 as usize).and_then(|renderer| *renderer)
    }
}

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

pub struct SkillContext<'a> {
    entities: &'a mut EntityArena,
    world: &'a mut WorldArena,
    template_slots: &'a TemplateSlotStorage,
    slots: &'a mut BattleSlotStorage,
    queue: &'a mut EffectQueue,
    updates: &'a mut RunUpdates,
    rng: &'a mut RC4,
    owner: EntityIdx,
    capabilities: &'a [ExtensionCapability],
}

impl<'a> SkillContext<'a> {
    pub fn new(
        entities: &'a mut EntityArena,
        world: &'a mut WorldArena,
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
            template_slots,
            slots,
            queue,
            updates,
            rng,
            owner: entry.owner,
            capabilities,
        }
    }

    pub fn owner_idx(&self) -> EntityIdx { self.owner }

    pub fn owner(&self) -> Option<&EntityRecord> { self.entities.get(self.owner) }

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

pub struct StateContext<'a> {
    entities: &'a mut EntityArena,
    world: &'a mut WorldArena,
    template_slots: &'a TemplateSlotStorage,
    slots: &'a mut BattleSlotStorage,
    queue: &'a mut EffectQueue,
    updates: &'a mut RunUpdates,
    rng: &'a mut RC4,
    owner: EntityIdx,
    capabilities: &'a [ExtensionCapability],
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
            owner: entry.owner,
            capabilities,
        }
    }

    pub fn owner_idx(&self) -> EntityIdx { self.owner }

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

    pub fn add_update(&mut self, update: RunUpdate) { self.updates.add(update); }

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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectQueue {
    effects: VecDeque<QueuedEffect>,
}

impl EffectQueue {
    pub fn push(&mut self, effect: QueuedEffect) { self.effects.push_back(effect); }

    pub fn push_nested(&mut self, effect: QueuedEffect) { self.effects.push_front(effect); }

    pub fn pop_next(&mut self) -> Option<QueuedEffect> { self.effects.pop_front() }

    pub fn len(&self) -> usize { self.effects.len() }

    pub fn is_empty(&self) -> bool { self.effects.is_empty() }

    pub fn clear(&mut self) { self.effects.clear(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::{ExtensionRegistryBuilder, ProcMask, SkillPriority, StateId, TargetPolicy};

    fn damage(amount: i32) -> QueuedEffect {
        QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount,
        }
    }

    fn custom(handler: EffectHandlerId, amount: i32) -> QueuedEffect {
        QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(amount),
        ))
    }

    #[test]
    fn effect_queue_pops_root_effects_in_batch_order() {
        let mut queue = EffectQueue::default();
        queue.push(damage(1));
        queue.push(damage(2));

        assert_eq!(queue.pop_next(), Some(damage(1)));
        assert_eq!(queue.pop_next(), Some(damage(2)));
        assert_eq!(queue.pop_next(), None);
    }

    #[test]
    fn effect_queue_pops_nested_effects_before_older_siblings() {
        let mut queue = EffectQueue::default();
        queue.push(damage(1));
        queue.push(damage(2));

        assert_eq!(queue.pop_next(), Some(damage(1)));
        queue.push_nested(damage(3));

        assert_eq!(queue.pop_next(), Some(damage(3)));
        assert_eq!(queue.pop_next(), Some(damage(2)));
        assert_eq!(queue.pop_next(), None);
    }

    #[test]
    fn effect_queue_pops_custom_effects_in_nested_order() {
        let mut queue = EffectQueue::default();
        queue.push(custom(EffectHandlerId(0), 1));
        queue.push(custom(EffectHandlerId(0), 2));

        assert_eq!(queue.pop_next(), Some(custom(EffectHandlerId(0), 1)));
        queue.push_nested(custom(EffectHandlerId(0), 3));

        assert_eq!(queue.pop_next(), Some(custom(EffectHandlerId(0), 3)));
        assert_eq!(queue.pop_next(), Some(custom(EffectHandlerId(0), 2)));
        assert_eq!(queue.pop_next(), None);
    }

    #[test]
    fn effect_handlers_size_from_registry_and_reject_unknown_ids() {
        let mut builder = ExtensionRegistryBuilder::default();
        let handler = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut handlers = EffectHandlers::from_registry(&registry);

        handlers.set(handler, |_, _| {});

        assert!(handlers.get(handler).is_some());
        assert!(handlers.get(EffectHandlerId(1)).is_none());
    }

    #[test]
    fn skill_handlers_size_from_registry_and_reject_unknown_ids() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill("custom", "mark", "custom.mark", TargetPolicy::Enemy, SkillPriority(0))
            .expect("skill should register");
        let registry = builder.build();
        let mut handlers = SkillHandlers::from_registry(&registry);

        handlers.set(skill, |_, _| {});

        assert!(handlers.get(skill).is_some());
        assert!(handlers.get(SkillId(1)).is_none());
    }

    #[test]
    fn state_handlers_size_from_registry_and_reject_unknown_ids() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut handlers = StateHandlers::from_registry(&registry);

        handlers.set(state, |_, _| {});

        assert!(handlers.get(state).is_some());
        assert!(handlers.get(StateId(1)).is_none());
    }

    #[test]
    fn renderer_tables_size_from_registry_and_reject_unknown_ids() {
        let mut builder = ExtensionRegistryBuilder::default();
        let replay = builder
            .register_replay_renderer("custom", "replay", "custom.replay", SkillPriority(0))
            .expect("replay renderer should register");
        let show = builder
            .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
            .expect("show renderer should register");
        let registry = builder.build();
        let mut replay_renderers = ReplayRenderers::from_registry(&registry);
        let mut show_renderers = ShowRenderers::from_registry(&registry);

        replay_renderers.set(replay, |_| None);
        show_renderers.set(show, |_| None);

        assert!(replay_renderers.get(replay).is_some());
        assert!(replay_renderers.get(ReplayRendererId(1)).is_none());
        assert!(show_renderers.get(show).is_some());
        assert!(show_renderers.get(ShowRendererId(1)).is_none());
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeFrame {
    pub updates: RunUpdates,
}

impl RuntimeFrame {
    pub fn damage_update(caster: usize, target: usize, amount: i32) -> RunUpdate {
        RunUpdate::new("[0]攻击[1]", caster, target, amount.max(0) as u32)
    }

    pub fn heal_update(caster: usize, target: usize, amount: i32) -> RunUpdate {
        RunUpdate::new("[1]回复体力[2]点", caster, target, amount.max(0) as u32)
    }

    pub fn spawn_update(caster: usize, spawned: usize) -> RunUpdate {
        RunUpdate::new("出现一个新的[1]", caster, spawned, 0)
    }

    pub fn add_state_update(target: usize) -> RunUpdate { RunUpdate::new("[1]状态改变", target, target, 0) }

    pub fn clear_state_update(target: usize) -> RunUpdate { RunUpdate::new("[1]状态解除", target, target, 0) }

    pub fn revive_update(caster: usize, target: usize, hp: i32) -> RunUpdate {
        RunUpdate::new("[1][复活]了", caster, target, hp.max(0) as u32)
    }

    pub fn remove_update(caster: usize, target: usize) -> RunUpdate { RunUpdate::new("[1]消失了", caster, target, 0) }

    pub fn replay_update(caster: usize, target: usize, message: impl Into<String>, score: u32) -> RunUpdate {
        RunUpdate::new(message.into(), caster, target, score)
    }

    pub fn single_damage(caster: usize, target: usize, amount: i32) -> Self {
        let mut updates = RunUpdates::new();
        updates.add(Self::damage_update(caster, target, amount));
        Self { updates }
    }

    pub fn render_core_replay(&self) -> Vec<CoreReplayEvent> {
        self.updates
            .updates
            .iter()
            .map(|update| CoreReplayEvent {
                message: update.message.to_string(),
                caster: update.caster,
                target: update.target,
                targets: update.targets.iter().copied().collect(),
                param: update.param,
                score: update.score,
            })
            .collect()
    }

    pub fn render_core_show(&self) -> Vec<CoreShowEvent> {
        self.updates
            .updates
            .iter()
            .map(|update| CoreShowEvent {
                text: update.msg(),
                score: update.score,
            })
            .collect()
    }
}

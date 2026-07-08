use crate::engine::update::{RunUpdate, RunUpdates};
use crate::runtime_v2::entity::EntityIdx;
use crate::runtime_v2::extension::{EffectHandlerId, ExtensionRegistry, ReplayRendererId, ShowRendererId};
use crate::runtime_v2::{BattleSlotStorage, EntityArena, WorldArena};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueuedEffect {
    Damage {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
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
pub type ReplayRendererFn = fn(&RuntimeFrame) -> Option<RenderedReplay>;
pub type ShowRendererFn = fn(&RuntimeFrame) -> Option<RenderedShow>;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectHandlers {
    handlers: Vec<Option<EffectHandlerFn>>,
}

impl EffectHandlers {
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.effect_handlers().len()],
        }
    }

    pub fn set(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime_v2 effect handler id: {}", id.0);
        };
        *slot = Some(handler);
    }

    pub fn get(&self, id: EffectHandlerId) -> Option<EffectHandlerFn> {
        self.handlers.get(id.0 as usize).and_then(|handler| *handler)
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
    pub entities: &'a mut EntityArena,
    pub world: &'a mut WorldArena,
    pub slots: &'a mut BattleSlotStorage,
    pub queue: &'a mut EffectQueue,
    pub updates: &'a mut RunUpdates,
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
    use crate::runtime_v2::{ExtensionRegistryBuilder, SkillPriority};

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

    pub fn single_damage(caster: usize, target: usize, amount: i32) -> Self {
        let mut updates = RunUpdates::new();
        updates.add(Self::damage_update(caster, target, amount));
        Self { updates }
    }
}

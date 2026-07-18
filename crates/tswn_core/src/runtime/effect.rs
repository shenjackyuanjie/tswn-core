use crate::engine::update::{RunUpdate, RunUpdates};
use crate::rc4::RC4;
use crate::runtime::combat::PlainAttackOnDamage;
use crate::runtime::entity::{AccumulateRuntime, ChargeRuntime, EntityIdx, PlayerTemplate, StateEntry, StatePayload};
use crate::runtime::extension::{
    EffectHandlerId, ExtensionCapability, ExtensionRegistry, ReplayRendererId, ShowRendererId, SkillId, StateId,
};
use crate::runtime::scheduler::{SkillHookPlanEntry, StateHookPlanEntry};
use crate::runtime::{
    BattleSlotStorage, CompressedLegacyState, EntityArena, EntityRecord, EntitySlotId, ProcMask, ProtectLinkRuntime,
    RuntimeDefendValue, SlotError, SlotValue, TemplateSlotId, TemplateSlotStorage, WorldArena,
};
use std::collections::VecDeque;

mod context;
mod frame;
mod queue;
mod skill_context;
mod state_context;

pub use context::*;
pub use frame::*;
pub use queue::*;
pub use skill_context::*;
pub use state_context::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueuedEffect {
    Damage {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
    },
    ReflectedAttack {
        caster: EntityIdx,
        target: EntityIdx,
        atp_bits: u64,
        on_damage: PlainAttackOnDamage,
    },
    PoisonTick {
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
    },
    FireAttack {
        caster: EntityIdx,
        target: EntityIdx,
        fire_state_key: u32,
    },
    SummonExplode {
        caster: EntityIdx,
        target: EntityIdx,
        fire_state_key: u32,
    },
    DisperseAttack {
        caster: EntityIdx,
        target: EntityIdx,
    },
    DisperseHit {
        caster: EntityIdx,
        target: EntityIdx,
        damage: i32,
    },
    CovidContact {
        owner: EntityIdx,
        candidate: EntityIdx,
        boss: EntityIdx,
        mutation: i32,
    },
    CovidAttack {
        owner: EntityIdx,
        candidate: EntityIdx,
        boss: EntityIdx,
        mutation: i32,
    },
    CovidPneumonia {
        owner: EntityIdx,
        boss: EntityIdx,
        mutation: i32,
    },
    LazyFlare {
        owner: EntityIdx,
        boss: EntityIdx,
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
    AddBerserkState {
        target: EntityIdx,
        legacy_order_key: u32,
        step: i32,
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
            panic!("unknown runtime effect handler id: {}", id.0);
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
            panic!("unknown runtime skill handler id: {}", id.0);
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
            panic!("unknown runtime state handler id: {}", id.0);
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
            panic!("unknown runtime replay renderer id: {}", id.0);
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
            panic!("unknown runtime show renderer id: {}", id.0);
        };
        *slot = Some(renderer);
    }

    pub fn get(&self, id: ShowRendererId) -> Option<ShowRendererFn> {
        self.renderers.get(id.0 as usize).and_then(|renderer| *renderer)
    }
}

#[cfg(test)]
mod tests;

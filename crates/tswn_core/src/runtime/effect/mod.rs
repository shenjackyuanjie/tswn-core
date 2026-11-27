//! hook 上下文、效果队列与可见事件出口。
//!
//! [`SkillContext`] 和 [`StateContext`] 为相应 hook 提供当前实体、随机数、受击值与
//! 受控修改入口；[`EffectContext`] 供自定义效果 handler 使用。
//! [`QueuedEffect`] 表达伤害、恢复、召唤、状态与复活等操作，由 Runtime 统一提交。
//!
//! # 两种入队顺序
//!
//! [`EffectQueue::push`] 追加到队尾；[`EffectQueue::push_nested`] 插到队首，优先执行
//! 嵌套的反弹、吞噬等链路。连续 push_nested 的相对次序由队首插入决定。
//! handler 可以继续入队，所以一次提交不等于只处理原来的队列长度。
//!
//! [`RuntimeFrame`] / [`RunUpdates`] 保存已执行事件的展示数据；实际伤害通过机制
//! 入口或效果提交。批量 completion 路径以机制与统计为出口，省去展示捕获。
//! 注册 handler 的元数据见 [`super::extension`]，实际绑定位于 [`super::CombatRuntime`]。

use crate::rc4::RC4;
use crate::runtime::combat::PlainAttackOnDamage;
use crate::runtime::entity::{AccumulateRuntime, ChargeRuntime, EntityIdx, PlayerTemplate, StateEntry, StatePayload};
use crate::runtime::extension::{
    EffectHandlerId, ExtensionCapability, ExtensionRegistry, ReplayRendererId, ShowRendererId, SkillId, StateId,
};
use crate::runtime::scheduler::{SkillHookPlanEntry, StateHookPlanEntry};
use crate::runtime::update::{RunUpdate, RunUpdates};
use crate::runtime::{
    BattleSlotStorage, CompressedLegacyState, DamageKind, EntityArena, EntityRecord, EntitySlotId, ProcMask, ProtectLinkRuntime,
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

/// 待提交的战斗效果：伤害、传染、治疗、召唤、状态变更与自定义效果，由 Runtime 统一结算。
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
        kind: DamageKind,
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
        order: Option<u64>,
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
        template: Box<PlayerTemplate>,
    },
    SpawnSilent {
        caster: EntityIdx,
        template: Box<PlayerTemplate>,
    },
    SpawnWithMessage {
        caster: EntityIdx,
        template: Box<PlayerTemplate>,
        message: std::borrow::Cow<'static, str>,
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

/// 自定义效果：交给注册的 effect handler 解释的执行请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomEffect {
    pub handler: EffectHandlerId,
    pub caster: EntityIdx,
    pub target: Option<EntityIdx>,
    pub payload: CustomEffectPayload,
}

impl CustomEffect {
    /// 构造一个自定义效果；`target` 可为空，由 handler 自行解释。
    pub fn new(handler: EffectHandlerId, caster: EntityIdx, target: Option<EntityIdx>, payload: CustomEffectPayload) -> Self {
        Self {
            handler,
            caster,
            target,
            payload,
        }
    }
}

/// 自定义效果载荷：无数据、整数或文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomEffectPayload {
    None,
    Int(i32),
    Text(String),
}

/// effect handler 的函数签名：读取上下文并提交后续效果。
pub type EffectHandlerFn = fn(&mut EffectContext<'_>, &CustomEffect);
/// skill handler 的函数签名：按 hook 计划条目执行技能逻辑。
pub type SkillHandlerFn = fn(&mut SkillContext<'_>, &SkillHookPlanEntry);
/// state handler 的函数签名：按 hook 计划条目执行状态逻辑。
pub type StateHandlerFn = fn(&mut StateContext<'_>, &StateHookPlanEntry);
/// replay 渲染器签名：从 RuntimeFrame 产出可选的回放事件。
pub type ReplayRendererFn = fn(&RuntimeFrame) -> Option<RenderedReplay>;
/// show 渲染器签名：从 RuntimeFrame 产出可选的展示事件。
pub type ShowRendererFn = fn(&RuntimeFrame) -> Option<RenderedShow>;

/// hook 上下文错误：缺少能力、实体不存在或槽操作失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContextError {
    MissingCapability(ExtensionCapability),
    UnknownEntity(EntityIdx),
    Slot(SlotError),
}

impl From<SlotError> for EffectContextError {
    /// 把槽位错误转换为 [`EffectContextError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 按 EffectHandlerId 索引的 effect handler 表。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EffectHandlers {
    handlers: Vec<Option<EffectHandlerFn>>,
    capabilities: Vec<Vec<ExtensionCapability>>,
}

impl EffectHandlers {
    /// 按注册表中的 effect handler 数量建空表。
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.effect_handlers().len()],
            capabilities: vec![Vec::new(); registry.effect_handlers().len()],
        }
    }

    /// 绑定 effect handler，不声明额外能力。
    pub fn set(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.set_with_capabilities(id, handler, &[]); }

    /// 绑定 effect handler 并记录其扩展能力；ID 越界时 panic。
    pub fn set_with_capabilities(&mut self, id: EffectHandlerId, handler: EffectHandlerFn, capabilities: &[ExtensionCapability]) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime effect handler id: {}", id.0);
        };
        *slot = Some(handler);
        self.capabilities[id.0 as usize] = capabilities.to_vec();
    }

    /// 查询已绑定的 effect handler；未绑定返回 None。
    pub fn get(&self, id: EffectHandlerId) -> Option<EffectHandlerFn> {
        self.handlers.get(id.0 as usize).and_then(|handler| *handler)
    }

    /// 查询该 handler 声明的扩展能力。
    pub fn capabilities(&self, id: EffectHandlerId) -> Option<&[ExtensionCapability]> {
        self.capabilities.get(id.0 as usize).map(Vec::as_slice)
    }
}

/// 按 SkillId 索引的 skill handler 表。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkillHandlers {
    handlers: Vec<Option<SkillHandlerFn>>,
    capabilities: Vec<Vec<ExtensionCapability>>,
}

impl SkillHandlers {
    /// 按注册表中的技能数量建空表。
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.skills().len()],
            capabilities: vec![Vec::new(); registry.skills().len()],
        }
    }

    /// 绑定 skill handler，不声明额外能力。
    pub fn set(&mut self, id: SkillId, handler: SkillHandlerFn) { self.set_with_capabilities(id, handler, &[]); }

    /// 绑定 skill handler 并记录其扩展能力；ID 越界时 panic。
    pub fn set_with_capabilities(&mut self, id: SkillId, handler: SkillHandlerFn, capabilities: &[ExtensionCapability]) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime skill handler id: {}", id.0);
        };
        *slot = Some(handler);
        self.capabilities[id.0 as usize] = capabilities.to_vec();
    }

    /// 查询已绑定的 skill handler；未绑定返回 None。
    pub fn get(&self, id: SkillId) -> Option<SkillHandlerFn> { self.handlers.get(id.0 as usize).and_then(|handler| *handler) }

    /// 查询该 handler 声明的扩展能力。
    pub fn capabilities(&self, id: SkillId) -> Option<&[ExtensionCapability]> {
        self.capabilities.get(id.0 as usize).map(Vec::as_slice)
    }
}

/// 按 StateId 索引的 state handler 表。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateHandlers {
    handlers: Vec<Option<StateHandlerFn>>,
    capabilities: Vec<Vec<ExtensionCapability>>,
}

impl StateHandlers {
    /// 按注册表中的状态数量建空表。
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            handlers: vec![None; registry.states().len()],
            capabilities: vec![Vec::new(); registry.states().len()],
        }
    }

    /// 绑定 state handler，不声明额外能力。
    pub fn set(&mut self, id: StateId, handler: StateHandlerFn) { self.set_with_capabilities(id, handler, &[]); }

    /// 绑定 state handler 并记录其扩展能力；ID 越界时 panic。
    pub fn set_with_capabilities(&mut self, id: StateId, handler: StateHandlerFn, capabilities: &[ExtensionCapability]) {
        let Some(slot) = self.handlers.get_mut(id.0 as usize) else {
            panic!("unknown runtime state handler id: {}", id.0);
        };
        *slot = Some(handler);
        self.capabilities[id.0 as usize] = capabilities.to_vec();
    }

    /// 查询已绑定的 state handler；未绑定返回 None。
    pub fn get(&self, id: StateId) -> Option<StateHandlerFn> { self.handlers.get(id.0 as usize).and_then(|handler| *handler) }

    /// 查询该 handler 声明的扩展能力。
    pub fn capabilities(&self, id: StateId) -> Option<&[ExtensionCapability]> {
        self.capabilities.get(id.0 as usize).map(Vec::as_slice)
    }
}

/// 一条由渲染器产出的回放事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedReplay {
    pub renderer: ReplayRendererId,
    pub payload: String,
}

impl RenderedReplay {
    /// 用渲染器 ID 与载荷构造回放事件。
    pub fn new(renderer: ReplayRendererId, payload: impl Into<String>) -> Self {
        Self {
            renderer,
            payload: payload.into(),
        }
    }
}

/// 一条由渲染器产出的展示事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedShow {
    pub renderer: ShowRendererId,
    pub payload: String,
}

impl RenderedShow {
    /// 用渲染器 ID 与载荷构造展示事件。
    pub fn new(renderer: ShowRendererId, payload: impl Into<String>) -> Self {
        Self {
            renderer,
            payload: payload.into(),
        }
    }
}

/// 核心回放事件的投影字段，供渲染器直接引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreReplayEvent {
    pub message: String,
    pub caster: usize,
    pub target: usize,
    pub targets: Vec<usize>,
    pub param: Option<u32>,
    pub score: u32,
}

/// 核心展示事件的投影字段，供渲染器直接引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreShowEvent {
    pub text: String,
    pub score: u32,
}

/// 按 ReplayRendererId 索引的 replay 渲染器表。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReplayRenderers {
    renderers: Vec<Option<ReplayRendererFn>>,
}

impl ReplayRenderers {
    /// 按注册表中的 replay 渲染器数量建空表。
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            renderers: vec![None; registry.replay_renderers().len()],
        }
    }

    /// 绑定 replay 渲染器；ID 越界时 panic。
    pub fn set(&mut self, id: ReplayRendererId, renderer: ReplayRendererFn) {
        let Some(slot) = self.renderers.get_mut(id.0 as usize) else {
            panic!("unknown runtime replay renderer id: {}", id.0);
        };
        *slot = Some(renderer);
    }

    /// 查询已绑定的 replay 渲染器；未绑定返回 None。
    pub fn get(&self, id: ReplayRendererId) -> Option<ReplayRendererFn> {
        self.renderers.get(id.0 as usize).and_then(|renderer| *renderer)
    }
}

/// 按 ShowRendererId 索引的 show 渲染器表。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ShowRenderers {
    renderers: Vec<Option<ShowRendererFn>>,
}

impl ShowRenderers {
    /// 按注册表中的 show 渲染器数量建空表。
    pub fn from_registry(registry: &ExtensionRegistry) -> Self {
        Self {
            renderers: vec![None; registry.show_renderers().len()],
        }
    }

    /// 绑定 show 渲染器；ID 越界时 panic。
    pub fn set(&mut self, id: ShowRendererId, renderer: ShowRendererFn) {
        let Some(slot) = self.renderers.get_mut(id.0 as usize) else {
            panic!("unknown runtime show renderer id: {}", id.0);
        };
        *slot = Some(renderer);
    }

    /// 查询已绑定的 show 渲染器；未绑定返回 None。
    pub fn get(&self, id: ShowRendererId) -> Option<ShowRendererFn> {
        self.renderers.get(id.0 as usize).and_then(|renderer| *renderer)
    }
}

#[cfg(test)]
mod tests;

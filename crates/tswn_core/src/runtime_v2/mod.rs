mod combat;
pub mod effect;
pub mod entity;
pub mod extension;
mod handlers;
pub mod oracle;
mod plain_assassinate;
mod plain_summon;
mod plain_zombie;
mod prepared_init;
mod profile;
mod runner;
pub mod scheduler;
pub mod scratch;
pub mod slot;
#[cfg(not(feature = "no_debug"))]
pub mod trace;
pub mod world;

use crate::engine::update::RunUpdates;
use crate::player::PlrId;
use crate::rc4::RC4;

pub use effect::{
    CoreReplayEvent, CoreShowEvent, CustomEffect, CustomEffectPayload, EffectContext, EffectContextError, EffectHandlerFn,
    EffectHandlers, EffectQueue, QueuedEffect, RenderedReplay, RenderedShow, ReplayRendererFn, ReplayRenderers, RuntimeFrame,
    ShowRendererFn, ShowRenderers, SkillContext, SkillHandlerFn, SkillHandlers, StateContext, StateHandlerFn, StateHandlers,
};
pub use entity::{
    AssassinateRuntime, CloneBuildData, CloneDerivedStats, CounterRuntime, CovidInfectionEntry, EntityArena, EntityIdx,
    EntityRecord, HideRuntime, MoveState, PlayerPolicyOverrides, PlayerRuntime, PlayerTemplate, ProtectLinkRuntime,
    RuntimeCorpseKind, SkillLoadout, StateEntry, StatePayload, StateStore,
};
pub use extension::{
    BattleSlotId, BattleSlotSpec, DamageSharePolicy, EffectHandlerId, EffectHandlerSpec, EntitySlotId, EntitySlotSpec,
    ExtensionCapability, ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder, ExtensionVersion, InstalledExtensionSpec,
    MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, PlayerKindSpec, ProcMask,
    RegistrationOrder, ReplayRendererId, ReplayRendererSpec, ShowRendererId, ShowRendererSpec, SkillId, SkillPostActionPhase,
    SkillPriority, SkillSpec, StateId, StateSpec, TargetPolicy, TemplateSlotId, TemplateSlotSpec, TswnExtension,
};
pub use oracle::{
    NormalizedOutcome, NormalizedUpdateFrame, StrictDiff, StrictRunDiff, normalize_legacy_run, strict_diff, strict_diff_runs,
};
pub use scheduler::{
    ActionPlan, ActionSchedulerMode, PhaseScheduler, SkillHookPlan, SkillHookPlanEntry, StateHookPlan, StateHookPlanEntry,
};
pub use scratch::BattleScratch;
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
#[cfg(not(feature = "no_debug"))]
pub use trace::{RngCheckpoint, RuntimeTrace, TraceAction, TraceFrame};
pub use world::WorldArena;

pub use combat::{CombatRuntime, PreparedBuiltinSkillAction, PreparedPlainAction, RoundOutcome, SelectedBuiltinSkill};
pub use handlers::*;
pub use plain_assassinate::PlainSkillPreActionOutcome;
pub use prepared_init::{PreparedBattleInit, RuntimeV2BattleInitError};
pub use profile::*;
pub use runner::*;

#[cfg(test)]
mod tests;

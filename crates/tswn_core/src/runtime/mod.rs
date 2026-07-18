mod batch;
mod combat;
mod cqp;
pub mod effect;
pub mod entity;
pub mod extension;
mod handlers;
pub mod lang;
mod normalized;
mod plain_assassinate;
mod plain_summon;
mod plain_zombie;
mod prepared_init;
mod profile;
mod runner;
pub mod scheduler;
pub mod scratch;
mod session;
pub mod slot;
#[cfg(not(feature = "no_debug"))]
pub mod trace;
pub mod update;
pub mod world;

use crate::rc4::RC4;

/// Runtime 实体的稳定数值 ID。
pub type PlrId = usize;

/// 行动条触发一次行动所需的移动点数。
pub const MOVE_POINT_THRESHOLD: i32 = 2048;

/// namerena 评分机制里的第一个靶子。
pub const PROFILE_START: u32 = 3355_4431;

pub use effect::{
    CoreReplayEvent, CoreShowEvent, CustomEffect, CustomEffectPayload, EffectContext, EffectContextError, EffectHandlerFn,
    EffectHandlers, EffectQueue, QueuedEffect, RenderedReplay, RenderedShow, ReplayRendererFn, ReplayRenderers, RuntimeFrame,
    ShowRendererFn, ShowRenderers, SkillContext, SkillHandlerFn, SkillHandlers, StateContext, StateHandlerFn, StateHandlers,
};
pub use entity::{
    AssassinateRuntime, CloneBuildData, CloneDerivedStats, CompressedLegacyState, CounterRuntime, CovidInfectionEntry,
    EntityArena, EntityIdx, EntityRecord, HideRuntime, MoveState, PlayerPolicyOverrides, PlayerRuntime, PlayerTemplate,
    ProtectLinkRuntime, RuntimeCorpseKind, SkillLoadout, StateEntry, StatePayload, StateStore,
};
pub use extension::{
    BattleSlotId, BattleSlotSpec, DamageSharePolicy, EffectHandlerId, EffectHandlerSpec, EntitySlotId, EntitySlotSpec,
    ExtensionCapability, ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder, ExtensionVersion, InstalledExtensionSpec,
    MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, PlayerKindSpec, ProcMask,
    RegistrationOrder, ReplayRendererId, ReplayRendererSpec, ShowRendererId, ShowRendererSpec, SkillId, SkillPostActionPhase,
    SkillPriority, SkillSpec, StateId, StateSpec, TargetPolicy, TemplateSlotId, TemplateSlotSpec, TswnExtension,
};
pub use normalized::{NormalizedActionBoundary, NormalizedOutcome, NormalizedRngCheckpoint, NormalizedUpdateFrame};
pub use scheduler::{
    ActionPlan, ActionSchedulerMode, PhaseScheduler, SkillHookPlan, SkillHookPlanEntry, StateHookPlan, StateHookPlanEntry,
};
pub use scratch::BattleScratch;
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
#[cfg(not(feature = "no_debug"))]
pub use trace::{RngCheckpoint, RuntimeTrace, TraceAction, TraceFrame};
pub use world::WorldArena;

pub use batch::{
    RuntimeBatchError, RuntimeBatchSummary, prepared_runtime_win_rate, prepared_runtime_win_rate_range, runtime_groups_win_rate,
    runtime_score, runtime_score_range,
};
pub use combat::{
    CombatRuntime, PreparedBuiltinSkillAction, PreparedPlainAction, PreparedTargetList, RoundOutcome, SelectedBuiltinSkill,
};
pub use cqp::{RuntimeCqpBatchResult, RuntimeCqpMatchup, RuntimeCqpMatchupResult, resolve_cqp_workers, runtime_cqp_matchups};
pub use handlers::*;
pub use plain_assassinate::PlainSkillPreActionOutcome;
pub use prepared_init::{PreparedBattleInit, PreparedBattleRoster, RuntimeBattleInitError};
pub(crate) use prepared_init::{ScoreIdentityBuffer, ScoreRosterBuffers, ScoreRoundScratch};
pub use profile::*;
pub use runner::*;
pub use session::*;
pub use update::{RunUpdate, RunUpdates, UpdateType};

#[cfg(test)]
mod tests;

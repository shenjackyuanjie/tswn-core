//! 战斗机制层：构建模板经过导入后，由这里的唯一执行链推进。
//!
//! # 数据与执行
//!
//! ```text
//! PreparedRunner / PlayerTemplate
//!        ↓ 按 seed 构造或复位
//! Runner → CombatRuntime
//!            ├─ EntityArena：模板、当前数值、技能、状态和实体槽
//!            ├─ WorldArena：行动顺序、队伍 roster 与存活视图
//!            ├─ PhaseScheduler：行动条与 hook 执行计划
//!            ├─ EffectQueue：伤害、恢复、召唤、死亡相关效果
//!            └─ RC4：本场战斗随机流
//!        ↓
//! RunUpdates / RuntimePlayerSnapshot / BattleModelState
//! ```
//!
//! # 接入与扩展
//!
//! 普通应用使用 [`crate::cli_api::battle::BattleSession`]；这里的 [`RuntimeRunner`]
//! 适合直接研究机制，调用者选择回合上限并消费结果。
//! [`PreparedRuntimeRunner`] 固化阵容与 `eval_rq`，复用时重新应用 seed 和复位可变状态。
//!
//! 自定义规则先用 [`extension`] 注册元数据，再给 [`CombatRuntime`] 安装 handler。
//! 注册提供元数据，handler 提供实现；[`RuntimeRunner::validate_ready`] 检查
//! 已使用技能的绑定情况。默认游戏规则的注册、导入与 handler 绑定由
//! [`default_custom_runtime_import_config`] 统一组装。
//!
//! # 阅读顺序
//!
//! [`entity`] → [`world`] → [`scheduler`] → [`effect`]，然后按需要查看
//! [`extension`]、[`slot`]、[`model_state`] 与 [`update`]。
//! `LegacyStep` 是当前 Runtime 内用于对齐游戏行动条的调度方式。

mod batch;
mod combat;
pub mod control;
mod cqp;
pub mod effect;
#[cfg(feature = "effective-skill-stats")]
pub mod effective_skill_stats;
pub mod entity;
pub mod extension;
pub mod forced_action;
mod handlers;
pub mod lang;
pub mod model_state;
pub mod nn_eval;
mod normalized;
mod prepared_init;
mod profile;
mod runner;
pub mod scheduler;
/// Clever 搜索与局面估值入口，供 CLI 和跨语言适配层使用。
pub mod search;
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

/// 行动恢复与技能返蓝共用的 MP 上限。
///
/// 取 100 是既有行为基线：仓库自 `7e65ecd8` 起，行动恢复（每次行动 +40）一直是
/// `.min(100)`；随 KODU16 接线一并引入的能量药水（+32）与蓄力返蓝（+16）两处，在
/// 他 fork 被 `22c7af3e` 改动之前的基线同样是 `.min(100)`。
///
/// 128 出自 KODU16 的 `22c7af3e`（修被子 quilt 时顺手把三处 `.min(100)` 换成了这个
/// 常量），没有原版规则依据。该上限作用于普通对局也走的路径，取 128 会改变既有行为
/// 对齐，而仓库里没有任何测试覆盖该上限，故保持 100。
///
/// 保留具名常量只为消除魔法数字；将来若确认原版规则是 128，改这一行即可。
pub const MAGIC_POINT_CAP: i32 = 100;

/// namerena 评分机制里的第一个靶子。
pub const PROFILE_START: u32 = 3355_4431;

pub use combat::BattleScratch;
pub use effect::{
    CoreReplayEvent, CoreShowEvent, CustomEffect, CustomEffectPayload, EffectContext, EffectContextError, EffectHandlerFn,
    EffectHandlers, EffectQueue, QueuedEffect, RenderedReplay, RenderedShow, ReplayRendererFn, ReplayRenderers, RuntimeFrame,
    ShowRendererFn, ShowRenderers, SkillContext, SkillHandlerFn, SkillHandlers, StateContext, StateHandlerFn, StateHandlers,
};
pub use entity::{
    AssassinateRuntime, CloneBuildData, CloneDerivedStats, CloneStatAdjustments, CompressedLegacyState, CounterRuntime,
    CovidInfectionEntry, EntityArena, EntityIdx, EntityRecord, HideRuntime, MoveState, PlayerPolicyOverrides, PlayerRuntime,
    PlayerTemplate, ProtectLinkRuntime, RinickTemplateData, RuntimeCorpseKind, ScoreCloneSkillBoostPlan, SkillLoadout,
    StateEntry, StatePayload, StatePayloadKind, StateStore,
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
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
#[cfg(not(feature = "no_debug"))]
pub use trace::{RngCheckpoint, RuntimeTrace, TraceAction, TraceFrame};
pub use world::WorldArena;

/// 上游原版评分会话保留独立命名空间。
pub use batch::score_session as upstream_score_test;
pub use batch::test_session::{ScoreTestProgress, ScoreTestSession, TestSkillFrequency};
pub use batch::{
    RuntimeBatchError, RuntimeBatchSummary, SCORE_TEST_TOTAL, ScoreSkillFrequency, ScoreTestError, prepared_runtime_win_rate,
    prepared_runtime_win_rate_range, prepared_runtime_win_rate_range_timed, prepared_runtime_win_rate_timed,
    runtime_groups_win_rate, runtime_groups_win_rate_timed, runtime_score, runtime_score_range, runtime_score_range_timed,
    runtime_score_timed,
};
pub use combat::PlainSkillPreActionOutcome;
pub use combat::boss::slime;
pub use combat::{
    CombatRuntime, PreparedBossSkillAction, PreparedBuiltinSkillAction, PreparedPlainAction, PreparedTargetList, RoundOutcome,
    SelectedBuiltinSkill,
};
pub use control::{
    BattleControl, DamageDecisionBatch, DamageDecisionPhase, ForcedAction, PUBLIC_BASIC_ATTACK, PassiveDecision, PassivePause,
    PassiveRequest, PlannedAction, PlannedActionError, PlannedSkill, ThunderContext, decode_actions, public_builtin_skill,
};
pub use cqp::{
    RuntimeCqpBatchResult, RuntimeCqpMatchup, RuntimeCqpMatchupResult, resolve_cqp_workers, runtime_cqp_matchups,
    runtime_cqp_matchups_observed,
};
pub use handlers::*;
pub use prepared_init::{PreparedBattleInit, PreparedBattleRoster, RuntimeBattleInitError};
pub use prepared_init::{ScoreIdentityBuffer, ScoreRosterBuffers, ScoreRoundScratch};
pub use profile::*;
pub use runner::*;
pub use session::*;
pub use update::{RunUpdate, RunUpdates, UpdateType};

#[cfg(test)]
mod tests;

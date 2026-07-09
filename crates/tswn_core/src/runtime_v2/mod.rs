pub mod effect;
pub mod entity;
pub mod extension;
pub mod oracle;
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
    EntityArena, EntityIdx, EntityRecord, MoveState, PlayerPolicyOverrides, PlayerRuntime, PlayerTemplate, SkillLoadout,
    StateEntry, StatePayload, StateStore,
};
pub use extension::{
    BattleSlotId, BattleSlotSpec, DamageSharePolicy, EffectHandlerId, EffectHandlerSpec, EntitySlotId, EntitySlotSpec,
    ExtensionCapability, ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder, ExtensionVersion, InstalledExtensionSpec,
    MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, PlayerKindSpec, ProcMask,
    RegistrationOrder, ReplayRendererId, ReplayRendererSpec, ShowRendererId, ShowRendererSpec, SkillId, SkillPostActionPhase,
    SkillPriority, SkillSpec, StateId, StateSpec, TargetPolicy, TemplateSlotId, TemplateSlotSpec, TswnExtension,
};
pub use oracle::{NormalizedOutcome, NormalizedUpdateFrame, StrictDiff, strict_diff};
pub use scheduler::{ActionPlan, PhaseScheduler, SkillHookPlan, SkillHookPlanEntry, StateHookPlan, StateHookPlanEntry};
pub use scratch::BattleScratch;
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
#[cfg(not(feature = "no_debug"))]
pub use trace::{RngCheckpoint, RuntimeTrace, TraceAction, TraceFrame};
pub use world::WorldArena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCombatTemplate {
    pub players: Vec<PlayerTemplate>,
    pub registry: ExtensionRegistry,
    pub slots: TemplateSlotStorage,
}

impl PreparedCombatTemplate {
    pub fn new(players: Vec<PlayerTemplate>) -> Self { Self::with_registry(players, ExtensionRegistry::default()) }

    pub fn with_registry(players: Vec<PlayerTemplate>, registry: ExtensionRegistry) -> Self {
        let slots = TemplateSlotStorage::from_registry(&registry);
        Self {
            players,
            registry,
            slots,
        }
    }

    pub fn minimal_1v1(left_hp: i32, right_hp: i32, attack: i32) -> Self {
        Self::new(vec![
            PlayerTemplate::new(1, "left", 0, left_hp, attack),
            PlayerTemplate::new(2, "right", 1, right_hp, attack),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeV2Runner {
    runtime: CombatRuntime,
}

#[derive(Debug, Clone)]
pub struct RuntimeV2RunSummary {
    pub rounds: Vec<RoundOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
}

impl RuntimeV2RunSummary {
    pub fn last_outcome(&self) -> Option<&RoundOutcome> { self.rounds.last() }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeV2NormalizedRun {
    pub rounds: Vec<NormalizedOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
    pub total_score: u64,
}

impl RuntimeV2NormalizedRun {
    pub fn last_outcome(&self) -> Option<&NormalizedOutcome> { self.rounds.last() }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeDefendValue {
    Atp {
        value: f64,
        caster: EntityIdx,
        target: EntityIdx,
    },
    Damage {
        value: i32,
        caster: EntityIdx,
        target: EntityIdx,
    },
}

impl RuntimeDefendValue {
    pub fn atp(self) -> Option<f64> {
        match self {
            Self::Atp { value, .. } => Some(value),
            Self::Damage { .. } => None,
        }
    }

    pub fn set_atp(&mut self, atp: f64) {
        match self {
            Self::Atp { value, .. } => *value = atp,
            Self::Damage { .. } => panic!("runtime_v2 defend value is damage, not atp"),
        }
    }

    pub fn damage(self) -> Option<i32> {
        match self {
            Self::Atp { .. } => None,
            Self::Damage { value, .. } => Some(value),
        }
    }

    pub fn set_damage(&mut self, damage: i32) {
        match self {
            Self::Atp { .. } => panic!("runtime_v2 defend value is atp, not damage"),
            Self::Damage { value, .. } => *value = damage,
        }
    }

    pub fn caster(self) -> EntityIdx {
        match self {
            Self::Atp { caster, .. } | Self::Damage { caster, .. } => caster,
        }
    }

    pub fn target(self) -> EntityIdx {
        match self {
            Self::Atp { target, .. } | Self::Damage { target, .. } => target,
        }
    }
}

impl RuntimeV2Runner {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        Self {
            runtime: CombatRuntime::from_template(template),
        }
    }

    pub fn from_custom_bed2_roster(
        raw_groups: &[Vec<String>],
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        let CustomRuntimeV2ImportConfig {
            registry,
            bed2_kind,
            bed2_summon_skill,
            bed2_minion_overlays,
        } = config;
        match bed2_minion_overlays {
            Some(minion_overlays) => {
                Self::from_bed2_roster_with_minion_overlays(raw_groups, registry, bed2_kind, bed2_summon_skill, minion_overlays)
                    .map_err(CustomRuntimeV2ImportError::Bed2MinionOverlay)
            }
            None => Self::from_bed2_roster(raw_groups, registry, bed2_kind, bed2_summon_skill)
                .map_err(CustomRuntimeV2ImportError::Bed2Roster),
        }
    }

    pub fn from_custom_bed2_namerena_raw(
        raw_input: String,
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_custom_bed2_roster(&raw_groups, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_custom_mixed_roster(
        raw_groups: &[Vec<String>],
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        let CustomRuntimeV2ImportConfig {
            registry,
            bed2_kind,
            bed2_summon_skill,
            bed2_minion_overlays,
        } = config;
        match bed2_minion_overlays {
            Some(minion_overlays) => {
                Self::from_mixed_roster_with_minion_overlays(raw_groups, registry, bed2_kind, bed2_summon_skill, minion_overlays)
                    .map_err(CustomRuntimeV2ImportError::Bed2MinionOverlay)
            }
            None => Self::from_mixed_roster(raw_groups, registry, bed2_kind, bed2_summon_skill)
                .map_err(CustomRuntimeV2ImportError::MixedRoster),
        }
    }

    pub fn from_custom_mixed_namerena_raw(
        raw_input: String,
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_custom_mixed_roster(&raw_groups, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_bed2_roster(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Self, CustomBed2RosterImportError> {
        let template = CustomBed2Import::roster_into_prepared_template(raw_groups, registry, kind, summon_skill)?;
        Ok(Self::from_template(template))
    }

    pub fn from_bed2_roster_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2SummonTemplateImportError> {
        let template = CustomBed2Import::roster_into_prepared_template_with_summon_overlay(
            raw_groups,
            registry,
            kind,
            summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_bed2_roster_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ShadowTemplateImportError> {
        let template = CustomBed2Import::roster_into_prepared_template_with_shadow_overlay(
            raw_groups,
            registry,
            kind,
            summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_bed2_roster_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ZombieTemplateImportError> {
        let template = CustomBed2Import::roster_into_prepared_template_with_zombie_overlay(
            raw_groups,
            registry,
            kind,
            summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_bed2_roster_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<Self, CustomBed2MinionOverlayImportError> {
        let template = CustomBed2Import::roster_into_prepared_template_with_minion_overlays(
            raw_groups,
            registry,
            kind,
            summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_bed2_namerena_raw(
        raw_input: String,
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Self, CustomBed2RosterImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_bed2_roster(&raw_groups, registry, kind, summon_skill)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_bed2_namerena_raw_with_summon_overlay(
        raw_input: String,
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2SummonTemplateImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_bed2_roster_with_summon_overlay(&raw_groups, registry, kind, summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_bed2_namerena_raw_with_shadow_overlay(
        raw_input: String,
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ShadowTemplateImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_bed2_roster_with_shadow_overlay(&raw_groups, registry, kind, summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_bed2_namerena_raw_with_zombie_overlay(
        raw_input: String,
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ZombieTemplateImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_bed2_roster_with_zombie_overlay(&raw_groups, registry, kind, summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_bed2_namerena_raw_with_minion_overlays(
        raw_input: String,
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<Self, CustomBed2MinionOverlayImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_bed2_roster_with_minion_overlays(&raw_groups, registry, kind, summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_mixed_roster(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Self, CustomMixedRosterImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template(raw_groups, registry, bed2_kind, bed2_summon_skill)?;
        Ok(Self::from_template(template))
    }

    pub fn from_mixed_roster_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2SummonTemplateImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template_with_summon_overlay(
            raw_groups,
            registry,
            bed2_kind,
            bed2_summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_mixed_roster_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ShadowTemplateImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template_with_shadow_overlay(
            raw_groups,
            registry,
            bed2_kind,
            bed2_summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_mixed_roster_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ZombieTemplateImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template_with_zombie_overlay(
            raw_groups,
            registry,
            bed2_kind,
            bed2_summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_mixed_roster_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<Self, CustomBed2MinionOverlayImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template_with_minion_overlays(
            raw_groups,
            registry,
            bed2_kind,
            bed2_summon_skill,
            config,
        )?;
        Ok(Self::from_template(template))
    }

    pub fn from_mixed_namerena_raw(
        raw_input: String,
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Self, CustomMixedRosterImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner = Self::from_mixed_roster(&raw_groups, registry, bed2_kind, bed2_summon_skill)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_mixed_namerena_raw_with_summon_overlay(
        raw_input: String,
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2SummonTemplateImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            Self::from_mixed_roster_with_summon_overlay(&raw_groups, registry, bed2_kind, bed2_summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_mixed_namerena_raw_with_shadow_overlay(
        raw_input: String,
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ShadowTemplateImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            Self::from_mixed_roster_with_shadow_overlay(&raw_groups, registry, bed2_kind, bed2_summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_mixed_namerena_raw_with_zombie_overlay(
        raw_input: String,
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<Self, CustomBed2ZombieTemplateImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            Self::from_mixed_roster_with_zombie_overlay(&raw_groups, registry, bed2_kind, bed2_summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    pub fn from_mixed_namerena_raw_with_minion_overlays(
        raw_input: String,
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<Self, CustomBed2MinionOverlayImportError> {
        let (raw_groups, _) = crate::Runner::split_namerena_into_groups(raw_input);
        let mut runner =
            Self::from_mixed_roster_with_minion_overlays(&raw_groups, registry, bed2_kind, bed2_summon_skill, config)?;
        runner.sync_legacy_raw_state(&raw_groups);
        Ok(runner)
    }

    fn sync_legacy_raw_state(&mut self, raw_groups: &[Vec<String>]) {
        let raw_input = raw_groups.iter().map(|group| group.join("\n")).collect::<Vec<String>>().join("\n\n");
        if let Ok(legacy_runner) = crate::Runner::new_from_namerena_raw(raw_input) {
            self.sync_legacy_raw_world(&legacy_runner.world);
            self.runtime.rng = legacy_runner.randomer;
        }
    }

    fn sync_legacy_raw_world(&mut self, legacy_world: &crate::engine::world_state::WorldState) {
        for (team_idx, group) in legacy_world.groups.iter().enumerate() {
            for plr_id in group {
                let entity_idx = Self::entity_idx_from_legacy_plr(*plr_id);
                let entity = self.runtime.entities.get_mut(entity_idx).unwrap_or_else(|| {
                    panic!(
                        "legacy raw world contains player id {} missing from runtime_v2 entities",
                        plr_id
                    )
                });
                entity.template.team = team_idx;
                entity.runtime.team = team_idx;
            }
        }

        let round_order = Self::entity_order_from_legacy_plrs(&legacy_world.players);
        let team_alive = (0..legacy_world.groups.len())
            .map(|team| legacy_world.team_alive(team).map(Self::entity_order_from_legacy_plrs).unwrap_or_default())
            .collect();
        let flat_alive = Self::entity_order_from_legacy_plrs(&legacy_world.flat_alive);
        self.runtime
            .world
            .sync_initial_views(&self.runtime.entities, round_order, team_alive, flat_alive);
    }

    fn entity_order_from_legacy_plrs(plrs: &[crate::player::PlrId]) -> Vec<EntityIdx> {
        plrs.iter().copied().map(Self::entity_idx_from_legacy_plr).collect()
    }

    fn entity_idx_from_legacy_plr(plr_id: crate::player::PlrId) -> EntityIdx {
        EntityIdx(plr_id.try_into().expect("legacy raw player id overflowed runtime_v2 entity index"))
    }

    pub fn runtime(&self) -> &CombatRuntime { &self.runtime }

    pub fn runtime_mut(&mut self) -> &mut CombatRuntime { &mut self.runtime }

    pub fn run_round(&mut self) -> RoundOutcome { self.runtime.run_minimal_round() }

    pub fn run_round_normalized(&mut self) -> NormalizedOutcome {
        let outcome = self.run_round();
        NormalizedOutcome::from_runtime(&self.runtime, &outcome)
    }

    pub fn run_until_winner(&mut self, max_rounds: usize) -> RuntimeV2RunSummary {
        let mut rounds = Vec::new();
        let mut winner_team = self.runtime.world.sync_winner(&self.runtime.entities);
        while winner_team.is_none() && rounds.len() < max_rounds {
            let outcome = self.run_round();
            winner_team = outcome.winner_team;
            let made_progress = outcome.action.is_some() || outcome.frame.is_some();
            rounds.push(outcome);
            if winner_team.is_some() || !made_progress {
                break;
            }
        }
        RuntimeV2RunSummary {
            guard_exhausted: winner_team.is_none() && rounds.len() == max_rounds,
            rounds,
            winner_team,
        }
    }

    pub fn run_until_winner_normalized(&mut self, max_rounds: usize) -> (RuntimeV2RunSummary, NormalizedOutcome) {
        let summary = self.run_until_winner(max_rounds);
        let final_outcome = summary.last_outcome().cloned().unwrap_or(RoundOutcome {
            action: None,
            frame: None,
            winner_team: summary.winner_team,
        });
        let normalized = NormalizedOutcome::from_runtime(&self.runtime, &final_outcome);
        (summary, normalized)
    }

    pub fn run_until_winner_normalized_rounds(&mut self, max_rounds: usize) -> RuntimeV2NormalizedRun {
        let mut rounds = Vec::new();
        let mut winner_team = self.runtime.world.sync_winner(&self.runtime.entities);
        while winner_team.is_none() && rounds.len() < max_rounds {
            let outcome = self.run_round();
            winner_team = outcome.winner_team;
            let made_progress = outcome.action.is_some() || outcome.frame.is_some();
            rounds.push(NormalizedOutcome::from_runtime(&self.runtime, &outcome));
            if winner_team.is_some() || !made_progress {
                break;
            }
        }
        let total_score = rounds.iter().map(|outcome| outcome.total_score).sum();
        RuntimeV2NormalizedRun {
            guard_exhausted: winner_team.is_none() && rounds.len() == max_rounds,
            rounds,
            winner_team,
            total_score,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeV2SummonHandlerError {
    Context(EffectContextError),
    MissingTemplateSlot(TemplateSlotId),
    InvalidTemplateSlot(TemplateSlotId),
    RememberedSummonAlive(EntityIdx),
}

impl From<EffectContextError> for RuntimeV2SummonHandlerError {
    fn from(error: EffectContextError) -> Self { Self::Context(error) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeV2MinionHandlerError {
    Context(EffectContextError),
    MissingTemplateSlot(TemplateSlotId),
    InvalidTemplateSlot(TemplateSlotId),
    InvalidCounterSlot(EntitySlotId),
    CounterOverflow(EntitySlotId),
}

impl From<EffectContextError> for RuntimeV2MinionHandlerError {
    fn from(error: EffectContextError) -> Self { Self::Context(error) }
}

pub fn push_summon_from_template_slot(
    context: &mut SkillContext<'_>,
    template_slot: TemplateSlotId,
) -> Result<(), RuntimeV2SummonHandlerError> {
    push_summon_from_template_slot_with_message(context, template_slot, "出现一个新的[1]")
}

pub fn push_summon_from_template_slot_with_message(
    context: &mut SkillContext<'_>,
    template_slot: TemplateSlotId,
    message: impl Into<String>,
) -> Result<(), RuntimeV2SummonHandlerError> {
    let summon_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeV2SummonHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeV2SummonHandlerError::MissingTemplateSlot(template_slot)),
    };
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: context.owner_idx(),
        template: summon_template,
        message: message.into(),
    });
    Ok(())
}

pub fn push_summon_recast_from_entity_slot(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    push_summon_recast_from_entity_slot_with_messages(
        context,
        entity_slot,
        summon_template,
        revive_hp,
        "出现一个新的[1]",
        "[1][复活]了",
    )
}

pub fn push_summon_recast_from_entity_slot_with_message(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let message = message.into();
    push_summon_recast_from_entity_slot_with_messages(context, entity_slot, summon_template, revive_hp, message.clone(), message)
}

pub fn push_summon_recast_from_entity_slot_with_messages(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    summon_template: PlayerTemplate,
    revive_hp: i32,
    spawn_message: impl Into<String>,
    revive_message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let owner = context.owner_idx();
    let spawn_message = spawn_message.into();
    let revive_message = revive_message.into();
    let remembered = context
        .owner()
        .and_then(|entity| entity.slots.get(entity_slot))
        .and_then(|value| match value {
            SlotValue::U64(idx) => Some(EntityIdx(*idx as u32)),
            _ => None,
        });
    if let Some(summon) = remembered {
        let entity = context.entity(summon)?;
        if entity.runtime.alive {
            return Err(RuntimeV2SummonHandlerError::RememberedSummonAlive(summon));
        }
        context.push_nested(QueuedEffect::ReviveWithMessage {
            caster: owner,
            target: summon,
            hp: revive_hp,
            message: revive_message,
        });
        return Ok(summon);
    }

    let next_entity = EntityIdx(context.entity_count().try_into().expect("runtime_v2 entity index overflow"));
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: owner,
        template: summon_template,
        message: spawn_message,
    });
    context.set_entity_slot(owner, entity_slot, SlotValue::U64(u64::from(next_entity.0)))?;
    Ok(next_entity)
}

pub fn push_summon_recast_from_template_slot_with_messages(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
    spawn_message: impl Into<String>,
    revive_message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let summon_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeV2SummonHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeV2SummonHandlerError::MissingTemplateSlot(template_slot)),
    };
    push_summon_recast_from_entity_slot_with_messages(
        context,
        entity_slot,
        summon_template,
        revive_hp,
        spawn_message,
        revive_message,
    )
}

pub fn push_summon_recast_from_template_slot(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    push_summon_recast_from_template_slot_with_messages(
        context,
        entity_slot,
        template_slot,
        revive_hp,
        "出现一个新的[1]",
        "[1][复活]了",
    )
}

pub fn push_summon_recast_from_template_slot_with_message(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2SummonHandlerError> {
    let message = message.into();
    push_summon_recast_from_template_slot_with_messages(context, entity_slot, template_slot, revive_hp, message.clone(), message)
}

pub fn run_legacy_summon_recast_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    entity_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    revive_hp: i32,
) {
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]使用[血祭]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    push_summon_recast_from_template_slot_with_messages(context, entity_slot, template_slot, revive_hp, "召唤出[1]", "召唤出[1]")
        .expect("legacy summon recast handler should spawn or revive template-slot summon");
}

pub fn run_legacy_summon_recast_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_legacy_summon_recast_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0), 10);
}

pub fn summon_default_skill_loadout(fire_skill: SkillId, explode_skill: SkillId, active_order: [usize; 3]) -> SkillLoadout {
    SkillLoadout::from_skills([fire_skill, fire_skill, explode_skill]).with_active_order(active_order)
}

pub fn push_summon_explode(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::SummonExplode {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

pub fn push_disperse_attack(context: &mut SkillContext<'_>, target: EntityIdx) {
    context.push_nested(QueuedEffect::DisperseAttack {
        caster: context.owner_idx(),
        target,
    });
}

pub fn run_disperse_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_disperse_attack(context, target);
}

pub fn score_disperse_target(entities: &EntityArena, world: &WorldArena, target: EntityIdx, smart: bool, rng: &mut RC4) -> f64 {
    let Some(target_entity) = entities.get(target) else {
        return f64::MIN;
    };
    let rate_hi_hp = |hp: i32| -> f64 {
        if hp < 20 {
            30.0
        } else if hp > 300 {
            300.0
        } else {
            hp as f64
        }
    };
    let target_runtime = &target_entity.runtime;
    let mut score = if smart {
        if world.alive_group_count() > 2 {
            rate_hi_hp(target_runtime.hp) * world.alive_group_len_containing(target) as f64 * target_runtime.attract()
        } else {
            (1.0 / rate_hi_hp(target_runtime.hp)) * target_runtime.atk_sum as f64 * target_runtime.attract()
        }
    } else {
        rng.rFFFF() as f64 + target_runtime.attract()
    };
    if smart && target_runtime.flags.contains(PlayerKindFlags::MINION) && target_runtime.hp > 100 {
        score *= 2.0;
    }
    score
}

pub fn select_disperse_targets(
    entities: &EntityArena,
    world: &WorldArena,
    actor: EntityIdx,
    smart: bool,
    rng: &mut RC4,
) -> Vec<EntityIdx> {
    let Some(actor_entity) = entities.get(actor) else {
        return Vec::new();
    };
    let candidates = world
        .flat_alive()
        .iter()
        .copied()
        .filter(|target| {
            entities
                .get(*target)
                .is_some_and(|target_entity| target_entity.runtime.team != actor_entity.runtime.team)
        })
        .collect::<Vec<_>>();
    select_disperse_targets_from_candidates(entities, world, &candidates, smart, rng)
}

fn select_disperse_targets_from_candidates(
    entities: &EntityArena,
    world: &WorldArena,
    candidates: &[EntityIdx],
    smart: bool,
    rng: &mut RC4,
) -> Vec<EntityIdx> {
    let select_count = if smart { 3 } else { 2 };
    let mut selected = Vec::new();
    let mut dup = 0usize;
    let mut invalid = -(select_count as i32);
    while dup <= select_count && invalid <= select_count as i32 {
        let Some(idx) = rng.pick(candidates) else {
            return Vec::new();
        };
        let target = candidates[idx];
        if entities.get(target).is_none() {
            invalid += 1;
            continue;
        }
        if selected.contains(&target) {
            dup += 1;
            continue;
        }
        selected.push(target);
        if selected.len() >= select_count {
            break;
        }
    }
    if selected.is_empty() {
        return Vec::new();
    }
    if selected.len() == 1 {
        let target = selected[0];
        let _ = score_disperse_target(entities, world, target, smart, rng);
        return vec![target];
    }

    let mut scored = selected
        .into_iter()
        .map(|target| (target, score_disperse_target(entities, world, target, smart, rng)))
        .collect::<Vec<_>>();
    scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(target, _)| target).collect()
}

pub fn run_charge_post_action_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    context.tick_owner_charge_post_action().expect("charge post_action owner should exist");
}

pub fn run_accumulate_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    if !context.activate_owner_accumulate_runtime().expect("accumulate owner should exist") {
        return;
    }

    let owner = context.owner_idx();
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]开始[聚气]",
        owner.0 as usize,
        owner.0 as usize,
        1,
    ));
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]攻击力上升",
        owner.0 as usize,
        owner.0 as usize,
        0,
    ));
}

pub fn run_shield_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::ShieldValue(shield)) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if shield <= 0 {
        return;
    }
    let damage = context.defend_damage().expect("shield state should run during POST_DEFEND");
    if damage > shield {
        context
            .set_owner_state_payload(entry.legacy_order_key, StatePayload::ShieldValue(0))
            .expect("shield state payload should still exist");
    } else {
        context.set_defend_damage(0);
        context
            .set_owner_state_payload(entry.legacy_order_key, StatePayload::ShieldValue(shield - damage))
            .expect("shield state payload should still exist");
    }
}

pub fn run_curse_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Curse { prob, multiply }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    let damage = context.defend_damage().expect("curse state should run during POST_DEFEND");
    if damage <= 0 {
        return;
    }

    if (context.rng_next_u8() as u32) & 63 < prob as u32 {
        let caster = context.defend_caster().expect("curse state should receive incoming defend caster");
        let target = context.defend_target().expect("curse state should receive incoming defend target");
        context.add_update(crate::engine::update::RunUpdate::new(
            "[诅咒]使伤害加倍",
            caster.0 as usize,
            target.0 as usize,
            0,
        ));
        context.set_defend_damage(damage * multiply);
    }
}

pub fn run_poison_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Poison {
        caster,
        target,
        atp_bits,
        count,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    let Some(owner) = context.owner() else {
        return;
    };
    if !owner.runtime.alive {
        return;
    }

    let atp = f64::from_bits(atp_bits);
    let tick_atp = atp * (1.0 + (count - 1) as f64 * 0.10000000149011612) / count as f64;
    let next_atp = atp - tick_atp;
    let damage = (tick_atp / (owner.runtime.magic + 64) as f64).ceil() as i32;
    let next_count = count - 1;
    let poison_caster = caster.map_or(context.owner_idx(), EntityIdx);

    context.add_update(crate::engine::update::RunUpdate::new(
        "[1][毒性发作]",
        poison_caster.0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
    context.push_nested(QueuedEffect::PoisonTick {
        caster: poison_caster,
        target: context.owner_idx(),
        amount: damage,
    });

    if next_count > 0 {
        context
            .set_owner_state_payload(
                entry.legacy_order_key,
                StatePayload::Poison {
                    caster,
                    target,
                    atp_bits: next_atp.to_bits(),
                    count: next_count,
                },
            )
            .expect("poison state payload should still exist");
        return;
    }

    context
        .clear_owner_state(entry.legacy_order_key)
        .expect("poison state payload should still exist");
}

pub fn run_haste_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Haste { faster, step }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    run_timed_release_post_action_state(
        context,
        entry,
        StatePayload::Haste { faster, step: step - 1 },
        step,
        "[1]从[疾走]中解除",
    );
}

pub fn run_charm_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Charm {
        group_id,
        effective_team_idx,
        source_team_idx,
        target,
        step,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    run_timed_release_post_action_state(
        context,
        entry,
        StatePayload::Charm {
            group_id,
            effective_team_idx,
            source_team_idx,
            target,
            step: step - 1,
        },
        step,
        "[1]从[魅惑]中解除",
    );
}

pub fn run_slow_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Slow { step }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    run_timed_release_post_action_state(context, entry, StatePayload::Slow { step: step - 1 }, step, "[1]从[迟缓]中解除");
}

fn run_timed_release_post_action_state(
    context: &mut StateContext<'_>,
    entry: &StateHookPlanEntry,
    next_payload: StatePayload,
    step: i32,
    release_message: &'static str,
) {
    let next_step = step - 1;
    if next_step > 0 {
        context
            .set_owner_state_payload(entry.legacy_order_key, next_payload)
            .expect("timed state payload should still exist");
        return;
    }

    context
        .clear_owner_state(entry.legacy_order_key)
        .expect("timed state payload should still exist");
    let alive = context.owner().map(|owner| owner.runtime.alive).unwrap_or(false);
    if alive {
        context.add_newline();
        context.add_update(crate::engine::update::RunUpdate::new(
            release_message,
            context.owner_idx().0 as usize,
            context.owner_idx().0 as usize,
            0,
        ));
    }
}

pub fn run_iron_post_defend_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::Iron { protect, step }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if context.defend_damage().is_none() {
        run_iron_post_action_state(context, entry, protect, step);
        return;
    }
    if step <= 0 || protect <= 0 {
        return;
    }

    let damage = context.defend_damage().expect("iron state should run during POST_DEFEND");
    if damage <= 0 {
        context.set_defend_damage(0);
        return;
    }

    let caster = context.defend_caster().expect("iron state should receive incoming defend caster");
    let target = context.defend_target().expect("iron state should receive incoming defend target");
    if damage <= protect {
        let defended = context
            .last_non_newline_update()
            .map(|update| {
                update.message == "[0][防御]" && update.caster == target.0 as usize && update.target == caster.0 as usize
            })
            .unwrap_or(false);
        context.set_defend_damage(if defended { 0 } else { 1 });
        return;
    }

    let remaining = damage - protect;
    context
        .set_owner_state_payload(entry.legacy_order_key, StatePayload::Iron { protect: 0, step: 0 })
        .expect("iron state payload should still exist");
    context.set_defend_damage(remaining);
    context.add_newline();
    context.add_update(crate::engine::update::RunUpdate::new(
        "[1]的[铁壁]被打消了",
        caster.0 as usize,
        target.0 as usize,
        0,
    ));
}

fn run_iron_post_action_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry, protect: i32, step: i32) {
    if step <= 0 {
        context
            .clear_owner_state(entry.legacy_order_key)
            .expect("iron state payload should still exist");
        return;
    }

    let next_step = step - 1;
    if next_step > 0 {
        context
            .set_owner_state_payload(
                entry.legacy_order_key,
                StatePayload::Iron {
                    protect,
                    step: next_step,
                },
            )
            .expect("iron state payload should still exist");
        return;
    }

    context
        .clear_owner_state(entry.legacy_order_key)
        .expect("iron state payload should still exist");
    context.adjust_owner_speed_points(-128).expect("iron state owner should still exist");
    context.add_newline();
    context.add_update(crate::engine::update::RunUpdate::new(
        "[1]从[铁壁]中解除",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    ));
}

pub fn next_minion_name_from_entity_slot(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
) -> Result<String, RuntimeV2MinionHandlerError> {
    let owner = context.owner().ok_or(EffectContextError::UnknownEntity(context.owner_idx()))?;
    let root_owner_idx = owner.runtime.root_owner;
    let root_owner = context.entity(root_owner_idx)?;
    let root_name = root_owner.template.name.clone();
    let next = match root_owner.slots.get(counter_slot) {
        Some(SlotValue::U64(next)) => *next,
        Some(_) => return Err(RuntimeV2MinionHandlerError::InvalidCounterSlot(counter_slot)),
        None => 0,
    };
    let following = next.checked_add(1).ok_or(RuntimeV2MinionHandlerError::CounterOverflow(counter_slot))?;
    context.set_entity_slot(root_owner_idx, counter_slot, SlotValue::U64(following))?;
    Ok(format!("{root_name}?{next}"))
}

pub fn push_minion_from_template_with_allocated_name(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    mut minion_template: PlayerTemplate,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2MinionHandlerError> {
    let minion_name = next_minion_name_from_entity_slot(context, counter_slot)?;
    let next_entity = EntityIdx(context.entity_count().try_into().expect("runtime_v2 entity index overflow"));
    minion_template.name = minion_name;
    context.push_nested(QueuedEffect::SpawnWithMessage {
        caster: context.owner_idx(),
        template: minion_template,
        message: message.into(),
    });
    Ok(next_entity)
}

pub fn push_minion_from_template_with_allocated_name_silent(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    mut minion_template: PlayerTemplate,
) -> Result<EntityIdx, RuntimeV2MinionHandlerError> {
    let minion_name = next_minion_name_from_entity_slot(context, counter_slot)?;
    let next_entity = EntityIdx(context.entity_count().try_into().expect("runtime_v2 entity index overflow"));
    minion_template.name = minion_name;
    context.push_nested(QueuedEffect::SpawnSilent {
        caster: context.owner_idx(),
        template: minion_template,
    });
    Ok(next_entity)
}

pub fn push_minion_from_template_slot_with_allocated_name(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    message: impl Into<String>,
) -> Result<EntityIdx, RuntimeV2MinionHandlerError> {
    let minion_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeV2MinionHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeV2MinionHandlerError::MissingTemplateSlot(template_slot)),
    };
    push_minion_from_template_with_allocated_name(context, counter_slot, minion_template, message)
}

pub fn push_minion_from_template_slot_with_allocated_name_silent(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
) -> Result<EntityIdx, RuntimeV2MinionHandlerError> {
    let minion_template = match context.template_slot(template_slot)? {
        Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
        Some(_) => return Err(RuntimeV2MinionHandlerError::InvalidTemplateSlot(template_slot)),
        None => return Err(RuntimeV2MinionHandlerError::MissingTemplateSlot(template_slot)),
    };
    push_minion_from_template_with_allocated_name_silent(context, counter_slot, minion_template)
}

pub fn run_shadow_minion_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
) {
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]使用[幻术]",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        60,
    ));
    push_minion_from_template_slot_with_allocated_name(context, counter_slot, template_slot, "召唤出[1]")
        .expect("shadow minion handler should spawn template-slot minion");
}

pub fn run_shadow_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_shadow_minion_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0));
}

pub fn run_zombie_minion_from_template_slot_with_config(
    context: &mut SkillContext<'_>,
    counter_slot: EntitySlotId,
    template_slot: TemplateSlotId,
    killed_target: EntityIdx,
) {
    let zombie = push_minion_from_template_slot_with_allocated_name_silent(context, counter_slot, template_slot)
        .expect("zombie minion handler should spawn template-slot minion");
    context.add_update(crate::engine::update::RunUpdate::new_newline());
    let mut summon_update =
        crate::engine::update::RunUpdate::new("[0][召唤亡灵]", context.owner_idx().0 as usize, killed_target.0 as usize, 60);
    summon_update.delay0 = 1500;
    context.add_update(summon_update);
    let mut zombied = crate::engine::update::RunUpdate::new("[2]变成了[1]", context.owner_idx().0 as usize, zombie.0 as usize, 0);
    zombied.targets.push(killed_target.0 as usize);
    context.add_update(zombied);
}

pub fn run_zombie_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    run_zombie_minion_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0), EntityIdx(1));
}

pub fn minion_display_index_for_entity(entity: Option<&EntityRecord>) -> usize {
    let Some(entity) = entity else {
        return 0;
    };
    if !entity.runtime.flags.contains(PlayerKindFlags::MINION) {
        return 0;
    }
    entity
        .template
        .name
        .rsplit_once('?')
        .and_then(|(_, index)| index.parse::<usize>().ok())
        .map(|index| index + 1)
        .unwrap_or(1)
}

pub const DEFAULT_BED2_HP: i32 = 3000;
pub const DEFAULT_BED2_DEFENSE: i32 = 99;
pub const DEFAULT_BED2_RESISTANCE: i32 = 99;

pub const DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT: &str = "custom.summon";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2Import {
    pub name: String,
    pub team: Option<String>,
    pub hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2RosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2SummonTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub summon_kind: PlayerKindId,
    pub fire_skill_export_name: &'a str,
    pub explode_skill_export_name: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2ShadowTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub shadow_kind: PlayerKindId,
    pub possess_skill_export_name: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2ZombieTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub zombie_kind: PlayerKindId,
    pub skill_export_name_prefix: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2MinionOverlayConfig<'a> {
    pub summon: CustomBed2SummonTemplateConfig<'a>,
    pub shadow: CustomBed2ShadowTemplateConfig<'a>,
    pub zombie: CustomBed2ZombieTemplateConfig<'a>,
}

pub struct CustomRuntimeV2ImportConfig<'a> {
    pub registry: ExtensionRegistry,
    pub bed2_kind: PlayerKindId,
    pub bed2_summon_skill: SkillId,
    pub bed2_minion_overlays: Option<CustomBed2MinionOverlayConfig<'a>>,
}

impl<'a> CustomRuntimeV2ImportConfig<'a> {
    pub fn new(registry: ExtensionRegistry, bed2_kind: PlayerKindId, bed2_summon_skill: SkillId) -> Self {
        Self {
            registry,
            bed2_kind,
            bed2_summon_skill,
            bed2_minion_overlays: None,
        }
    }

    pub fn with_bed2_minion_overlays(mut self, config: CustomBed2MinionOverlayConfig<'a>) -> Self {
        self.bed2_minion_overlays = Some(config);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultCustomRuntimeV2ProfileError {
    Registry(ExtensionError),
}

impl From<ExtensionError> for DefaultCustomRuntimeV2ProfileError {
    fn from(error: ExtensionError) -> Self { Self::Registry(error) }
}

pub fn default_custom_runtime_v2_import_config()
-> Result<CustomRuntimeV2ImportConfig<'static>, DefaultCustomRuntimeV2ProfileError> {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder.register_skill(
        "custom",
        "summon",
        DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT,
        TargetPolicy::Enemy,
        SkillPriority(0),
    )?;
    let bed2 = builder.register_player_kind_with_policies(
        "custom",
        "bed2",
        "custom.bed2",
        PlayerKindFlags::BED2,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: false,
        },
    )?;
    Ok(CustomRuntimeV2ImportConfig::new(builder.build(), bed2, summon))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2SummonTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2SummonTemplateImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2SummonTemplateImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2SummonTemplateImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ShadowTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ShadowTemplateImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ShadowTemplateImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ShadowTemplateImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ZombieTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ZombieTemplateImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ZombieTemplateImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ZombieTemplateImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2MinionOverlayImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Summon(CustomBed2SummonTemplateImportError),
    Shadow(CustomBed2ShadowTemplateImportError),
    Zombie(CustomBed2ZombieTemplateImportError),
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2SummonTemplateImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2SummonTemplateImportError) -> Self { Self::Summon(error) }
}

impl From<CustomBed2ShadowTemplateImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2ShadowTemplateImportError) -> Self { Self::Shadow(error) }
}

impl From<CustomBed2ZombieTemplateImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2ZombieTemplateImportError) -> Self { Self::Zombie(error) }
}

impl From<SlotError> for CustomBed2MinionOverlayImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomRuntimeV2ImportError {
    Bed2Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Bed2MinionOverlay(CustomBed2MinionOverlayImportError),
}

impl From<CustomBed2RosterImportError> for CustomRuntimeV2ImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Bed2Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomRuntimeV2ImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2MinionOverlayImportError> for CustomRuntimeV2ImportError {
    fn from(error: CustomBed2MinionOverlayImportError) -> Self { Self::Bed2MinionOverlay(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMixedRosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
    pub message: String,
}

impl CustomBed2Import {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }

        let (name, team, plus_rest, team_marker_hp) = if let Some((name, team_and_rest)) = raw.split_once('@') {
            let (team_part, plus_rest) = team_and_rest.split_once('+').unwrap_or((team_and_rest, ""));
            let (team, hp) = Self::split_bed2_team_marker(team_part.trim());
            (name.trim(), team, plus_rest, hp)
        } else if let Some((name, plus_rest)) = raw.split_once('+') {
            (name.trim(), None, plus_rest, None)
        } else {
            return None;
        };

        let hp = Self::parse_bed2_plus_segments(plus_rest).or(team_marker_hp)?;
        Some(Self {
            name: name.to_owned(),
            team,
            hp,
        })
    }

    pub fn parse_player_facade_raw(raw: &str) -> Option<Self> {
        let marker_import = Self::parse(raw)?;
        let id_name = crate::player::Player::raw_namerena_to_idname(raw.trim());
        let (name, team, facade_hp) = Self::parse_facade_id_name(&id_name);
        Some(Self {
            name,
            team,
            hp: facade_hp.unwrap_or(marker_import.hp),
        })
    }

    pub fn into_player_template(self, id: PlrId, kind: PlayerKindId, team: usize, summon_skill: SkillId) -> PlayerTemplate {
        PlayerTemplate::with_kind(id, self.name, kind, team, self.hp, 0)
            .with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE)
            .with_skills([summon_skill])
    }

    pub fn roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomBed2RosterImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    pub fn roster_into_prepared_template_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2SummonTemplateImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_summon_overlay(raw_groups, registry, players, config)
    }

    pub fn roster_into_prepared_template_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ShadowTemplateImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_shadow_overlay(raw_groups, registry, players, config)
    }

    pub fn roster_into_prepared_template_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ZombieTemplateImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_zombie_overlay(raw_groups, registry, players, config)
    }

    pub fn roster_into_prepared_template_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2MinionOverlayImportError> {
        let players = Self::roster_into_player_templates(raw_groups, kind, summon_skill)?;
        Self::prepared_template_with_minion_overlays(raw_groups, registry, players, config)
    }

    fn prepared_template_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2SummonTemplateImportError> {
        let fire_skill = registry.skill_id_by_export_name(config.fire_skill_export_name).ok_or_else(|| {
            CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.fire_skill_export_name.to_owned(),
            }
        })?;
        let explode_skill = registry.skill_id_by_export_name(config.explode_skill_export_name).ok_or_else(|| {
            CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.explode_skill_export_name.to_owned(),
            }
        })?;
        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(summon_template) =
            Self::first_summon_template_from_roster(raw_groups, config.summon_kind, fire_skill, explode_skill)
        {
            template
                .slots
                .set(config.template_slot, SlotValue::PlayerTemplate(Box::new(summon_template)))?;
        }
        Ok(template)
    }

    fn prepared_template_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ShadowTemplateImportError> {
        let possess_skill = registry.skill_id_by_export_name(config.possess_skill_export_name).ok_or_else(|| {
            CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                export_name: config.possess_skill_export_name.to_owned(),
            }
        })?;
        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(shadow_template) = Self::first_shadow_template_from_roster(raw_groups, config.shadow_kind, possess_skill) {
            template
                .slots
                .set(config.template_slot, SlotValue::PlayerTemplate(Box::new(shadow_template)))?;
        }
        Ok(template)
    }

    fn prepared_template_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ZombieTemplateImportError> {
        let zombie_template =
            Self::first_zombie_template_from_roster(raw_groups, config.zombie_kind, &registry, config.skill_export_name_prefix)?;
        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(zombie_template) = zombie_template {
            template
                .slots
                .set(config.template_slot, SlotValue::PlayerTemplate(Box::new(zombie_template)))?;
        }
        Ok(template)
    }

    fn prepared_template_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        players: Vec<PlayerTemplate>,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2MinionOverlayImportError> {
        let fire_skill = registry.skill_id_by_export_name(config.summon.fire_skill_export_name).ok_or_else(|| {
            CustomBed2MinionOverlayImportError::Summon(CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.summon.fire_skill_export_name.to_owned(),
            })
        })?;
        let explode_skill = registry.skill_id_by_export_name(config.summon.explode_skill_export_name).ok_or_else(|| {
            CustomBed2MinionOverlayImportError::Summon(CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: config.summon.explode_skill_export_name.to_owned(),
            })
        })?;
        let possess_skill = registry.skill_id_by_export_name(config.shadow.possess_skill_export_name).ok_or_else(|| {
            CustomBed2MinionOverlayImportError::Shadow(CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                export_name: config.shadow.possess_skill_export_name.to_owned(),
            })
        })?;
        let zombie_template = Self::first_zombie_template_from_roster(
            raw_groups,
            config.zombie.zombie_kind,
            &registry,
            config.zombie.skill_export_name_prefix,
        )
        .map_err(CustomBed2MinionOverlayImportError::Zombie)?;

        let mut template = PreparedCombatTemplate::with_registry(players, registry);
        if let Some(summon_template) =
            Self::first_summon_template_from_roster(raw_groups, config.summon.summon_kind, fire_skill, explode_skill)
        {
            template.slots.set(
                config.summon.template_slot,
                SlotValue::PlayerTemplate(Box::new(summon_template)),
            )?;
        }
        if let Some(shadow_template) =
            Self::first_shadow_template_from_roster(raw_groups, config.shadow.shadow_kind, possess_skill)
        {
            template.slots.set(
                config.shadow.template_slot,
                SlotValue::PlayerTemplate(Box::new(shadow_template)),
            )?;
        }
        if let Some(zombie_template) = zombie_template {
            template.slots.set(
                config.zombie.template_slot,
                SlotValue::PlayerTemplate(Box::new(zombie_template)),
            )?;
        }
        Ok(template)
    }

    pub fn roster_into_player_templates(
        raw_groups: &[Vec<String>],
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomBed2RosterImportError> {
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    return Err(CustomBed2RosterImportError {
                        team_index,
                        player_index,
                        raw: raw.clone(),
                    });
                };
                players.push(import.into_player_template(next_id, kind, team_index, summon_skill));
                next_id += 1;
            }
        }
        Ok(players)
    }

    pub fn mixed_roster_into_prepared_template(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<PreparedCombatTemplate, CustomMixedRosterImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Ok(PreparedCombatTemplate::with_registry(players, registry))
    }

    pub fn mixed_roster_into_prepared_template_with_summon_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2SummonTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2SummonTemplateImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_summon_overlay(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_prepared_template_with_shadow_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ShadowTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ShadowTemplateImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_shadow_overlay(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_prepared_template_with_zombie_overlay(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2ZombieTemplateConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2ZombieTemplateImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_zombie_overlay(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_prepared_template_with_minion_overlays(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
        config: CustomBed2MinionOverlayConfig<'_>,
    ) -> Result<PreparedCombatTemplate, CustomBed2MinionOverlayImportError> {
        let players = Self::mixed_roster_into_player_templates(raw_groups, bed2_kind, bed2_summon_skill)?;
        Self::prepared_template_with_minion_overlays(raw_groups, registry, players, config)
    }

    pub fn mixed_roster_into_player_templates(
        raw_groups: &[Vec<String>],
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Vec<PlayerTemplate>, CustomMixedRosterImportError> {
        let storage = crate::engine::storage::Storage::new_arc();
        let mut players = Vec::new();
        let mut next_id = 1;
        for (team_index, group) in raw_groups.iter().enumerate() {
            for (player_index, raw) in group.iter().enumerate() {
                let raw_trimmed = raw.trim();
                if crate::player::Player::check_is_seed(raw_trimmed) {
                    continue;
                }

                let template = if let Some(import) = Self::parse_player_facade_raw(raw_trimmed) {
                    import.into_player_template(next_id, bed2_kind, team_index, bed2_summon_skill)
                } else {
                    let mut player =
                        crate::player::Player::new_from_namerena_raw(raw.clone(), storage.clone()).map_err(|error| {
                            CustomMixedRosterImportError {
                                team_index,
                                player_index,
                                raw: raw.clone(),
                                message: format!("{error:?}"),
                            }
                        })?;
                    player.build();
                    let status = player.get_status();
                    if status.max_hp <= 0 || status.attack < 0 || status.defense < 0 || status.resistance < 0 {
                        return Err(CustomMixedRosterImportError {
                            team_index,
                            player_index,
                            raw: raw.clone(),
                            message: format!(
                                "legacy player facade produced unsupported status max_hp={} attack={} defense={} resistance={}",
                                status.max_hp, status.attack, status.defense, status.resistance
                            ),
                        });
                    }
                    PlayerTemplate::new(next_id, player.id_name(), team_index, status.max_hp, status.attack)
                        .with_magic(status.magic)
                        .with_magic_point(status.magic_point)
                        .with_wisdom(status.wisdom)
                        .with_agility(status.agility)
                        .with_at_boost_millionths((status.at_boost * 1_000_000.0).round() as i64)
                        .with_target_score_stats(status.attr_sum, status.atk_sum, status.attract)
                        .with_def_res(status.defense, status.resistance)
                };
                players.push(template);
                next_id += 1;
            }
        }
        Ok(players)
    }

    fn parse_facade_id_name(id_name: &str) -> (String, Option<String>, Option<i32>) {
        let (base, plus_rest) = id_name.split_once('+').unwrap_or((id_name, ""));
        let (name, team, team_marker_hp) = Self::split_facade_name_team(base);
        let plus_marker_hp = Self::parse_bed2_plus_segments(plus_rest);
        (name, team, plus_marker_hp.or(team_marker_hp))
    }

    fn split_facade_name_team(raw: &str) -> (String, Option<String>, Option<i32>) {
        if let Some((name, team)) = raw.split_once('@') {
            let (team, hp) = Self::split_bed2_team_marker(team.trim());
            (name.trim().to_owned(), team, hp)
        } else {
            (raw.trim().to_owned(), None, None)
        }
    }

    fn split_bed2_team_marker(team: &str) -> (Option<String>, Option<i32>) {
        if team == "bed2" {
            return (None, Some(DEFAULT_BED2_HP));
        }
        match team.rsplit_once('@') {
            Some((team, "bed2")) if !team.is_empty() => (Some(team.to_owned()), Some(DEFAULT_BED2_HP)),
            _ if team.is_empty() => (None, None),
            _ => (Some(team.to_owned()), None),
        }
    }

    fn parse_bed2_plus_segments(raw: &str) -> Option<i32> { raw.split('+').filter_map(Self::parse_bed2_plus_marker).last() }

    fn parse_bed2_plus_marker(segment: &str) -> Option<i32> {
        let rest = segment.trim().strip_prefix("bed2[")?;
        let hp = rest.strip_suffix(']')?.trim().parse::<i32>().ok()?;
        (hp > 0).then_some(hp)
    }

    fn first_summon_template_from_roster(
        raw_groups: &[Vec<String>],
        summon_kind: PlayerKindId,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> Option<PlayerTemplate> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    continue;
                };
                let Some(overlay) = Self::player_overlay_from_raw(raw) else {
                    continue;
                };
                let Some(summon_overlay) = overlay.summon.as_ref() else {
                    continue;
                };
                return Some(Self::summon_template_from_overlay(
                    &import,
                    team_index,
                    summon_kind,
                    summon_overlay,
                    fire_skill,
                    explode_skill,
                ));
            }
        }
        None
    }

    fn first_shadow_template_from_roster(
        raw_groups: &[Vec<String>],
        shadow_kind: PlayerKindId,
        possess_skill: SkillId,
    ) -> Option<PlayerTemplate> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    continue;
                };
                let Some(overlay) = Self::player_overlay_from_raw(raw) else {
                    continue;
                };
                let Some(shadow_overlay) = overlay.shadow.as_ref() else {
                    continue;
                };
                return Some(Self::shadow_template_from_overlay(
                    &import,
                    team_index,
                    shadow_kind,
                    shadow_overlay,
                    possess_skill,
                ));
            }
        }
        None
    }

    fn first_zombie_template_from_roster(
        raw_groups: &[Vec<String>],
        zombie_kind: PlayerKindId,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<Option<PlayerTemplate>, CustomBed2ZombieTemplateImportError> {
        for (team_index, group) in raw_groups.iter().enumerate() {
            for raw in group {
                if crate::player::Player::check_is_seed(raw.trim()) {
                    continue;
                }
                let Some(import) = Self::parse_player_facade_raw(raw) else {
                    continue;
                };
                let Some(overlay) = Self::player_overlay_from_raw(raw) else {
                    continue;
                };
                let Some(zombie_overlay) = overlay.zombie.as_ref() else {
                    continue;
                };
                return Ok(Some(Self::zombie_template_from_overlay(
                    &import,
                    team_index,
                    zombie_kind,
                    zombie_overlay,
                    registry,
                    skill_export_name_prefix,
                )?));
            }
        }
        Ok(None)
    }

    fn summon_template_from_overlay(
        import: &Self,
        team: usize,
        summon_kind: PlayerKindId,
        overlay: &crate::player::overlay::MinionOverlay,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> PlayerTemplate {
        let attrs = overlay.attrs.unwrap_or([0, DEFAULT_BED2_DEFENSE, 0, 0, 0, DEFAULT_BED2_RESISTANCE, 0, 1]);
        let skills = Self::summon_skill_loadout_from_overlay(overlay, fire_skill, explode_skill);
        PlayerTemplate::with_kind(
            0,
            format!("{}?0", import.name),
            summon_kind,
            team,
            attrs[7].max(1),
            attrs[0].max(0),
        )
        .with_def_res(attrs[1].max(0), attrs[5].max(0))
        .with_agility(attrs[3].max(0))
        .with_magic(attrs[4].max(0))
        .with_magic_point(attrs[6].max(0) >> 1)
        .with_wisdom(attrs[6].max(0))
        .with_speed_points(attrs[2].max(0) + 160)
        .with_policy_overrides(PlayerPolicyOverrides::default().with_inherit_owner_def_res(overlay.inherit_owner_def_res))
        .with_skill_loadout(skills)
    }

    fn shadow_template_from_overlay(
        import: &Self,
        team: usize,
        shadow_kind: PlayerKindId,
        overlay: &crate::player::overlay::MinionOverlay,
        possess_skill: SkillId,
    ) -> PlayerTemplate {
        let attrs = overlay.attrs.unwrap_or([0, 0, 0, 0, 0, 0, 0, 1]);
        let skills = Self::shadow_skill_loadout_from_overlay(overlay, possess_skill);
        PlayerTemplate::with_kind(
            0,
            format!("{}?shadow", import.name),
            shadow_kind,
            team,
            attrs[7].max(1),
            attrs[0].max(0),
        )
        .with_def_res(attrs[1].max(0), attrs[5].max(0))
        .with_agility(attrs[3].max(0))
        .with_magic(attrs[4].max(0))
        .with_magic_point(attrs[6].max(0) >> 1)
        .with_wisdom(attrs[6].max(0))
        .with_speed_points(-2048)
        .with_skill_loadout(skills)
    }

    fn zombie_template_from_overlay(
        import: &Self,
        team: usize,
        zombie_kind: PlayerKindId,
        overlay: &crate::player::overlay::MinionOverlay,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<PlayerTemplate, CustomBed2ZombieTemplateImportError> {
        let attrs = overlay.attrs.unwrap_or([0, 0, 0, 0, 0, 0, 0, 1]);
        let skills = Self::zombie_skill_loadout_from_overlay(overlay, registry, skill_export_name_prefix)?;
        Ok(PlayerTemplate::with_kind(
            0,
            format!("{}?zombie", import.name),
            zombie_kind,
            team,
            attrs[7].max(1),
            attrs[0].max(0),
        )
        .with_def_res(attrs[1].max(0), attrs[5].max(0))
        .with_agility(attrs[3].max(0))
        .with_magic(attrs[4].max(0))
        .with_magic_point(attrs[6].max(0) >> 1)
        .with_wisdom(attrs[6].max(0))
        .with_speed_points(0)
        .with_skill_loadout(skills))
    }

    fn summon_skill_loadout_from_overlay(
        overlay: &crate::player::overlay::MinionOverlay,
        fire_skill: SkillId,
        explode_skill: SkillId,
    ) -> SkillLoadout {
        let mut active_order = Vec::new();
        if let Some(skill_levels) = overlay.skills.as_ref() {
            for (name, _) in skill_levels {
                let Some(lane) = Self::summon_overlay_skill_lane(name) else {
                    continue;
                };
                if !active_order.contains(&lane) {
                    active_order.push(lane);
                }
            }
        }
        if active_order.is_empty() {
            active_order.extend([0, 1, 2]);
        }
        summon_default_skill_loadout(fire_skill, explode_skill, [0, 1, 2]).with_active_order(active_order)
    }

    fn summon_overlay_skill_lane(name: &str) -> Option<usize> {
        let skill_ref = crate::player::skill::parse_prefixed_classified_skill_name(name)
            .or_else(|| crate::player::skill::summon_slot_skill_ref_from_name(name))?;
        match skill_ref {
            crate::player::skill::ClassifiedSkillRef::SummonFire1 => Some(0),
            crate::player::skill::ClassifiedSkillRef::SummonFire2 => Some(1),
            crate::player::skill::ClassifiedSkillRef::SummonExplode => Some(2),
            _ => None,
        }
    }

    fn shadow_skill_loadout_from_overlay(
        overlay: &crate::player::overlay::MinionOverlay,
        possess_skill: SkillId,
    ) -> SkillLoadout {
        let mut active_order = Vec::new();
        if let Some(skill_levels) = overlay.skills.as_ref() {
            for (name, _) in skill_levels {
                let Some(lane) = Self::shadow_overlay_skill_lane(name) else {
                    continue;
                };
                if !active_order.contains(&lane) {
                    active_order.push(lane);
                }
            }
        }
        if active_order.is_empty() {
            active_order.push(0);
        }
        SkillLoadout::from_skills([possess_skill]).with_active_order(active_order)
    }

    fn shadow_overlay_skill_lane(name: &str) -> Option<usize> {
        let skill_ref = crate::player::skill::parse_prefixed_classified_skill_name(name)
            .or_else(|| crate::player::skill::phantom_skill_ref_from_name(name))?;
        match skill_ref {
            crate::player::skill::ClassifiedSkillRef::PhantomPossess => Some(0),
            _ => None,
        }
    }

    fn zombie_skill_loadout_from_overlay(
        overlay: &crate::player::overlay::MinionOverlay,
        registry: &ExtensionRegistry,
        skill_export_name_prefix: &str,
    ) -> Result<SkillLoadout, CustomBed2ZombieTemplateImportError> {
        let Some(skill_levels) = overlay.skills.as_ref() else {
            return Ok(SkillLoadout::default());
        };
        let mut skills = Vec::new();
        for (raw_name, _) in skill_levels {
            let Some(suffix) = Self::zombie_overlay_skill_export_suffix(raw_name) else {
                continue;
            };
            let export_name = if skill_export_name_prefix.is_empty() {
                suffix
            } else {
                format!("{skill_export_name_prefix}.{suffix}")
            };
            let skill_id = registry.skill_id_by_export_name(&export_name).ok_or_else(|| {
                CustomBed2ZombieTemplateImportError::MissingSkillExportName {
                    export_name: export_name.clone(),
                }
            })?;
            if !skills.contains(&skill_id) {
                skills.push(skill_id);
            }
        }
        Ok(SkillLoadout::from_skills(skills))
    }

    fn zombie_overlay_skill_export_suffix(name: &str) -> Option<String> {
        match crate::player::skill::player_classified_skill_ref_from_name(name) {
            Some(crate::player::skill::ClassifiedSkillRef::Normal(skill_id)) => {
                return Some(Self::normal_skill_export_suffix(skill_id));
            }
            Some(crate::player::skill::ClassifiedSkillRef::SummonFire1) => return Some("summon_fire1".to_owned()),
            Some(crate::player::skill::ClassifiedSkillRef::SummonFire2) => return Some("summon_fire2".to_owned()),
            Some(crate::player::skill::ClassifiedSkillRef::SummonExplode) => return Some("explode".to_owned()),
            Some(crate::player::skill::ClassifiedSkillRef::PhantomPossess) => return Some("possess".to_owned()),
            None => {}
        }
        match Self::normalize_minion_overlay_skill_name(name).as_str() {
            "possess" | "possession" => Some("possess".to_owned()),
            "explode" | "selfdestruct" | "self_destruct" | "summonexplode" => Some("explode".to_owned()),
            _ => crate::player::skill::skill_name_to_id(name).map(Self::normal_skill_export_suffix),
        }
    }

    fn normal_skill_export_suffix(skill_id: usize) -> String {
        let export_name = crate::player::skill::skill_name_for_export(skill_id);
        export_name.strip_prefix("skl").unwrap_or(export_name.as_str()).to_ascii_lowercase()
    }

    fn normalize_minion_overlay_skill_name(name: &str) -> String {
        let lower = name.trim().to_ascii_lowercase();
        lower
            .strip_prefix("skl")
            .or_else(|| lower.strip_prefix("skill"))
            .unwrap_or(lower.as_str())
            .to_string()
    }

    fn player_overlay_from_raw(raw: &str) -> Option<crate::player::overlay::PlayerOverlay> {
        Self::split_by_plus_outside_json(raw)
            .into_iter()
            .filter_map(|segment| crate::player::overlay::PlayerOverlay::parse_inline(segment.trim()))
            .last()
    }

    fn split_by_plus_outside_json(raw: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut escaped = false;
        let mut brace_depth = 0usize;
        let mut bracket_depth = 0usize;
        for ch in raw.chars() {
            if in_string {
                current.push(ch);
                if escaped {
                    escaped = false;
                    continue;
                }
                match ch {
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
            } else if ch == '+' && brace_depth == 0 && bracket_depth == 0 {
                segments.push(std::mem::take(&mut current));
            } else {
                current.push(ch);
                match ch {
                    '"' => in_string = true,
                    '{' => brace_depth += 1,
                    '}' => brace_depth = brace_depth.saturating_sub(1),
                    '[' => bracket_depth += 1,
                    ']' => bracket_depth = bracket_depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        segments.push(current);
        segments
    }
}

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub action: Option<ActionPlan>,
    pub frame: Option<RuntimeFrame>,
    pub winner_team: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CombatRuntime {
    pub entities: EntityArena,
    pub world: WorldArena,
    pub scheduler: PhaseScheduler,
    pub effects: EffectQueue,
    pub effect_handlers: EffectHandlers,
    pub skill_handlers: SkillHandlers,
    pub state_handlers: StateHandlers,
    pub replay_renderers: ReplayRenderers,
    pub show_renderers: ShowRenderers,
    pub scratch: BattleScratch,
    pub template_slots: TemplateSlotStorage,
    pub slots: BattleSlotStorage,
    pub registry: ExtensionRegistry,
    pub rng: RC4,
    #[cfg(not(feature = "no_debug"))]
    pub trace: Option<RuntimeTrace>,
    pub round: u64,
}

impl CombatRuntime {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let PreparedCombatTemplate {
            players,
            registry,
            slots: template_slots,
        } = template;
        let entities = EntityArena::from_templates_with_registry(players, &registry);
        let world = WorldArena::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&registry);
        let effect_handlers = EffectHandlers::from_registry(&registry);
        let skill_handlers = SkillHandlers::from_registry(&registry);
        let state_handlers = StateHandlers::from_registry(&registry);
        let replay_renderers = ReplayRenderers::from_registry(&registry);
        let show_renderers = ShowRenderers::from_registry(&registry);
        Self {
            entities,
            world,
            scheduler: PhaseScheduler,
            effects: EffectQueue::default(),
            effect_handlers,
            skill_handlers,
            state_handlers,
            replay_renderers,
            show_renderers,
            scratch: BattleScratch::default(),
            template_slots,
            slots,
            registry,
            rng: RC4::default(),
            #[cfg(not(feature = "no_debug"))]
            trace: None,
            round: 0,
        }
    }

    #[cfg(not(feature = "no_debug"))]
    pub fn enable_trace(&mut self) { self.trace = Some(RuntimeTrace::default()); }

    #[cfg(not(feature = "no_debug"))]
    pub fn trace(&self) -> Option<&RuntimeTrace> { self.trace.as_ref() }

    pub fn set_effect_handler(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.effect_handlers.set(id, handler); }

    pub fn set_effect_handler_with_capabilities(
        &mut self,
        id: EffectHandlerId,
        handler: EffectHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.effect_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_skill_handler(&mut self, id: SkillId, handler: SkillHandlerFn) { self.skill_handlers.set(id, handler); }

    pub fn set_skill_handler_with_capabilities(
        &mut self,
        id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.skill_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_state_handler(&mut self, id: StateId, handler: StateHandlerFn) { self.state_handlers.set(id, handler); }

    pub fn set_state_handler_with_capabilities(
        &mut self,
        id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.state_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_replay_renderer(&mut self, id: ReplayRendererId, renderer: ReplayRendererFn) {
        self.replay_renderers.set(id, renderer);
    }

    pub fn set_show_renderer(&mut self, id: ShowRendererId, renderer: ShowRendererFn) { self.show_renderers.set(id, renderer); }

    pub fn render_replay_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedReplay> {
        self.registry
            .replay_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.replay_renderers.get(spec.id) else {
                    panic!("missing runtime_v2 replay renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    pub fn render_show_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedShow> {
        self.registry
            .show_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.show_renderers.get(spec.id) else {
                    panic!("missing runtime_v2 show renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    pub fn run_skill_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, owner, hook);
        self.flush_skill_hook_plan(&plan)
    }

    fn flush_skill_hook_plan(&mut self, plan: &SkillHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_skill_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_skill_hook_plan_into(&mut self, plan: &SkillHookPlan, updates: &mut RunUpdates) {
        self.drain_skill_hook_plan_with_selected_target_into(plan, updates, None);
    }

    fn drain_skill_hook_plan_with_selected_target_into(
        &mut self,
        plan: &SkillHookPlan,
        updates: &mut RunUpdates,
        selected_target: Option<EntityIdx>,
    ) {
        for entry in &plan.entries {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
            };
            {
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(&[]);
                let mut context = {
                    let context = SkillContext::new(
                        &mut self.entities,
                        &mut self.world,
                        &self.template_slots,
                        &mut self.slots,
                        &mut self.effects,
                        updates,
                        &mut self.rng,
                        *entry,
                        capabilities,
                    );
                    match selected_target {
                        Some(target) => context.with_selected_target(target),
                        None => context,
                    }
                };
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    fn drain_skill_hook_plan_with_defend_value_into(
        &mut self,
        plan: &SkillHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        for entry in &plan.entries {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
            };
            {
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(&[]);
                let mut context = SkillContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    capabilities,
                )
                .with_defend_value(defend_value);
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    pub fn run_state_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.state_hook_plan(&self.entities, owner, hook);
        self.flush_state_hook_plan(&plan)
    }

    fn flush_state_hook_plan(&mut self, plan: &StateHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_state_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_state_hook_plan_into(&mut self, plan: &StateHookPlan, updates: &mut RunUpdates) {
        for entry in &plan.entries {
            let Some(state_id) = entry.state_id else {
                continue;
            };
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
            };
            {
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(&[]);
                let mut context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    capabilities,
                );
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    fn drain_state_hook_plan_with_defend_value_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        for entry in &plan.entries {
            let Some(state_id) = entry.state_id else {
                continue;
            };
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
            };
            {
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(&[]);
                let mut context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    capabilities,
                )
                .with_defend_value(defend_value);
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    pub fn run_minimal_round(&mut self) -> RoundOutcome {
        if let Some(winner_team) = self.world.sync_winner(&self.entities) {
            return RoundOutcome {
                action: None,
                frame: None,
                winner_team: Some(winner_team),
            };
        }

        let Some(action) = self.scheduler.select_minimal_action(&mut self.world, &self.entities) else {
            return RoundOutcome {
                action: None,
                frame: None,
                winner_team: None,
            };
        };
        self.scratch.selected_actor_round = self.round;
        #[cfg(not(feature = "no_debug"))]
        let action_rng_before = RngCheckpoint::from_rc4(&self.rng);
        let smart = self.roll_actor_smart(action.actor);
        #[cfg(not(feature = "no_debug"))]
        let action_rng_after = RngCheckpoint::from_rc4(&self.rng);
        #[cfg(not(feature = "no_debug"))]
        if let Some(trace) = &mut self.trace {
            trace.record_action(TraceAction {
                round: self.round + 1,
                actor: action.actor,
                target: action.target,
                amount: action.amount,
                rng_before: Some(action_rng_before),
                rng_after: Some(action_rng_after),
            });
        }

        let mut updates = RunUpdates::new();
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_ACTION);
        let selected_target = self.selected_pre_action_target(&skill_plan, action.actor, smart).unwrap_or(action.target);
        self.drain_skill_hook_plan_with_selected_target_into(&skill_plan, &mut updates, Some(selected_target));
        let pre_damage_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_DAMAGE);
        self.drain_skill_hook_plan_into(&pre_damage_skill_plan, &mut updates);
        let pre_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_DAMAGE);
        self.drain_state_hook_plan_into(&pre_damage_state_plan, &mut updates);
        self.effects.push(QueuedEffect::Damage {
            caster: action.actor,
            target: action.target,
            amount: action.amount,
        });
        self.drain_effects_into(&mut updates);
        let post_damage_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_DAMAGE);
        self.drain_skill_hook_plan_into(&post_damage_skill_plan, &mut updates);
        let post_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_DAMAGE);
        self.drain_state_hook_plan_into(&post_damage_state_plan, &mut updates);
        let post_action_skill_plan =
            self.scheduler
                .skill_post_action_hook_plan(&self.entities, &self.registry, action.actor, SkillPostActionPhase::Early);
        self.drain_skill_hook_plan_into(&post_action_skill_plan, &mut updates);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_ACTION);
        self.drain_state_hook_plan_into(&state_plan, &mut updates);
        let post_action_late_skill_plan =
            self.scheduler
                .skill_post_action_hook_plan(&self.entities, &self.registry, action.actor, SkillPostActionPhase::Late);
        self.drain_skill_hook_plan_into(&post_action_late_skill_plan, &mut updates);
        let frame = updates.had_updates().then_some(RuntimeFrame { updates });
        self.round += 1;
        let winner_team = self.world.sync_winner(&self.entities);
        #[cfg(not(feature = "no_debug"))]
        if let (Some(trace), Some(frame)) = (&mut self.trace, &frame) {
            trace.record_frame(self.round, frame, winner_team, Some(RngCheckpoint::from_rc4(&self.rng)));
        }
        RoundOutcome {
            action: Some(action),
            frame,
            winner_team,
        }
    }

    fn roll_actor_smart(&mut self, actor: EntityIdx) -> bool {
        let smart_byte = self.rng.next_u8();
        let smart_roll = (smart_byte & 63) as i32;
        self.entities.get(actor).is_some_and(|entity| entity.runtime.wisdom > smart_roll)
    }

    fn selected_pre_action_target(&mut self, plan: &SkillHookPlan, actor: EntityIdx, smart: bool) -> Option<EntityIdx> {
        let has_disperse = plan.entries.iter().any(|entry| {
            self.registry
                .skill(entry.skill_id)
                .is_some_and(|spec| spec.export_name == "core.disperse")
        });
        if !has_disperse {
            return None;
        }
        select_disperse_targets(&self.entities, &self.world, actor, smart, &mut self.rng)
            .into_iter()
            .next()
    }

    #[cfg(test)]
    fn flush_effects(&mut self) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_effects_into(&mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_effects_into(&mut self, updates: &mut RunUpdates) {
        while let Some(effect) = self.effects.pop_next() {
            match effect {
                QueuedEffect::Damage { caster, target, amount } => {
                    self.ensure_effect_entity("damage", "caster", caster);
                    self.ensure_effect_entity("damage", "target", target);
                    let resolved_target = self.resolve_damage_target(target);
                    self.ensure_effect_entity("damage", "resolved target", resolved_target);
                    let share_targets = self.resolve_damage_share_targets(target, resolved_target);
                    if self.apply_damage_into(caster, resolved_target, amount, updates) {
                        self.drain_lethal_damage_hooks_into(caster, resolved_target, updates);
                    }
                    for share_target in share_targets {
                        self.ensure_effect_entity("damage", "share target", share_target);
                        if self.apply_damage_into(caster, share_target, amount, updates) {
                            self.drain_lethal_damage_hooks_into(caster, share_target, updates);
                        }
                    }
                }
                QueuedEffect::PoisonTick { caster, target, amount } => {
                    self.ensure_effect_entity("poison tick", "caster", caster);
                    self.ensure_effect_entity("poison tick", "target", target);
                    if self.apply_poison_tick_damage_into(caster, target, amount, updates) {
                        self.drain_lethal_damage_hooks_into(caster, target, updates);
                    } else if self.entities.get(target).map(|entity| entity.runtime.alive).unwrap_or(false) {
                        self.emit_poison_release_if_cleared(target, updates);
                    }
                }
                QueuedEffect::SummonExplode {
                    caster,
                    target,
                    fire_state_key,
                } => {
                    self.ensure_effect_entity("summon-explode", "caster", caster);
                    self.ensure_effect_entity("summon-explode", "target", target);
                    let fire_mag = self.entities.get(target).unwrap().states.fire_mag(fire_state_key);
                    let atp = self.entities.get(caster).unwrap().runtime.get_at(true, &mut self.rng);
                    let mut defend_value = RuntimeDefendValue::Atp {
                        value: atp * (4.0 + fire_mag),
                        caster,
                        target,
                    };
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        "[0]使用[自爆]",
                        0,
                    ));
                    let killed_caster = self.kill_entity_without_damage_into(caster, updates);
                    self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
                    let Some(atp) = defend_value.atp() else {
                        panic!("runtime_v2 PRE_DEFEND hooks must leave an atp value");
                    };
                    if atp == 0.0 {
                        if killed_caster {
                            self.drain_die_hooks_into(caster, updates);
                        }
                        continue;
                    }
                    if self.magic_attack_dodged(caster, target) {
                        updates.add(RuntimeFrame::replay_update(
                            target.0 as usize,
                            caster.0 as usize,
                            "[0][回避]了攻击",
                            20,
                        ));
                    } else {
                        let amount = (atp / self.entities.get(target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
                        let mut defend_value = RuntimeDefendValue::Damage {
                            value: amount,
                            caster,
                            target,
                        };
                        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
                        let Some(amount) = defend_value.damage() else {
                            panic!("runtime_v2 POST_DEFEND hooks must leave a damage value");
                        };
                        if self.apply_damage_into(caster, target, amount, updates) {
                            self.drain_lethal_damage_hooks_into(caster, target, updates);
                        } else if amount > 0 {
                            self.apply_fire_on_damage(target, fire_state_key);
                        }
                    }
                    if killed_caster {
                        self.drain_die_hooks_into(caster, updates);
                    }
                }
                QueuedEffect::DisperseAttack { caster, target } => {
                    self.ensure_effect_entity("disperse-attack", "caster", caster);
                    self.ensure_effect_entity("disperse-attack", "target", target);
                    let mut atp = self.entities.get(caster).unwrap().runtime.get_at(true, &mut self.rng);
                    if self.entities.get(target).unwrap().runtime.flags.contains(PlayerKindFlags::MINION) {
                        atp *= 2.0;
                    }
                    let mut defend_value = RuntimeDefendValue::Atp {
                        value: atp,
                        caster,
                        target,
                    };
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        "[0]使用[净化]",
                        20,
                    ));
                    self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
                    let Some(atp) = defend_value.atp() else {
                        panic!("runtime_v2 PRE_DEFEND hooks must leave an atp value");
                    };
                    if atp == 0.0 {
                        continue;
                    }
                    if self.magic_attack_dodged(caster, target) {
                        updates.add(RuntimeFrame::replay_update(
                            target.0 as usize,
                            caster.0 as usize,
                            "[0][回避]了攻击",
                            20,
                        ));
                    } else {
                        let amount = (atp / self.entities.get(target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
                        let mut defend_value = RuntimeDefendValue::Damage {
                            value: amount,
                            caster,
                            target,
                        };
                        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
                        let Some(amount) = defend_value.damage() else {
                            panic!("runtime_v2 POST_DEFEND hooks must leave a damage value");
                        };
                        if self.apply_disperse_attack_damage_into(caster, target, amount, updates) {
                            self.drain_lethal_damage_hooks_into(caster, target, updates);
                        }
                    }
                }
                QueuedEffect::DisperseHit { caster, target, damage } => {
                    self.ensure_effect_entity("disperse-hit", "caster", caster);
                    self.ensure_effect_entity("disperse-hit", "target", target);
                    if damage > 0 {
                        self.apply_disperse_hit_into(caster, target, updates);
                    }
                }
                QueuedEffect::Heal { caster, target, amount } => {
                    self.ensure_effect_entity("heal", "caster", caster);
                    self.ensure_effect_entity("heal", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 heal target entity: {}", target.0);
                    };
                    let was_alive = target_entity.runtime.alive;
                    target_entity.runtime.hp = (target_entity.runtime.hp + amount.max(0)).min(target_entity.template.max_hp);
                    if target_entity.runtime.hp > 0 {
                        target_entity.runtime.alive = true;
                    }
                    let team = target_entity.runtime.team;
                    updates.add(RuntimeFrame::heal_update(caster.0 as usize, target.0 as usize, amount));
                    if !was_alive && target_entity.runtime.alive {
                        self.world.revive_round_actor(target);
                        self.world.revive_alive(target, team);
                    }
                }
                QueuedEffect::Spawn { caster, template } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                    updates.add(RuntimeFrame::spawn_update(caster.0 as usize, spawned.0 as usize));
                }
                QueuedEffect::SpawnSilent { caster, template } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                }
                QueuedEffect::SpawnWithMessage {
                    caster,
                    template,
                    message,
                } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                    updates.add(RuntimeFrame::replay_update(caster.0 as usize, spawned.0 as usize, message, 0));
                }
                QueuedEffect::AddState { target, state } => {
                    self.ensure_effect_entity("add-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 add-state target entity: {}", target.0);
                    };
                    if target_entity.states.add_entry(state) {
                        updates.add(RuntimeFrame::add_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::ClearState {
                    target,
                    legacy_order_key,
                } => {
                    self.ensure_effect_entity("clear-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 clear-state target entity: {}", target.0);
                    };
                    if target_entity.states.clear_legacy_key(legacy_order_key) {
                        updates.add(RuntimeFrame::clear_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::Revive { caster, target, hp } => {
                    self.ensure_effect_entity("revive", "caster", caster);
                    self.ensure_effect_entity("revive", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 revive target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = hp.max(1).min(target_entity.template.max_hp);
                    target_entity.runtime.alive = true;
                    let team = target_entity.runtime.team;
                    self.world.revive_round_actor(target);
                    self.world.revive_alive(target, team);
                    updates.add(RuntimeFrame::revive_update(caster.0 as usize, target.0 as usize, hp));
                }
                QueuedEffect::ReviveWithMessage {
                    caster,
                    target,
                    hp,
                    message,
                } => {
                    self.ensure_effect_entity("revive", "caster", caster);
                    self.ensure_effect_entity("revive", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 revive target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = hp.max(1).min(target_entity.template.max_hp);
                    target_entity.runtime.alive = true;
                    let team = target_entity.runtime.team;
                    self.world.revive_round_actor(target);
                    self.world.revive_alive(target, team);
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        message,
                        hp.max(0) as u32,
                    ));
                }
                QueuedEffect::Remove { caster, target } => {
                    self.ensure_effect_entity("remove", "caster", caster);
                    self.ensure_effect_entity("remove", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 remove target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = 0;
                    target_entity.runtime.alive = false;
                    let team = target_entity.runtime.team;
                    self.world.remove_round_actor(target);
                    self.world.remove_alive(target, team);
                    updates.add(RuntimeFrame::remove_update(caster.0 as usize, target.0 as usize));
                    self.cleanup_linked_minions_for_owner(target, updates);
                }
                QueuedEffect::Merge { caster, target } => {
                    self.ensure_effect_entity("merge", "caster", caster);
                    self.ensure_effect_entity("merge", "target", target);
                    let target_skills = self.entities.get(target).unwrap().template.skills.clone();
                    let policy = self.entities.get(caster).unwrap().runtime.policies.merge;
                    let Some(caster_entity) = self.entities.get_mut(caster) else {
                        panic!("unknown runtime_v2 merge caster entity: {}", caster.0);
                    };
                    if caster_entity.template.skills.merge_fixed_lanes_from(&target_skills, policy) {
                        updates.add(crate::engine::update::RunUpdate::new(
                            "[0][吞噬]了[1]",
                            caster.0 as usize,
                            target.0 as usize,
                            60,
                        ));
                        updates.add(crate::engine::update::RunUpdate::new(
                            "[0]属性上升",
                            caster.0 as usize,
                            target.0 as usize,
                            0,
                        ));
                    }
                }
                QueuedEffect::Replay {
                    caster,
                    target,
                    message,
                    score,
                } => {
                    self.ensure_effect_entity("replay", "caster", caster);
                    self.ensure_effect_entity("replay", "target", target);
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        message,
                        score,
                    ));
                }
                QueuedEffect::Custom(custom) => {
                    self.ensure_effect_entity("custom", "caster", custom.caster);
                    if let Some(target) = custom.target {
                        self.ensure_effect_entity("custom", "target", target);
                    }
                    let Some(handler) = self.effect_handlers.get(custom.handler) else {
                        panic!("missing runtime_v2 effect handler implementation: {}", custom.handler.0);
                    };
                    let capabilities = self.effect_handlers.capabilities(custom.handler).unwrap_or(&[]);
                    let mut context = EffectContext::new(
                        &mut self.entities,
                        &mut self.world,
                        &self.template_slots,
                        &mut self.slots,
                        &mut self.effects,
                        updates,
                        &mut self.rng,
                        &custom,
                        capabilities,
                    );
                    handler(&mut context, &custom);
                }
            }
        }
    }

    fn apply_disperse_hit_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 disperse target entity: {}", target.0);
        };
        let clear_messages = target_entity.clear_positive_messages();
        let mp = target_entity.runtime.magic_point;
        target_entity.runtime.magic_point = if mp > 64 {
            mp - 64
        } else if mp > 32 {
            0
        } else {
            mp - 32
        };
        for (_, message) in clear_messages {
            updates.add_newline();
            updates.add(RuntimeFrame::replay_update(caster.0 as usize, target.0 as usize, message, 0));
        }
    }

    fn drain_lethal_damage_hooks_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.drain_die_hooks_into(target, updates);
        self.drain_kill_hooks_into(caster, updates);
    }

    fn drain_pre_defend_hooks_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, target, ProcMask::PRE_DEFEND);
        self.drain_skill_hook_plan_with_defend_value_into(&skill_plan, updates, defend_value);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::PRE_DEFEND);
        self.drain_state_hook_plan_with_defend_value_into(&state_plan, updates, defend_value);
    }

    fn drain_post_defend_hooks_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, target, ProcMask::POST_DEFEND);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::POST_DEFEND);

        #[derive(Clone, Copy)]
        enum DefendHookPlanEntry {
            Skill(SkillHookPlanEntry),
            State(StateHookPlanEntry),
        }

        let mut entries = Vec::with_capacity(skill_plan.entries.len() + state_plan.entries.len());
        entries.extend(skill_plan.entries.iter().copied().map(|entry| {
            (
                entry.priority,
                0_u8,
                entry.active_order,
                entry.registration_order,
                DefendHookPlanEntry::Skill(entry),
            )
        }));
        entries.extend(state_plan.entries.iter().copied().map(|entry| {
            (
                entry.priority,
                1_u8,
                usize::MAX,
                entry.registration_order,
                DefendHookPlanEntry::State(entry),
            )
        }));
        entries.sort_by_key(|(priority, kind_order, active_order, registration_order, _)| {
            (*priority, *kind_order, *active_order, *registration_order)
        });

        for (_, _, _, _, entry) in entries {
            match entry {
                DefendHookPlanEntry::Skill(entry) => {
                    let plan = SkillHookPlan {
                        owner: skill_plan.owner,
                        hook: skill_plan.hook,
                        loadout_len: skill_plan.loadout_len,
                        entries: vec![entry],
                    };
                    self.drain_skill_hook_plan_with_defend_value_into(&plan, updates, defend_value);
                }
                DefendHookPlanEntry::State(entry) => {
                    let plan = StateHookPlan {
                        hook: state_plan.hook,
                        store_generation: state_plan.store_generation,
                        entries: vec![entry],
                    };
                    self.drain_state_hook_plan_with_defend_value_into(&plan, updates, defend_value);
                }
            }
        }
    }

    fn drain_die_hooks_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let die_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, target, ProcMask::DIE);
        self.drain_skill_hook_plan_into(&die_skill_plan, updates);
        let die_state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::DIE);
        self.drain_state_hook_plan_into(&die_state_plan, updates);
    }

    fn drain_kill_hooks_into(&mut self, caster: EntityIdx, updates: &mut RunUpdates) {
        let kill_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, caster, ProcMask::KILL);
        self.drain_skill_hook_plan_into(&kill_skill_plan, updates);
        let kill_state_plan = self.scheduler.state_hook_plan(&self.entities, caster, ProcMask::KILL);
        self.drain_state_hook_plan_into(&kill_state_plan, updates);
    }

    fn apply_damage_into(&mut self, caster: EntityIdx, target: EntityIdx, amount: i32, updates: &mut RunUpdates) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 damage target entity: {}", target.0);
        };
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        if killed {
            target_entity.runtime.alive = false;
        }
        let team = target_entity.runtime.team;
        updates.add(RuntimeFrame::damage_update(caster.0 as usize, target.0 as usize, amount));
        if killed {
            self.world.remove_alive(target, team);
            self.cleanup_linked_minions_for_owner(target, updates);
        }
        killed
    }

    fn apply_poison_tick_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 poison tick target entity: {}", target.0);
        };
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        if killed {
            target_entity.runtime.alive = false;
        }
        let team = target_entity.runtime.team;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        if killed {
            self.world.remove_alive(target, team);
            self.cleanup_linked_minions_for_owner(target, updates);
        }
        killed
    }

    fn apply_disperse_attack_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 disperse damage target entity: {}", target.0);
        };
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        let team = target_entity.runtime.team;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        if amount > 0 {
            self.apply_disperse_hit_into(caster, target, updates);
        }
        if killed {
            let Some(target_entity) = self.entities.get_mut(target) else {
                panic!("unknown runtime_v2 disperse damage target entity: {}", target.0);
            };
            target_entity.runtime.alive = false;
            self.world.remove_alive(target, team);
            self.cleanup_linked_minions_for_owner(target, updates);
        }
        killed
    }

    fn emit_poison_release_if_cleared(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 poison release target entity: {}", target.0);
        };
        if target_entity
            .states
            .entries()
            .iter()
            .any(|entry| matches!(entry.payload, StatePayload::Poison { .. }))
        {
            return;
        }
        updates.add_newline();
        updates.add(RuntimeFrame::replay_update(
            target.0 as usize,
            target.0 as usize,
            "[1]从[中毒]中解除",
            0,
        ));
    }

    fn magic_attack_dodged(&mut self, caster: EntityIdx, target: EntityIdx) -> bool {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 magic attack dodge target entity: {}", target.0);
        };
        if !target_entity.runtime.alive {
            return false;
        }

        let accuracy = self.entities.get(caster).unwrap().runtime.magic_accuracy();
        let dodge_value = target_entity.runtime.magic_dodge();
        PlayerRuntime::dodge(accuracy, dodge_value, &mut self.rng)
    }

    fn apply_fire_on_damage(&mut self, target: EntityIdx, fire_state_key: u32) {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 fire target entity: {}", target.0);
        };
        if target_entity.runtime.hp <= 0 || self.fire_immune(target) {
            return;
        }

        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 fire target entity: {}", target.0);
        };
        target_entity.states.add_fire_mag_half_step(fire_state_key);
    }

    fn fire_immune(&mut self, target: EntityIdx) -> bool {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 fire immune target entity: {}", target.0);
        };
        if target_entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_entity.template.name, "fire");
            return (self.rng.next_u8() as i32) < threshold;
        }
        if target_entity.runtime.flags.contains(PlayerKindFlags::BOOST) {
            return self.rng.r127() < crate::player::boost_value(&target_entity.template.name);
        }
        false
    }

    fn kill_entity_without_damage_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 self-death target entity: {}", target.0);
        };
        let killed = target_entity.runtime.alive;
        target_entity.runtime.hp = 0;
        target_entity.runtime.alive = false;
        let team = target_entity.runtime.team;
        if killed {
            self.world.remove_alive(target, team);
            self.cleanup_linked_minions_for_owner(target, updates);
        }
        killed
    }

    fn cleanup_linked_minions_for_owner(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let linked_minions = self
            .entities
            .iter()
            .filter_map(|(idx, entity)| {
                (idx != owner
                    && entity.runtime.alive
                    && entity.runtime.owner == owner
                    && entity.runtime.flags.contains(PlayerKindFlags::MINION))
                .then_some(idx)
            })
            .collect::<Vec<_>>();

        for minion in linked_minions {
            let Some(minion_entity) = self.entities.get_mut(minion) else {
                panic!("unknown runtime_v2 linked minion entity: {}", minion.0);
            };
            minion_entity.runtime.hp = 0;
            minion_entity.runtime.alive = false;
            let team = minion_entity.runtime.team;
            self.world.remove_round_actor(minion);
            self.world.remove_alive(minion, team);
            updates.add(RuntimeFrame::remove_update(owner.0 as usize, minion.0 as usize));
        }
    }

    fn resolve_damage_target(&self, target: EntityIdx) -> EntityIdx {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 damage target entity: {}", target.0));
        match target_entity.runtime.policies.owner_resolution {
            OwnerResolutionPolicy::SelfEntity => target,
            OwnerResolutionPolicy::RootOwner => target_entity.runtime.root_owner,
        }
    }

    fn resolve_damage_share_targets(&self, target: EntityIdx, resolved_target: EntityIdx) -> Vec<EntityIdx> {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 damage target entity: {}", target.0));
        match target_entity.runtime.policies.damage_share {
            DamageSharePolicy::None => Vec::new(),
            DamageSharePolicy::ShareToOwner => {
                let owner = target_entity.runtime.owner;
                (owner != resolved_target).then_some(owner).into_iter().collect()
            }
            DamageSharePolicy::ShareToSummons => {
                if resolved_target != target {
                    return Vec::new();
                }
                self.entities
                    .iter()
                    .filter_map(|(idx, entity)| {
                        (idx != target && entity.runtime.alive && entity.runtime.owner == target).then_some(idx)
                    })
                    .collect()
            }
        }
    }

    fn ensure_effect_entity(&self, effect: &'static str, role: &'static str, entity: EntityIdx) {
        if self.entities.get(entity).is_none() {
            panic!("unknown runtime_v2 {effect} {role} entity: {}", entity.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalized_rng_checkpoint(i: u32, j: u32) -> crate::runtime_v2::oracle::NormalizedRngCheckpoint {
        crate::runtime_v2::oracle::NormalizedRngCheckpoint {
            i,
            j,
            #[cfg(not(feature = "no_debug"))]
            byte_count: 0,
        }
    }

    #[test]
    fn minimal_1v1_template_builds_runtime() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(runtime.world.winner_team(), None);
        assert!(runtime.effects.is_empty());
        assert!(runtime.slots.is_empty());
    }

    #[test]
    fn runtime_from_template_reserves_registered_slot_storage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let template_slot = builder
            .reserve_template_slot("custom", "template", "custom.template")
            .expect("template slot should reserve");
        let battle_slot = builder
            .reserve_battle_slot("custom", "battle", "custom.battle")
            .expect("battle slot should reserve");
        let entity_slot = builder
            .reserve_entity_slot("custom", "entity", "custom.entity")
            .expect("entity slot should reserve");
        let registry = builder.build();
        let mut template = PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], registry);
        template
            .slots
            .set(template_slot, SlotValue::Text("seed".to_owned()))
            .expect("template slot should write");

        let mut runtime = CombatRuntime::from_template(template);
        runtime.slots.set(battle_slot, SlotValue::U64(1)).expect("battle slot should write");
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(entity_slot, SlotValue::Bool(true))
            .expect("entity slot should write");

        assert_eq!(
            runtime.template_slots.get(template_slot),
            Some(&SlotValue::Text("seed".to_owned()))
        );
        assert_eq!(runtime.slots.get(battle_slot), Some(&SlotValue::U64(1)));
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(entity_slot),
            Some(&SlotValue::Bool(true))
        );
    }

    #[test]
    fn custom_bed2_fixture_maps_kind_skill_and_marker_slots() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill_with_hooks(
                "custom",
                "summon",
                "custom.summon",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let summon_template = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let hp_marker = builder
            .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
            .expect("hp marker slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let registry = builder.build();
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "bed2", bed2, 0, 3000, 0)
                    .with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE)
                    .with_skills([summon]),
            ],
            registry,
        );
        let bed2_summon_template = PlayerTemplate::with_kind(2, "bed2?0", summon_kind, 0, 1000, 1)
            .with_def_res(99, 99)
            .with_skills([fire, explode]);
        template
            .slots
            .set(
                summon_template,
                SlotValue::PlayerTemplate(Box::new(bed2_summon_template.clone())),
            )
            .expect("bed2 summon template slot should write");

        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            summon,
            skill_bed2_template_slot_summon_handler,
            &[ExtensionCapability::ReadTemplateSlots],
        );
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(hp_marker, SlotValue::Bool(true))
            .expect("hp marker slot should write");
        let entity = runtime.entities.get(EntityIdx(0)).expect("bed2 entity should exist");

        assert_eq!(entity.template.max_hp, 3000);
        assert_eq!(entity.template.skills.skills(), &[summon]);
        assert!(entity.runtime.flags.contains(PlayerKindFlags::BED2));
        assert_eq!(entity.runtime.policies.owner_resolution, OwnerResolutionPolicy::RootOwner);
        assert_eq!(entity.runtime.policies.damage_share, DamageSharePolicy::ShareToOwner);
        assert_eq!(entity.runtime.policies.merge, MergePolicy::FixedLane);
        assert_eq!(entity.slots.get(hp_marker), Some(&SlotValue::Bool(true)));
        assert_eq!(
            runtime.template_slots.get(summon_template),
            Some(&SlotValue::PlayerTemplate(Box::new(bed2_summon_template.clone())))
        );
        let SlotValue::PlayerTemplate(stored_template) =
            runtime.template_slots.get(summon_template).expect("bed2 summon template should persist")
        else {
            panic!("bed2 summon template slot should hold a PlayerTemplate payload");
        };
        assert_eq!(stored_template.kind, summon_kind);
        assert_eq!(stored_template.max_hp, 1000);
        assert_eq!(stored_template.defense, DEFAULT_BED2_DEFENSE);
        assert_eq!(stored_template.resistance, DEFAULT_BED2_RESISTANCE);
        assert_eq!(stored_template.skills.skills(), &[fire, explode]);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("bed2 summon handler should spawn template payload");

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
        assert_eq!(frame.updates.updates[0].target, 1);
        let summoned = runtime.entities.get(EntityIdx(1)).expect("bed2 summon should spawn from template slot");
        assert_eq!(summoned.template.kind, summon_kind);
        assert_eq!(summoned.template.max_hp, 1000);
        assert_eq!(summoned.template.skills.skills(), &[fire, explode]);
        assert_eq!(summoned.runtime.owner, EntityIdx(0));
        assert_eq!(summoned.runtime.root_owner, EntityIdx(0));
        assert_eq!(summoned.runtime.defense, DEFAULT_BED2_DEFENSE);
        assert_eq!(summoned.runtime.resistance, DEFAULT_BED2_RESISTANCE);
    }

    #[test]
    fn push_summon_from_template_slot_reports_missing_template_payload() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill_with_hooks(
                "custom",
                "summon",
                "custom.summon",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon skill should register");
        builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "bed2", 0, 3000, 0).with_skills([summon])],
            registry,
        ));
        runtime.set_skill_handler_with_capabilities(
            summon,
            skill_records_missing_template_slot_error,
            &[ExtensionCapability::ReadTemplateSlots],
        );

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("missing template payload should be recorded");

        assert_eq!(runtime.entities.len(), 1);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "missing summon template");
    }

    #[test]
    fn push_summon_from_template_slot_can_emit_legacy_summon_message() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill_with_hooks(
                "custom",
                "summon",
                "custom.summon",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon skill should register");
        let summon_template = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let registry = builder.build();
        let mut template = PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "bed2", 0, 3000, 0).with_skills([summon])],
            registry,
        );
        let payload = PlayerTemplate::new(2, "bed2?0", 0, 1000, 1).with_skills([summon]);
        template
            .slots
            .set(summon_template, SlotValue::PlayerTemplate(Box::new(payload.clone())))
            .expect("bed2 summon template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            summon,
            skill_bed2_template_slot_legacy_summon_handler,
            &[ExtensionCapability::ReadTemplateSlots],
        );

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("bed2 summon handler should spawn template payload");

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
        assert_eq!(frame.updates.updates[0].target, 1);
        let summoned = runtime.entities.get(EntityIdx(1)).expect("summon should spawn");
        assert_eq!(summoned.template.name, payload.name);
        assert_eq!(summoned.template.skills.skills(), payload.skills.skills());
        assert_eq!(summoned.runtime.owner, EntityIdx(0));
        assert_eq!(summoned.runtime.root_owner, EntityIdx(0));
    }

    #[test]
    fn custom_bed2_import_fixture_parses_markers_into_v2_template() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let plus = CustomBed2Import::parse("alpha@red+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}")
            .expect("bed2 plus marker should parse");
        let legacy_team = CustomBed2Import::parse("beta@blue@bed2").expect("legacy bed2 team marker should parse");
        let bare = CustomBed2Import::parse("gamma+bed2[2500]").expect("bare bed2 marker should parse");

        assert_eq!(plus.name, "alpha");
        assert_eq!(plus.team.as_deref(), Some("red"));
        assert_eq!(plus.hp, 4500);
        assert_eq!(legacy_team.name, "beta");
        assert_eq!(legacy_team.team.as_deref(), Some("blue"));
        assert_eq!(legacy_team.hp, DEFAULT_BED2_HP);
        assert_eq!(bare.name, "gamma");
        assert_eq!(bare.team, None);
        assert_eq!(bare.hp, 2500);
        assert_eq!(CustomBed2Import::parse("alpha@red+bed2[0]"), None);

        let facade_bridge =
            CustomBed2Import::parse_player_facade_raw("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}")
                .expect("bed2 raw should bridge through player facade id name");
        assert_eq!(
            crate::player::Player::raw_namerena_to_idname("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}"),
            "alpha@red"
        );
        assert_eq!(facade_bridge.name, "alpha");
        assert_eq!(facade_bridge.team.as_deref(), Some("red"));
        assert_eq!(facade_bridge.hp, 4500);

        let same_team_bridge = CustomBed2Import::parse_player_facade_raw("same@same+bed2[1800]")
            .expect("same-team bed2 raw should bridge through normalized player facade id name");
        assert_eq!(crate::player::Player::raw_namerena_to_idname("same@same+bed2[1800]"), "same");
        assert_eq!(same_team_bridge.name, "same");
        assert_eq!(same_team_bridge.team, None);
        assert_eq!(same_team_bridge.hp, 1800);

        let template = plus.into_player_template(1, bed2, 0, summon);
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(vec![template], registry));
        let entity = runtime.entities.get(EntityIdx(0)).expect("bed2 entity should import");

        assert_eq!(entity.template.name, "alpha");
        assert_eq!(entity.template.max_hp, 4500);
        assert_eq!(entity.template.attack, 0);
        assert_eq!(entity.template.defense, DEFAULT_BED2_DEFENSE);
        assert_eq!(entity.template.resistance, DEFAULT_BED2_RESISTANCE);
        assert_eq!(entity.template.skills.skills(), &[summon]);
        assert!(entity.runtime.flags.contains(PlayerKindFlags::BED2));
    }

    #[test]
    fn custom_bed2_roster_import_builds_prepared_template_from_grouped_raw_players() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec![
                "alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}".to_owned(),
                "seed:custom-seed@!".to_owned(),
            ],
            vec!["beta@blue@bed2".to_owned(), "same@same+bed2[1800]".to_owned()],
        ];

        let template = CustomBed2Import::roster_into_prepared_template(&raw_groups, registry, bed2, summon)
            .expect("grouped bed2 raw roster should build a prepared template");

        assert_eq!(template.players.len(), 3);
        assert_eq!(template.players[0].id, 1);
        assert_eq!(template.players[0].name, "alpha");
        assert_eq!(template.players[0].team, 0);
        assert_eq!(template.players[0].max_hp, 4500);
        assert_eq!(template.players[0].skills.skills(), &[summon]);
        assert_eq!(template.players[1].id, 2);
        assert_eq!(template.players[1].name, "beta");
        assert_eq!(template.players[1].team, 1);
        assert_eq!(template.players[1].max_hp, DEFAULT_BED2_HP);
        assert_eq!(template.players[2].id, 3);
        assert_eq!(template.players[2].name, "same");
        assert_eq!(template.players[2].team, 1);
        assert_eq!(template.players[2].max_hp, 1800);
        assert!(template.players.iter().all(|player| player.kind == bed2
            && player.attack == 0
            && player.defense == DEFAULT_BED2_DEFENSE
            && player.resistance == DEFAULT_BED2_RESISTANCE));

        let runtime = CombatRuntime::from_template(template);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1), EntityIdx(2)].as_slice()));
        assert!(
            runtime
                .entities
                .iter()
                .all(|(_, entity)| entity.runtime.flags.contains(PlayerKindFlags::BED2))
        );
    }

    #[test]
    fn custom_bed2_roster_import_exports_ol_summon_overlay_to_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec![
                "alpha@red+bed2[4500]".to_owned(),
                "seed:custom-seed@!".to_owned(),
            ],
            vec![r#"beta@blue@bed2+ol:{"summon":{"attrs":[36,86,56,55,36,89,88,89],"skills":{"sklfire2":4,"sklexplode":3,"sklfire1":"2*14"},"reuse_skills_on_recast":true,"inherit_owner_def_res":true}}"#.to_owned()],
        ];

        let template = CustomBed2Import::roster_into_prepared_template_with_summon_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: "custom.summon.fire",
                explode_skill_export_name: "custom.summon.explode",
            },
        )
        .expect("bed2 roster with summon overlay should build prepared template");

        assert_eq!(template.players.len(), 2);
        assert_eq!(template.players[0].name, "alpha");
        assert_eq!(template.players[0].skills.skills(), &[summon]);
        let SlotValue::PlayerTemplate(summon_template) = template
            .slots
            .get(summon_template_slot)
            .expect("summon overlay should populate template slot")
        else {
            panic!("summon overlay slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.name, "beta?0");
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.team, 1);
        assert_eq!(summon_template.max_hp, 89);
        assert_eq!(summon_template.attack, 0);
        assert_eq!(summon_template.defense, 50);
        assert_eq!(summon_template.resistance, 53);
        assert_eq!(summon_template.agility, 19);
        assert_eq!(summon_template.magic, 0);
        assert_eq!(summon_template.wisdom, 52);
        assert_eq!(summon_template.magic_point, 26);
        assert_eq!(summon_template.move_state.speed_points, 180);
        assert_eq!(summon_template.policy_overrides.inherit_owner_def_res, Some(true));
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[1, 2, 0]);
    }

    #[test]
    fn custom_bed2_summon_overlay_import_rejects_missing_skill_export_name() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let bed2 = builder
            .register_player_kind("custom", "bed2", "custom.bed2")
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind("custom", "bed2-summon", "custom.bed2.summon")
            .expect("bed2 summon kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec![
            r#"beta@blue@bed2+ol:{"summon":{"attrs":[36,86,56,55,36,89,88,89],"skills":{"sklfire1":5}}}"#.to_owned(),
        ]];

        let err = CustomBed2Import::roster_into_prepared_template_with_summon_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: "custom.summon.fire",
                explode_skill_export_name: "custom.summon.explode",
            },
        )
        .expect_err("missing explode skill export should reject parser-facing import");

        assert_eq!(fire, SkillId(1));
        assert_eq!(
            err,
            CustomBed2SummonTemplateImportError::MissingSkillExportName {
                export_name: "custom.summon.explode".to_owned(),
            }
        );
    }

    #[test]
    fn custom_bed2_roster_import_exports_ol_shadow_overlay_to_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let possess = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("possess skill should register");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let shadow_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-shadow",
                "custom.bed2.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 shadow kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["alpha@red+bed2[4500]".to_owned()],
            vec![r#"beta@blue@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"sklpossess":5}}}"#.to_owned()],
        ];

        let template = CustomBed2Import::roster_into_prepared_template_with_shadow_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: "custom.minion.possess",
            },
        )
        .expect("bed2 roster with shadow overlay should build prepared template");

        assert_eq!(template.players.len(), 2);
        assert_eq!(template.players[0].skills.skills(), &[summon]);
        let SlotValue::PlayerTemplate(shadow_template) = template
            .slots
            .get(shadow_template_slot)
            .expect("shadow overlay should populate template slot")
        else {
            panic!("shadow overlay slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.name, "beta?shadow");
        assert_eq!(shadow_template.kind, shadow_kind);
        assert_eq!(shadow_template.team, 1);
        assert_eq!(shadow_template.max_hp, 88);
        assert_eq!(shadow_template.attack, 11);
        assert_eq!(shadow_template.defense, 12);
        assert_eq!(shadow_template.resistance, 16);
        assert_eq!(shadow_template.agility, 14);
        assert_eq!(shadow_template.magic, 15);
        assert_eq!(shadow_template.wisdom, 17);
        assert_eq!(shadow_template.magic_point, 8);
        assert_eq!(shadow_template.move_state.speed_points, -2048);
        assert_eq!(shadow_template.skills.skills(), &[possess]);
        assert_eq!(shadow_template.skills.active_order(), &[0]);
    }

    #[test]
    fn custom_bed2_shadow_overlay_import_rejects_missing_skill_export_name() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let bed2 = builder
            .register_player_kind("custom", "bed2", "custom.bed2")
            .expect("bed2 kind should register");
        let shadow_kind = builder
            .register_player_kind("custom", "bed2-shadow", "custom.bed2.shadow")
            .expect("bed2 shadow kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec![
            r#"beta@blue@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"sklpossess":5}}}"#.to_owned(),
        ]];

        let err = CustomBed2Import::roster_into_prepared_template_with_shadow_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: "custom.minion.possess",
            },
        )
        .expect_err("missing possess skill export should reject parser-facing import");

        assert_eq!(
            err,
            CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                export_name: "custom.minion.possess".to_owned(),
            }
        );
    }

    #[test]
    fn custom_bed2_roster_import_exports_ol_zombie_overlay_to_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let zombie_heal = builder
            .register_skill(
                "custom",
                "zombie-heal",
                "custom.minion.heal",
                TargetPolicy::Ally,
                SkillPriority(1),
            )
            .expect("zombie heal skill should register");
        let zombie_explode = builder
            .register_skill(
                "custom",
                "zombie-explode",
                "custom.minion.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("zombie explode skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let zombie_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-zombie",
                "custom.bed2.zombie",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["alpha@red+bed2[4500]".to_owned()],
            vec![
                r#"beta@blue@bed2+ol:{"zombie":{"attrs":[46,47,48,49,50,51,52,77],"skills":{"sklheal":3,"sklexplode":4}}}"#
                    .to_owned(),
            ],
        ];

        let template = CustomBed2Import::roster_into_prepared_template_with_zombie_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: "custom.minion",
            },
        )
        .expect("bed2 roster with zombie overlay should build prepared template");

        assert_eq!(template.players.len(), 2);
        assert_eq!(template.players[0].skills.skills(), &[summon]);
        let SlotValue::PlayerTemplate(zombie_template) = template
            .slots
            .get(zombie_template_slot)
            .expect("zombie overlay should populate template slot")
        else {
            panic!("zombie overlay slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.name, "beta?zombie");
        assert_eq!(zombie_template.kind, zombie_kind);
        assert_eq!(zombie_template.team, 1);
        assert_eq!(zombie_template.max_hp, 77);
        assert_eq!(zombie_template.attack, 10);
        assert_eq!(zombie_template.defense, 11);
        assert_eq!(zombie_template.resistance, 15);
        assert_eq!(zombie_template.agility, 13);
        assert_eq!(zombie_template.magic, 14);
        assert_eq!(zombie_template.wisdom, 16);
        assert_eq!(zombie_template.magic_point, 8);
        assert_eq!(zombie_template.move_state.speed_points, 0);
        assert_eq!(zombie_template.skills.skills(), &[zombie_heal, zombie_explode]);
        assert_eq!(zombie_template.skills.active_order(), &[0, 1]);
    }

    #[test]
    fn custom_bed2_roster_import_exports_all_ol_minion_overlays_to_template_slots() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let possess = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(3),
            )
            .expect("possess skill should register");
        let zombie_heal = builder
            .register_skill(
                "custom",
                "zombie-heal",
                "custom.minion.heal",
                TargetPolicy::Ally,
                SkillPriority(4),
            )
            .expect("zombie heal skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let shadow_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-shadow",
                "custom.bed2.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 shadow kind should register");
        let zombie_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-zombie",
                "custom.bed2.zombie",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec![
                r#"alpha@red@bed2+ol:{"summon":{"attrs":[46,47,48,49,50,51,52,123],"skills":{"sklfire2":4,"sklfire1":5},"inherit_owner_def_res":true}}"#.to_owned(),
                r#"beta@red@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"phantom:sklpossess":5}}}"#.to_owned(),
                r#"gamma@red@bed2+ol:{"zombie":{"attrs":[46,47,48,49,50,51,52,77],"skills":{"sklheal":3}}}"#.to_owned(),
            ],
            vec!["delta@blue+bed2[8]".to_owned()],
        ];

        let template = CustomBed2Import::roster_into_prepared_template_with_minion_overlays(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2MinionOverlayConfig {
                summon: CustomBed2SummonTemplateConfig {
                    template_slot: summon_template_slot,
                    summon_kind,
                    fire_skill_export_name: "custom.summon.fire",
                    explode_skill_export_name: "custom.summon.explode",
                },
                shadow: CustomBed2ShadowTemplateConfig {
                    template_slot: shadow_template_slot,
                    shadow_kind,
                    possess_skill_export_name: "custom.minion.possess",
                },
                zombie: CustomBed2ZombieTemplateConfig {
                    template_slot: zombie_template_slot,
                    zombie_kind,
                    skill_export_name_prefix: "custom.minion",
                },
            },
        )
        .expect("combined minion overlay import should build prepared template");

        assert_eq!(template.players.len(), 4);
        let SlotValue::PlayerTemplate(summon_template) = template
            .slots
            .get(summon_template_slot)
            .expect("combined import should populate summon template slot")
        else {
            panic!("summon overlay slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.name, "alpha?0");
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.max_hp, 123);
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[1, 0]);

        let SlotValue::PlayerTemplate(shadow_template) = template
            .slots
            .get(shadow_template_slot)
            .expect("combined import should populate shadow template slot")
        else {
            panic!("shadow overlay slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.name, "beta?shadow");
        assert_eq!(shadow_template.kind, shadow_kind);
        assert_eq!(shadow_template.max_hp, 88);
        assert_eq!(shadow_template.move_state.speed_points, -2048);
        assert_eq!(shadow_template.skills.skills(), &[possess]);
        assert_eq!(shadow_template.skills.active_order(), &[0]);

        let SlotValue::PlayerTemplate(zombie_template) = template
            .slots
            .get(zombie_template_slot)
            .expect("combined import should populate zombie template slot")
        else {
            panic!("zombie overlay slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.name, "gamma?zombie");
        assert_eq!(zombie_template.kind, zombie_kind);
        assert_eq!(zombie_template.max_hp, 77);
        assert_eq!(zombie_template.move_state.speed_points, 0);
        assert_eq!(zombie_template.skills.skills(), &[zombie_heal]);
        assert_eq!(zombie_template.skills.active_order(), &[0]);
    }

    #[test]
    fn custom_bed2_zombie_overlay_import_rejects_missing_skill_export_name() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let bed2 = builder
            .register_player_kind("custom", "bed2", "custom.bed2")
            .expect("bed2 kind should register");
        let zombie_kind = builder
            .register_player_kind("custom", "bed2-zombie", "custom.bed2.zombie")
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec![
            r#"beta@blue@bed2+ol:{"zombie":{"attrs":[46,47,48,49,50,51,52,77],"skills":{"sklheal":3}}}"#.to_owned(),
        ]];

        let err = CustomBed2Import::roster_into_prepared_template_with_zombie_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: "custom.minion",
            },
        )
        .expect_err("missing zombie skill export should reject parser-facing import");

        assert_eq!(
            err,
            CustomBed2ZombieTemplateImportError::MissingSkillExportName {
                export_name: "custom.minion.heal".to_owned(),
            }
        );
    }

    #[test]
    fn custom_bed2_roster_import_rejects_non_bed2_players() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec!["alpha+bed2[4500]".to_owned()], vec!["plain".to_owned()]];

        let err = CustomBed2Import::roster_into_prepared_template(&raw_groups, registry, bed2, summon)
            .expect_err("non-bed2 raw players should be rejected by the bed2 roster importer");

        assert_eq!(
            err,
            CustomBed2RosterImportError {
                team_index: 1,
                player_index: 0,
                raw: "plain".to_owned(),
            }
        );
    }

    #[test]
    fn custom_mixed_roster_import_bridges_bed2_and_legacy_player_templates() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["plain@red".to_owned(), "alpha@red+bed2[4500]".to_owned()],
            vec!["seed:custom-seed@!".to_owned(), "beta@blue@bed2".to_owned()],
        ];

        let template = CustomBed2Import::mixed_roster_into_prepared_template(&raw_groups, registry, bed2, summon)
            .expect("mixed legacy/bed2 raw roster should build a prepared template");
        let legacy_storage = crate::engine::storage::Storage::new_arc();
        let mut legacy_plain = crate::player::Player::new_from_namerena_raw("plain@red".to_owned(), legacy_storage)
            .expect("legacy player facade should parse plain player");
        legacy_plain.build();
        let legacy_status = legacy_plain.get_status();

        assert_eq!(template.players.len(), 3);
        assert_eq!(template.players[0].id, 1);
        assert_eq!(template.players[0].name, legacy_plain.id_name());
        assert_eq!(template.players[0].kind, PlayerTemplate::DEFAULT_KIND);
        assert_eq!(template.players[0].team, 0);
        assert_eq!(template.players[0].max_hp, legacy_status.max_hp);
        assert_eq!(template.players[0].attack, legacy_status.attack);
        assert_eq!(template.players[0].defense, legacy_status.defense);
        assert_eq!(template.players[0].resistance, legacy_status.resistance);
        assert_eq!(template.players[1].id, 2);
        assert_eq!(template.players[1].kind, bed2);
        assert_eq!(template.players[1].name, "alpha");
        assert_eq!(template.players[1].team, 0);
        assert_eq!(template.players[1].max_hp, 4500);
        assert_eq!(template.players[1].skills.skills(), &[summon]);
        assert_eq!(template.players[2].id, 3);
        assert_eq!(template.players[2].kind, bed2);
        assert_eq!(template.players[2].name, "beta");
        assert_eq!(template.players[2].team, 1);
        assert_eq!(template.players[2].max_hp, DEFAULT_BED2_HP);

        let runtime = CombatRuntime::from_template(template);
        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(2)].as_slice()));
    }

    #[test]
    fn custom_mixed_roster_import_exports_bed2_ol_summon_overlay_to_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec![
                "plain@red".to_owned(),
                "alpha@red+bed2[4500]".to_owned(),
            ],
            vec![
                "seed:custom-seed@!".to_owned(),
                r#"beta@blue@bed2+ol:{"summon":{"attrs":[46,47,48,49,50,51,52,123],"skills":{"sklexplode":3,"sklfire1":5},"inherit_owner_def_res":true}}"#.to_owned(),
            ],
        ];

        let template = CustomBed2Import::mixed_roster_into_prepared_template_with_summon_overlay(
            &raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: "custom.summon.fire",
                explode_skill_export_name: "custom.summon.explode",
            },
        )
        .expect("mixed roster should carry bed2 summon overlay into template slot");

        assert_eq!(template.players.len(), 3);
        assert_eq!(template.players[0].kind, PlayerTemplate::DEFAULT_KIND);
        assert_eq!(template.players[1].kind, bed2);
        assert_eq!(template.players[2].kind, bed2);
        let SlotValue::PlayerTemplate(summon_template) = template
            .slots
            .get(summon_template_slot)
            .expect("mixed roster summon overlay should populate template slot")
        else {
            panic!("mixed roster summon overlay slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.name, "beta?0");
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.team, 1);
        assert_eq!(summon_template.max_hp, 123);
        assert_eq!(summon_template.attack, 10);
        assert_eq!(summon_template.defense, 11);
        assert_eq!(summon_template.resistance, 15);
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[2, 0]);
        assert_eq!(summon_template.policy_overrides.inherit_owner_def_res, Some(true));
    }

    #[test]
    fn runtime_v2_runner_constructs_and_runs_mixed_roster() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["plain@red".to_owned(), "alpha@red+bed2[9]".to_owned()],
            vec!["seed:custom-seed@!".to_owned(), "beta@blue@bed2".to_owned()],
        ];

        let mut runner = RuntimeV2Runner::from_mixed_roster(&raw_groups, registry, bed2, summon)
            .expect("mixed roster should construct a runtime v2 runner");

        assert_eq!(
            runner.runtime().world.team_alive(0),
            Some([EntityIdx(0), EntityIdx(1)].as_slice())
        );
        assert_eq!(runner.runtime().world.team_alive(1), Some([EntityIdx(2)].as_slice()));
        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.max_hp, 9);
        assert!(
            runner
                .runtime()
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .runtime
                .flags
                .contains(PlayerKindFlags::BED2)
        );

        let actor_attack = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.attack;
        let plain_hp = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.max_hp;
        let plain_mp = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.magic_point;
        let plain_defense = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.defense;
        let plain_resistance = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.resistance;

        let actual = runner.run_round_normalized();
        let expected = NormalizedOutcome {
            winner_team: None,
            round: 1,
            total_score: actor_attack as u64,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
            entity_ids: vec![1, 2, 3],
            teams: vec![0, 0, 1],
            hp: vec![plain_hp, 9, DEFAULT_BED2_HP - actor_attack],
            magic_point: vec![plain_mp, 0, 0],
            defense: vec![plain_defense, 99, 99],
            resistance: vec![plain_resistance, 99, 99],
            alive: vec![true, true, true],
            round_order: vec![0, 1, 2],
            flat_alive: vec![0, 1, 2],
            team_alive: vec![vec![0, 1], vec![2]],
            alive_group_count: 2,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 0,
                target: 2,
                amount: actor_attack,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 2,
                targets: Vec::new(),
                param: None,
                score: actor_attack as u32,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        };

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_v2_runner_constructs_from_bed2_namerena_raw_fixture_shape() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_input = "alpha@red+bed2[5]\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

        let runner = RuntimeV2Runner::from_bed2_namerena_raw(raw_input.to_owned(), registry, bed2, summon)
            .expect("bed2 namerena raw should construct runtime v2 runner");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_eq!(runner.runtime().entities.len(), 2);
        assert_eq!(runner.runtime().entities.get(EntityIdx(0)).unwrap().template.max_hp, 5);
        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.max_hp, 8);
        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        assert_eq!(runner.runtime().rng.i, legacy.randomer.i);
        assert_eq!(runner.runtime().rng.j, legacy.randomer.j);
        assert_eq!(runner.runtime().rng.main_val, legacy.randomer.main_val);
    }

    #[test]
    fn runtime_v2_runner_bed2_raw_can_import_ol_summon_overlay_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let registry = builder.build();
        let raw_input = "alpha@red+bed2[5]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5}}}\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

        let runner = RuntimeV2Runner::from_bed2_namerena_raw_with_summon_overlay(
            raw_input.to_owned(),
            registry,
            bed2,
            summon,
            CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: "custom.summon.fire",
                explode_skill_export_name: "custom.summon.explode",
            },
        )
        .expect("bed2 raw runner should import summon overlay template slot");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        let SlotValue::PlayerTemplate(summon_template) = runner
            .runtime()
            .template_slots
            .get(summon_template_slot)
            .expect("runner should preserve imported summon template slot")
        else {
            panic!("runner summon template slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.name, "alpha?0");
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.max_hp, 123);
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[1, 0]);
    }

    #[test]
    fn runtime_v2_runner_bed2_raw_can_import_ol_shadow_overlay_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let possess = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("possess skill should register");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let shadow_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-shadow",
                "custom.bed2.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 shadow kind should register");
        let registry = builder.build();
        let raw_input = "alpha@red+bed2[5]+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

        let runner = RuntimeV2Runner::from_bed2_namerena_raw_with_shadow_overlay(
            raw_input.to_owned(),
            registry,
            bed2,
            summon,
            CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: "custom.minion.possess",
            },
        )
        .expect("bed2 raw runner should import shadow overlay template slot");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        let SlotValue::PlayerTemplate(shadow_template) = runner
            .runtime()
            .template_slots
            .get(shadow_template_slot)
            .expect("runner should preserve imported shadow template slot")
        else {
            panic!("runner shadow template slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.name, "alpha?shadow");
        assert_eq!(shadow_template.kind, shadow_kind);
        assert_eq!(shadow_template.max_hp, 88);
        assert_eq!(shadow_template.move_state.speed_points, -2048);
        assert_eq!(shadow_template.skills.skills(), &[possess]);
        assert_eq!(shadow_template.skills.active_order(), &[0]);
    }

    #[test]
    fn runtime_v2_runner_bed2_raw_can_import_ol_zombie_overlay_template_slot() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let zombie_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-zombie",
                "custom.bed2.zombie",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let raw_input = "alpha@red+bed2[5]+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77]}}\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

        let runner = RuntimeV2Runner::from_bed2_namerena_raw_with_zombie_overlay(
            raw_input.to_owned(),
            registry,
            bed2,
            summon,
            CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: "custom.minion",
            },
        )
        .expect("bed2 raw runner should import zombie overlay template slot");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        let SlotValue::PlayerTemplate(zombie_template) = runner
            .runtime()
            .template_slots
            .get(zombie_template_slot)
            .expect("runner should preserve imported zombie template slot")
        else {
            panic!("runner zombie template slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.name, "alpha?zombie");
        assert_eq!(zombie_template.kind, zombie_kind);
        assert_eq!(zombie_template.max_hp, 77);
        assert_eq!(zombie_template.move_state.speed_points, 0);
        assert!(zombie_template.skills.is_empty());
    }

    #[test]
    fn runtime_v2_runner_bed2_raw_can_import_all_ol_minion_overlay_template_slots() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let possess = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(3),
            )
            .expect("possess skill should register");
        let zombie_heal = builder
            .register_skill(
                "custom",
                "zombie-heal",
                "custom.minion.heal",
                TargetPolicy::Ally,
                SkillPriority(4),
            )
            .expect("zombie heal skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let shadow_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-shadow",
                "custom.bed2.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 shadow kind should register");
        let zombie_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-zombie",
                "custom.bed2.zombie",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let raw_input = "alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{\"sklheal\":3}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";

        let runner = RuntimeV2Runner::from_bed2_namerena_raw_with_minion_overlays(
            raw_input.to_owned(),
            registry,
            bed2,
            summon,
            CustomBed2MinionOverlayConfig {
                summon: CustomBed2SummonTemplateConfig {
                    template_slot: summon_template_slot,
                    summon_kind,
                    fire_skill_export_name: "custom.summon.fire",
                    explode_skill_export_name: "custom.summon.explode",
                },
                shadow: CustomBed2ShadowTemplateConfig {
                    template_slot: shadow_template_slot,
                    shadow_kind,
                    possess_skill_export_name: "custom.minion.possess",
                },
                zombie: CustomBed2ZombieTemplateConfig {
                    template_slot: zombie_template_slot,
                    zombie_kind,
                    skill_export_name_prefix: "custom.minion",
                },
            },
        )
        .expect("bed2 raw runner should import all minion overlay template slots");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        let SlotValue::PlayerTemplate(summon_template) = runner
            .runtime()
            .template_slots
            .get(summon_template_slot)
            .expect("runner should preserve imported summon template slot")
        else {
            panic!("runner summon template slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.name, "alpha?0");
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.max_hp, 123);
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[1, 0]);

        let SlotValue::PlayerTemplate(shadow_template) = runner
            .runtime()
            .template_slots
            .get(shadow_template_slot)
            .expect("runner should preserve imported shadow template slot")
        else {
            panic!("runner shadow template slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.name, "beta?shadow");
        assert_eq!(shadow_template.kind, shadow_kind);
        assert_eq!(shadow_template.max_hp, 88);
        assert_eq!(shadow_template.skills.skills(), &[possess]);
        assert_eq!(shadow_template.skills.active_order(), &[0]);

        let SlotValue::PlayerTemplate(zombie_template) = runner
            .runtime()
            .template_slots
            .get(zombie_template_slot)
            .expect("runner should preserve imported zombie template slot")
        else {
            panic!("runner zombie template slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.name, "gamma?zombie");
        assert_eq!(zombie_template.kind, zombie_kind);
        assert_eq!(zombie_template.max_hp, 77);
        assert_eq!(zombie_template.skills.skills(), &[zombie_heal]);
        assert_eq!(zombie_template.skills.active_order(), &[0]);
    }

    #[test]
    fn runtime_v2_custom_import_profile_builds_mixed_raw_with_minion_overlays() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let fire = builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        let explode = builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let possess = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(3),
            )
            .expect("possess skill should register");
        let zombie_heal = builder
            .register_skill(
                "custom",
                "zombie-heal",
                "custom.minion.heal",
                TargetPolicy::Ally,
                SkillPriority(4),
            )
            .expect("zombie heal skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-summon",
                "custom.bed2.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 summon kind should register");
        let shadow_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-shadow",
                "custom.bed2.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 shadow kind should register");
        let zombie_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-zombie",
                "custom.bed2.zombie",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let config =
            CustomRuntimeV2ImportConfig::new(registry, bed2, summon).with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
                summon: CustomBed2SummonTemplateConfig {
                    template_slot: summon_template_slot,
                    summon_kind,
                    fire_skill_export_name: "custom.summon.fire",
                    explode_skill_export_name: "custom.summon.explode",
                },
                shadow: CustomBed2ShadowTemplateConfig {
                    template_slot: shadow_template_slot,
                    shadow_kind,
                    possess_skill_export_name: "custom.minion.possess",
                },
                zombie: CustomBed2ZombieTemplateConfig {
                    template_slot: zombie_template_slot,
                    zombie_kind,
                    skill_export_name_prefix: "custom.minion",
                },
            });
        let raw_input = "plain@red\n\
alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{\"sklheal\":3}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";

        let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw_input.to_owned(), config)
            .expect("custom import profile should construct mixed runner");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        assert_eq!(
            runner.runtime().entities.get(EntityIdx(0)).unwrap().template.kind,
            PlayerTemplate::DEFAULT_KIND
        );
        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
        let SlotValue::PlayerTemplate(summon_template) = runner
            .runtime()
            .template_slots
            .get(summon_template_slot)
            .expect("profile import should populate summon template slot")
        else {
            panic!("profile summon overlay slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.kind, summon_kind);
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[1, 0]);

        let SlotValue::PlayerTemplate(shadow_template) = runner
            .runtime()
            .template_slots
            .get(shadow_template_slot)
            .expect("profile import should populate shadow template slot")
        else {
            panic!("profile shadow overlay slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.kind, shadow_kind);
        assert_eq!(shadow_template.skills.skills(), &[possess]);

        let SlotValue::PlayerTemplate(zombie_template) = runner
            .runtime()
            .template_slots
            .get(zombie_template_slot)
            .expect("profile import should populate zombie template slot")
        else {
            panic!("profile zombie overlay slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.kind, zombie_kind);
        assert_eq!(zombie_template.skills.skills(), &[zombie_heal]);
    }

    #[test]
    fn runtime_v2_custom_import_profile_wraps_missing_overlay_skill_errors() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        builder
            .register_skill(
                "custom",
                "summon-fire",
                "custom.summon.fire",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("summon fire skill should register");
        builder
            .register_skill(
                "custom",
                "summon-explode",
                "custom.summon.explode",
                TargetPolicy::Enemy,
                SkillPriority(2),
            )
            .expect("summon explode skill should register");
        let summon_template_slot = builder
            .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
            .expect("bed2 summon template slot should reserve");
        let shadow_template_slot = builder
            .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
            .expect("bed2 shadow template slot should reserve");
        let zombie_template_slot = builder
            .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
            .expect("bed2 zombie template slot should reserve");
        let bed2 = builder
            .register_player_kind("custom", "bed2", "custom.bed2")
            .expect("bed2 kind should register");
        let summon_kind = builder
            .register_player_kind("custom", "bed2-summon", "custom.bed2.summon")
            .expect("bed2 summon kind should register");
        let shadow_kind = builder
            .register_player_kind("custom", "bed2-shadow", "custom.bed2.shadow")
            .expect("bed2 shadow kind should register");
        let zombie_kind = builder
            .register_player_kind("custom", "bed2-zombie", "custom.bed2.zombie")
            .expect("bed2 zombie kind should register");
        let registry = builder.build();
        let config =
            CustomRuntimeV2ImportConfig::new(registry, bed2, summon).with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
                summon: CustomBed2SummonTemplateConfig {
                    template_slot: summon_template_slot,
                    summon_kind,
                    fire_skill_export_name: "custom.summon.fire",
                    explode_skill_export_name: "custom.summon.explode",
                },
                shadow: CustomBed2ShadowTemplateConfig {
                    template_slot: shadow_template_slot,
                    shadow_kind,
                    possess_skill_export_name: "custom.minion.possess",
                },
                zombie: CustomBed2ZombieTemplateConfig {
                    template_slot: zombie_template_slot,
                    zombie_kind,
                    skill_export_name_prefix: "custom.minion",
                },
            });

        let err = RuntimeV2Runner::from_custom_bed2_namerena_raw(
            r#"alpha@red@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"sklpossess":5}}}"#.to_owned(),
            config,
        )
        .expect_err("custom profile should wrap missing shadow possess export");

        assert_eq!(
            err,
            CustomRuntimeV2ImportError::Bed2MinionOverlay(CustomBed2MinionOverlayImportError::Shadow(
                CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                    export_name: "custom.minion.possess".to_owned(),
                }
            ))
        );
    }

    #[test]
    fn default_custom_runtime_v2_profile_builds_mixed_raw_runner() {
        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let bed2 = config.bed2_kind;
        let summon = config.bed2_summon_skill;
        assert_eq!(
            config
                .registry
                .skill_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT)
                .map(|spec| spec.id),
            Some(summon)
        );
        assert_eq!(config.registry.player_kind(bed2).unwrap().export_name, "custom.bed2");

        let raw_input = "plain@red\nalpha@red@bed2\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";
        let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw_input.to_owned(), config)
            .expect("default custom profile should construct mixed runner");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
        assert_eq!(
            runner.runtime().entities.get(EntityIdx(1)).unwrap().template.skills.skills(),
            &[summon]
        );
    }

    #[test]
    fn runtime_v2_runner_runs_mixed_namerena_raw_fixture_shape() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_input = "plain@red\nalpha@red+bed2[9]\n\nseed:custom-seed@!\n\nbeta@blue+bed2[3]\n";

        let mut runner = RuntimeV2Runner::from_mixed_namerena_raw(raw_input.to_owned(), registry, bed2, summon)
            .expect("mixed namerena raw should construct runtime v2 runner");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");
        runner.runtime_mut().set_skill_handler(summon, skill_noop);
        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        let initial_rng = crate::runtime_v2::oracle::NormalizedRngCheckpoint::from_runtime(runner.runtime());
        let plain = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.clone();

        let (summary, actual) = runner.run_until_winner_normalized(8);

        assert_eq!(initial_rng.i, legacy.randomer.i);
        assert_eq!(initial_rng.j, legacy.randomer.j);
        assert_eq!(summary.rounds.len(), 2);
        assert_eq!(summary.winner_team, Some(1));
        assert!(!summary.guard_exhausted);
        let expected = NormalizedOutcome {
            winner_team: Some(1),
            round: 2,
            total_score: plain.attack as u64,
            rng: actual.rng.clone(),
            entity_ids: vec![1, 2, 3],
            teams: vec![1, 1, 0],
            hp: vec![plain.max_hp, 9, 0],
            magic_point: vec![plain.magic_point, 0, 0],
            defense: vec![plain.defense, DEFAULT_BED2_DEFENSE, DEFAULT_BED2_DEFENSE],
            resistance: vec![plain.resistance, DEFAULT_BED2_RESISTANCE, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, true, false],
            round_order: vec![2, 0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![Vec::new(), vec![0, 1]],
            alive_group_count: 1,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 2,
                actor: 0,
                target: 2,
                amount: plain.attack,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 2,
                targets: Vec::new(),
                param: None,
                score: plain.attack as u32,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        };

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn runtime_v2_runner_aligns_large_raw_initial_state_with_legacy_world() {
        let raw_input =
            "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

        let (runner, legacy) = mixed_raw_runner_for_plain_fixture(raw_input);

        assert_eq!(runner.runtime().entities.len(), 2);
        assert_eq!(legacy.world.all_plr_len(), 2);
        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        assert_runtime_rng_matches_legacy(runner.runtime(), &legacy);
    }

    #[test]
    fn runtime_v2_runner_large_prefix_normalized_run_matches_golden() {
        let raw_input =
            "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

        let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let run = runner.run_until_winner_normalized_rounds(3);

        assert_eq!(run.winner_team, None);
        assert!(run.guard_exhausted);
        assert_eq!(run.total_score, 151);
        assert_eq!(run.rounds.len(), 3);
        let expected_rounds = vec![
            NormalizedOutcome {
                winner_team: None,
                round: 1,
                total_score: 57,
                rng: normalized_rng_checkpoint(226, 30),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![350, 265],
                magic_point: vec![28, 29],
                defense: vec![58, 52],
                resistance: vec![49, 57],
                alive: vec![true, true],
                round_order: vec![0, 1],
                flat_alive: vec![0, 1],
                team_alive: vec![vec![0], vec![1]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 1,
                    actor: 0,
                    target: 1,
                    amount: 57,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 57,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
            NormalizedOutcome {
                winner_team: None,
                round: 2,
                total_score: 37,
                rng: normalized_rng_checkpoint(227, 87),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![313, 265],
                magic_point: vec![28, 29],
                defense: vec![58, 52],
                resistance: vec![49, 57],
                alive: vec![true, true],
                round_order: vec![0, 1],
                flat_alive: vec![0, 1],
                team_alive: vec![vec![0], vec![1]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 2,
                    actor: 1,
                    target: 0,
                    amount: 37,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 1,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 37,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
            NormalizedOutcome {
                winner_team: None,
                round: 3,
                total_score: 57,
                rng: normalized_rng_checkpoint(228, 178),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![313, 208],
                magic_point: vec![28, 29],
                defense: vec![58, 52],
                resistance: vec![49, 57],
                alive: vec![true, true],
                round_order: vec![0, 1],
                flat_alive: vec![0, 1],
                team_alive: vec![vec![0], vec![1]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 3,
                    actor: 0,
                    target: 1,
                    amount: 57,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 57,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
        ];

        for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
            assert_eq!(strict_diff(expected, actual), Ok(()));
        }
    }

    #[test]
    fn runtime_v2_runner_aligns_fight_multi_raw_initial_state_with_legacy_world() {
        let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

        let (runner, legacy) = mixed_raw_runner_for_plain_fixture(raw_input);

        assert_eq!(runner.runtime().entities.len(), 10);
        assert_eq!(legacy.world.all_plr_len(), 10);
        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        assert_runtime_rng_matches_legacy(runner.runtime(), &legacy);
    }

    #[test]
    fn runtime_v2_runner_fight_multi_prefix_normalized_run_matches_golden() {
        let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

        let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let run = runner.run_until_winner_normalized_rounds(3);

        assert_eq!(run.winner_team, None);
        assert!(run.guard_exhausted);
        assert_eq!(run.total_score, 129);
        assert_eq!(run.rounds.len(), 3);
        let expected_rounds = vec![
            NormalizedOutcome {
                winner_team: None,
                round: 1,
                total_score: 50,
                rng: normalized_rng_checkpoint(215, 67),
                entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
                hp: vec![342, 331, 387, 267, 359, 331, 344, 327, 332, 335],
                magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
                defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
                resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
                alive: vec![true, true, true, true, true, true, true, true, true, true],
                round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
                flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
                team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 1,
                    actor: 6,
                    target: 3,
                    amount: 50,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 6,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 50,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
            NormalizedOutcome {
                winner_team: None,
                round: 2,
                total_score: 20,
                rng: normalized_rng_checkpoint(216, 115),
                entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
                hp: vec![342, 331, 387, 267, 359, 331, 324, 327, 332, 335],
                magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
                defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
                resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
                alive: vec![true, true, true, true, true, true, true, true, true, true],
                round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
                flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
                team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 2,
                    actor: 3,
                    target: 6,
                    amount: 20,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 3,
                    target: 6,
                    targets: Vec::new(),
                    param: None,
                    score: 20,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
            NormalizedOutcome {
                winner_team: None,
                round: 3,
                total_score: 59,
                rng: normalized_rng_checkpoint(217, 59),
                entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
                hp: vec![342, 331, 387, 208, 359, 331, 324, 327, 332, 335],
                magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
                defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
                resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
                alive: vec![true, true, true, true, true, true, true, true, true, true],
                round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
                flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
                team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 3,
                    actor: 9,
                    target: 3,
                    amount: 59,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 9,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 59,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
        ];

        for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
            assert_eq!(strict_diff(expected, actual), Ok(()));
        }
    }

    #[test]
    fn runtime_v2_runner_rejects_plain_rows_in_bed2_roster() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies::default(),
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![vec!["plain".to_owned()], vec!["beta@blue@bed2".to_owned()]];

        let err = RuntimeV2Runner::from_bed2_roster(&raw_groups, registry, bed2, summon)
            .expect_err("bed2-only runner constructor should reject non-bed2 rows");

        assert_eq!(
            err,
            CustomBed2RosterImportError {
                team_index: 0,
                player_index: 0,
                raw: "plain".to_owned(),
            }
        );
    }

    #[test]
    fn runtime_v2_runner_runs_mixed_roster_until_winner() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let raw_groups = vec![
            vec!["plain@red".to_owned(), "alpha@red+bed2[9]".to_owned()],
            vec!["seed:custom-seed@!".to_owned(), "beta@blue+bed2[3]".to_owned()],
        ];

        let mut runner = RuntimeV2Runner::from_mixed_roster(&raw_groups, registry, bed2, summon)
            .expect("mixed roster should construct a runtime v2 runner");
        let plain = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.clone();

        let (summary, actual) = runner.run_until_winner_normalized(8);

        assert_eq!(summary.rounds.len(), 1);
        assert_eq!(summary.winner_team, Some(0));
        assert!(!summary.guard_exhausted);
        let expected = NormalizedOutcome {
            winner_team: Some(0),
            round: 1,
            total_score: plain.attack as u64,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
            entity_ids: vec![1, 2, 3],
            teams: vec![0, 0, 1],
            hp: vec![plain.max_hp, 9, 0],
            magic_point: vec![plain.magic_point, 0, 0],
            defense: vec![plain.defense, DEFAULT_BED2_DEFENSE, DEFAULT_BED2_DEFENSE],
            resistance: vec![plain.resistance, DEFAULT_BED2_RESISTANCE, DEFAULT_BED2_RESISTANCE],
            alive: vec![true, true, false],
            round_order: vec![0, 1, 2],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0, 1], Vec::new()],
            alive_group_count: 1,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 1,
                actor: 0,
                target: 2,
                amount: plain.attack,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 0,
                target: 2,
                targets: Vec::new(),
                param: None,
                score: plain.attack as u32,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        };

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn custom_runner_multi_round_normalized_run_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2-runner",
                "custom.bed2_runner",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 runner kind should register");
        let registry = builder.build();
        let mut runner = RuntimeV2Runner::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 8, 3),
                PlayerTemplate::with_kind(2, "bed2", bed2, 1, 5, 0).with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE),
            ],
            registry,
        ));

        let run = runner.run_until_winner_normalized_rounds(8);

        assert_eq!(run.winner_team, Some(0));
        assert!(!run.guard_exhausted);
        assert_eq!(run.total_score, 6);
        assert_eq!(run.rounds.len(), 3);
        let expected_rounds = vec![
            NormalizedOutcome {
                winner_team: None,
                round: 1,
                total_score: 3,
                rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(1),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![8, 2],
                magic_point: vec![0, 0],
                defense: vec![0, DEFAULT_BED2_DEFENSE],
                resistance: vec![0, DEFAULT_BED2_RESISTANCE],
                alive: vec![true, true],
                round_order: vec![0, 1],
                flat_alive: vec![0, 1],
                team_alive: vec![vec![0], vec![1]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 1,
                    actor: 0,
                    target: 1,
                    amount: 3,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 3,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
            NormalizedOutcome {
                winner_team: None,
                round: 2,
                total_score: 0,
                rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(2),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![8, 2],
                magic_point: vec![0, 0],
                defense: vec![0, DEFAULT_BED2_DEFENSE],
                resistance: vec![0, DEFAULT_BED2_RESISTANCE],
                alive: vec![true, true],
                round_order: vec![0, 1],
                flat_alive: vec![0, 1],
                team_alive: vec![vec![0], vec![1]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 2,
                    actor: 1,
                    target: 0,
                    amount: 0,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 1,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
            NormalizedOutcome {
                winner_team: Some(0),
                round: 3,
                total_score: 3,
                rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::after_next_u8(3),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![8, 0],
                magic_point: vec![0, 0],
                defense: vec![0, DEFAULT_BED2_DEFENSE],
                resistance: vec![0, DEFAULT_BED2_RESISTANCE],
                alive: vec![true, false],
                round_order: vec![0, 1],
                flat_alive: vec![0],
                team_alive: vec![vec![0], Vec::new()],
                alive_group_count: 1,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 3,
                    actor: 0,
                    target: 1,
                    amount: 3,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 3,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                }],
            },
        ];

        for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
            assert_eq!(strict_diff(expected, actual), Ok(()));
        }
    }

    #[cfg(not(feature = "no_debug"))]
    #[test]
    fn run_minimal_round_records_trace_when_enabled() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.enable_trace();

        runtime.run_minimal_round();

        let trace = runtime.trace().expect("trace should be enabled");
        assert_eq!(trace.actions.len(), 1);
        assert_eq!(trace.actions[0].actor, EntityIdx(0));
        assert_eq!(trace.actions[0].target, EntityIdx(1));
        assert_eq!(
            trace.actions[0].rng_before,
            Some(RngCheckpoint {
                i: 0,
                j: 0,
                byte_count: 0,
            })
        );
        assert_eq!(
            trace.actions[0].rng_after,
            Some(RngCheckpoint {
                i: 1,
                j: 1,
                byte_count: 0,
            })
        );
        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].total_score, 3);
        assert_eq!(trace.frames[0].winner_team, None);
        assert_eq!(
            trace.frames[0].rng_after,
            Some(RngCheckpoint {
                i: 1,
                j: 1,
                byte_count: 0,
            })
        );
        assert_eq!(trace.frames[0].updates[0].score, 3);
    }

    #[test]
    fn run_minimal_round_applies_damage_frame() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        let outcome = runtime.run_minimal_round();

        assert_eq!(outcome.winner_team, None);
        assert!(outcome.frame.is_some());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(runtime.round, 1);
    }

    #[test]
    fn run_minimal_round_reports_winner_after_lethal_damage() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 3, 3));
        let outcome = runtime.run_minimal_round();

        assert_eq!(outcome.winner_team, Some(0));
        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    }

    #[test]
    fn run_minimal_round_dispatches_die_and_kill_state_hooks_after_lethal_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let kill_state = builder
            .register_state("custom", "kill", "custom.kill", ProcMask::KILL, SkillPriority(0))
            .expect("kill state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 3, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(kill_state),
            hook_mask: ProcMask::KILL,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(die_state, state_marks_update);
        runtime.set_state_handler(kill_state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("lethal attack should emit hooks");

        assert_eq!(outcome.winner_team, Some(0));
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 22);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 11);
    }

    #[test]
    fn flush_effects_dispatches_die_and_kill_skill_hooks_after_lethal_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let die_skill = builder
            .register_skill_with_hooks(
                "custom",
                "die-skill",
                "custom.die_skill",
                ProcMask::DIE,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("die skill should register");
        let kill_skill = builder
            .register_skill_with_hooks(
                "custom",
                "kill-skill",
                "custom.kill_skill",
                ProcMask::KILL,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("kill skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([kill_skill]),
                PlayerTemplate::new(2, "right", 1, 3, 3).with_skills([die_skill]),
            ],
            registry,
        ));
        runtime.set_skill_handler(die_skill, skill_marks_update);
        runtime.set_skill_handler(kill_skill, skill_marks_update);
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 3,
        });

        let frame = runtime.flush_effects().expect("lethal damage should emit hooks");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "skill mark");
        assert_eq!(frame.updates.updates[1].score, die_skill.0);
        assert_eq!(frame.updates.updates[2].message, "skill mark");
        assert_eq!(frame.updates.updates[2].score, kill_skill.0);
    }

    #[test]
    fn flush_effects_routes_root_owner_damage_to_owner_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("routed damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].score, 4);
    }

    #[test]
    fn flush_effects_runs_die_hook_on_resolved_root_owner() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 4, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 99,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(die_state, state_marks_update);
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("lethal routed damage should emit hooks");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 99);
    }

    #[test]
    fn flush_effects_shares_summon_damage_to_owner_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("shared damage should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 1);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[0].score, 4);
        assert_eq!(frame.updates.updates[1].target, 0);
        assert_eq!(frame.updates.updates[1].score, 4);
    }

    #[test]
    fn flush_effects_runs_die_hook_on_damage_share_owner() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 4, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 101,
            extension_state_id: Some(die_state),
            hook_mask: ProcMask::DIE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(die_state, state_marks_update);
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1),
        });
        runtime.flush_effects().expect("spawn should emit update");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });

        let frame = runtime.flush_effects().expect("shared lethal damage should emit hooks");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[1].target, 0);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 101);
    }

    #[test]
    fn flush_effects_removes_lethal_damage_target_from_alive_views() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 4,
        });

        runtime.flush_effects().expect("lethal damage should emit update");

        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
        assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities), None);
    }

    #[test]
    fn flush_effects_shares_owner_damage_to_alive_summons() {
        let mut builder = ExtensionRegistryBuilder::default();
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "owner",
                "custom.owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("owner kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(3, "summon-a", 0, 5, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(4, "summon-b", 0, 5, 1),
        });
        runtime.flush_effects().expect("spawns should emit updates");
        runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.alive = false;
        runtime.entities.get_mut(EntityIdx(3)).unwrap().runtime.hp = 0;
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 3,
        });

        let frame = runtime.flush_effects().expect("shared summon damage should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 7);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 2);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].score, 3);
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].score, 3);
    }

    #[test]
    fn custom_summon_fixture_combines_owner_route_share_and_skill_reuse() {
        let mut builder = ExtensionRegistryBuilder::default();
        let recast_skill = builder
            .register_skill(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon-owner",
                "custom.summon_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 5, 1).with_skills([recast_skill]),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "summon-recast", summon_kind, 0, 5, 1).with_skills([recast_skill]),
        });
        runtime.flush_effects().expect("summon spawns should emit updates");

        assert_eq!(
            runtime.entities.get(EntityIdx(2)).unwrap().template.skills.skills(),
            &[recast_skill]
        );
        assert_eq!(
            runtime.entities.get(EntityIdx(3)).unwrap().template.skills.skills(),
            &[recast_skill]
        );
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.root_owner, EntityIdx(0));

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });
        let routed = runtime.flush_effects().expect("summon/root owner damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 5);
        assert_eq!(routed.updates.updates.len(), 1);
        assert_eq!(routed.updates.updates[0].target, 0);
        assert_eq!(routed.updates.updates[0].score, 4);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 2,
        });
        let shared = runtime.flush_effects().expect("owner damage should share to summons");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 4);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 3);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 3);
        assert_eq!(shared.updates.updates.len(), 3);
        assert_eq!(shared.updates.updates[0].target, 0);
        assert_eq!(shared.updates.updates[1].target, 2);
        assert_eq!(shared.updates.updates[2].target, 3);
    }

    #[test]
    fn custom_summon_fixture_inherits_owner_defense_and_resistance() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_def_res(77, 88),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
        });
        runtime.flush_effects().expect("summon spawn should emit update");

        let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
        assert_eq!(summon.template.defense, 77);
        assert_eq!(summon.template.resistance, 88);
        assert_eq!(summon.runtime.defense, 77);
        assert_eq!(summon.runtime.resistance, 88);
    }

    #[test]
    fn summon_default_skill_loadout_keeps_fixed_lanes_and_active_order() {
        let fire = SkillId(11);
        let explode = SkillId(12);
        let loadout = summon_default_skill_loadout(fire, explode, [2, 0, 1]);

        assert_eq!(loadout.skills(), &[fire, fire, explode]);
        assert_eq!(loadout.active_order(), &[2, 0, 1]);
    }

    #[test]
    fn charged_summon_template_can_disable_share_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 3),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "charged-summon", summon_kind, 0, 5, 1)
                .with_damage_share_policy(DamageSharePolicy::None)
                .with_speed_points(2048),
        });
        runtime.flush_effects().expect("charged summon spawn should emit update");

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 4,
        });
        let frame = runtime.flush_effects().expect("summon damage should emit update");

        let summon = runtime.entities.get(EntityIdx(2)).expect("charged summon should exist");
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 10);
        assert_eq!(summon.runtime.hp, 1);
        assert_eq!(summon.runtime.move_state, MoveState { speed_points: 2048 });
        assert_eq!(summon.runtime.policies.damage_share, DamageSharePolicy::None);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[0].score, 4);
    }

    #[test]
    fn summon_explode_effect_emits_legacy_replay_and_kills_summon() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
        ]));
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::fire_mag(91, 3));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 5.5;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 2.0);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[0].caster, 2);
        assert_eq!(frame.updates.updates[0].target, 1);
        assert_eq!(frame.updates.updates[0].score, 0);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].caster, 2);
        assert_eq!(frame.updates.updates[1].target, 1);
        assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
    }

    #[test]
    fn summon_explode_can_be_dodged_after_self_death() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "owner", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 512).with_agility(512),
            PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(0),
        ]));
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::fire_mag(91, 3));
        let mut expected_rng = RC4::default();
        let _ = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng);
        assert!(PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("dodged summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 1.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0][回避]了攻击");
        assert_eq!(frame.updates.updates[1].caster, 1);
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].score, 20);
    }

    #[test]
    fn summon_explode_runs_pre_defend_before_dodge_and_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_defend = builder
            .register_skill_with_hooks(
                "custom",
                "pre-defend",
                "custom.pre_defend",
                ProcMask::PRE_DEFEND,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("pre-defend skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                    .with_def_res(0, 16)
                    .with_skills([pre_defend]),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_skill_handler(pre_defend, skill_halves_defend_atp);
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::fire_mag(91, 3));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 5.5 / 2.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("pre-defend summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 2.0);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "pre defend skill");
        assert_eq!(frame.updates.updates[1].caster, 1);
        assert_eq!(frame.updates.updates[2].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].score, expected_amount as u32);
    }

    #[test]
    fn summon_explode_pre_defend_zero_stops_before_dodge_damage_and_fire() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_defend = builder
            .register_skill_with_hooks(
                "custom",
                "pre-defend-zero",
                "custom.pre_defend_zero",
                ProcMask::PRE_DEFEND,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("pre-defend skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                    .with_def_res(0, 512)
                    .with_agility(512)
                    .with_skills([pre_defend]),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(0),
            ],
            registry,
        ));
        runtime.set_skill_handler(pre_defend, skill_zeroes_defend_atp);
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::fire_mag(91, 3));
        let mut expected_rng = RC4::default();
        let _ = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("pre-defend zero should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 1.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "pre defend zero");
    }

    #[test]
    fn summon_explode_runs_post_defend_skill_and_state_in_priority_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let post_skill = builder
            .register_skill_with_hooks(
                "custom",
                "post-defend-skill",
                "custom.post_defend_skill",
                ProcMask::POST_DEFEND,
                TargetPolicy::None,
                SkillPriority(2000),
            )
            .expect("post-defend skill should register");
        let post_state = builder
            .register_state(
                "custom",
                "post-defend-state",
                "custom.post_defend_state",
                ProcMask::POST_DEFEND,
                SkillPriority(1000),
            )
            .expect("post-defend state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                    .with_def_res(0, 16)
                    .with_skills([post_skill]),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_skill_handler(post_skill, skill_halves_defend_damage);
        runtime.set_state_handler(post_state, state_adds_defend_damage);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(post_state),
            hook_mask: ProcMask::POST_DEFEND,
            priority: SkillPriority(1000),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        let expected_amount = (raw_amount + 3) / 2;
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("post-defend summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 4);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "post defend state");
        assert_eq!(frame.updates.updates[2].message, "post defend skill");
        assert_eq!(frame.updates.updates[3].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[3].score, expected_amount as u32);
    }

    #[test]
    fn summon_explode_post_defend_shield_absorbs_damage_and_consumes_payload() {
        let mut builder = ExtensionRegistryBuilder::default();
        let shield_state = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
            .expect("shield state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(shield_state, run_shield_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
            77,
            shield_state,
            500,
            SkillPriority(6000),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!(raw_amount < 500);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("shielded summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(77)
                .and_then(StateEntry::shield_value),
            Some(500 - raw_amount)
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, 0);
    }

    #[test]
    fn summon_explode_post_defend_shield_breaks_before_remaining_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let shield_state = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
            .expect("shield state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(shield_state, run_shield_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
            77,
            shield_state,
            3,
            SkillPriority(6000),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!(raw_amount > 3);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("shield break summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - raw_amount);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(77)
                .and_then(StateEntry::shield_value),
            Some(0)
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, raw_amount as u32);
    }

    #[test]
    fn summon_explode_post_defend_iron_reduces_absorbed_damage_to_one() {
        let mut builder = ExtensionRegistryBuilder::default();
        let iron_state = builder
            .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            500,
            3,
            SkillPriority(10),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!((1..=500).contains(&raw_amount));
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("iron absorbed summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 9_999);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(79)
                .and_then(StateEntry::iron_value),
            Some((500, 3))
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, 1);
    }

    #[test]
    fn summon_explode_post_defend_iron_reduces_defended_damage_to_zero() {
        let mut builder = ExtensionRegistryBuilder::default();
        let defend_skill = builder
            .register_skill_with_hooks(
                "custom",
                "defend-marker",
                "custom.defend_marker",
                ProcMask::POST_DEFEND,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("defend marker skill should register");
        let iron_state = builder
            .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3)
                    .with_def_res(0, 16)
                    .with_skills([defend_skill]),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_skill_handler(defend_skill, skill_marks_defend_replay);
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            500,
            3,
            SkillPriority(10),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!((1..=500).contains(&raw_amount));
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("defended iron summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(79)
                .and_then(StateEntry::iron_value),
            Some((500, 3))
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0][防御]");
        assert_eq!(frame.updates.updates[1].caster, 1);
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[2].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].score, 0);
    }

    #[test]
    fn summon_explode_post_defend_iron_breaks_and_emits_cancel_replay() {
        let mut builder = ExtensionRegistryBuilder::default();
        let iron_state = builder
            .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            3,
            3,
            SkillPriority(10),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!(raw_amount > 3);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("broken iron summon explode should emit updates");

        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp,
            10_000 - (raw_amount - 3)
        );
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(79)
                .and_then(StateEntry::iron_value),
            Some((0, 0))
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 4);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(
            frame.updates.updates[1].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[2].message, "[1]的[铁壁]被打消了");
        assert_eq!(frame.updates.updates[2].caster, 2);
        assert_eq!(frame.updates.updates[2].target, 1);
        assert_eq!(frame.updates.updates[3].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[3].score, (raw_amount - 3) as u32);
    }

    #[test]
    fn summon_explode_post_defend_iron_skips_state_change_when_damage_is_zero() {
        let mut builder = ExtensionRegistryBuilder::default();
        let shield_state = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(0))
            .expect("shield state should register");
        let iron_state = builder
            .register_state("core", "iron", "core.iron", ProcMask::POST_DEFEND, SkillPriority(10))
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(shield_state, run_shield_post_defend_state);
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
            77,
            shield_state,
            500,
            SkillPriority(0),
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            300,
            3,
            SkillPriority(10),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!(raw_amount < 500);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("zero damage iron summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(79)
                .and_then(StateEntry::iron_value),
            Some((300, 3))
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, 0);
    }

    #[test]
    fn summon_explode_post_defend_curse_doubles_damage_and_emits_replay() {
        let mut builder = ExtensionRegistryBuilder::default();
        let curse_state = builder
            .register_state("core", "curse", "core.curse", ProcMask::POST_DEFEND, SkillPriority(10_000))
            .expect("curse state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(curse_state, run_curse_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::curse(
            78,
            curse_state,
            64,
            2,
            SkillPriority(10_000),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        let curse_roll = expected_rng.next_u8() as u32 & 63;
        assert!(curse_roll < 64);
        let expected_amount = raw_amount * 2;
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("curse summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[诅咒]使伤害加倍");
        assert_eq!(frame.updates.updates[1].caster, 2);
        assert_eq!(frame.updates.updates[1].target, 1);
        assert_eq!(frame.updates.updates[1].score, 0);
        assert_eq!(frame.updates.updates[2].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].score, expected_amount as u32);
    }

    #[test]
    fn summon_explode_post_defend_curse_consumes_rng_without_trigger() {
        let mut builder = ExtensionRegistryBuilder::default();
        let curse_state = builder
            .register_state("core", "curse", "core.curse", ProcMask::POST_DEFEND, SkillPriority(10_000))
            .expect("curse state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(curse_state, run_curse_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::curse(
            78,
            curse_state,
            0,
            2,
            SkillPriority(10_000),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        let _curse_roll = expected_rng.next_u8() as u32 & 63;
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("curse miss summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - raw_amount);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, raw_amount as u32);
    }

    #[test]
    fn summon_explode_post_defend_curse_skips_rng_when_damage_is_zero() {
        let mut builder = ExtensionRegistryBuilder::default();
        let shield_state = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
            .expect("shield state should register");
        let curse_state = builder
            .register_state("core", "curse", "core.curse", ProcMask::POST_DEFEND, SkillPriority(10_000))
            .expect("curse state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime.set_state_handler(shield_state, run_shield_post_defend_state);
        runtime.set_state_handler(curse_state, run_curse_post_defend_state);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::shield(
            77,
            shield_state,
            500,
            SkillPriority(6000),
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().states.add_entry(StateEntry::curse(
            78,
            curse_state,
            64,
            2,
            SkillPriority(10_000),
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 4.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let raw_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!(raw_amount < 500);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("shielded curse summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 0.0);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, 0);
    }

    #[test]
    fn summon_explode_fire_stack_respects_boss_fire_immune() {
        let mut builder = ExtensionRegistryBuilder::default();
        let boss_kind = builder
            .register_player_kind_with_policies(
                "core",
                "boss",
                "core.boss",
                PlayerKindFlags::BOSS,
                PlayerKindPolicies::default(),
            )
            .expect("boss kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::with_kind(2, "saitama", boss_kind, 1, 10_000, 3).with_def_res(0, 16),
                PlayerTemplate::new(3, "summon", 0, 5, 1).with_magic(80),
            ],
            registry,
        ));
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::fire_mag(91, 3));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(2)).unwrap().runtime.get_at(true, &mut expected_rng) * 5.5;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        let threshold = crate::player::boss::boss_immune_threshold("saitama", "fire");
        assert!((expected_rng.next_u8() as i32) < threshold);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("boss immune summon explode should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(91), 1.5);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
    }

    #[test]
    fn summon_explode_runs_summon_die_and_target_kill_hooks() {
        let mut builder = ExtensionRegistryBuilder::default();
        let die_skill = builder
            .register_skill_with_hooks(
                "custom",
                "die-skill",
                "custom.die_skill",
                ProcMask::DIE,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("die skill should register");
        let kill_skill = builder
            .register_skill_with_hooks(
                "custom",
                "kill-skill",
                "custom.kill_skill",
                ProcMask::KILL,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("kill skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 3, 3).with_def_res(0, 0).with_skills([die_skill]),
                PlayerTemplate::new(3, "summon", 0, 5, 1)
                    .with_magic(80)
                    .with_skills([die_skill, kill_skill]),
            ],
            registry,
        ));
        runtime.set_skill_handler(die_skill, skill_marks_update);
        runtime.set_skill_handler(kill_skill, skill_marks_update);
        runtime.effects.push(QueuedEffect::SummonExplode {
            caster: EntityIdx(2),
            target: EntityIdx(1),
            fire_state_key: 91,
        });

        let frame = runtime.flush_effects().expect("summon explode should emit hook updates");

        assert_eq!(frame.updates.updates.len(), 5);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[自爆]");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].target, 1);
        assert_eq!(frame.updates.updates[2].message, "skill mark");
        assert_eq!(frame.updates.updates[2].caster, 1);
        assert_eq!(frame.updates.updates[2].score, die_skill.0);
        assert_eq!(frame.updates.updates[3].message, "skill mark");
        assert_eq!(frame.updates.updates[3].caster, 2);
        assert_eq!(frame.updates.updates[3].score, kill_skill.0);
        assert_eq!(frame.updates.updates[4].message, "skill mark");
        assert_eq!(frame.updates.updates[4].caster, 2);
        assert_eq!(frame.updates.updates[4].target, 2);
        assert_eq!(frame.updates.updates[4].score, die_skill.0);
    }

    #[test]
    fn custom_summon_recast_handler_revives_existing_summon_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summoned_slot = builder
            .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
            .expect("summoned entity slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon-owner",
                "custom.summon_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                    .with_def_res(77, 88)
                    .with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            skill_summon_recast_fixture_handler,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("first summon cast should emit spawn update");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(first.updates.updates.len(), 1);
        assert_eq!(first.updates.updates[0].message, "出现一个新的[1]");
        assert_eq!(first.updates.updates[0].target, 2);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
            Some(&SlotValue::U64(2))
        );
        let summon = runtime.entities.get(EntityIdx(2)).expect("summon should exist");
        assert_eq!(summon.template.kind, summon_kind);
        assert_eq!(summon.template.skills.skills(), &[recast_skill]);
        assert_eq!(summon.runtime.owner, EntityIdx(0));
        assert_eq!(summon.runtime.root_owner, EntityIdx(0));
        assert_eq!(summon.runtime.defense, 77);
        assert_eq!(summon.runtime.resistance, 88);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 10,
        });
        runtime.flush_effects().expect("lethal summon damage should emit update");
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));

        let recast = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("summon recast should revive existing entity");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(recast.updates.updates.len(), 1);
        assert_eq!(recast.updates.updates[0].message, "[1][复活]了");
        assert_eq!(recast.updates.updates[0].target, 2);
        let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
        assert!(revived.runtime.alive);
        assert_eq!(revived.runtime.hp, 10);
        assert_eq!(revived.template.skills.skills(), &[recast_skill]);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
    }

    #[test]
    fn custom_summon_recast_handler_can_emit_legacy_summon_messages() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summoned_slot = builder
            .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
            .expect("summoned entity slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon-owner",
                "custom.summon_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            skill_legacy_summon_recast_fixture_handler,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("legacy summon cast should emit updates");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(first.updates.updates.len(), 2);
        assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
        assert_eq!(first.updates.updates[0].score, 60);
        assert_eq!(first.updates.updates[1].message, "召唤出[1]");
        assert_eq!(first.updates.updates[1].target, 2);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
            Some(&SlotValue::U64(2))
        );
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.kind, summon_kind);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 10,
        });
        runtime.flush_effects().expect("lethal summon damage should emit update");
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

        let recast = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("legacy summon recast should emit updates");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(recast.updates.updates.len(), 2);
        assert_eq!(recast.updates.updates[0].message, "[0]使用[血祭]");
        assert_eq!(recast.updates.updates[0].score, 60);
        assert_eq!(recast.updates.updates[1].message, "召唤出[1]");
        assert_eq!(recast.updates.updates[1].target, 2);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    }

    #[test]
    fn summon_recast_from_template_slot_uses_payload_and_revives_existing_entity() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summoned_slot = builder
            .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
            .expect("summoned entity slot should reserve");
        let template_slot = builder
            .reserve_template_slot("custom", "summon-template", "custom.summon.template")
            .expect("summon template slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon-owner",
                "custom.summon_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("summon owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let payload = PlayerTemplate::with_kind(3, "summon-template", summon_kind, 0, 10, 1)
            .with_def_res(11, 22)
            .with_skills([recast_skill])
            .with_speed_points(2048);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3)
                    .with_def_res(77, 88)
                    .with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        );
        template
            .slots
            .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
            .expect("summon template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            run_legacy_summon_recast_from_template_slot,
            &[
                ExtensionCapability::ReadTemplateSlots,
                ExtensionCapability::ReadAllies,
                ExtensionCapability::MutateEntitySlots,
            ],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("template-slot summon cast should emit updates");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(first.updates.updates.len(), 2);
        assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
        assert_eq!(first.updates.updates[0].score, 60);
        assert_eq!(first.updates.updates[1].message, "召唤出[1]");
        assert_eq!(first.updates.updates[1].target, 2);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
            Some(&SlotValue::U64(2))
        );
        let summon = runtime.entities.get(EntityIdx(2)).expect("summon should spawn");
        assert_eq!(summon.template.name, payload.name);
        assert_eq!(summon.template.kind, summon_kind);
        assert_eq!(summon.template.skills.skills(), &[recast_skill]);
        assert_eq!(summon.runtime.owner, EntityIdx(0));
        assert_eq!(summon.runtime.root_owner, EntityIdx(0));
        assert_eq!(summon.runtime.defense, 77);
        assert_eq!(summon.runtime.resistance, 88);
        assert_eq!(summon.template.move_state, payload.move_state);
        assert_eq!(summon.runtime.move_state, payload.move_state);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 10,
        });
        runtime.flush_effects().expect("lethal summon damage should emit update");
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

        let recast = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("template-slot summon recast should revive existing entity");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(recast.updates.updates.len(), 2);
        assert_eq!(recast.updates.updates[0].message, "[0]使用[血祭]");
        assert_eq!(recast.updates.updates[1].message, "召唤出[1]");
        assert_eq!(recast.updates.updates[1].target, 2);
        let revived = runtime.entities.get(EntityIdx(2)).expect("summon should revive in place");
        assert!(revived.runtime.alive);
        assert_eq!(revived.runtime.hp, 10);
        assert_eq!(revived.template.skills.skills(), &[recast_skill]);
        assert_eq!(revived.runtime.move_state, payload.move_state);
    }

    #[test]
    fn configured_summon_recast_handler_uses_non_default_slots() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .reserve_entity_slot("custom", "unused-entity-slot", "custom.summon.unused_entity")
            .expect("unused entity slot should reserve");
        let summoned_slot = builder
            .reserve_entity_slot("custom", "configured-summon-slot", "custom.summon.configured_entity")
            .expect("configured summoned entity slot should reserve");
        builder
            .reserve_template_slot("custom", "unused-template-slot", "custom.summon.unused_template")
            .expect("unused template slot should reserve");
        let template_slot = builder
            .reserve_template_slot("custom", "configured-summon-template", "custom.summon.configured_template")
            .expect("configured summon template slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "configured-summon-recast",
                "custom.configured_summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("configured summon recast skill should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "configured-summon",
                "custom.configured_summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("configured summon kind should register");
        let registry = builder.build();
        let payload = PlayerTemplate::with_kind(3, "configured-summon", summon_kind, 0, 7, 1);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        );
        template
            .slots
            .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
            .expect("configured summon template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            skill_configured_summon_recast_handler,
            &[
                ExtensionCapability::ReadTemplateSlots,
                ExtensionCapability::ReadAllies,
                ExtensionCapability::MutateEntitySlots,
            ],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("configured summon recast should emit updates");

        assert_eq!(first.updates.updates[0].message, "[0]使用[血祭]");
        assert_eq!(first.updates.updates[1].message, "召唤出[1]");
        assert_eq!(first.updates.updates[1].target, 2);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(EntitySlotId(0)), None);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(summoned_slot),
            Some(&SlotValue::U64(2))
        );
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, payload.name);

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(2),
            amount: 7,
        });
        runtime.flush_effects().expect("configured summon damage should flush");
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);

        let recast = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("configured summon recast should revive existing entity");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(recast.updates.updates[1].target, 2);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn push_summon_recast_from_entity_slot_reports_alive_remembered_summon() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summoned_slot = builder
            .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
            .expect("summoned entity slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
                PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1),
            ],
            registry,
        ));
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(summoned_slot, SlotValue::U64(2))
            .expect("remembered summon slot should write");
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            skill_records_alive_summon_recast_error,
            &[ExtensionCapability::ReadAllies],
        );

        let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.len(), 3);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
    }

    #[test]
    fn push_summon_recast_from_entity_slot_reports_missing_read_allies_capability() {
        let mut builder = ExtensionRegistryBuilder::default();
        let summoned_slot = builder
            .reserve_entity_slot("custom", "summoned-entity", "custom.summon.summoned_entity")
            .expect("summoned entity slot should reserve");
        let recast_skill = builder
            .register_skill_with_hooks(
                "custom",
                "summon-recast",
                "custom.summon_recast",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("summon recast skill should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("summon kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([recast_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
                PlayerTemplate::with_kind(3, "summon", summon_kind, 0, 10, 1),
            ],
            registry,
        ));
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(summoned_slot, SlotValue::U64(2))
            .expect("remembered summon slot should write");
        runtime.set_skill_handler_with_capabilities(
            recast_skill,
            skill_records_missing_recast_read_allies_error,
            &[ExtensionCapability::MutateEntitySlots],
        );

        let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.len(), 3);
        assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
    }

    #[test]
    fn next_minion_name_from_entity_slot_allocates_from_root_owner() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let minion_skill = builder
            .register_skill_with_hooks(
                "custom",
                "minion-name",
                "custom.minion_name",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("minion name skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill])],
            registry,
        ));
        runtime.set_skill_handler_with_capabilities(
            minion_skill,
            skill_records_next_minion_name,
            &[ExtensionCapability::MutateEntitySlots],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("first minion name should emit update");
        let second = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("second minion name should emit update");

        assert_eq!(first.updates.updates[0].message, "owner?0");
        assert_eq!(second.updates.updates[0].message, "owner?1");
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(2))
        );
    }

    #[test]
    fn next_minion_name_from_entity_slot_uses_root_owner_for_child_minions() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let minion_skill = builder
            .register_skill_with_hooks(
                "custom",
                "minion-name",
                "custom.minion_name",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("minion name skill should register");
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?shadow", minion_kind, 0, 5, 1).with_skills([minion_skill]),
        });
        runtime.flush_effects().expect("child minion spawn should emit update");
        runtime.set_skill_handler_with_capabilities(
            minion_skill,
            skill_records_next_minion_name,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
        );

        let first = runtime
            .run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION)
            .expect("first child minion name should emit update");
        let second = runtime
            .run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION)
            .expect("second child minion name should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
        assert_eq!(first.updates.updates[0].message, "owner?0");
        assert_eq!(second.updates.updates[0].message, "owner?1");
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(2))
        );
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().slots.get(counter_slot), None);
    }

    #[test]
    fn next_minion_name_from_entity_slot_requires_root_owner_read_capability() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let minion_skill = builder
            .register_skill_with_hooks(
                "custom",
                "minion-name",
                "custom.minion_name",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("minion name skill should register");
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?shadow", minion_kind, 0, 5, 1).with_skills([minion_skill]),
        });
        runtime.flush_effects().expect("child minion spawn should emit update");
        runtime.set_skill_handler_with_capabilities(
            minion_skill,
            skill_records_missing_minion_name_read_allies_error,
            &[ExtensionCapability::MutateEntitySlots],
        );

        let frame = runtime.run_skill_hooks(EntityIdx(2), ProcMask::PRE_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot), None);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().slots.get(counter_slot), None);
    }

    #[test]
    fn push_minion_from_template_with_allocated_name_sets_name_and_spawns() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let minion_skill = builder
            .register_skill_with_hooks(
                "custom",
                "minion-spawn",
                "custom.minion_spawn",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("minion spawn skill should register");
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.set_skill_handler_with_capabilities(
            minion_skill,
            skill_pushes_named_minion_spawn,
            &[ExtensionCapability::MutateEntitySlots],
        );

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("named minion spawn should emit update");

        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
        assert_eq!(frame.updates.updates[0].caster, 0);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(1))
        );
        let minion = runtime.entities.get(EntityIdx(2)).expect("minion should spawn");
        assert_eq!(minion.template.name, "owner?0");
        assert_eq!(minion.template.kind, minion_kind);
        assert_eq!(minion.runtime.owner, EntityIdx(0));
        assert_eq!(minion.runtime.root_owner, EntityIdx(0));
    }

    #[test]
    fn push_minion_from_template_slot_with_allocated_name_reads_template_and_spawns() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let template_slot = builder
            .reserve_template_slot("custom", "shadow-template", "custom.minion.shadow_template")
            .expect("shadow template slot should reserve");
        let minion_skill = builder
            .register_skill_with_hooks(
                "custom",
                "minion-template-spawn",
                "custom.minion_template_spawn",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("minion template spawn skill should register");
        let inherited_skill = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("inherited minion skill should register");
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "shadow",
                "custom.minion.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("shadow minion kind should register");
        let registry = builder.build();
        let payload = PlayerTemplate::with_kind(3, "placeholder-shadow", minion_kind, 0, 5, 1)
            .with_def_res(2, 3)
            .with_skills([inherited_skill])
            .with_speed_points(-2048);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([minion_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        );
        template
            .slots
            .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload.clone())))
            .expect("shadow template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            minion_skill,
            skill_pushes_named_minion_from_template_slot,
            &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
        );

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("named minion template spawn should emit update");

        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(1))
        );
        let minion = runtime.entities.get(EntityIdx(2)).expect("template minion should spawn");
        assert_eq!(minion.template.name, "owner?0");
        assert_eq!(minion.template.kind, minion_kind);
        assert_eq!(minion.template.max_hp, payload.max_hp);
        assert_eq!(minion.template.defense, payload.defense);
        assert_eq!(minion.template.resistance, payload.resistance);
        assert_eq!(minion.template.skills.skills(), &[inherited_skill]);
        assert_eq!(minion.runtime.owner, EntityIdx(0));
        assert_eq!(minion.runtime.root_owner, EntityIdx(0));
        assert_eq!(minion.template.move_state, payload.move_state);
        assert_eq!(minion.runtime.move_state, payload.move_state);
    }

    #[test]
    fn shadow_style_minion_template_handler_emits_legacy_replay_sequence() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let template_slot = builder
            .reserve_template_slot("custom", "shadow-template", "custom.minion.shadow_template")
            .expect("shadow template slot should reserve");
        let shadow_skill = builder
            .register_skill_with_hooks(
                "custom",
                "shadow",
                "custom.minion.shadow_skill",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("shadow skill should register");
        let possess_skill = builder
            .register_skill(
                "custom",
                "possess",
                "custom.minion.possess",
                TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("possess skill should register");
        let shadow_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "shadow",
                "custom.minion.shadow",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("shadow minion kind should register");
        let registry = builder.build();
        let payload = PlayerTemplate::with_kind(3, "owner?shadow", shadow_kind, 0, 5, 1)
            .with_skills([possess_skill])
            .with_speed_points(-2048);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([shadow_skill]),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        );
        template
            .slots
            .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
            .expect("shadow template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            shadow_skill,
            run_shadow_minion_from_template_slot,
            &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
        );

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("shadow-style minion spawn should emit legacy updates");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[幻术]");
        assert_eq!(frame.updates.updates[0].caster, 0);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].score, 60);
        assert_eq!(frame.updates.updates[1].message, "召唤出[1]");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(1))
        );
        let shadow = runtime.entities.get(EntityIdx(2)).expect("shadow minion should spawn");
        assert_eq!(shadow.template.name, "owner?0");
        assert_eq!(shadow.template.kind, shadow_kind);
        assert_eq!(shadow.template.skills.skills(), &[possess_skill]);
        assert_eq!(shadow.runtime.owner, EntityIdx(0));
        assert_eq!(shadow.runtime.root_owner, EntityIdx(0));
        assert_eq!(shadow.runtime.move_state, MoveState { speed_points: -2048 });
    }

    #[test]
    fn zombie_style_minion_template_handler_emits_legacy_replay_sequence() {
        let mut builder = ExtensionRegistryBuilder::default();
        let counter_slot = builder
            .reserve_entity_slot("custom", "minion-counter", "custom.minion.counter")
            .expect("minion counter slot should reserve");
        let template_slot = builder
            .reserve_template_slot("custom", "zombie-template", "custom.minion.zombie_template")
            .expect("zombie template slot should reserve");
        let zombie_skill = builder
            .register_skill_with_hooks(
                "custom",
                "zombie",
                "custom.minion.zombie_skill",
                ProcMask::KILL,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("zombie skill should register");
        let zombie_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "zombie",
                "custom.minion.zombie",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("zombie minion kind should register");
        let registry = builder.build();
        let payload = PlayerTemplate::with_kind(3, "owner?zombie", zombie_kind, 0, 4, 1).with_speed_points(1020);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([zombie_skill]),
                PlayerTemplate::new(2, "victim", 1, 10, 1),
            ],
            registry,
        );
        template
            .slots
            .set(template_slot, SlotValue::PlayerTemplate(Box::new(payload)))
            .expect("zombie template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            zombie_skill,
            run_zombie_minion_from_template_slot,
            &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
        );

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::KILL)
            .expect("zombie-style minion spawn should emit legacy updates");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "\n");
        assert_eq!(frame.updates.updates[1].message, "[0][召唤亡灵]");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 1);
        assert_eq!(frame.updates.updates[1].score, 60);
        assert_eq!(frame.updates.updates[1].delay0, 1500);
        assert_eq!(frame.updates.updates[2].message, "[2]变成了[1]");
        assert_eq!(frame.updates.updates[2].caster, 0);
        assert_eq!(frame.updates.updates[2].target, 2);
        assert_eq!(frame.updates.updates[2].targets.as_slice(), &[1]);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(1))
        );
        let zombie = runtime.entities.get(EntityIdx(2)).expect("zombie minion should spawn");
        assert_eq!(zombie.template.name, "owner?0");
        assert_eq!(zombie.template.kind, zombie_kind);
        assert_eq!(zombie.runtime.owner, EntityIdx(0));
        assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
        assert_eq!(zombie.runtime.move_state, MoveState { speed_points: 1020 });
    }

    #[test]
    fn configured_minion_handlers_use_non_default_slots_and_targets() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .reserve_entity_slot("custom", "unused-counter", "custom.minion.unused_counter")
            .expect("unused counter slot should reserve");
        let counter_slot = builder
            .reserve_entity_slot("custom", "configured-counter", "custom.minion.configured_counter")
            .expect("configured counter slot should reserve");
        builder
            .reserve_template_slot("custom", "unused-template", "custom.minion.unused_template")
            .expect("unused template slot should reserve");
        let shadow_template_slot = builder
            .reserve_template_slot(
                "custom",
                "configured-shadow-template",
                "custom.minion.configured_shadow_template",
            )
            .expect("configured shadow template slot should reserve");
        let zombie_template_slot = builder
            .reserve_template_slot(
                "custom",
                "configured-zombie-template",
                "custom.minion.configured_zombie_template",
            )
            .expect("configured zombie template slot should reserve");
        let shadow_skill = builder
            .register_skill_with_hooks(
                "custom",
                "configured-shadow",
                "custom.configured_shadow",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("configured shadow skill should register");
        let zombie_skill = builder
            .register_skill_with_hooks(
                "custom",
                "configured-zombie",
                "custom.configured_zombie",
                ProcMask::KILL,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("configured zombie skill should register");
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "configured-minion",
                "custom.configured_minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("configured minion kind should register");
        let registry = builder.build();
        let shadow_payload = PlayerTemplate::with_kind(3, "shadow-template", minion_kind, 0, 5, 1);
        let zombie_payload = PlayerTemplate::with_kind(4, "zombie-template", minion_kind, 0, 6, 1);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([shadow_skill, zombie_skill]),
                PlayerTemplate::new(2, "victim-a", 1, 10, 1),
                PlayerTemplate::new(3, "victim-b", 1, 10, 1),
            ],
            registry,
        );
        template
            .slots
            .set(shadow_template_slot, SlotValue::PlayerTemplate(Box::new(shadow_payload)))
            .expect("configured shadow template slot should write");
        template
            .slots
            .set(zombie_template_slot, SlotValue::PlayerTemplate(Box::new(zombie_payload)))
            .expect("configured zombie template slot should write");
        let mut runtime = CombatRuntime::from_template(template);
        runtime.set_skill_handler_with_capabilities(
            shadow_skill,
            skill_configured_shadow_minion_handler,
            &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
        );
        runtime.set_skill_handler_with_capabilities(
            zombie_skill,
            skill_configured_zombie_minion_handler,
            &[ExtensionCapability::ReadTemplateSlots, ExtensionCapability::MutateEntitySlots],
        );

        let shadow_frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("configured shadow minion should emit updates");

        assert_eq!(shadow_frame.updates.updates[0].message, "[0]使用[幻术]");
        assert_eq!(shadow_frame.updates.updates[1].target, 3);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().slots.get(EntitySlotId(0)), None);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(1))
        );
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.name, "owner?0");

        let zombie_frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::KILL)
            .expect("configured zombie minion should emit updates");

        assert_eq!(zombie_frame.updates.updates[1].message, "[0][召唤亡灵]");
        assert_eq!(zombie_frame.updates.updates[1].target, 2);
        assert_eq!(zombie_frame.updates.updates[2].message, "[2]变成了[1]");
        assert_eq!(zombie_frame.updates.updates[2].target, 4);
        assert_eq!(zombie_frame.updates.updates[2].targets.as_slice(), &[2]);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(2))
        );
        assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().template.name, "owner?1");
    }

    #[test]
    fn minion_display_index_for_entity_matches_legacy_name_suffix() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3),
                PlayerTemplate::with_kind(2, "owner?0", minion_kind, 0, 5, 1),
                PlayerTemplate::with_kind(3, "owner?12", minion_kind, 0, 5, 1),
                PlayerTemplate::with_kind(4, "owner?shadow", minion_kind, 0, 5, 1),
                PlayerTemplate::with_kind(5, "shadow", minion_kind, 0, 5, 1),
            ],
            registry,
        ));

        assert_eq!(minion_display_index_for_entity(None), 0);
        assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(0))), 0);
        assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(1))), 1);
        assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(2))), 13);
        assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(3))), 1);
        assert_eq!(minion_display_index_for_entity(runtime.entities.get(EntityIdx(4))), 1);
    }

    #[test]
    fn custom_minion_heal_fixture_does_not_share_with_owner_or_summons() {
        let mut builder = ExtensionRegistryBuilder::default();
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion-owner",
                "custom.minion_owner",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3),
                PlayerTemplate::new(2, "healer", 0, 10, 1),
                PlayerTemplate::new(3, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "summon-a", summon_kind, 0, 10, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(5, "summon-b", summon_kind, 0, 10, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(2),
            target: EntityIdx(0),
            amount: 4,
        });
        let shared_damage = runtime.flush_effects().expect("owner damage should share to minions");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
        assert_eq!(shared_damage.updates.updates.len(), 3);
        assert_eq!(shared_damage.updates.updates[0].target, 0);
        assert_eq!(shared_damage.updates.updates[1].target, 3);
        assert_eq!(shared_damage.updates.updates[2].target, 4);

        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(1),
            target: EntityIdx(3),
            amount: 3,
        });
        let minion_heal = runtime.flush_effects().expect("minion heal should emit one update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 16);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 9);
        assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.hp, 6);
        assert_eq!(minion_heal.updates.updates.len(), 1);
        assert_eq!(minion_heal.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(minion_heal.updates.updates[0].target, 3);
        assert_eq!(minion_heal.updates.updates[0].score, 3);
    }

    #[test]
    fn custom_minion_owner_death_removes_linked_minions_in_entity_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");

        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 10,
        });
        let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].target, 3);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    }

    #[test]
    fn custom_minion_owner_remove_cleans_linked_minions_in_entity_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");

        runtime.effects.push(QueuedEffect::Remove {
            caster: EntityIdx(1),
            target: EntityIdx(0),
        });
        let frame = runtime.flush_effects().expect("owner remove should cleanup linked minions");

        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(2)).unwrap().runtime.alive);
        assert!(!runtime.entities.get(EntityIdx(3)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.hp, 0);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].message, "[1]消失了");
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].target, 3);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
    }

    #[test]
    fn custom_runner_fixture_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let owner_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-owner",
                "custom.runner_owner",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToSummons,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("runner owner kind should register");
        let summon_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-summon",
                "custom.runner_summon",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: true,
                },
            )
            .expect("runner summon kind should register");
        let hp_marker = builder
            .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
            .expect("hp marker slot should reserve");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "owner", owner_kind, 0, 20, 3).with_def_res(77, 88),
                PlayerTemplate::new(2, "healer", 0, 10, 1),
                PlayerTemplate::new(3, "enemy", 1, 10, 1),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "summon", summon_kind, 0, 10, 1).with_def_res(11, 22),
        });
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(2),
            target: EntityIdx(0),
            amount: 4,
        });
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(1),
            target: EntityIdx(3),
            amount: 2,
        });
        runtime.effects.push(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(0),
            message: "[0]还剩[2]点血".to_owned(),
            score: 87,
        });

        let frame = runtime.flush_effects().expect("custom runner fixture should emit updates");
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(hp_marker, SlotValue::Bool(true))
            .expect("hp marker slot should write");
        let outcome = RoundOutcome {
            action: None,
            frame: Some(frame),
            winner_team: runtime.world.sync_winner(&runtime.entities),
        };
        let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
        let expected = NormalizedOutcome {
            winner_team: None,
            round: 0,
            total_score: 97,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2, 3, 4],
            teams: vec![0, 0, 1, 0],
            hp: vec![16, 10, 10, 8],
            magic_point: vec![0, 0, 0, 0],
            defense: vec![77, 0, 0, 77],
            resistance: vec![88, 0, 0, 88],
            alive: vec![true, true, true, true],
            round_order: vec![0, 1, 2, 3],
            flat_alive: vec![0, 1, 3, 2],
            team_alive: vec![vec![0, 1, 3], vec![2]],
            alive_group_count: 2,
            actions: Vec::new(),
            frames: vec![
                NormalizedUpdateFrame {
                    message: "出现一个新的[1]".to_owned(),
                    caster: 0,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 2,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 4,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 2,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 4,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[1]回复体力[2]点".to_owned(),
                    caster: 1,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 2,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]还剩[2]点血".to_owned(),
                    caster: 0,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 87,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
            ],
        };

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(hp_marker),
            Some(&SlotValue::Bool(true))
        );
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.defense, 77);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().template.resistance, 88);
        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn custom_runner_minion_owner_death_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-linked-minion",
                "custom.runner_linked_minion",
                PlayerKindFlags::MINION | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("runner minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 10, 3),
                PlayerTemplate::new(2, "enemy", 1, 10, 1),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(3, "owner?0", minion_kind, 0, 4, 1),
        });
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::with_kind(4, "owner?1", minion_kind, 0, 4, 1),
        });
        runtime.flush_effects().expect("minion spawns should emit updates");
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(1),
            target: EntityIdx(0),
            amount: 10,
        });

        let frame = runtime.flush_effects().expect("owner death should cleanup linked minions");
        let outcome = RoundOutcome {
            action: None,
            frame: Some(frame),
            winner_team: runtime.world.sync_winner(&runtime.entities),
        };
        let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
        let expected = NormalizedOutcome {
            winner_team: Some(1),
            round: 0,
            total_score: 10,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2, 3, 4],
            teams: vec![0, 1, 0, 0],
            hp: vec![0, 10, 0, 0],
            magic_point: vec![0, 0, 0, 0],
            defense: vec![0, 0, 0, 0],
            resistance: vec![0, 0, 0, 0],
            alive: vec![false, true, false, false],
            round_order: vec![0, 1],
            flat_alive: vec![1],
            team_alive: vec![Vec::new(), vec![1]],
            alive_group_count: 1,
            actions: Vec::new(),
            frames: vec![
                NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 1,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 10,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[1]消失了".to_owned(),
                    caster: 0,
                    target: 2,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[1]消失了".to_owned(),
                    caster: 0,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
            ],
        };

        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    #[test]
    fn custom_runner_merge_matches_strict_diff_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill_a = builder
            .register_skill("custom", "runner-a", "custom.runner_a", TargetPolicy::Enemy, SkillPriority(0))
            .expect("runner skill should register");
        let skill_b = builder
            .register_skill("custom", "runner-b", "custom.runner_b", TargetPolicy::Enemy, SkillPriority(1))
            .expect("runner skill should register");
        let skill_c = builder
            .register_skill("custom", "runner-c", "custom.runner_c", TargetPolicy::Enemy, SkillPriority(2))
            .expect("runner skill should register");
        let merge_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "runner-merge",
                "custom.runner_merge",
                PlayerKindFlags::NONE,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("runner merge kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "merge-owner", merge_kind, 0, 10, 3).with_skills([skill_a]),
                PlayerTemplate::new(2, "merge-target", 1, 10, 3).with_skills([skill_b, skill_c]),
            ],
            registry,
        ));

        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("runner merge should emit updates");
        let outcome = RoundOutcome {
            action: None,
            frame: Some(frame),
            winner_team: runtime.world.sync_winner(&runtime.entities),
        };
        let actual = NormalizedOutcome::from_runtime(&runtime, &outcome);
        let expected = NormalizedOutcome {
            winner_team: None,
            round: 0,
            total_score: 60,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: vec![10, 10],
            magic_point: vec![0, 0],
            defense: vec![0, 0],
            resistance: vec![0, 0],
            alive: vec![true, true],
            round_order: vec![0, 1],
            flat_alive: vec![0, 1],
            team_alive: vec![vec![0], vec![1]],
            alive_group_count: 2,
            actions: Vec::new(),
            frames: vec![
                NormalizedUpdateFrame {
                    message: "[0][吞噬]了[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 60,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "[0]属性上升".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
            ],
        };

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(),
            &[skill_b, skill_c]
        );
        assert_eq!(strict_diff(&expected, &actual), Ok(()));
    }

    fn custom_marks_update(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Text(message) = &effect.payload else {
            panic!("custom test effect expects text payload");
        };
        context.add_update(crate::engine::update::RunUpdate::new(
            message.clone(),
            effect.caster.0 as usize,
            effect.target.unwrap().0 as usize,
            0,
        ));
    }

    fn custom_spawns_nested_damage(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(amount) = effect.payload else {
            panic!("custom test effect expects int payload");
        };
        context.push_nested(QueuedEffect::Damage {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
        });
    }

    fn custom_spawns_nested_heal(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(amount) = effect.payload else {
            panic!("custom test effect expects int payload");
        };
        context.push_nested(QueuedEffect::Heal {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
        });
    }

    fn custom_rejects_cross_entity_read(context: &mut EffectContext<'_>, _: &CustomEffect) {
        assert_eq!(
            context.entity(EntityIdx(2)),
            Err(EffectContextError::MissingCapability(ExtensionCapability::ReadEnemies))
        );
        context.add_update(crate::engine::update::RunUpdate::new("read denied", 0, 0, 0));
    }

    fn custom_reads_cross_entity(context: &mut EffectContext<'_>, _: &CustomEffect) {
        let observed = context.entity(EntityIdx(2)).expect("capability should allow cross-entity read");
        context.add_update(crate::engine::update::RunUpdate::new(observed.template.name.clone(), 0, 2, 0));
    }

    fn custom_mutates_entity_slot(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(slot) = effect.payload else {
            panic!("custom test effect expects entity slot id payload");
        };
        context
            .set_entity_slot(
                effect.target.expect("custom test effect needs target"),
                EntitySlotId(slot as u32),
                SlotValue::Bool(true),
            )
            .expect("capability should allow entity slot mutation");
        context.add_update(crate::engine::update::RunUpdate::new("slot set", 0, 0, 0));
    }

    fn custom_consumes_rng(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(max) = effect.payload else {
            panic!("custom test effect expects rng max payload");
        };
        let value = context.rng_next_i32(max);
        let next_byte = context.rng_next_u8();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("rng:{value}:{next_byte}"),
            effect.caster.0 as usize,
            effect.target.unwrap().0 as usize,
            value as u32,
        ));
    }

    fn skill_marks_update(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "skill mark",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.skill_id.0,
        ));
    }

    fn state_marks_charge_boost(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        let owner = context.owner().expect("state owner should exist");
        let message = if owner.runtime.charge.active && owner.runtime.at_boost_millionths == 3_000_000 {
            "charge boosted"
        } else {
            "charge inactive"
        };
        context.add_update(crate::engine::update::RunUpdate::new(
            message,
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.legacy_order_key,
        ));
    }

    fn skill_clears_positive_runtime(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let messages = context
            .clear_owner_positive_runtime_messages()
            .expect("clear-positive owner should exist");
        let owner = context.owner_idx();
        for (priority, message) in messages {
            context.add_update(crate::engine::update::RunUpdate::new(
                message,
                owner.0 as usize,
                owner.0 as usize,
                priority as u32,
            ));
        }
    }

    fn skill_clears_positive_states(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let messages = context.clear_owner_positive_state_messages().expect("clear-positive owner should exist");
        let owner = context.owner_idx();
        for (priority, message) in messages {
            context.add_update(crate::engine::update::RunUpdate::new(
                message,
                owner.0 as usize,
                owner.0 as usize,
                priority as u32,
            ));
        }
    }

    fn skill_clears_positive(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let messages = context.clear_owner_positive_messages().expect("clear-positive owner should exist");
        let owner = context.owner_idx();
        for (priority, message) in messages {
            context.add_update(crate::engine::update::RunUpdate::new(
                message,
                owner.0 as usize,
                owner.0 as usize,
                priority as u32,
            ));
        }
    }

    fn skill_noop(_: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {}

    fn skill_pushes_nested_damage(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        context.push_nested(QueuedEffect::Damage {
            caster: context.owner_idx(),
            target: EntityIdx(1),
            amount: 2,
        });
    }

    fn skill_halves_defend_atp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        let atp = context.defend_atp().expect("pre-defend skill should receive atp");
        context.add_update(crate::engine::update::RunUpdate::new(
            "pre defend skill",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            atp as u32,
        ));
        context.set_defend_atp(atp / 2.0);
    }

    fn skill_zeroes_defend_atp(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        assert!(context.defend_atp().expect("pre-defend skill should receive atp") > 0.0);
        context.add_update(crate::engine::update::RunUpdate::new(
            "pre defend zero",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.skill_id.0,
        ));
        context.set_defend_atp(0.0);
    }

    fn skill_marks_defend_replay(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let caster = context.defend_caster().expect("post-defend skill should receive incoming caster");
        let target = context.defend_target().expect("post-defend skill should receive incoming target");
        context.add_update(crate::engine::update::RunUpdate::new(
            "[0][防御]",
            target.0 as usize,
            caster.0 as usize,
            0,
        ));
    }

    fn skill_halves_defend_damage(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        let damage = context.defend_damage().expect("post-defend skill should receive damage");
        context.add_update(crate::engine::update::RunUpdate::new(
            "post defend skill",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            damage as u32,
        ));
        context.set_defend_damage(damage / 2);
    }

    fn skill_bed2_template_slot_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        push_summon_from_template_slot(context, TemplateSlotId(0))
            .expect("bed2 summon handler should read template slot payload");
    }

    fn skill_bed2_template_slot_legacy_summon_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        push_summon_from_template_slot_with_message(context, TemplateSlotId(0), "召唤出[1]")
            .expect("bed2 summon handler should read template slot payload");
    }

    fn skill_records_missing_template_slot_error(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        assert_eq!(
            push_summon_from_template_slot(context, TemplateSlotId(0)),
            Err(RuntimeV2SummonHandlerError::MissingTemplateSlot(TemplateSlotId(0)))
        );
        context.add_update(crate::engine::update::RunUpdate::new(
            "missing summon template",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            0,
        ));
    }

    fn skill_consumes_rng(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        let value = context.rng_next_i32(10);
        let next_byte = context.rng_next_u8();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("skill-rng:{value}:{next_byte}"),
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            value as u32,
        ));
    }

    fn skill_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
            .with_def_res(11, 22)
            .with_skills([SkillId(0)]);
        push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10)
            .expect("summon recast fixture should spawn or revive summon");
    }

    fn skill_legacy_summon_recast_fixture_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "[0]使用[血祭]",
            context.owner_idx().0 as usize,
            context.owner_idx().0 as usize,
            60,
        ));
        let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
            .with_def_res(11, 22)
            .with_skills([SkillId(0)]);
        push_summon_recast_from_entity_slot_with_messages(
            context,
            EntitySlotId(0),
            summon_template,
            10,
            "召唤出[1]",
            "召唤出[1]",
        )
        .expect("legacy summon recast fixture should spawn or revive summon");
    }

    fn skill_configured_summon_recast_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        run_legacy_summon_recast_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(1), 7);
    }

    fn skill_records_alive_summon_recast_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
            .with_def_res(11, 22)
            .with_skills([SkillId(0)]);
        assert_eq!(
            push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10),
            Err(RuntimeV2SummonHandlerError::RememberedSummonAlive(EntityIdx(2)))
        );
    }

    fn skill_records_missing_recast_read_allies_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let summon_template = PlayerTemplate::with_kind(3, "summon", PlayerKindId(1), 0, 10, 1)
            .with_def_res(11, 22)
            .with_skills([SkillId(0)]);
        assert_eq!(
            push_summon_recast_from_entity_slot(context, EntitySlotId(0), summon_template, 10),
            Err(RuntimeV2SummonHandlerError::Context(EffectContextError::MissingCapability(
                ExtensionCapability::ReadAllies
            )))
        );
    }

    fn skill_records_next_minion_name(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let name =
            next_minion_name_from_entity_slot(context, EntitySlotId(0)).expect("minion name helper should allocate a name");
        context.add_update(crate::engine::update::RunUpdate::new(
            name,
            context.owner_idx().0 as usize,
            context.owner_idx().0 as usize,
            0,
        ));
    }

    fn skill_records_missing_minion_name_read_allies_error(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        assert_eq!(
            next_minion_name_from_entity_slot(context, EntitySlotId(0)),
            Err(RuntimeV2MinionHandlerError::Context(EffectContextError::MissingCapability(
                ExtensionCapability::ReadAllies
            )))
        );
    }

    fn skill_pushes_named_minion_spawn(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        let minion_template = PlayerTemplate::with_kind(3, "placeholder", PlayerKindId(0), 0, 5, 1);
        assert_eq!(
            push_minion_from_template_with_allocated_name(context, EntitySlotId(0), minion_template, "召唤出[1]"),
            Ok(EntityIdx(2))
        );
    }

    fn skill_pushes_named_minion_from_template_slot(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        assert_eq!(
            push_minion_from_template_slot_with_allocated_name(context, EntitySlotId(0), TemplateSlotId(0), "召唤出[1]"),
            Ok(EntityIdx(2))
        );
    }

    fn skill_configured_shadow_minion_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        run_shadow_minion_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(1));
    }

    fn skill_configured_zombie_minion_handler(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        run_zombie_minion_from_template_slot_with_config(context, EntitySlotId(1), TemplateSlotId(2), EntityIdx(2));
    }

    fn state_marks_update(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "state mark",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.legacy_order_key,
        ));
    }

    fn state_pushes_nested_heal(context: &mut StateContext<'_>, _: &StateHookPlanEntry) {
        context.push_nested(QueuedEffect::Heal {
            caster: context.owner_idx(),
            target: context.owner_idx(),
            amount: 2,
        });
    }

    fn state_consumes_rng(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        let value = context.rng_next_i32(10);
        let next_byte = context.rng_next_u8();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("state-rng:{value}:{next_byte}"),
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            value as u32,
        ));
    }

    fn state_adds_defend_damage(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        let damage = context.defend_damage().expect("post-defend state should receive damage");
        context.add_update(crate::engine::update::RunUpdate::new(
            "post defend state",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.legacy_order_key,
        ));
        context.set_defend_damage(damage + 3);
    }

    fn render_first_message_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
        Some(RenderedReplay::new(
            ReplayRendererId(0),
            frame.updates.updates.first()?.message.to_string(),
        ))
    }

    fn render_update_count_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
        Some(RenderedReplay::new(
            ReplayRendererId(1),
            frame.updates.updates.len().to_string(),
        ))
    }

    fn render_first_message_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
        Some(RenderedShow::new(
            ShowRendererId(0),
            frame.updates.updates.first()?.message.to_string(),
        ))
    }

    fn render_hp_marker_bar_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
        let hp_report = frame.updates.updates.iter().find(|update| update.message == "[0]还剩[2]点血")?;
        Some(RenderedShow::new(
            ShowRendererId(0),
            format!(
                "hp-bar:actor={}:value={}:text={}",
                hp_report.caster,
                hp_report.param.unwrap_or(hp_report.score),
                hp_report.msg()
            ),
        ))
    }

    fn mixed_raw_runner_for_plain_fixture(raw_input: &str) -> (RuntimeV2Runner, crate::Runner) {
        let mut builder = ExtensionRegistryBuilder::default();
        let summon = builder
            .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
            .expect("summon skill should register");
        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("bed2 kind should register");
        let registry = builder.build();
        let runner = RuntimeV2Runner::from_mixed_namerena_raw(raw_input.to_owned(), registry, bed2, summon)
            .expect("plain raw fixture should construct runtime v2 runner");
        let legacy =
            crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("plain raw fixture should construct legacy runner");
        (runner, legacy)
    }

    fn assert_runtime_rng_matches_legacy(runtime: &CombatRuntime, legacy: &crate::Runner) {
        assert_eq!(runtime.rng.i, legacy.randomer.i);
        assert_eq!(runtime.rng.j, legacy.randomer.j);
        assert_eq!(runtime.rng.main_val, legacy.randomer.main_val);
    }

    fn assert_runtime_world_matches_legacy_raw_world(
        runtime: &CombatRuntime,
        legacy_world: &crate::engine::world_state::WorldState,
    ) {
        assert_eq!(
            runtime.world.round_order(),
            legacy_entity_order(&legacy_world.players).as_slice()
        );
        assert_eq!(
            runtime.world.flat_alive(),
            legacy_entity_order(&legacy_world.flat_alive).as_slice()
        );
        assert_eq!(runtime.world.alive_group_count(), legacy_world.alive_group_count());
        for team_idx in 0..legacy_world.groups.len() {
            assert_eq!(
                runtime.world.team_alive(team_idx),
                Some(legacy_entity_order(legacy_world.team_alive(team_idx).unwrap_or_default()).as_slice())
            );
        }
        for (team_idx, group) in legacy_world.groups.iter().enumerate() {
            for plr in group {
                let entity = runtime
                    .entities
                    .get(EntityIdx(
                        (*plr).try_into().expect("legacy fixture player id should fit EntityIdx"),
                    ))
                    .expect("legacy raw world player should exist in runtime_v2");
                assert_eq!(entity.runtime.team, team_idx);
                assert_eq!(entity.template.team, team_idx);
            }
        }
    }

    fn legacy_entity_order(plrs: &[crate::player::PlrId]) -> Vec<EntityIdx> {
        plrs.iter()
            .copied()
            .map(|plr| EntityIdx(plr.try_into().expect("legacy fixture player id should fit EntityIdx")))
            .collect()
    }

    #[test]
    fn run_skill_hooks_dispatches_registered_skill_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_skill_with_hooks(
                "custom",
                "marker",
                "custom.marker",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker])],
            registry,
        ));
        runtime.set_skill_handler(marker, skill_marks_update);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("skill handler should emit update");

        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[0].score, marker.0);
    }

    #[test]
    fn run_skill_hooks_exposes_controlled_rng_to_skill_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "rng-skill",
                "custom.rng_skill",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill])],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_consumes_rng);
        let mut expected_rng = RC4::default();
        let expected_value = expected_rng.next_i32(10);
        let expected_byte = expected_rng.next_u8();

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("skill rng handler should emit update");

        assert_eq!(
            frame.updates.updates[0].message,
            format!("skill-rng:{expected_value}:{expected_byte}")
        );
        assert_eq!(frame.updates.updates[0].score, expected_value as u32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn run_skill_hooks_flushes_nested_effects() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "damage",
                "custom.damage",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_pushes_nested_damage);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("nested damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 8);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].score, 2);
    }

    #[test]
    fn run_skill_hooks_disperse_without_selected_target_noops() {
        let mut builder = ExtensionRegistryBuilder::default();
        let disperse = builder
            .register_skill_with_hooks(
                "core",
                "disperse",
                "core.disperse",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("disperse skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([disperse]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(disperse, run_disperse_skill);

        let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
    }

    #[test]
    fn run_minimal_round_dispatches_pre_action_skill_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_skill_with_hooks(
                "custom",
                "marker",
                "custom.marker",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(marker, skill_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("skill plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_pre_action_skill_effect_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "damage",
                "custom.damage",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_pushes_nested_damage);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("skill damage plus attack should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 5);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].score, 2);
        assert_eq!(frame.updates.updates[1].score, 3);
    }

    #[test]
    fn run_minimal_round_disperse_skill_uses_selected_target_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let disperse = builder
            .register_skill_with_hooks(
                "core",
                "disperse",
                "core.disperse",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("disperse skill should register");
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3)
                    .with_magic(80)
                    .with_wisdom(64)
                    .with_skills([disperse]),
                PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16).with_magic_point(96),
            ],
            registry,
        ));
        runtime.set_skill_handler(disperse, run_disperse_skill);
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        let mut expected_rng = RC4::default();
        let _smart_byte = expected_rng.next_u8();
        let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_disperse_damage =
            (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("disperse plus attack should emit update");

        assert_eq!(outcome.action.unwrap().target, EntityIdx(1));
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp,
            1_000 - expected_disperse_damage - 3
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, 32);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| update.message.as_ref())
                .collect::<Vec<_>>(),
            vec!["[0]使用[净化]", "[1]受到[2]点伤害", "[1]从[疾走]中解除", "[0]攻击[1]"]
        );
        assert_eq!(frame.updates.updates.last().unwrap().score, 3);
    }

    #[test]
    fn run_minimal_round_disperse_skill_scores_multiple_enemy_targets() {
        let mut builder = ExtensionRegistryBuilder::default();
        let disperse = builder
            .register_skill_with_hooks(
                "core",
                "disperse",
                "core.disperse",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("disperse skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 0)
                    .with_magic(80)
                    .with_wisdom(64)
                    .with_skills([disperse]),
                PlayerTemplate::new(2, "first", 1, 200, 0)
                    .with_def_res(0, 16)
                    .with_magic_point(96)
                    .with_target_score_stats(0, 30, 1.0),
                PlayerTemplate::new(3, "best", 1, 200, 0)
                    .with_def_res(0, 16)
                    .with_magic_point(96)
                    .with_target_score_stats(0, 300, 1.0),
                PlayerTemplate::new(4, "also-picked", 1, 200, 0)
                    .with_def_res(0, 16)
                    .with_magic_point(96)
                    .with_target_score_stats(0, 60, 1.0),
            ],
            registry,
        ));
        runtime.set_skill_handler(disperse, run_disperse_skill);
        let mut expected_rng = RC4::default();
        let _smart_byte = expected_rng.next_u8();
        let selected_targets = select_disperse_targets(&runtime.entities, &runtime.world, EntityIdx(0), true, &mut expected_rng);
        let selected_target = selected_targets[0];
        assert_eq!(selected_target, EntityIdx(2));
        let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(selected_target).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_disperse_damage =
            (atp / runtime.entities.get(selected_target).unwrap().runtime.magic_defense() as f64).ceil() as i32;

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("disperse should emit update");

        assert_eq!(outcome.action.unwrap().target, EntityIdx(1));
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 200);
        assert_eq!(
            runtime.entities.get(selected_target).unwrap().runtime.hp,
            200 - expected_disperse_damage
        );
        assert_eq!(runtime.entities.get(selected_target).unwrap().runtime.magic_point, 32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| (update.message.as_ref(), update.target))
                .collect::<Vec<_>>(),
            vec![("[0]使用[净化]", 2), ("[1]受到[2]点伤害[s_dmg120]", 2), ("[0]攻击[1]", 1)]
        );
    }

    #[test]
    fn score_disperse_target_matches_legacy_smart_two_team_formula() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::new(2, "target", 1, 80, 3).with_target_score_stats(77, 120, 2.5),
        ]));
        let mut rng = RC4::default();

        let score = score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), true, &mut rng);

        assert_eq!(score, (1.0 / 80.0) * 120.0 * 2.5);
        let expected_rng = RC4::default();
        assert_eq!(rng.i, expected_rng.i);
        assert_eq!(rng.j, expected_rng.j);
        assert_eq!(rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn score_disperse_target_matches_legacy_smart_multi_team_and_minion_formula() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies::default(),
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3),
                PlayerTemplate::with_kind(2, "target", minion_kind, 1, 400, 3).with_target_score_stats(77, 120, 2.5),
                PlayerTemplate::new(3, "team-2", 2, 10, 3),
                PlayerTemplate::new(4, "team-1-ally", 1, 10, 3),
            ],
            registry,
        ));
        let mut rng = RC4::default();

        let score = score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), true, &mut rng);

        assert_eq!(score, 300.0 * 2.0 * 2.5 * 2.0);
        let expected_rng = RC4::default();
        assert_eq!(rng.i, expected_rng.i);
        assert_eq!(rng.j, expected_rng.j);
        assert_eq!(rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn score_disperse_target_matches_legacy_random_formula_and_unknown_target() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "caster", 0, 10, 3),
            PlayerTemplate::new(2, "target", 1, 80, 3).with_target_score_stats(77, 120, 2.5),
        ]));
        let mut rng = RC4::default();
        let mut expected_rng = RC4::default();
        let expected = expected_rng.rFFFF() as f64 + 2.5;

        assert_eq!(
            score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(1), false, &mut rng),
            expected
        );
        assert_eq!(rng.i, expected_rng.i);
        assert_eq!(rng.j, expected_rng.j);
        assert_eq!(rng.main_val, expected_rng.main_val);
        assert_eq!(
            score_disperse_target(&runtime.entities, &runtime.world, EntityIdx(99), false, &mut rng),
            f64::MIN
        );
    }

    #[test]
    fn run_minimal_round_dispatches_damage_skill_hooks_around_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_damage = builder
            .register_skill_with_hooks(
                "custom",
                "pre-damage",
                "custom.pre_damage",
                ProcMask::PRE_DAMAGE,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("pre-damage skill should register");
        let post_damage = builder
            .register_skill_with_hooks(
                "custom",
                "post-damage",
                "custom.post_damage",
                ProcMask::POST_DAMAGE,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("post-damage skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([pre_damage, post_damage]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(pre_damage, skill_marks_update);
        runtime.set_skill_handler(post_damage, skill_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("damage skill hooks plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[0].score, pre_damage.0);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].message, "skill mark");
        assert_eq!(frame.updates.updates[2].score, post_damage.0);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_dispatches_post_action_skill_before_state() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "post-action-skill",
                "custom.post_action_skill",
                ProcMask::POST_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("post-action skill should register");
        let state = builder
            .register_state(
                "custom",
                "post-action-state",
                "custom.post_action_state",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_skill_handler(skill, skill_marks_update);
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-action skill and state plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "skill mark");
        assert_eq!(frame.updates.updates[1].score, skill.0);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 55);
    }

    #[test]
    fn run_minimal_round_dispatches_late_post_action_skill_after_state() {
        let mut builder = ExtensionRegistryBuilder::default();
        let early = builder
            .register_skill_with_hooks(
                "custom",
                "early-post-action",
                "custom.early_post_action",
                ProcMask::POST_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("early post-action skill should register");
        let late = builder
            .register_skill_with_hooks_and_post_action_phase(
                "custom",
                "late-post-action",
                "custom.late_post_action",
                ProcMask::POST_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
                SkillPostActionPhase::Late,
            )
            .expect("late post-action skill should register");
        let state = builder
            .register_state(
                "custom",
                "post-action-state",
                "custom.post_action_state",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([late, early]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_skill_handler(early, skill_marks_update);
        runtime.set_skill_handler(late, skill_marks_update);
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-action hooks plus attack should emit update");

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![
                ("[0]攻击[1]", 3),
                ("skill mark", early.0),
                ("state mark", 55),
                ("skill mark", late.0),
            ]
        );
    }

    #[test]
    fn run_minimal_round_dispatches_post_action_state_after_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack plus state hook should emit update");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 77);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_post_action_state_effect_after_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 88,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(state, state_pushes_nested_heal);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack plus state heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[1].score, 2);
    }

    #[test]
    fn run_minimal_round_dispatches_damage_state_hooks_around_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_damage = builder
            .register_state(
                "custom",
                "pre-damage",
                "custom.pre_damage",
                ProcMask::PRE_DAMAGE,
                SkillPriority(0),
            )
            .expect("pre-damage state should register");
        let post_damage = builder
            .register_state(
                "custom",
                "post-damage",
                "custom.post_damage",
                ProcMask::POST_DAMAGE,
                SkillPriority(0),
            )
            .expect("post-damage state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 11,
                extension_state_id: Some(pre_damage),
                hook_mask: ProcMask::PRE_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
                payload: StatePayload::None,
            });
            store.add_entry(StateEntry {
                legacy_order_key: 22,
                extension_state_id: Some(post_damage),
                hook_mask: ProcMask::POST_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
                payload: StatePayload::None,
            });
        }
        runtime.set_state_handler(pre_damage, state_marks_update);
        runtime.set_state_handler(post_damage, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("damage state hooks plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(frame.updates.updates[0].score, 11);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 22);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_post_damage_state_effect_before_post_action() {
        let mut builder = ExtensionRegistryBuilder::default();
        let post_damage = builder
            .register_state(
                "custom",
                "post-damage-regen",
                "custom.post_damage_regen",
                ProcMask::POST_DAMAGE,
                SkillPriority(0),
            )
            .expect("post-damage state should register");
        let post_action = builder
            .register_state(
                "custom",
                "post-action-marker",
                "custom.post_action_marker",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 33,
                extension_state_id: Some(post_damage),
                hook_mask: ProcMask::POST_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
                payload: StatePayload::None,
            });
            store.add_entry(StateEntry {
                legacy_order_key: 44,
                extension_state_id: Some(post_action),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
                payload: StatePayload::None,
            });
        }
        runtime.set_state_handler(post_damage, state_pushes_nested_heal);
        runtime.set_state_handler(post_action, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-damage effect plus post-action state should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 44);
    }

    #[test]
    fn run_state_hooks_dispatches_registered_state_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(state, state_marks_update);

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state handler should emit update");

        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(frame.updates.updates[0].score, 42);
    }

    #[test]
    fn run_state_hooks_exposes_controlled_rng_to_state_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state(
                "custom",
                "rng-state",
                "custom.rng_state",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 88,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        });
        runtime.set_state_handler(state, state_consumes_rng);
        let mut expected_rng = RC4::default();
        let expected_value = expected_rng.next_i32(10);
        let expected_byte = expected_rng.next_u8();

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state rng handler should emit update");

        assert_eq!(
            frame.updates.updates[0].message,
            format!("state-rng:{expected_value}:{expected_byte}")
        );
        assert_eq!(frame.updates.updates[0].score, expected_value as u32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn run_state_hooks_flushes_nested_effects_and_skips_legacy_entries() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_legacy_key(11);
            store.add_entry(StateEntry {
                legacy_order_key: 22,
                extension_state_id: Some(state),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
                payload: StatePayload::None,
            });
        }
        runtime.set_state_handler(state, state_pushes_nested_heal);

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[0].score, 2);
    }

    #[test]
    fn run_state_hooks_poison_post_action_ticks_damage_and_keeps_state() {
        let mut builder = ExtensionRegistryBuilder::default();
        let poison_state = builder
            .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
            .expect("poison state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 40, 3).with_magic(16),
                PlayerTemplate::new(2, "right", 1, 40, 3),
            ],
            registry,
        ));
        runtime.set_state_handler(poison_state, run_poison_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(1),
            Some(0),
            160.0,
            4,
            SkillPriority(150),
        ));

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("poison tick should emit updates");

        let remaining_atp = 160.0 - (160.0 * (1.0 + 3.0 * 0.10000000149011612) / 4.0);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 39);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .states
                .entry(75)
                .and_then(StateEntry::poison_value),
            Some((Some(1), Some(0), remaining_atp, 3))
        );
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[1][毒性发作]");
        assert_eq!(frame.updates.updates[0].caster, 1);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
        assert_eq!(frame.updates.updates[1].caster, 1);
        assert_eq!(frame.updates.updates[1].target, 0);
        assert_eq!(frame.updates.updates[1].score, 1);
        assert_eq!(frame.updates.updates[1].delay0, 1002);
    }

    #[test]
    fn run_state_hooks_poison_post_action_clears_and_emits_release_after_tick() {
        let mut builder = ExtensionRegistryBuilder::default();
        let poison_state = builder
            .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
            .expect("poison state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 40, 3).with_magic(16),
                PlayerTemplate::new(2, "right", 1, 40, 3),
            ],
            registry,
        ));
        runtime.set_state_handler(poison_state, run_poison_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(1),
            Some(0),
            80.0,
            1,
            SkillPriority(150),
        ));

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("poison clear should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 39);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(75), None);
        assert_eq!(frame.updates.updates.len(), 4);
        assert_eq!(frame.updates.updates[0].message, "[1][毒性发作]");
        assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
        assert_eq!(
            frame.updates.updates[2].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[3].message, "[1]从[中毒]中解除");
        assert_eq!(frame.updates.updates[3].caster, 0);
        assert_eq!(frame.updates.updates[3].target, 0);
    }

    #[test]
    fn run_state_hooks_poison_post_action_clears_without_release_when_tick_kills() {
        let mut builder = ExtensionRegistryBuilder::default();
        let poison_state = builder
            .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
            .expect("poison state should register");
        let die_state = builder
            .register_state("custom", "die", "custom.die", ProcMask::DIE, SkillPriority(0))
            .expect("die state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 3, 3).with_magic(16),
                PlayerTemplate::new(2, "right", 1, 40, 3),
            ],
            registry,
        ));
        runtime.set_state_handler(poison_state, run_poison_post_action_state);
        runtime.set_state_handler(die_state, state_marks_update);
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry::poison(
                75,
                poison_state,
                Some(1),
                Some(0),
                240.0,
                1,
                SkillPriority(150),
            ));
            store.add_entry(StateEntry {
                legacy_order_key: 44,
                extension_state_id: Some(die_state),
                hook_mask: ProcMask::DIE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
                payload: StatePayload::None,
            });
        }

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("lethal poison tick should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 0);
        assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(75), None);
        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| update.message.as_ref())
                .collect::<Vec<_>>(),
            vec!["[1][毒性发作]", "[1]受到[2]点伤害", "state mark"]
        );
    }

    #[test]
    fn run_state_hooks_poison_post_action_skips_dead_owner_without_mutation() {
        let mut builder = ExtensionRegistryBuilder::default();
        let poison_state = builder
            .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(150))
            .expect("poison state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 40, 3).with_magic(16)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
        runtime.set_state_handler(poison_state, run_poison_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::poison(
            75,
            poison_state,
            Some(1),
            Some(0),
            160.0,
            4,
            SkillPriority(150),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .states
                .entry(75)
                .and_then(StateEntry::poison_value),
            Some((Some(1), Some(0), 160.0, 4))
        );
    }

    #[test]
    fn run_state_hooks_iron_post_action_decrements_step_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let iron_state = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
            registry,
        ));
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            300,
            3,
            SkillPriority(10),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .states
                .entry(79)
                .and_then(StateEntry::iron_value),
            Some((300, 2))
        );
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            2048
        );
    }

    #[test]
    fn run_state_hooks_haste_post_action_decrements_step_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let haste_state = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_state_handler(haste_state, run_haste_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
            77,
            haste_state,
            4,
            3,
            SkillPriority(100),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .states
                .entry(77)
                .and_then(StateEntry::haste_value),
            Some((4, 2))
        );
    }

    #[test]
    fn run_state_hooks_haste_post_action_clears_and_emits_release() {
        let mut builder = ExtensionRegistryBuilder::default();
        let haste_state = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_state_handler(haste_state, run_haste_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
            77,
            haste_state,
            2,
            1,
            SkillPriority(100),
        ));

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("haste release should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(77), None);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(
            frame.updates.updates[0].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[1].message, "[1]从[疾走]中解除");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 0);
    }

    #[test]
    fn run_state_hooks_haste_post_action_clears_dead_owner_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let haste_state = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
        runtime.set_state_handler(haste_state, run_haste_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::haste(
            77,
            haste_state,
            2,
            1,
            SkillPriority(100),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(77), None);
    }

    #[test]
    fn run_state_hooks_charm_post_action_decrements_step_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let charm_state = builder
            .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("charm state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_state_handler(charm_state, run_charm_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
            76,
            charm_state,
            7,
            Some(1),
            Some(2),
            Some(3),
            3,
            SkillPriority(100),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .states
                .entry(76)
                .and_then(StateEntry::charm_value),
            Some((7, Some(1), Some(2), Some(3), 2))
        );
    }

    #[test]
    fn run_state_hooks_charm_post_action_clears_and_emits_release() {
        let mut builder = ExtensionRegistryBuilder::default();
        let charm_state = builder
            .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("charm state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_state_handler(charm_state, run_charm_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
            76,
            charm_state,
            7,
            Some(1),
            Some(2),
            Some(3),
            1,
            SkillPriority(100),
        ));

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("charm release should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76), None);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(
            frame.updates.updates[0].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[1].message, "[1]从[魅惑]中解除");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 0);
    }

    #[test]
    fn run_state_hooks_charm_post_action_clears_dead_owner_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let charm_state = builder
            .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("charm state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
        runtime.set_state_handler(charm_state, run_charm_post_action_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::charm(
            76,
            charm_state,
            7,
            Some(1),
            Some(2),
            Some(3),
            1,
            SkillPriority(100),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(76), None);
    }

    #[test]
    fn run_state_hooks_slow_post_action_decrements_step_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let slow_state = builder
            .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("slow state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_state_handler(slow_state, run_slow_post_action_state);
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .states
            .add_entry(StateEntry::slow(78, slow_state, 2, SkillPriority(100)));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(0))
                .unwrap()
                .states
                .entry(78)
                .and_then(StateEntry::slow_value),
            Some(1)
        );
    }

    #[test]
    fn run_state_hooks_slow_post_action_clears_and_emits_release() {
        let mut builder = ExtensionRegistryBuilder::default();
        let slow_state = builder
            .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("slow state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_state_handler(slow_state, run_slow_post_action_state);
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .states
            .add_entry(StateEntry::slow(78, slow_state, 1, SkillPriority(100)));

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("slow release should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(78), None);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(
            frame.updates.updates[0].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[1].message, "[1]从[迟缓]中解除");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 0);
    }

    #[test]
    fn run_skill_hooks_charge_post_action_decrements_step_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let charge = builder
            .register_skill_with_hooks_and_post_action_phase(
                "core",
                "charge",
                "core.charge",
                ProcMask::POST_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
                SkillPostActionPhase::Late,
            )
            .expect("charge skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([charge])],
            registry,
        ));
        runtime.set_skill_handler(charge, run_charge_post_action_skill);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().activate_charge_runtime();

        let frame = runtime.run_skill_hooks(EntityIdx(0), ProcMask::POST_ACTION);
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();

        assert!(frame.is_none());
        assert_eq!(
            owner.runtime.charge,
            crate::runtime_v2::entity::ChargeRuntime {
                active: true,
                post_action_active: true,
                step: 1,
            }
        );
        assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);
    }

    #[test]
    fn run_minimal_round_charge_late_post_action_clears_after_state_hooks() {
        let mut builder = ExtensionRegistryBuilder::default();
        let charge = builder
            .register_skill_with_hooks_and_post_action_phase(
                "core",
                "charge",
                "core.charge",
                ProcMask::POST_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
                SkillPostActionPhase::Late,
            )
            .expect("charge skill should register");
        let state = builder
            .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([charge]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        {
            let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            owner.activate_charge_runtime();
            owner.runtime.charge.step = 1;
            owner.states.add_entry(StateEntry {
                legacy_order_key: 55,
                extension_state_id: Some(state),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
                payload: StatePayload::None,
            });
        }
        runtime.set_skill_handler(charge, run_charge_post_action_skill);
        runtime.set_state_handler(state, state_marks_charge_boost);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack and charge-observing state should emit updates");
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();

        assert_eq!(
            frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
            vec!["[0]攻击[1]", "charge boosted"]
        );
        assert_eq!(
            owner.runtime.charge,
            crate::runtime_v2::entity::ChargeRuntime {
                active: false,
                post_action_active: false,
                step: 0,
            }
        );
        assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
    }

    #[test]
    fn run_skill_hooks_accumulate_activates_runtime_and_boosts_move() {
        let mut builder = ExtensionRegistryBuilder::default();
        let accumulate = builder
            .register_skill_with_hooks(
                "core",
                "accumulate",
                "core.accumulate",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("accumulate skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(100).with_skills([accumulate])],
            registry,
        ));
        runtime.set_skill_handler(accumulate, run_accumulate_skill);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("accumulate act should emit updates");
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![("[0]开始[聚气]", 1), ("[0]攻击力上升", 0)]
        );
        assert!(owner.runtime.accumulate.active);
        assert_eq!(owner.runtime.accumulate.charge_bonus(), 0.0);
        assert_eq!(owner.runtime.move_state.speed_points, 500);
        assert_eq!(owner.runtime.at_boost_millionths, 1_700_000);
    }

    #[test]
    fn run_minimal_round_accumulate_uses_charge_bonus_until_late_charge_clear() {
        let mut builder = ExtensionRegistryBuilder::default();
        let accumulate = builder
            .register_skill_with_hooks(
                "core",
                "accumulate",
                "core.accumulate",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("accumulate skill should register");
        let charge = builder
            .register_skill_with_hooks_and_post_action_phase(
                "core",
                "charge",
                "core.charge",
                ProcMask::POST_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
                SkillPostActionPhase::Late,
            )
            .expect("charge skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3)
                    .with_speed_points(100)
                    .with_skills([accumulate, charge]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        {
            let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            owner.activate_charge_runtime();
            owner.runtime.charge.step = 1;
        }
        runtime.set_skill_handler(accumulate, run_accumulate_skill);
        runtime.set_skill_handler(charge, run_charge_post_action_skill);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("accumulate, attack, and charge tick should emit frame");
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();

        assert_eq!(
            frame.updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
            vec!["[0]开始[聚气]", "[0]攻击力上升", "[0]攻击[1]"]
        );
        assert!(owner.runtime.accumulate.active);
        assert_eq!(owner.runtime.accumulate.charge_bonus(), 1.0);
        assert_eq!(owner.runtime.move_state.speed_points, 1000);
        assert_eq!(owner.runtime.charge.active, false);
        assert_eq!(owner.runtime.at_boost_millionths, 2_700_000);
    }

    #[test]
    fn run_skill_hooks_clear_positive_runtime_orders_accumulate_before_charge() {
        let mut builder = ExtensionRegistryBuilder::default();
        let clear = builder
            .register_skill_with_hooks(
                "custom",
                "clear-positive",
                "custom.clear_positive",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("clear-positive skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
            registry,
        ));
        {
            let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            owner.activate_charge_runtime();
            owner.activate_accumulate_runtime();
        }
        runtime.set_skill_handler(clear, skill_clears_positive_runtime);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("clear-positive runtime should emit messages");
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![(("[1]的[聚气]被打消了"), 100), (("[1]的[蓄力]被中止了"), 200)]
        );
        assert!(!owner.runtime.accumulate.active);
        assert!(!owner.runtime.charge.active);
        assert_eq!(owner.runtime.accumulate.acc(), 1.600000023841858);
        assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
    }

    #[test]
    fn run_skill_hooks_clear_positive_states_removes_shield_and_orders_messages() {
        let mut builder = ExtensionRegistryBuilder::default();
        let clear = builder
            .register_skill_with_hooks(
                "custom",
                "clear-positive-states",
                "custom.clear_positive_states",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("clear-positive skill should register");
        let shield = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
            .expect("shield state should register");
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let iron = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
            registry,
        ));
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
            store.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
            store.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        }
        runtime.set_skill_handler(clear, skill_clears_positive_states);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("clear-positive state messages should emit");
        let store = &runtime.entities.get(EntityIdx(0)).unwrap().states;

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![("[1]从[疾走]中解除", 300), ("[1]的[铁壁]被打消了", 400)]
        );
        assert_eq!(store.entry(74), None);
        assert_eq!(store.entry(77), None);
        assert_eq!(store.entry(79), None);
    }

    #[test]
    fn run_skill_hooks_clear_positive_states_suppresses_dead_haste_message() {
        let mut builder = ExtensionRegistryBuilder::default();
        let clear = builder
            .register_skill_with_hooks(
                "custom",
                "clear-positive-states",
                "custom.clear_positive_states",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("clear-positive skill should register");
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let iron = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
            registry,
        ));
        {
            let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            owner.runtime.alive = false;
            owner.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
            owner.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
        }
        runtime.set_skill_handler(clear, skill_clears_positive_states);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("iron clear message should emit");
        let store = &runtime.entities.get(EntityIdx(0)).unwrap().states;

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![("[1]的[铁壁]被打消了", 400)]
        );
        assert_eq!(store.entry(77), None);
        assert_eq!(store.entry(79), None);
    }

    #[test]
    fn run_skill_hooks_clear_positive_combines_runtime_and_state_messages() {
        let mut builder = ExtensionRegistryBuilder::default();
        let clear = builder
            .register_skill_with_hooks(
                "custom",
                "clear-positive",
                "custom.clear_positive",
                ProcMask::PRE_ACTION,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("clear-positive skill should register");
        let shield = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
            .expect("shield state should register");
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let iron = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([clear])],
            registry,
        ));
        {
            let owner = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            owner.activate_charge_runtime();
            owner.activate_accumulate_runtime();
            owner.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
            owner.states.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
            owner.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        }
        runtime.set_skill_handler(clear, skill_clears_positive);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("combined clear-positive should emit messages");
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![
                ("[1]的[聚气]被打消了", 100),
                ("[1]的[蓄力]被中止了", 200),
                ("[1]从[疾走]中解除", 300),
                ("[1]的[铁壁]被打消了", 400),
            ]
        );
        assert!(!owner.runtime.accumulate.active);
        assert!(!owner.runtime.charge.active);
        assert_eq!(owner.runtime.accumulate.acc(), 1.600000023841858);
        assert_eq!(owner.runtime.at_boost_millionths, 1_000_000);
        assert_eq!(owner.states.entry(74), None);
        assert_eq!(owner.states.entry(77), None);
        assert_eq!(owner.states.entry(79), None);
    }

    #[test]
    fn flush_effects_disperse_hit_clears_positives_and_spends_mp() {
        let mut builder = ExtensionRegistryBuilder::default();
        let shield = builder
            .register_state("core", "shield", "core.shield", ProcMask::POST_DEFEND, SkillPriority(6000))
            .expect("shield state should register");
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let iron = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3),
                PlayerTemplate::new(2, "target", 1, 10, 3).with_magic_point(96),
            ],
            registry,
        ));
        {
            let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
            target.activate_charge_runtime();
            target.activate_accumulate_runtime();
            target.states.add_entry(StateEntry::iron(79, iron, 300, 1, SkillPriority(10)));
            target.states.add_entry(StateEntry::shield(74, shield, 50, SkillPriority(6000)));
            target.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        }
        runtime.effects.push(QueuedEffect::DisperseHit {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            damage: 1,
        });

        let frame = runtime.flush_effects().expect("disperse hit should emit clear-positive messages");
        let target = runtime.entities.get(EntityIdx(1)).unwrap();

        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| update.message.as_ref())
                .collect::<Vec<_>>(),
            vec![
                "[1]的[聚气]被打消了",
                "[1]的[蓄力]被中止了",
                "[1]从[疾走]中解除",
                "[1]的[铁壁]被打消了"
            ]
        );
        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .filter(|update| matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .count(),
            4
        );
        assert_eq!(target.runtime.magic_point, 32);
        assert!(!target.runtime.accumulate.active);
        assert!(!target.runtime.charge.active);
        assert_eq!(target.runtime.at_boost_millionths, 1_000_000);
        assert_eq!(target.states.entry(74), None);
        assert_eq!(target.states.entry(77), None);
        assert_eq!(target.states.entry(79), None);
    }

    #[test]
    fn flush_effects_disperse_hit_uses_legacy_mp_thresholds_and_skips_zero_damage() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3),
                PlayerTemplate::new(2, "high", 1, 10, 3).with_magic_point(65),
                PlayerTemplate::new(3, "mid", 1, 10, 3).with_magic_point(33),
                PlayerTemplate::new(4, "low", 1, 10, 3).with_magic_point(32),
                PlayerTemplate::new(5, "missed", 1, 10, 3).with_magic_point(96),
            ],
            ExtensionRegistry::default(),
        ));
        runtime.effects.push(QueuedEffect::DisperseHit {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            damage: 1,
        });
        runtime.effects.push(QueuedEffect::DisperseHit {
            caster: EntityIdx(0),
            target: EntityIdx(2),
            damage: 1,
        });
        runtime.effects.push(QueuedEffect::DisperseHit {
            caster: EntityIdx(0),
            target: EntityIdx(3),
            damage: 1,
        });
        runtime.effects.push(QueuedEffect::DisperseHit {
            caster: EntityIdx(0),
            target: EntityIdx(4),
            damage: 0,
        });

        let frame = runtime.flush_effects();

        assert!(frame.is_none());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_point, 1);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.magic_point, 0);
        assert_eq!(runtime.entities.get(EntityIdx(3)).unwrap().runtime.magic_point, 0);
        assert_eq!(runtime.entities.get(EntityIdx(4)).unwrap().runtime.magic_point, 96);
    }

    #[test]
    fn flush_effects_disperse_attack_emits_legacy_damage_then_clears_positive() {
        let mut builder = ExtensionRegistryBuilder::default();
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(80),
                PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16).with_magic_point(96),
            ],
            registry,
        ));
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        runtime.effects.push(QueuedEffect::DisperseAttack {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("disperse attack should emit updates");
        let target = runtime.entities.get(EntityIdx(1)).unwrap();

        assert_eq!(target.runtime.hp, 1_000 - expected_amount);
        assert_eq!(target.runtime.magic_point, 32);
        assert_eq!(target.states.entry(77), None);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 4);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[净化]");
        assert_eq!(frame.updates.updates[0].score, 20);
        assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 1);
        assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
        assert_eq!(
            frame.updates.updates[2].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[3].message, "[1]从[疾走]中解除");
    }

    #[test]
    fn flush_effects_disperse_attack_dodge_skips_damage_clear_and_mp_spend() {
        let mut builder = ExtensionRegistryBuilder::default();
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(0),
                PlayerTemplate::new(2, "target", 1, 1_000, 3)
                    .with_def_res(0, 512)
                    .with_agility(512)
                    .with_magic_point(96),
            ],
            registry,
        ));
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        let mut expected_rng = RC4::default();
        let _ = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        assert!(PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        runtime.effects.push(QueuedEffect::DisperseAttack {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("dodged disperse should emit replay");
        let target = runtime.entities.get(EntityIdx(1)).unwrap();

        assert_eq!(target.runtime.hp, 1_000);
        assert_eq!(target.runtime.magic_point, 96);
        assert!(target.states.entry(77).is_some());
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[净化]");
        assert_eq!(frame.updates.updates[1].message, "[0][回避]了攻击");
        assert_eq!(frame.updates.updates[1].caster, 1);
        assert_eq!(frame.updates.updates[1].target, 0);
        assert_eq!(frame.updates.updates[1].score, 20);
    }

    #[test]
    fn flush_effects_disperse_attack_pre_defend_zero_stops_before_dodge_and_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_defend = builder
            .register_skill_with_hooks(
                "custom",
                "pre-defend-zero",
                "custom.pre_defend_zero",
                ProcMask::PRE_DEFEND,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("pre-defend skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(0),
                PlayerTemplate::new(2, "target", 1, 1_000, 3)
                    .with_def_res(0, 512)
                    .with_agility(512)
                    .with_magic_point(96)
                    .with_skills([pre_defend]),
            ],
            registry,
        ));
        runtime.set_skill_handler(pre_defend, skill_zeroes_defend_atp);
        let mut expected_rng = RC4::default();
        let _ = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        runtime.effects.push(QueuedEffect::DisperseAttack {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("pre-defend zero should emit replay");
        let target = runtime.entities.get(EntityIdx(1)).unwrap();

        assert_eq!(target.runtime.hp, 1_000);
        assert_eq!(target.runtime.magic_point, 96);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[净化]");
        assert_eq!(frame.updates.updates[1].message, "pre defend zero");
    }

    #[test]
    fn flush_effects_disperse_attack_doubles_atp_against_minion_targets() {
        let mut builder = ExtensionRegistryBuilder::default();
        let minion_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "minion",
                "custom.minion",
                PlayerKindFlags::MINION,
                PlayerKindPolicies::default(),
            )
            .expect("minion kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(80),
                PlayerTemplate::with_kind(2, "minion", minion_kind, 1, 10_000, 3).with_def_res(0, 16),
            ],
            registry,
        ));
        let mut expected_rng = RC4::default();
        let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 2.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng
        ));
        let expected_amount = (atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        runtime.effects.push(QueuedEffect::DisperseAttack {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("minion disperse should emit damage");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10_000 - expected_amount);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[1].message, "[1]受到[2]点伤害");
        assert_eq!(frame.updates.updates[1].score, expected_amount as u32);
    }

    #[test]
    fn flush_effects_disperse_attack_lethal_hit_clears_haste_before_death() {
        let mut builder = ExtensionRegistryBuilder::default();
        let haste = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let die_skill = builder
            .register_skill_with_hooks(
                "custom",
                "die",
                "custom.die",
                ProcMask::DIE,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("die skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 10, 3).with_magic(80),
                PlayerTemplate::new(2, "target", 1, 1, 3)
                    .with_def_res(0, 0)
                    .with_magic_point(96)
                    .with_skills([die_skill]),
            ],
            registry,
        ));
        runtime.set_skill_handler(die_skill, skill_marks_update);
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(100)));
        runtime.effects.push(QueuedEffect::DisperseAttack {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("lethal disperse should emit updates");
        let messages = frame
            .updates
            .updates
            .iter()
            .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
            .map(|update| update.message.as_ref())
            .collect::<Vec<_>>();

        assert_eq!(
            messages,
            vec!["[0]使用[净化]", "[1]受到[2]点伤害", "[1]从[疾走]中解除", "skill mark"]
        );
        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    }

    #[test]
    fn run_state_hooks_iron_post_action_clears_and_emits_release() {
        let mut builder = ExtensionRegistryBuilder::default();
        let iron_state = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
            registry,
        ));
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            300,
            1,
            SkillPriority(10),
        ));

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("iron release should emit updates");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(79), None);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            1920
        );
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(
            frame.updates.updates[0].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[1].message, "[1]从[铁壁]中解除");
        assert_eq!(frame.updates.updates[1].caster, 0);
        assert_eq!(frame.updates.updates[1].target, 0);
    }

    #[test]
    fn run_state_hooks_iron_post_action_clears_expired_without_update() {
        let mut builder = ExtensionRegistryBuilder::default();
        let iron_state = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
            registry,
        ));
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            300,
            0,
            SkillPriority(10),
        ));

        let frame = runtime.run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION);

        assert!(frame.is_none());
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().states.entry(79), None);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            2048
        );
    }

    #[test]
    fn run_state_hooks_iron_post_action_runs_at_legacy_priority() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker_state = builder
            .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("marker state should register");
        let iron_state = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(marker_state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(100),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::iron(
            79,
            iron_state,
            300,
            1,
            SkillPriority(10),
        ));
        runtime.set_state_handler(marker_state, state_marks_update);
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);

        let plan = runtime.scheduler.state_hook_plan(&runtime.entities, EntityIdx(0), ProcMask::POST_ACTION);
        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("marker and iron release should emit updates");

        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| (entry.legacy_order_key, entry.priority))
                .collect::<Vec<_>>(),
            vec![(42, SkillPriority(100)), (79, SkillPriority(210))]
        );
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(
            frame.updates.updates[1].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[2].message, "[1]从[铁壁]中解除");
    }

    #[test]
    fn run_state_hooks_haste_charm_slow_and_iron_share_legacy_post_action_priority() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker_state = builder
            .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("marker state should register");
        let poison_state = builder
            .register_state("core", "poison", "core.poison", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("poison state should register");
        let haste_state = builder
            .register_state("core", "haste", "core.haste", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("haste state should register");
        let charm_state = builder
            .register_state("core", "charm", "core.charm", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("charm state should register");
        let slow_state = builder
            .register_state("core", "slow", "core.slow", ProcMask::POST_ACTION, SkillPriority(100))
            .expect("slow state should register");
        let iron_state = builder
            .register_state(
                "core",
                "iron",
                "core.iron",
                ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("iron state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048)],
            registry,
        ));
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 42,
                extension_state_id: Some(marker_state),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(100),
                registration_order: RegistrationOrder(1),
                payload: StatePayload::None,
            });
            store.add_entry(StateEntry::poison(
                75,
                poison_state,
                Some(0),
                Some(0),
                80.0,
                2,
                SkillPriority(0),
            ));
            store.add_entry(StateEntry::haste(77, haste_state, 2, 1, SkillPriority(100)));
            store.add_entry(StateEntry::charm(
                76,
                charm_state,
                7,
                Some(1),
                Some(2),
                Some(3),
                1,
                SkillPriority(100),
            ));
            store.add_entry(StateEntry::slow(78, slow_state, 1, SkillPriority(100)));
            store.add_entry(StateEntry::iron(79, iron_state, 300, 1, SkillPriority(10)));
        }
        runtime.set_state_handler(marker_state, state_marks_update);
        runtime.set_state_handler(poison_state, run_poison_post_action_state);
        runtime.set_state_handler(haste_state, run_haste_post_action_state);
        runtime.set_state_handler(charm_state, run_charm_post_action_state);
        runtime.set_state_handler(slow_state, run_slow_post_action_state);
        runtime.set_state_handler(iron_state, run_iron_post_defend_state);

        let plan = runtime.scheduler.state_hook_plan(&runtime.entities, EntityIdx(0), ProcMask::POST_ACTION);
        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("marker and timed release states should emit updates");

        assert_eq!(
            plan.entries
                .iter()
                .map(|entry| (entry.legacy_order_key, entry.priority))
                .collect::<Vec<_>>(),
            vec![
                (42, SkillPriority(100)),
                (75, SkillPriority(150)),
                (77, SkillPriority(210)),
                (76, SkillPriority(210)),
                (78, SkillPriority(210)),
                (79, SkillPriority(210)),
            ]
        );
        assert_eq!(
            frame
                .updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| update.message.as_ref())
                .collect::<Vec<_>>(),
            vec![
                "state mark",
                "[1][毒性发作]",
                "[1]受到[2]点伤害",
                "[1]从[疾走]中解除",
                "[1]从[魅惑]中解除",
                "[1]从[迟缓]中解除",
                "[1]从[铁壁]中解除",
            ]
        );
    }

    #[test]
    fn flush_effects_dispatches_custom_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("custom mark".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("custom handler should emit update");
        assert_eq!(frame.updates.updates[0].message, "custom mark");
    }

    #[test]
    fn flush_effects_exposes_controlled_rng_to_custom_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let rng_handler = builder
            .register_effect_handler("custom", "rng", "custom.rng", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(rng_handler, custom_consumes_rng);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            rng_handler,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(10),
        )));
        let mut expected_rng = RC4::default();
        let expected_value = expected_rng.next_i32(10);
        let expected_byte = expected_rng.next_u8();

        let frame = runtime.flush_effects().expect("custom rng handler should emit update");

        assert_eq!(
            frame.updates.updates[0].message,
            format!("rng:{expected_value}:{expected_byte}")
        );
        assert_eq!(frame.updates.updates[0].score, expected_value as u32);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn flush_effects_runs_nested_custom_effect_before_older_siblings() {
        let mut builder = ExtensionRegistryBuilder::default();
        let nested_damage = builder
            .register_effect_handler("custom", "nested-damage", "custom.nested_damage", SkillPriority(0))
            .expect("handler should register");
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(nested_damage, custom_spawns_nested_damage);
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            nested_damage,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(4),
        )));
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("after nested".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("nested damage should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "after nested");
    }

    #[test]
    fn flush_effects_applies_heal_without_exceeding_max_hp() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 4;
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 20,
        });

        let frame = runtime.flush_effects().expect("heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    }

    #[test]
    fn flush_effects_readds_healed_dead_target_to_alive_views() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 4, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 4,
        });
        runtime.flush_effects().expect("lethal damage should emit update");
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 2,
        });

        runtime.flush_effects().expect("heal should emit update");

        assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 2);
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 2);
        assert_eq!(
            runtime.world.first_alive_enemy(EntityIdx(0), &runtime.entities),
            Some(EntityIdx(1))
        );
    }

    #[test]
    fn flush_effects_spawns_entity_and_adds_it_to_round_order() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(3, "summoned", 0, 5, 2),
        });

        let frame = runtime.flush_effects().expect("spawn should emit update");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, "summoned");
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.root_owner, EntityIdx(0));
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
        assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
    }

    #[test]
    fn spawned_entity_can_be_selected_by_scheduler() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            ExtensionRegistry::default(),
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(2, "enemy", 1, 8, 4),
        });
        runtime.flush_effects().expect("spawn should emit update");

        assert_eq!(runtime.world.sync_winner(&runtime.entities), None);
        assert_eq!(
            runtime.scheduler.select_minimal_action(&mut runtime.world, &runtime.entities),
            Some(ActionPlan {
                actor: EntityIdx(0),
                target: EntityIdx(1),
                amount: 3,
            })
        );
    }

    #[test]
    fn flush_effects_adds_and_clears_state_entries() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        let state = StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(StateId(1)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(5),
            registration_order: RegistrationOrder(2),
            payload: StatePayload::None,
        };

        runtime.effects.push(QueuedEffect::AddState {
            target: EntityIdx(1),
            state,
        });
        let add_frame = runtime.flush_effects().expect("add state should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), Some(&state));
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 1);
        assert_eq!(add_frame.updates.updates[0].message, "[1]状态改变");

        runtime.effects.push(QueuedEffect::ClearState {
            target: EntityIdx(1),
            legacy_order_key: 77,
        });
        let clear_frame = runtime.flush_effects().expect("clear state should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 2);
        assert_eq!(clear_frame.updates.updates[0].message, "[1]状态解除");
    }

    #[test]
    fn flush_effects_runs_nested_heal_before_older_siblings() {
        let mut builder = ExtensionRegistryBuilder::default();
        let nested_heal = builder
            .register_effect_handler("custom", "nested-heal", "custom.nested_heal", SkillPriority(0))
            .expect("handler should register");
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 3;
        runtime.set_effect_handler(nested_heal, custom_spawns_nested_heal);
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            nested_heal,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(4),
        )));
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("after heal".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("nested heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[1].message, "after heal");
    }

    #[test]
    fn flush_effects_revives_dead_entity_with_capped_hp() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.world.remove_round_actor(EntityIdx(1));
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 0;
        target.runtime.alive = false;
        runtime.effects.push(QueuedEffect::Revive {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            hp: 20,
        });

        let frame = runtime.flush_effects().expect("revive should emit update");

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.hp, 10);
        assert!(target.runtime.alive);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(frame.updates.updates[0].message, "[1][复活]了");
    }

    #[test]
    fn flush_effects_removes_entity_from_alive_set() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Remove {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("remove should emit update");

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.hp, 0);
        assert!(!target.runtime.alive);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0)]);
        assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
        assert_eq!(frame.updates.updates[0].message, "[1]消失了");
    }

    #[test]
    fn flush_effects_emits_replay_effect_without_state_mutation() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            message: "custom replay".to_owned(),
            score: 7,
        });

        let frame = runtime.flush_effects().expect("replay should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
        assert_eq!(frame.updates.updates[0].message, "custom replay");
        assert_eq!(frame.updates.updates[0].score, 7);
    }

    #[test]
    fn flush_effects_merges_fixed_lane_skill_loadout() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill_a = builder
            .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
            .expect("skill should register");
        let skill_b = builder
            .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
            .expect("skill should register");
        let skill_c = builder
            .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
            .expect("skill should register");
        let merge_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "merge",
                "custom.merge",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: false,
                },
            )
            .expect("merge kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3).with_skills([skill_a]),
                PlayerTemplate::new(2, "right", 1, 10, 3).with_skills([skill_b, skill_c]),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("merge should emit update");

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(),
            &[skill_b, skill_c]
        );
        assert_eq!(frame.updates.updates[0].message, "[0][吞噬]了[1]");
        assert_eq!(frame.updates.updates[0].score, 60);
        assert_eq!(frame.updates.updates[1].message, "[0]属性上升");
        assert_eq!(frame.updates.updates[1].score, 0);
    }

    #[test]
    fn flush_effects_merge_drops_unmapped_skills_when_policy_requires() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill_a = builder
            .register_skill("custom", "a", "custom.a", TargetPolicy::Enemy, SkillPriority(0))
            .expect("skill should register");
        let skill_b = builder
            .register_skill("custom", "b", "custom.b", TargetPolicy::Enemy, SkillPriority(1))
            .expect("skill should register");
        let skill_c = builder
            .register_skill("custom", "c", "custom.c", TargetPolicy::Enemy, SkillPriority(2))
            .expect("skill should register");
        let merge_kind = builder
            .register_player_kind_with_policies(
                "custom",
                "merge",
                "custom.merge",
                PlayerKindFlags::default(),
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::SelfEntity,
                    damage_share: DamageSharePolicy::None,
                    merge: MergePolicy::DropUnmappedSkills,
                    inherit_owner_def_res: false,
                },
            )
            .expect("merge kind should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3).with_skills([skill_a]),
                PlayerTemplate::new(2, "right", 1, 10, 3).with_skills([skill_b, skill_c]),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        runtime.flush_effects().expect("merge should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_b]);
    }

    #[test]
    fn flush_effects_panics_on_unknown_damage_target() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            amount: 1,
        });

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(result.is_err());
    }

    fn assert_effect_panics(effect: QueuedEffect) {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(effect);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(result.is_err());
    }

    fn dummy_state_entry() -> StateEntry {
        StateEntry {
            legacy_order_key: 999,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
            payload: StatePayload::None,
        }
    }

    #[test]
    fn flush_effects_panics_on_unknown_effect_entities() {
        assert_effect_panics(QueuedEffect::Damage {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::SummonExplode {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            fire_state_key: 91,
        });
        assert_effect_panics(QueuedEffect::SummonExplode {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            fire_state_key: 91,
        });
        assert_effect_panics(QueuedEffect::DisperseAttack {
            caster: EntityIdx(99),
            target: EntityIdx(1),
        });
        assert_effect_panics(QueuedEffect::DisperseAttack {
            caster: EntityIdx(0),
            target: EntityIdx(99),
        });
        assert_effect_panics(QueuedEffect::DisperseHit {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            damage: 1,
        });
        assert_effect_panics(QueuedEffect::DisperseHit {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            damage: 1,
        });
        assert_effect_panics(QueuedEffect::Heal {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Spawn {
            caster: EntityIdx(99),
            template: PlayerTemplate::new(3, "ghost", 1, 1, 0),
        });
        assert_effect_panics(QueuedEffect::AddState {
            target: EntityIdx(99),
            state: dummy_state_entry(),
        });
        assert_effect_panics(QueuedEffect::ClearState {
            target: EntityIdx(99),
            legacy_order_key: 999,
        });
        assert_effect_panics(QueuedEffect::Revive {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            hp: 1,
        });
        assert_effect_panics(QueuedEffect::Revive {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            hp: 1,
        });
        assert_effect_panics(QueuedEffect::Remove {
            caster: EntityIdx(99),
            target: EntityIdx(1),
        });
        assert_effect_panics(QueuedEffect::Remove {
            caster: EntityIdx(0),
            target: EntityIdx(99),
        });
        assert_effect_panics(QueuedEffect::Merge {
            caster: EntityIdx(99),
            target: EntityIdx(1),
        });
        assert_effect_panics(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(99),
        });
        assert_effect_panics(QueuedEffect::Replay {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            message: "bad caster".to_owned(),
            score: 0,
        });
        assert_effect_panics(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            message: "bad target".to_owned(),
            score: 0,
        });
    }

    #[test]
    fn flush_effects_panics_on_unknown_custom_effect_entities() {
        let mut builder = ExtensionRegistryBuilder::default();
        let handler = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(handler, custom_marks_update);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(99),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("bad caster".to_owned()),
        )));

        let bad_caster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(bad_caster.is_err());

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(0),
            Some(EntityIdx(99)),
            CustomEffectPayload::Text("bad target".to_owned()),
        )));

        let bad_target = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(bad_target.is_err());
    }

    #[test]
    fn custom_context_restricts_cross_entity_reads_by_capability() {
        let mut builder = ExtensionRegistryBuilder::default();
        let reader = builder
            .register_effect_handler("custom", "reader", "custom.reader", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
                PlayerTemplate::new(3, "third", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(reader, custom_rejects_cross_entity_read);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            reader,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::None,
        )));
        let denied = runtime.flush_effects().expect("denied read handler should emit update");
        assert_eq!(denied.updates.updates[0].message, "read denied");

        runtime.set_effect_handler_with_capabilities(reader, custom_reads_cross_entity, &[ExtensionCapability::ReadEnemies]);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            reader,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::None,
        )));
        let allowed = runtime.flush_effects().expect("allowed read handler should emit update");
        assert_eq!(allowed.updates.updates[0].message, "third");
    }

    #[test]
    fn custom_context_requires_capability_for_entity_slot_mutation() {
        let mut builder = ExtensionRegistryBuilder::default();
        let slot = builder
            .reserve_entity_slot("custom", "flag", "custom.flag")
            .expect("entity slot should reserve");
        let mutator = builder
            .register_effect_handler("custom", "mutator", "custom.mutator", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler_with_capabilities(
            mutator,
            custom_mutates_entity_slot,
            &[ExtensionCapability::MutateEntitySlots],
        );
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            mutator,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(slot.0 as i32),
        )));

        let frame = runtime.flush_effects().expect("slot mutation should emit update");

        assert_eq!(frame.updates.updates[0].message, "slot set");
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().slots.get(slot),
            Some(&SlotValue::Bool(true))
        );
    }

    #[test]
    fn runtime_dispatches_replay_renderers_in_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let late = builder
            .register_replay_renderer("custom", "late", "custom.late_replay", SkillPriority(10))
            .expect("late replay renderer should register");
        let early = builder
            .register_replay_renderer("custom", "early", "custom.early_replay", SkillPriority(1))
            .expect("early replay renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_replay_renderer(late, render_update_count_replay);
        runtime.set_replay_renderer(early, render_first_message_replay);
        let frame = RuntimeFrame::single_damage(0, 0, 3);

        let rendered = runtime.render_replay_frame(&frame);

        assert_eq!(
            rendered,
            vec![
                RenderedReplay::new(ReplayRendererId(0), "[0]攻击[1]"),
                RenderedReplay::new(ReplayRendererId(1), "1")
            ]
        );
    }

    #[test]
    fn runtime_dispatches_show_renderers_in_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let show = builder
            .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
            .expect("show renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_show_renderer(show, render_first_message_show);
        let frame = RuntimeFrame::single_damage(0, 0, 3);

        let rendered = runtime.render_show_frame(&frame);

        assert_eq!(rendered, vec![RenderedShow::new(ShowRendererId(0), "[0]攻击[1]")]);
    }

    #[test]
    fn runtime_dispatches_hp_marker_show_renderer_golden() {
        let mut builder = ExtensionRegistryBuilder::default();
        let show = builder
            .register_show_renderer("custom", "hp-marker", "custom.hp_marker.show", SkillPriority(0))
            .expect("hp marker show renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_show_renderer(show, render_hp_marker_bar_show);
        let mut updates = crate::engine::update::RunUpdates::new();
        let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
        hp_report.param = Some(87);
        updates.add(hp_report);
        let frame = RuntimeFrame { updates };

        let rendered = runtime.render_show_frame(&frame);

        assert_eq!(
            rendered,
            vec![RenderedShow::new(ShowRendererId(0), "hp-bar:actor=0:value=87:text=0还剩87点血")]
        );
    }

    #[test]
    fn runtime_frame_renders_core_replay_and_show_golden() {
        let mut frame = RuntimeFrame::single_damage(0, 1, 3);
        frame.updates.add(RuntimeFrame::replay_update(0, 1, "[0]属性上升", 0));

        assert_eq!(
            frame.render_core_replay(),
            vec![
                CoreReplayEvent {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 3,
                },
                CoreReplayEvent {
                    message: "[0]属性上升".to_owned(),
                    caster: 0,
                    target: 1,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                },
            ]
        );
        assert_eq!(
            frame.render_core_show(),
            vec![
                CoreShowEvent {
                    text: "0攻击1".to_owned(),
                    score: 3,
                },
                CoreShowEvent {
                    text: "0属性上升".to_owned(),
                    score: 0,
                },
            ]
        );
    }

    #[test]
    fn runtime_frame_renders_hp_marker_core_show_golden() {
        let mut updates = crate::engine::update::RunUpdates::new();
        let mut hp_report = RuntimeFrame::replay_update(0, 0, "[0]还剩[2]点血", 0);
        hp_report.param = Some(87);
        updates.add(hp_report);
        let frame = RuntimeFrame { updates };

        assert_eq!(
            frame.render_core_replay(),
            vec![CoreReplayEvent {
                message: "[0]还剩[2]点血".to_owned(),
                caster: 0,
                target: 0,
                targets: Vec::new(),
                param: Some(87),
                score: 0,
            }]
        );
        assert_eq!(
            frame.render_core_show(),
            vec![CoreShowEvent {
                text: "0还剩87点血".to_owned(),
                score: 0,
            }]
        );
    }
}

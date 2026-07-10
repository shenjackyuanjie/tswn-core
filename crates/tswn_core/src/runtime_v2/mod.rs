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
    CloneBuildData, CloneDerivedStats, CounterRuntime, CovidInfectionEntry, EntityArena, EntityIdx, EntityRecord, HideRuntime,
    MoveState, PlayerPolicyOverrides, PlayerRuntime, PlayerTemplate, ProtectLinkRuntime, SkillLoadout, StateEntry, StatePayload,
    StateStore,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeV2SkillSource {
    Entity(EntityIdx),
    TemplateSlot(TemplateSlotId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeV2MissingSkillHandler {
    pub skill_id: SkillId,
    pub export_name: Option<String>,
    pub sources: Vec<RuntimeV2SkillSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeV2ReadyError {
    pub missing_skill_handlers: Vec<RuntimeV2MissingSkillHandler>,
}

impl std::fmt::Display for RuntimeV2ReadyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("runtime v2 missing skill handlers: ")?;
        for (index, missing) in self.missing_skill_handlers.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            match &missing.export_name {
                Some(export_name) => write!(f, "{export_name} (id {})", missing.skill_id.0)?,
                None => write!(f, "unregistered skill id {}", missing.skill_id.0)?,
            }
            f.write_str(" used by ")?;
            for (source_index, source) in missing.sources.iter().enumerate() {
                if source_index > 0 {
                    f.write_str(", ")?;
                }
                match source {
                    RuntimeV2SkillSource::Entity(entity) => write!(f, "entity {}", entity.0)?,
                    RuntimeV2SkillSource::TemplateSlot(slot) => write!(f, "template slot {}", slot.0)?,
                }
            }
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeV2ReadyError {}

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
            skill_handlers,
            state_handlers,
        } = config;
        let mut runner = match bed2_minion_overlays {
            Some(minion_overlays) => {
                Self::from_bed2_roster_with_minion_overlays(raw_groups, registry, bed2_kind, bed2_summon_skill, minion_overlays)
                    .map_err(CustomRuntimeV2ImportError::Bed2MinionOverlay)
            }
            None => Self::from_bed2_roster(raw_groups, registry, bed2_kind, bed2_summon_skill)
                .map_err(CustomRuntimeV2ImportError::Bed2Roster),
        }?;
        runner.install_skill_handler_bindings(skill_handlers);
        runner.install_state_handler_bindings(state_handlers);
        runner.validate_ready()?;
        Ok(runner)
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
            skill_handlers,
            state_handlers,
        } = config;
        let mut runner = match bed2_minion_overlays {
            Some(minion_overlays) => {
                Self::from_mixed_roster_with_minion_overlays(raw_groups, registry, bed2_kind, bed2_summon_skill, minion_overlays)
                    .map_err(CustomRuntimeV2ImportError::Bed2MinionOverlay)
            }
            None => Self::from_mixed_roster(raw_groups, registry, bed2_kind, bed2_summon_skill)
                .map_err(CustomRuntimeV2ImportError::MixedRoster),
        }?;
        runner.install_skill_handler_bindings(skill_handlers);
        runner.install_state_handler_bindings(state_handlers);
        runner.validate_ready()?;
        Ok(runner)
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
            self.sync_legacy_raw_entities(&legacy_runner);
            self.sync_legacy_raw_world(&legacy_runner.world);
            self.runtime.rng = legacy_runner.randomer;
            self.runtime.scheduler.reset_action_mode_from_entities(&self.runtime.entities);
        }
    }

    fn template_from_legacy_player(
        player: &crate::player::Player,
        id: crate::player::PlrId,
        team: usize,
        skills: SkillLoadout,
    ) -> PlayerTemplate {
        let status = player.get_status();
        PlayerTemplate::new(id, player.id_name(), team, status.max_hp, status.attack)
            .with_display_name(player.display_name())
            .with_magic(status.magic)
            .with_magic_point(status.magic_point)
            .with_wisdom(status.wisdom)
            .with_speed(status.speed)
            .with_def_res(status.defense, status.resistance)
            .with_agility(status.agility)
            .with_at_boost_millionths((status.at_boost * 1_000_000.0).round() as i64)
            .with_target_score_stats(status.attr_sum, status.atk_sum, status.attract)
            .with_speed_points(player.move_point())
            .with_skill_loadout(skills)
    }

    fn sync_legacy_raw_entities(&mut self, legacy_runner: &crate::Runner) {
        let shadow_blueprint_slot = self
            .runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT);
        for index in 0..self.runtime.entities.len() {
            let entity_idx = EntityIdx(index.try_into().expect("runtime_v2 entity index overflow"));
            let legacy_player_id = index;
            let Some(legacy_player) = legacy_runner.storage.get_player(&legacy_player_id) else {
                continue;
            };
            if self
                .runtime
                .entities
                .get(entity_idx)
                .unwrap_or_else(|| panic!("runtime_v2 entity disappeared during legacy raw sync: {}", entity_idx.0))
                .template
                .kind
                != PlayerTemplate::DEFAULT_KIND
            {
                continue;
            }
            let status = legacy_player.get_status();
            let (clone_attrs, clone_weapon_attr_bonus, clone_name_factor) = legacy_player.clone_build_inputs();
            let clone_build = CloneBuildData::from_legacy(clone_attrs, clone_weapon_attr_bonus, clone_name_factor, status);
            let move_point = legacy_player.move_point();
            let snapshot = legacy_player.skill_loadout_snapshot();
            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_PROBE_LOADOUT").is_some() {
                eprintln!(
                    "[loadout_probe] entity={} name={} entries={:?}",
                    entity_idx.0,
                    legacy_player.id_name(),
                    snapshot
                        .entries
                        .iter()
                        .filter(|entry| entry.level > 0)
                        .map(|entry| (entry.key, entry.level, entry.runtime_kind))
                        .collect::<Vec<_>>(),
                );
            }
            let skills = import_plain_legacy_skill_loadout(&self.runtime.registry, &snapshot);
            let imported_shadow_skill = self
                .runtime
                .registry
                .skill_id_by_export_name(BuiltinActiveSkill::Shadow.export_name())
                .is_some_and(|shadow_skill| skills.skills().contains(&shadow_skill));
            let team = self.runtime.entities.get(entity_idx).unwrap().runtime.team;
            let kind = match legacy_player.player_type() {
                crate::player::PlayerType::Boss => self
                    .runtime
                    .registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_BOSS_KIND_EXPORT)
                    .expect("default runtime v2 profile must register core boss kind"),
                crate::player::PlayerType::Boost => self
                    .runtime
                    .registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_BOOST_KIND_EXPORT)
                    .expect("default runtime v2 profile must register core boost kind"),
                _ => PlayerTemplate::DEFAULT_KIND,
            };
            let boss_kind = crate::player::boss::boss_kind(&legacy_player.id_name());
            let covid_boss_mutation = matches!(boss_kind, crate::player::boss::BossKind::Covid).then_some(40);
            let lazy_boss_at_boost = matches!(boss_kind, crate::player::boss::BossKind::Lazy).then_some(1.0);
            let saitama_boss_state = matches!(boss_kind, crate::player::boss::BossKind::Saitama).then(|| {
                self.runtime
                    .registry
                    .state_id_by_export_name(DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT)
                    .expect("default runtime v2 profile must register saitama boss state")
            });
            let (kind_flags, kind_policies) = self
                .runtime
                .registry
                .player_kind(kind)
                .map_or((PlayerKindFlags::NONE, PlayerKindPolicies::default()), |spec| {
                    (spec.flags, spec.policies)
                });
            let shadow_blueprint = imported_shadow_skill.then(|| {
                let slot = shadow_blueprint_slot
                    .expect("runtime v2 registry importing core shadow skill must reserve core shadow blueprint slot");
                let shadow_player_kind = self
                    .runtime
                    .registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SHADOW_KIND_EXPORT)
                    .expect("runtime v2 registry importing core shadow skill must register core shadow kind");
                let shadow = crate::player::skill::act::shadow::build_shadow_minion(legacy_player_id, &legacy_runner.storage);
                let shadow_snapshot = shadow.skill_loadout_snapshot();
                let shadow_skills = import_plain_legacy_skill_loadout(&self.runtime.registry, &shadow_snapshot);
                let mut template = Self::template_from_legacy_player(&shadow, 0, team, shadow_skills);
                template.kind = shadow_player_kind;
                (slot, template)
            });
            let entity = self
                .runtime
                .entities
                .get_mut(entity_idx)
                .unwrap_or_else(|| panic!("runtime_v2 entity disappeared during legacy raw sync: {}", entity_idx.0));
            entity.template.max_hp = status.max_hp;
            entity.template.display_name = legacy_player.display_name();
            entity.template.kind = kind;
            entity.template.attack = status.attack;
            entity.template.magic = status.magic;
            entity.template.magic_point = status.magic_point;
            entity.template.wisdom = status.wisdom;
            entity.template.speed = status.speed;
            entity.template.defense = status.defense;
            entity.template.resistance = status.resistance;
            entity.template.agility = status.agility;
            entity.template.at_boost_millionths = (status.at_boost * 1_000_000.0).round() as i64;
            entity.template.attr_sum = status.attr_sum;
            entity.template.atk_sum = status.atk_sum;
            entity.template.attract_bits = status.attract.to_bits();
            entity.template.move_state.speed_points = move_point;
            entity.template.skills = skills;
            entity.template.clone_build = Some(clone_build);
            entity.runtime.hp = status.hp;
            entity.runtime.alive = status.alive();
            entity.runtime.kind = kind;
            entity.runtime.flags = kind_flags;
            entity.runtime.policies = entity.template.policy_overrides.apply_to(kind_policies);
            entity.runtime.attack = status.attack;
            entity.runtime.magic = status.magic;
            entity.runtime.magic_point = status.magic_point;
            entity.runtime.wisdom = status.wisdom;
            entity.runtime.speed = status.speed;
            entity.runtime.defense = status.defense;
            entity.runtime.resistance = status.resistance;
            entity.runtime.agility = status.agility;
            entity.runtime.at_boost_millionths = (status.at_boost * 1_000_000.0).round() as i64;
            entity.runtime.attr_sum = status.attr_sum;
            entity.runtime.atk_sum = status.atk_sum;
            entity.runtime.attract_bits = status.attract.to_bits();
            entity.runtime.move_state.speed_points = move_point;
            if let Some(mutation) = covid_boss_mutation {
                entity.states.add_entry(StateEntry::covid_boss(PLAIN_COVID_BOSS_STATE_KEY, mutation));
            }
            if let Some(at_boost) = lazy_boss_at_boost {
                entity.states.add_entry(StateEntry::lazy_boss(PLAIN_LAZY_BOSS_STATE_KEY, at_boost));
            }
            if let Some(state_id) = saitama_boss_state {
                entity.states.add_entry(StateEntry::saitama_boss(
                    PLAIN_SAITAMA_BOSS_STATE_KEY,
                    state_id,
                    SkillPriority(i32::MAX),
                ));
            }
            if let Some((slot, template)) = shadow_blueprint {
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                    .expect("runtime_v2 core shadow blueprint slot must exist");
            }
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
        let team_roster = legacy_world
            .groups
            .iter()
            .map(|group| Self::entity_order_from_legacy_plrs(group))
            .collect();
        let team_alive = (0..legacy_world.groups.len())
            .map(|team| legacy_world.team_alive(team).map(Self::entity_order_from_legacy_plrs).unwrap_or_default())
            .collect();
        let flat_alive = Self::entity_order_from_legacy_plrs(&legacy_world.flat_alive);
        self.runtime
            .world
            .sync_initial_views(&self.runtime.entities, round_order, team_roster, team_alive, flat_alive);
    }

    fn entity_order_from_legacy_plrs(plrs: &[crate::player::PlrId]) -> Vec<EntityIdx> {
        plrs.iter().copied().map(Self::entity_idx_from_legacy_plr).collect()
    }

    fn entity_idx_from_legacy_plr(plr_id: crate::player::PlrId) -> EntityIdx {
        EntityIdx(plr_id.try_into().expect("legacy raw player id overflowed runtime_v2 entity index"))
    }

    pub fn runtime(&self) -> &CombatRuntime { &self.runtime }

    pub fn runtime_mut(&mut self) -> &mut CombatRuntime { &mut self.runtime }

    pub fn validate_ready(&self) -> Result<(), RuntimeV2ReadyError> { self.runtime.validate_ready() }

    fn install_skill_handler_bindings(&mut self, bindings: Vec<RuntimeV2SkillHandlerBinding>) {
        for binding in bindings {
            self.runtime
                .set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
        }
    }

    fn install_state_handler_bindings(&mut self, bindings: Vec<RuntimeV2StateHandlerBinding>) {
        for binding in bindings {
            self.runtime
                .set_state_handler_with_capabilities(binding.state_id, binding.handler, &binding.capabilities);
        }
    }

    fn assert_ready(&self) {
        if let Err(error) = self.validate_ready() {
            panic!("{error}");
        }
    }

    fn run_round_unchecked(&mut self) -> RoundOutcome { self.runtime.run_minimal_round() }

    pub fn run_round(&mut self) -> RoundOutcome {
        self.assert_ready();
        self.run_round_unchecked()
    }

    pub fn run_round_normalized(&mut self) -> NormalizedOutcome {
        let outcome = self.run_round();
        NormalizedOutcome::from_runtime(&self.runtime, &outcome)
    }

    pub fn run_until_winner(&mut self, max_rounds: usize) -> RuntimeV2RunSummary {
        self.assert_ready();
        let mut rounds = Vec::new();
        let mut winner_team = self.runtime.world.sync_winner(&self.runtime.entities);
        while winner_team.is_none() && rounds.len() < max_rounds {
            let outcome = self.run_round_unchecked();
            winner_team = outcome.winner_team;
            rounds.push(outcome);
            if winner_team.is_some() {
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
        self.assert_ready();
        let mut rounds = Vec::new();
        let mut winner_team = self.runtime.world.sync_winner(&self.runtime.entities);
        while winner_team.is_none() && rounds.len() < max_rounds {
            let outcome = self.run_round_unchecked();
            winner_team = outcome.winner_team;
            rounds.push(NormalizedOutcome::from_runtime(&self.runtime, &outcome));
            if winner_team.is_some() {
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

pub fn push_summon_fire(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::FireAttack {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

pub fn run_summon_fire_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_summon_fire(context, target, 91);
}

pub fn push_summon_explode(context: &mut SkillContext<'_>, target: EntityIdx, fire_state_key: u32) {
    context.push_nested(QueuedEffect::SummonExplode {
        caster: context.owner_idx(),
        target,
        fire_state_key,
    });
}

pub fn run_summon_explode_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    push_summon_explode(context, target, 91);
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

pub fn run_possess_skill(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
    let Some(target) = context.selected_target() else {
        return;
    };
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0]使用[附体]",
        context.owner_idx().0 as usize,
        target.0 as usize,
        0,
    ));
    context.add_update(crate::engine::update::RunUpdate::new(
        "[1]进入[狂暴]状态",
        context.owner_idx().0 as usize,
        target.0 as usize,
        0,
    ));
    context.push_nested(QueuedEffect::Remove {
        caster: context.owner_idx(),
        target: context.owner_idx(),
    });
    context.push_nested(QueuedEffect::AddBerserkState {
        target,
        legacy_order_key: 10,
        step: 4,
    });
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

pub fn run_covid_infection_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::CovidInfection {
        mut entries,
        mut mutation_set,
        mut recovered,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    if entries.is_empty() {
        return;
    }

    if context.hook().intersects(ProcMask::PRE_ACTION) {
        let smart = context
            .action_smart()
            .expect("runtime_v2 covid PRE_ACTION state must receive the action smart roll");
        for infection in &mut entries {
            if context.rng_next_u8() < 64 {
                let mutation = context.rng_r127() as i32;
                infection.mutation = mutation;
                if !mutation_set.contains(&mutation) {
                    mutation_set.push(mutation);
                }
            }
        }

        let last_idx = entries.len() - 1;
        let boss = entries[last_idx].boss;
        let mutation = entries[last_idx].mutation;
        let days = entries[last_idx].days;
        let all_alive = context.flat_alive().expect("runtime_v2 covid state must read the complete alive list");

        let boss_team = context.entity(boss).expect("runtime_v2 covid boss must exist").runtime.team;
        let skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (context
                    .entity(*candidate)
                    .expect("runtime_v2 covid alive candidate must exist")
                    .runtime
                    .team
                    == boss_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        let invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if skip_indices.is_empty() {
                context.rng_pick_entity(&all_alive)
            } else {
                context.rng_pick_skip_range_entity(&all_alive, &skip_indices)
            };
            let Some(picked) = picked else {
                break;
            };
            let candidate = all_alive[picked];
            if selected.contains(&candidate) {
                duplicate_count += 1;
                continue;
            }
            selected.push(candidate);
            if selected.len() >= select_count {
                break;
            }
        }
        if !smart {
            for _ in &selected {
                let _ = context.rng_r_ffff();
            }
        }

        let owner = context.owner_idx();
        let owner_wisdom = context.owner().map(|entity| entity.runtime.wisdom).unwrap_or(0);
        if days == 0 || i32::from(context.rng_next_u8()) > owner_wisdom {
            entries[last_idx].days += i32::from(context.rng_next_u8() & 3);
            for _ in 0..5 {
                let Some(picked) = context.rng_pick_entity(&all_alive) else {
                    break;
                };
                let candidate = all_alive[picked];
                if candidate == owner || candidate == boss {
                    continue;
                }
                let candidate_entity = context.entity(candidate).expect("runtime_v2 covid spread candidate must exist");
                if !candidate_entity.runtime.alive {
                    continue;
                }
                let already_has_mutation = candidate_entity.states.entries().iter().any(|state| {
                    matches!(
                        &state.payload,
                        StatePayload::CovidInfection {
                            mutation_set,
                            ..
                        } if mutation_set.contains(&mutation)
                    )
                });
                if already_has_mutation {
                    continue;
                }
                let owner_team = context.owner().expect("runtime_v2 covid owner must exist").runtime.team;
                let effect = if candidate_entity.runtime.team == owner_team {
                    QueuedEffect::CovidContact {
                        owner,
                        candidate,
                        boss,
                        mutation,
                    }
                } else {
                    QueuedEffect::CovidAttack {
                        owner,
                        candidate,
                        boss,
                        mutation,
                    }
                };
                context
                    .set_owner_state_payload(
                        entry.legacy_order_key,
                        StatePayload::CovidInfection {
                            entries,
                            mutation_set,
                            recovered,
                        },
                    )
                    .expect("runtime_v2 covid state payload must still exist");
                context.push(effect);
                context.intercept_action();
                return;
            }
        }

        entries[last_idx].days += i32::from(context.rng_next_u8() & 3);
        let message = if entries[last_idx].days > 2 {
            "[1]在重症监护室无法行动"
        } else {
            "[1]在家中自我隔离"
        };
        context.add_update(crate::engine::update::RunUpdate::new(
            message,
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        context
            .set_owner_state_payload(
                entry.legacy_order_key,
                StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                },
            )
            .expect("runtime_v2 covid state payload must still exist");
        context.intercept_action();
        return;
    }

    if context.hook().intersects(ProcMask::POST_ACTION) {
        let owner = context.owner_idx();
        let alive = context.owner().is_some_and(|entity| entity.runtime.alive);
        for infection in &entries {
            if alive && infection.days > 1 {
                context.push(QueuedEffect::CovidPneumonia {
                    owner,
                    boss: infection.boss,
                    mutation: infection.mutation,
                });
            }
        }
        entries.retain(|infection| infection.days <= 6);
        if entries.is_empty() && !recovered {
            recovered = true;
        }
        context
            .set_owner_state_payload(
                entry.legacy_order_key,
                StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                },
            )
            .expect("runtime_v2 covid state payload must still exist");
    }
}

pub fn run_lazy_infection_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    let Some(StatePayload::LazyInfection { boss }) = context.owner_state_payload(entry.legacy_order_key) else {
        return;
    };
    if context.hook().intersects(ProcMask::PRE_ACTION) {
        if context.rng_next_u8() >= 128 {
            return;
        }
        let smart = context
            .action_smart()
            .expect("runtime_v2 lazy PRE_ACTION state must receive the action smart roll");
        let all_alive = context.flat_alive().expect("runtime_v2 lazy state must read the complete alive list");
        let boss_team = context.entity(boss).expect("runtime_v2 lazy boss must exist").runtime.team;
        let skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| {
                (context.entity(*candidate).expect("runtime_v2 lazy candidate must exist").runtime.team == boss_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        let invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if skip_indices.is_empty() {
                context.rng_pick_entity(&all_alive)
            } else if all_alive.len() > skip_indices.len() {
                context.rng_pick_skip_range_entity(&all_alive, &skip_indices)
            } else {
                None
            };
            let Some(picked) = picked else {
                break;
            };
            if selected.contains(&picked) {
                duplicate_count += 1;
                continue;
            }
            selected.push(picked);
            if selected.len() >= select_count {
                break;
            }
        }
        if !smart {
            for _ in &selected {
                let _ = context.rng_r_ffff();
            }
        }
        let activity = match context.rng_next_u8() {
            0..=49 => "Steam",
            50..=99 => "守望先锋",
            100..=149 => "文明6",
            150..=189 => "英雄联盟",
            190..=229 => "微博",
            _ => "朋友圈",
        };
        let owner = context.owner_idx();
        let owner_name = context.owner().expect("runtime_v2 lazy owner must exist").template.display_name.clone();
        context.add_update(crate::engine::update::RunUpdate::new(
            format!("{owner_name}打开了{activity}, 这回合什么也没做"),
            owner.0 as usize,
            owner.0 as usize,
            0,
        ));
        context.intercept_action();
        return;
    }

    if context.hook().intersects(ProcMask::POST_ACTION) && context.entity(boss).is_ok_and(|boss_entity| boss_entity.runtime.alive)
    {
        context.push(QueuedEffect::LazyFlare {
            owner: context.owner_idx(),
            boss,
        });
    }
}

pub fn run_saitama_boss_state(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
    if context.hook() != ProcMask::POST_DEFEND {
        return;
    }
    let Some(StatePayload::SaitamaBoss {
        turns,
        mut damages,
        mut hitters,
        mut minions,
    }) = context.owner_state_payload(entry.legacy_order_key)
    else {
        return;
    };
    let damage = context.defend_damage().expect("runtime_v2 saitama state requires POST_DEFEND damage");
    let caster = context.defend_caster().expect("runtime_v2 saitama state requires a damage caster");
    damages += damage;
    let caster_entity = context
        .entity(caster)
        .unwrap_or_else(|error| panic!("runtime_v2 saitama caster lookup failed: {error:?}"));
    let hitter = if caster_entity.runtime.flags.contains(PlayerKindFlags::MINION) && caster_entity.runtime.owner != caster {
        if !minions.contains(&caster) {
            minions.push(caster);
        }
        caster_entity.runtime.owner
    } else {
        caster
    };
    if !hitters.contains(&hitter) {
        hitters.push(hitter);
    }
    context
        .set_owner_state_payload(
            entry.legacy_order_key,
            StatePayload::SaitamaBoss {
                turns,
                damages,
                hitters,
                minions,
            },
        )
        .expect("runtime_v2 saitama state owner must exist");
    context.set_defend_damage(damage / 100);
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
    let killed_target = context.selected_target().unwrap_or(EntityIdx(1));
    run_zombie_minion_from_template_slot_with_config(context, EntitySlotId(0), TemplateSlotId(0), killed_target);
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

pub const DEFAULT_CORE_DEFEND_SKILL_EXPORT: &str = "core.skill.defend";
pub const DEFAULT_CORE_REFLECT_SKILL_EXPORT: &str = "core.skill.reflect";
pub const DEFAULT_CORE_PROTECT_SKILL_EXPORT: &str = "core.skill.protect";
pub const DEFAULT_CORE_SHIELD_SKILL_EXPORT: &str = "core.skill.shield";
pub const DEFAULT_CORE_UPGRADE_SKILL_EXPORT: &str = "core.skill.upgrade";
pub const DEFAULT_CORE_HIDE_SKILL_EXPORT: &str = "core.skill.hide";
pub const DEFAULT_CORE_COUNTER_SKILL_EXPORT: &str = "core.skill.counter";
pub const DEFAULT_CORE_MERGE_SKILL_EXPORT: &str = "core.skill.merge";
pub const DEFAULT_CORE_RERAISE_SKILL_EXPORT: &str = "core.skill.reraise";
pub const DEFAULT_CORE_CHARM_STATE_EXPORT: &str = "core.state.charm";
pub const DEFAULT_CORE_CURSE_STATE_EXPORT: &str = "core.state.curse";
pub const DEFAULT_CORE_POISON_STATE_EXPORT: &str = "core.state.poison";
pub const DEFAULT_CORE_HASTE_STATE_EXPORT: &str = "core.state.haste";
pub const DEFAULT_CORE_SLOW_STATE_EXPORT: &str = "core.state.slow";
pub const DEFAULT_CORE_IRON_STATE_EXPORT: &str = "core.state.iron";
pub const DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT: &str = "core.state.covid-infection";
pub const DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT: &str = "core.state.lazy-infection";
pub const DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT: &str = "core.state.saitama-boss";
pub const DEFAULT_CORE_SHADOW_KIND_EXPORT: &str = "core.kind.shadow";
pub const DEFAULT_CORE_BOSS_KIND_EXPORT: &str = "core.kind.boss";
pub const DEFAULT_CORE_BOOST_KIND_EXPORT: &str = "core.kind.boost";
pub const DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.shadow_blueprint";
pub const DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT: &str = "core.entity.minion_counter";
pub const DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT: &str = "custom.summon";
pub const DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT: &str = "custom.summon.fire";
pub const DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT: &str = "custom.summon.explode";
pub const DEFAULT_CUSTOM_BED2_SUMMON_ENTITY_EXPORT: &str = "custom.bed2.summoned_entity";
pub const DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT: &str = "custom.bed2.summon_template";
pub const DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT: &str = "custom.bed2.shadow_template";
pub const DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT: &str = "custom.bed2.zombie_template";
pub const DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT: &str = "custom.bed2.summon";
pub const DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT: &str = "custom.bed2.shadow";
pub const DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT: &str = "custom.bed2.zombie";
pub const DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT: &str = "custom.minion.possess";
pub const DEFAULT_CUSTOM_MINION_SKILL_EXPORT_PREFIX: &str = "custom.minion";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinActiveSkill {
    Fire,
    Ice,
    Thunder,
    Quake,
    Absorb,
    Poison,
    Rapid,
    Critical,
    Half,
    Exchange,
    Berserk,
    Charm,
    Haste,
    Slow,
    Curse,
    Heal,
    Revive,
    Disperse,
    Iron,
    Charge,
    Accumulate,
    Assassinate,
    Summon,
    Clone,
    Shadow,
    Possess,
}

const PLAIN_FIRE_STATE_KEY: u32 = 0;
const PLAIN_ICE_STATE_KEY: u32 = 1;
const PLAIN_BERSERK_STATE_KEY: u32 = 10;
const PLAIN_CURSE_STATE_KEY: u32 = 73;
const PLAIN_POISON_STATE_KEY: u32 = 75;
const PLAIN_HASTE_STATE_KEY: u32 = 77;
const PLAIN_IRON_STATE_KEY: u32 = 79;
const PLAIN_COVID_BOSS_STATE_KEY: u32 = 90;
const PLAIN_COVID_INFECTION_STATE_KEY: u32 = 91;
const PLAIN_LAZY_BOSS_STATE_KEY: u32 = 92;
const PLAIN_LAZY_INFECTION_STATE_KEY: u32 = 93;
const PLAIN_SAITAMA_BOSS_STATE_KEY: u32 = 94;

impl BuiltinActiveSkill {
    const CORE: [Self; 25] = [
        Self::Fire,
        Self::Ice,
        Self::Thunder,
        Self::Quake,
        Self::Absorb,
        Self::Poison,
        Self::Rapid,
        Self::Critical,
        Self::Half,
        Self::Exchange,
        Self::Berserk,
        Self::Charm,
        Self::Haste,
        Self::Slow,
        Self::Curse,
        Self::Heal,
        Self::Revive,
        Self::Disperse,
        Self::Iron,
        Self::Charge,
        Self::Accumulate,
        Self::Assassinate,
        Self::Summon,
        Self::Clone,
        Self::Shadow,
    ];

    const ALL: [Self; 26] = [
        Self::Fire,
        Self::Ice,
        Self::Thunder,
        Self::Quake,
        Self::Absorb,
        Self::Poison,
        Self::Rapid,
        Self::Critical,
        Self::Half,
        Self::Exchange,
        Self::Berserk,
        Self::Charm,
        Self::Haste,
        Self::Slow,
        Self::Curse,
        Self::Heal,
        Self::Revive,
        Self::Disperse,
        Self::Iron,
        Self::Charge,
        Self::Accumulate,
        Self::Assassinate,
        Self::Summon,
        Self::Clone,
        Self::Shadow,
        Self::Possess,
    ];

    const fn legacy_key(self) -> usize {
        match self {
            Self::Fire => 0,
            Self::Ice => 1,
            Self::Thunder => 2,
            Self::Quake => 3,
            Self::Absorb => 4,
            Self::Poison => 5,
            Self::Rapid => 6,
            Self::Critical => 7,
            Self::Half => 8,
            Self::Exchange => 9,
            Self::Berserk => 10,
            Self::Charm => 11,
            Self::Haste => 12,
            Self::Slow => 13,
            Self::Curse => 14,
            Self::Heal => 15,
            Self::Revive => 16,
            Self::Disperse => 17,
            Self::Iron => 18,
            Self::Charge => 19,
            Self::Accumulate => 20,
            Self::Assassinate => 21,
            Self::Summon => 22,
            Self::Clone => 23,
            Self::Shadow => 24,
            Self::Possess => 43,
        }
    }

    const fn local_name(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Ice => "ice",
            Self::Thunder => "thunder",
            Self::Quake => "quake",
            Self::Absorb => "absorb",
            Self::Poison => "poison",
            Self::Rapid => "rapid",
            Self::Critical => "critical",
            Self::Half => "half",
            Self::Exchange => "exchange",
            Self::Berserk => "berserk",
            Self::Charm => "charm",
            Self::Haste => "haste",
            Self::Slow => "slow",
            Self::Curse => "curse",
            Self::Heal => "heal",
            Self::Revive => "revive",
            Self::Disperse => "disperse",
            Self::Iron => "iron",
            Self::Charge => "charge",
            Self::Accumulate => "accumulate",
            Self::Assassinate => "assassinate",
            Self::Summon => "summon",
            Self::Clone => "clone",
            Self::Shadow => "shadow",
            Self::Possess => "minion-possess",
        }
    }

    const fn export_name(self) -> &'static str {
        match self {
            Self::Fire => "core.skill.fire",
            Self::Ice => "core.skill.ice",
            Self::Thunder => "core.skill.thunder",
            Self::Quake => "core.skill.quake",
            Self::Absorb => "core.skill.absorb",
            Self::Poison => "core.skill.poison",
            Self::Rapid => "core.skill.rapid",
            Self::Critical => "core.skill.critical",
            Self::Half => "core.skill.half",
            Self::Exchange => "core.skill.exchange",
            Self::Berserk => "core.skill.berserk",
            Self::Charm => "core.skill.charm",
            Self::Haste => "core.skill.haste",
            Self::Slow => "core.skill.slow",
            Self::Curse => "core.skill.curse",
            Self::Heal => "core.skill.heal",
            Self::Revive => "core.skill.revive",
            Self::Disperse => "core.skill.disperse",
            Self::Iron => "core.skill.iron",
            Self::Charge => "core.skill.charge",
            Self::Accumulate => "core.skill.accumulate",
            Self::Assassinate => "core.skill.assassinate",
            Self::Summon => "core.skill.summon",
            Self::Clone => "core.skill.clone",
            Self::Shadow => "core.skill.shadow",
            Self::Possess => DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT,
        }
    }

    const fn target_policy(self) -> TargetPolicy {
        match self {
            Self::Iron | Self::Charge | Self::Accumulate | Self::Summon | Self::Clone | Self::Shadow => TargetPolicy::None,
            Self::Haste | Self::Heal | Self::Revive => TargetPolicy::Ally,
            _ => TargetPolicy::Enemy,
        }
    }

    fn from_legacy_key(key: usize) -> Option<Self> { Self::ALL.into_iter().find(|skill| skill.legacy_key() == key) }

    fn from_export_name(export_name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|skill| skill.export_name() == export_name)
    }
}

fn import_plain_legacy_skill_loadout(
    registry: &ExtensionRegistry,
    snapshot: &crate::player::skill::store::SkillLoadoutSnapshot,
) -> SkillLoadout {
    let defend_kind = std::any::type_name::<crate::player::skill::defend::DefendSkill>();
    let defend_skill = registry.skill_id_by_export_name(DEFAULT_CORE_DEFEND_SKILL_EXPORT);
    let reflect_kind = std::any::type_name::<crate::player::skill::reflect::ReflectSkill>();
    let reflect_skill = registry.skill_id_by_export_name(DEFAULT_CORE_REFLECT_SKILL_EXPORT);
    let protect_kind = std::any::type_name::<crate::player::skill::protect::ProtectSkill>();
    let protect_skill = registry.skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT);
    let shield_kind = std::any::type_name::<crate::player::skill::shield::ShieldSkill>();
    let shield_skill = registry.skill_id_by_export_name(DEFAULT_CORE_SHIELD_SKILL_EXPORT);
    let upgrade_kind = std::any::type_name::<crate::player::skill::upgrade::UpgradeSkill>();
    let upgrade_skill = registry.skill_id_by_export_name(DEFAULT_CORE_UPGRADE_SKILL_EXPORT);
    let hide_kind = std::any::type_name::<crate::player::skill::hide::HideSkill>();
    let hide_skill = registry.skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT);
    let counter_kind = std::any::type_name::<crate::player::skill::counter::CounterSkill>();
    let counter_skill = registry.skill_id_by_export_name(DEFAULT_CORE_COUNTER_SKILL_EXPORT);
    let merge_kind = std::any::type_name::<crate::player::skill::merge::MergeSkill>();
    let merge_skill = registry.skill_id_by_export_name(DEFAULT_CORE_MERGE_SKILL_EXPORT);
    let reraise_kind = std::any::type_name::<crate::player::skill::reraise::ReraiseSkill>();
    let reraise_skill = registry.skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT);
    let possess_kind = std::any::type_name::<crate::player::skill::act::possess::PossessSkill>();
    let possess_skill = registry.skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT);
    let resolve = |entry: &crate::player::skill::store::SkillSnapshot| {
        let active_skill = if entry.runtime_kind == possess_kind {
            possess_skill
        } else {
            BuiltinActiveSkill::from_legacy_key(entry.key).and_then(|skill| registry.skill_id_by_export_name(skill.export_name()))
        };
        let skill_id = if let Some(skill_id) = active_skill {
            Some(skill_id)
        } else if entry.runtime_kind == defend_kind {
            defend_skill
        } else if entry.runtime_kind == reflect_kind {
            reflect_skill
        } else if entry.runtime_kind == protect_kind {
            protect_skill
        } else if entry.runtime_kind == shield_kind {
            shield_skill
        } else if entry.runtime_kind == upgrade_kind {
            upgrade_skill
        } else if entry.runtime_kind == hide_kind {
            hide_skill
        } else if entry.runtime_kind == counter_kind {
            counter_skill
        } else if entry.runtime_kind == merge_kind {
            merge_skill
        } else if entry.runtime_kind == reraise_kind {
            reraise_skill
        } else {
            None
        };
        skill_id.map(|skill_id| (entry.key, skill_id, entry.level, entry.boost.clone()))
    };

    let mut imported = Vec::new();
    for key in &snapshot.fixed_lanes {
        let Some(entry) = snapshot.entries.iter().find(|entry| entry.key == *key) else {
            continue;
        };
        if let Some(mapped) = resolve(entry) {
            imported.push(mapped);
        }
    }
    for entry in &snapshot.entries {
        if imported.iter().any(|(key, _, _, _)| *key == entry.key) {
            continue;
        }
        if let Some(mapped) = resolve(entry) {
            imported.push(mapped);
        }
    }

    let mut active_order = snapshot
        .active_order
        .iter()
        .filter_map(|key| imported.iter().position(|(imported_key, _, _, _)| imported_key == key))
        .collect::<Vec<_>>();
    for lane in 0..imported.len() {
        if !active_order.contains(&lane) {
            active_order.push(lane);
        }
    }

    let fixed_lane_keys = imported.iter().map(|(key, _, _, _)| *key).collect::<Vec<_>>();
    SkillLoadout::from_skill_levels_and_boosts(imported.into_iter().map(|(_, skill_id, level, boost)| (skill_id, level, boost)))
        .with_fixed_lane_keys(fixed_lane_keys)
        .with_active_order(active_order)
}

pub fn run_defend_post_defend_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let damage = context.defend_damage().expect("runtime_v2 defend skill must run during POST_DEFEND");
    let level = context.skill_level(entry);
    if context.rng_r255() >= level {
        return;
    }
    if !context.owner_mp_ready().expect("runtime_v2 defend skill owner must exist") {
        return;
    }
    let caster = context.defend_caster().expect("runtime_v2 defend skill must receive incoming caster");
    context.add_update(crate::engine::update::RunUpdate::new(
        "[0][防御]",
        context.owner_idx().0 as usize,
        caster.0 as usize,
        40,
    ));
    context.set_defend_damage(damage / 2);
}

pub fn run_reflect_pre_defend_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let atp = context.defend_atp().expect("runtime_v2 reflect skill must run during PRE_DEFEND");
    if !context.defend_caster_active().expect("runtime_v2 reflect incoming caster must exist") {
        return;
    }

    let level = context.skill_level(entry);
    if context.rng_r255() >= level
        || !context.rng_c50()
        || !context.owner_mp_ready().expect("runtime_v2 reflect skill owner must exist")
    {
        return;
    }

    let caster = context.defend_caster().expect("runtime_v2 reflect skill must receive incoming caster");
    let reflect_atp = (context.owner_attack_power(true).expect("runtime_v2 reflect skill owner must exist") * 0.5).min(atp);
    let mut update =
        crate::engine::update::RunUpdate::new("[0]使用[伤害反弹]", context.owner_idx().0 as usize, caster.0 as usize, 20);
    update.delay0 = 1500;
    context.add_update(update);
    context.set_defend_atp(0.0);
    context.push_nested(QueuedEffect::ReflectedAttack {
        caster: context.owner_idx(),
        target: caster,
        atp_bits: reflect_atp.to_bits(),
    });
}

pub fn run_shield_pre_action_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    let shield = context.owner_shield().expect("runtime_v2 shield skill owner must exist");
    if (level as i32) < shield {
        return;
    }
    let max = (1 + (level as i32 * 3 / 4)).max(1);
    let add = context.rng_next_i32(max) + 1;
    context.set_owner_shield(shield + add).expect("runtime_v2 shield skill owner must exist");
}

pub fn run_protect_post_action_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    context
        .refresh_owner_protect_target(level)
        .expect("runtime_v2 protect post-action context must access allies");
}

pub fn run_plain_passive_noop_skill(_: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {}

pub fn run_merge_kill_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    let roll = context.rng_r63();
    #[cfg(not(feature = "no_debug"))]
    if std::env::var_os("TSWN_PROBE_KILL").is_some() {
        eprintln!(
            "[kill_probe:v2:merge] owner={} target={:?} lane={} level={} roll={} pass={}",
            context.owner_idx().0,
            context.selected_target().map(|target| target.0),
            entry.fixed_lane,
            level,
            roll,
            roll < level,
        );
    }
    if roll >= level {
        return;
    }
    let Some(target) = context.selected_target() else {
        return;
    };
    context.push_nested(QueuedEffect::Merge {
        caster: context.owner_idx(),
        target,
    });
}

pub fn run_reraise_die_skill(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
    let level = context.skill_level(entry);
    if context.rng_r127() >= level {
        return;
    }
    let hp = context.rng_r16() as i32;
    context.reraise_owner(entry, hp).expect("runtime_v2 reraise owner must exist");
    let mut reraise_update = crate::engine::update::RunUpdate::new(
        "[0]使用[护身符]抵挡了一次死亡",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        80,
    );
    reraise_update.delay0 = 1500;
    context.add_update(reraise_update);
    let mut recover_update = crate::engine::update::RunUpdate::new(
        "[1]回复体力[2]点",
        context.owner_idx().0 as usize,
        context.owner_idx().0 as usize,
        0,
    );
    recover_update.param = Some(hp as u32);
    context.add_update(recover_update);
}

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
    skill_handlers: Vec<RuntimeV2SkillHandlerBinding>,
    state_handlers: Vec<RuntimeV2StateHandlerBinding>,
}

#[derive(Clone)]
struct RuntimeV2SkillHandlerBinding {
    skill_id: SkillId,
    handler: SkillHandlerFn,
    capabilities: Vec<ExtensionCapability>,
}

#[derive(Clone)]
struct RuntimeV2StateHandlerBinding {
    state_id: StateId,
    handler: StateHandlerFn,
    capabilities: Vec<ExtensionCapability>,
}

impl<'a> CustomRuntimeV2ImportConfig<'a> {
    pub fn new(registry: ExtensionRegistry, bed2_kind: PlayerKindId, bed2_summon_skill: SkillId) -> Self {
        Self {
            registry,
            bed2_kind,
            bed2_summon_skill,
            bed2_minion_overlays: None,
            skill_handlers: Vec::new(),
            state_handlers: Vec::new(),
        }
    }

    pub fn with_bed2_minion_overlays(mut self, config: CustomBed2MinionOverlayConfig<'a>) -> Self {
        self.bed2_minion_overlays = Some(config);
        self
    }

    pub fn with_skill_handler(mut self, skill_id: SkillId, handler: SkillHandlerFn) -> Self {
        self.skill_handlers.push(RuntimeV2SkillHandlerBinding {
            skill_id,
            handler,
            capabilities: Vec::new(),
        });
        self
    }

    pub fn with_skill_handler_with_capabilities(
        mut self,
        skill_id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) -> Self {
        self.skill_handlers.push(RuntimeV2SkillHandlerBinding {
            skill_id,
            handler,
            capabilities: capabilities.to_vec(),
        });
        self
    }

    pub fn with_state_handler(mut self, state_id: StateId, handler: StateHandlerFn) -> Self {
        self.state_handlers.push(RuntimeV2StateHandlerBinding {
            state_id,
            handler,
            capabilities: Vec::new(),
        });
        self
    }

    pub fn with_state_handler_with_capabilities(
        mut self,
        state_id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) -> Self {
        self.state_handlers.push(RuntimeV2StateHandlerBinding {
            state_id,
            handler,
            capabilities: capabilities.to_vec(),
        });
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
    let summon_fire = builder.register_skill(
        "custom",
        "summon-fire",
        DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT,
        TargetPolicy::Enemy,
        SkillPriority(1),
    )?;
    let summon_explode = builder.register_skill(
        "custom",
        "summon-explode",
        DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT,
        TargetPolicy::Enemy,
        SkillPriority(2),
    )?;
    let possess = builder.register_skill_with_hooks(
        "custom",
        "minion-possess",
        DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT,
        ProcMask::NONE,
        TargetPolicy::Enemy,
        SkillPriority(3),
    )?;
    builder.register_skill(
        "custom",
        "minion-heal",
        "custom.minion.heal",
        TargetPolicy::Ally,
        SkillPriority(4),
    )?;
    let mut charge = None;
    for builtin_skill in BuiltinActiveSkill::CORE {
        let skill_id = if builtin_skill == BuiltinActiveSkill::Charge {
            builder.register_skill_with_hooks_and_post_action_phase(
                "core",
                builtin_skill.local_name(),
                builtin_skill.export_name(),
                ProcMask::POST_ACTION,
                builtin_skill.target_policy(),
                SkillPriority(builtin_skill.legacy_key() as i32),
                SkillPostActionPhase::Late,
            )?
        } else {
            builder.register_skill_with_hooks(
                "core",
                builtin_skill.local_name(),
                builtin_skill.export_name(),
                ProcMask::NONE,
                builtin_skill.target_policy(),
                SkillPriority(builtin_skill.legacy_key() as i32),
            )?
        };
        if builtin_skill == BuiltinActiveSkill::Charge {
            charge = Some(skill_id);
        }
    }
    let charge = charge.expect("default runtime v2 profile must register ChargeSkill");
    let charm_state = builder.register_state(
        "core",
        "charm",
        DEFAULT_CORE_CHARM_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(210),
    )?;
    let curse_state = builder.register_state(
        "core",
        "curse",
        DEFAULT_CORE_CURSE_STATE_EXPORT,
        ProcMask::POST_DEFEND,
        SkillPriority(10_000),
    )?;
    let poison_state = builder.register_state(
        "core",
        "poison",
        DEFAULT_CORE_POISON_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(150),
    )?;
    let haste_state = builder.register_state(
        "core",
        "haste",
        DEFAULT_CORE_HASTE_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(210),
    )?;
    let slow_state = builder.register_state(
        "core",
        "slow",
        DEFAULT_CORE_SLOW_STATE_EXPORT,
        ProcMask::POST_ACTION,
        SkillPriority(210),
    )?;
    let iron_state = builder.register_state(
        "core",
        "iron",
        DEFAULT_CORE_IRON_STATE_EXPORT,
        ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
        SkillPriority(10),
    )?;
    let covid_infection_state = builder.register_state(
        "core",
        "covid-infection",
        DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT,
        ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
        SkillPriority(1000),
    )?;
    let lazy_infection_state = builder.register_state(
        "core",
        "lazy-infection",
        DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT,
        ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
        SkillPriority(1000),
    )?;
    let saitama_boss_state = builder.register_state(
        "core",
        "saitama-boss",
        DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT,
        ProcMask::POST_DEFEND,
        SkillPriority(i32::MAX),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "boss",
        DEFAULT_CORE_BOSS_KIND_EXPORT,
        PlayerKindFlags::BOSS,
        PlayerKindPolicies::default(),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "boost",
        DEFAULT_CORE_BOOST_KIND_EXPORT,
        PlayerKindFlags::BOOST,
        PlayerKindPolicies::default(),
    )?;
    builder.register_player_kind_with_policies(
        "core",
        "shadow",
        DEFAULT_CORE_SHADOW_KIND_EXPORT,
        PlayerKindFlags::MINION,
        PlayerKindPolicies::default(),
    )?;
    let shield = builder.register_skill_with_hooks(
        "core",
        "shield",
        DEFAULT_CORE_SHIELD_SKILL_EXPORT,
        ProcMask::PRE_ACTION,
        TargetPolicy::None,
        SkillPriority(0),
    )?;
    let protect = builder.register_skill_with_hooks(
        "core",
        "protect",
        DEFAULT_CORE_PROTECT_SKILL_EXPORT,
        ProcMask::POST_ACTION,
        TargetPolicy::Ally,
        SkillPriority(0),
    )?;
    let defend = builder.register_skill_with_hooks(
        "core",
        "defend",
        DEFAULT_CORE_DEFEND_SKILL_EXPORT,
        ProcMask::POST_DEFEND,
        TargetPolicy::None,
        SkillPriority(2000),
    )?;
    let reflect = builder.register_skill_with_hooks(
        "core",
        "reflect",
        DEFAULT_CORE_REFLECT_SKILL_EXPORT,
        ProcMask::PRE_DEFEND,
        TargetPolicy::None,
        SkillPriority(1000),
    )?;
    let upgrade = builder.register_skill(
        "core",
        "upgrade",
        DEFAULT_CORE_UPGRADE_SKILL_EXPORT,
        TargetPolicy::None,
        SkillPriority(33),
    )?;
    let hide = builder.register_skill(
        "core",
        "hide",
        DEFAULT_CORE_HIDE_SKILL_EXPORT,
        TargetPolicy::None,
        SkillPriority(34),
    )?;
    let counter = builder.register_skill_with_hooks(
        "core",
        "counter",
        DEFAULT_CORE_COUNTER_SKILL_EXPORT,
        ProcMask::POST_DAMAGE,
        TargetPolicy::None,
        SkillPriority(30),
    )?;
    let merge = builder.register_skill_with_hooks(
        "core",
        "merge",
        DEFAULT_CORE_MERGE_SKILL_EXPORT,
        ProcMask::KILL,
        TargetPolicy::Enemy,
        SkillPriority(31),
    )?;
    let reraise = builder.register_skill_with_hooks(
        "core",
        "reraise",
        DEFAULT_CORE_RERAISE_SKILL_EXPORT,
        ProcMask::DIE,
        TargetPolicy::None,
        SkillPriority(10),
    )?;
    builder.reserve_entity_slot("core", "shadow-blueprint", DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("core", "minion-counter", DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)?;
    builder.reserve_entity_slot("custom", "bed2-summoned-entity", DEFAULT_CUSTOM_BED2_SUMMON_ENTITY_EXPORT)?;
    let summon_template_slot =
        builder.reserve_template_slot("custom", "bed2-summon-template", DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT)?;
    let shadow_template_slot =
        builder.reserve_template_slot("custom", "bed2-shadow-template", DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT)?;
    let zombie_template_slot =
        builder.reserve_template_slot("custom", "bed2-zombie-template", DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT)?;
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
    let summon_kind = builder.register_player_kind_with_policies(
        "custom",
        "bed2-summon",
        DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT,
        PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: true,
        },
    )?;
    let shadow_kind = builder.register_player_kind_with_policies(
        "custom",
        "bed2-shadow",
        DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT,
        PlayerKindFlags::MINION,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: false,
        },
    )?;
    let zombie_kind = builder.register_player_kind_with_policies(
        "custom",
        "bed2-zombie",
        DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT,
        PlayerKindFlags::MINION,
        PlayerKindPolicies {
            owner_resolution: OwnerResolutionPolicy::RootOwner,
            damage_share: DamageSharePolicy::ShareToOwner,
            merge: MergePolicy::FixedLane,
            inherit_owner_def_res: false,
        },
    )?;
    Ok(CustomRuntimeV2ImportConfig::new(builder.build(), bed2, summon)
        .with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
            summon: CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT,
                explode_skill_export_name: DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT,
            },
            shadow: CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT,
            },
            zombie: CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: DEFAULT_CUSTOM_MINION_SKILL_EXPORT_PREFIX,
            },
        })
        .with_skill_handler_with_capabilities(
            summon,
            run_legacy_summon_recast_from_template_slot,
            &[
                ExtensionCapability::ReadTemplateSlots,
                ExtensionCapability::ReadAllies,
                ExtensionCapability::MutateEntitySlots,
            ],
        )
        .with_skill_handler(summon_fire, run_summon_fire_skill)
        .with_skill_handler(summon_explode, run_summon_explode_skill)
        .with_skill_handler(possess, run_possess_skill)
        .with_skill_handler(shield, run_shield_pre_action_skill)
        .with_skill_handler_with_capabilities(protect, run_protect_post_action_skill, &[ExtensionCapability::ReadAllies])
        .with_skill_handler(defend, run_defend_post_defend_skill)
        .with_skill_handler(reflect, run_reflect_pre_defend_skill)
        .with_skill_handler(charge, run_charge_post_action_skill)
        .with_skill_handler(upgrade, run_plain_passive_noop_skill)
        .with_skill_handler(hide, run_plain_passive_noop_skill)
        .with_skill_handler(counter, run_plain_passive_noop_skill)
        .with_skill_handler(merge, run_merge_kill_skill)
        .with_skill_handler(reraise, run_reraise_die_skill)
        .with_state_handler(charm_state, run_charm_post_action_state)
        .with_state_handler(curse_state, run_curse_post_defend_state)
        .with_state_handler(poison_state, run_poison_post_action_state)
        .with_state_handler(haste_state, run_haste_post_action_state)
        .with_state_handler(slow_state, run_slow_post_action_state)
        .with_state_handler(iron_state, run_iron_post_defend_state)
        .with_state_handler_with_capabilities(
            covid_infection_state,
            run_covid_infection_state,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::ReadEnemies],
        )
        .with_state_handler_with_capabilities(
            lazy_infection_state,
            run_lazy_infection_state,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::ReadEnemies],
        )
        .with_state_handler_with_capabilities(
            saitama_boss_state,
            run_saitama_boss_state,
            &[ExtensionCapability::ReadAllies, ExtensionCapability::ReadEnemies],
        ))
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
    NotReady(RuntimeV2ReadyError),
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

impl From<RuntimeV2ReadyError> for CustomRuntimeV2ImportError {
    fn from(error: RuntimeV2ReadyError) -> Self { Self::NotReady(error) }
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
                        .with_display_name(player.display_name())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectedBuiltinSkill {
    skill: BuiltinActiveSkill,
    fixed_lane: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedBuiltinSkillAction {
    selected: SelectedBuiltinSkill,
    targets: Vec<EntityIdx>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlainAttackOnDamage {
    None,
    Absorb,
    Berserk,
    Curse,
    Poison,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreparedPlainAction {
    BasicAttack {
        target: EntityIdx,
        use_magic: bool,
        amount: i32,
    },
    ForcedAttack {
        target: EntityIdx,
        amount: i32,
    },
    Saitama {
        target: Option<EntityIdx>,
    },
    BuiltinSkill(PreparedBuiltinSkillAction),
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
        let scheduler = PhaseScheduler::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&registry);
        let effect_handlers = EffectHandlers::from_registry(&registry);
        let skill_handlers = SkillHandlers::from_registry(&registry);
        let state_handlers = StateHandlers::from_registry(&registry);
        let replay_renderers = ReplayRenderers::from_registry(&registry);
        let show_renderers = ShowRenderers::from_registry(&registry);
        Self {
            entities,
            world,
            scheduler,
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

    pub fn validate_ready(&self) -> Result<(), RuntimeV2ReadyError> {
        let mut missing_skill_handlers = Vec::new();
        for (entity_idx, entity) in self.entities.iter() {
            self.collect_missing_skill_handlers(
                entity.template.skills.skills(),
                RuntimeV2SkillSource::Entity(entity_idx),
                &mut missing_skill_handlers,
            );
        }
        for (slot_id, value) in self.template_slots.iter() {
            if let SlotValue::PlayerTemplate(template) = value {
                self.collect_missing_skill_handlers(
                    template.skills.skills(),
                    RuntimeV2SkillSource::TemplateSlot(slot_id),
                    &mut missing_skill_handlers,
                );
            }
        }
        if missing_skill_handlers.is_empty() {
            Ok(())
        } else {
            Err(RuntimeV2ReadyError { missing_skill_handlers })
        }
    }

    fn collect_missing_skill_handlers(
        &self,
        skills: &[SkillId],
        source: RuntimeV2SkillSource,
        missing_skill_handlers: &mut Vec<RuntimeV2MissingSkillHandler>,
    ) {
        for skill_id in skills {
            if self.registry.skill(*skill_id).is_some_and(|spec| spec.hook_mask.is_empty()) {
                continue;
            }
            if self.skill_handlers.get(*skill_id).is_some() {
                continue;
            }
            if let Some(missing) = missing_skill_handlers.iter_mut().find(|missing| missing.skill_id == *skill_id) {
                if !missing.sources.contains(&source) {
                    missing.sources.push(source.clone());
                }
                continue;
            }
            missing_skill_handlers.push(RuntimeV2MissingSkillHandler {
                skill_id: *skill_id,
                export_name: self.registry.skill(*skill_id).map(|spec| spec.export_name.clone()),
                sources: vec![source.clone()],
            });
        }
    }

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
            if plan.hook.intersects(ProcMask::DIE) && self.entities.get(plan.owner).is_some_and(|entity| entity.runtime.hp > 0) {
                break;
            }
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

    fn drain_state_hook_plan_into(&mut self, plan: &StateHookPlan, updates: &mut RunUpdates) -> bool {
        self.drain_state_hook_plan_with_action_smart_into(plan, updates, None)
    }

    fn drain_state_hook_plan_with_action_smart_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        action_smart: Option<bool>,
    ) -> bool {
        let mut action_intercepted = false;
        for entry in &plan.entries {
            let Some(state_id) = entry.state_id else {
                continue;
            };
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
            };
            {
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(&[]);
                let context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    plan.hook,
                    capabilities,
                );
                let mut context = if let Some(smart) = action_smart {
                    context.with_action_smart(smart)
                } else {
                    context
                };
                handler(&mut context, entry);
                action_intercepted |= context.action_intercepted();
            }
            self.drain_effects_into(updates);
        }
        action_intercepted
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
                    plan.hook,
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

        let selected_action = self.scheduler.select_action(&mut self.world, &mut self.entities, &mut self.rng);
        let mut updates = RunUpdates::new();
        for target in self.scheduler.take_ice_release_events() {
            updates.add_newline();
            updates.add(RuntimeFrame::replay_update(
                target.0 as usize,
                target.0 as usize,
                "[1]从[冰冻]中解除",
                0,
            ));
        }
        let Some(mut action) = selected_action else {
            return self.finish_round(None, updates);
        };
        let legacy_plain_action = self.scheduler.uses_legacy_step_scheduler();
        self.scratch.selected_actor_round = self.round;
        #[cfg(not(feature = "no_debug"))]
        let debug_tick = std::env::var_os("TSWN_DEBUG_TICK").is_some();
        #[cfg(not(feature = "no_debug"))]
        let tick_rng_before = (self.rng.i, self.rng.j);
        #[cfg(not(feature = "no_debug"))]
        if debug_tick {
            let actor = self
                .entities
                .get(action.actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 debug tick actor: {}", action.actor.0));
            eprintln!(
                "[v2_tick] actor={} id={} mv={} hp={} rc4=({}, {})",
                actor.template.name,
                action.actor.0,
                actor.runtime.move_state.speed_points,
                actor.runtime.hp,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        let action_rng_before = RngCheckpoint::from_rc4(&self.rng);
        let smart = self.roll_actor_smart(action.actor);
        if legacy_plain_action {
            self.clear_plain_hide_before_action(action.actor);
        }

        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_ACTION);
        let selected_target = if legacy_plain_action {
            action.target
        } else {
            self.selected_pre_action_target(&skill_plan, action.actor, smart).unwrap_or(action.target)
        };
        self.drain_skill_hook_plan_with_selected_target_into(&skill_plan, &mut updates, Some(selected_target));
        let pre_action_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_ACTION);
        let state_intercepted_action =
            self.drain_state_hook_plan_with_action_smart_into(&pre_action_state_plan, &mut updates, Some(smart));
        let mut prepared_plain_action = None;
        if legacy_plain_action && !state_intercepted_action {
            let Some(prepared) = self.prepare_plain_action(action.actor, smart) else {
                return self.finish_round(None, updates);
            };
            match &prepared {
                PreparedPlainAction::BasicAttack { target, amount, .. } => {
                    action.target = *target;
                    action.amount = *amount;
                }
                PreparedPlainAction::ForcedAttack { target, amount } => {
                    action.target = *target;
                    action.amount = *amount;
                }
                PreparedPlainAction::Saitama { target } => {
                    action.target = target.unwrap_or(action.actor);
                    action.amount = 0;
                }
                PreparedPlainAction::BuiltinSkill(prepared) => {
                    action.target = prepared.targets.first().copied().unwrap_or(action.actor);
                    action.amount = 0;
                }
            }
            prepared_plain_action = Some(prepared);
        }
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
        if state_intercepted_action {
            // PRE_ACTION state handlers own this action. Legacy still performs
            // recovery and the full post-action chain after the state action.
        } else if let Some(PreparedPlainAction::BuiltinSkill(prepared)) = prepared_plain_action.clone() {
            self.drain_plain_builtin_skill_into(action.actor, prepared, &mut updates);
        } else {
            let pre_damage_skill_plan =
                self.scheduler
                    .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_DAMAGE);
            self.drain_skill_hook_plan_into(&pre_damage_skill_plan, &mut updates);
            let pre_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_DAMAGE);
            self.drain_state_hook_plan_into(&pre_damage_state_plan, &mut updates);
            match prepared_plain_action {
                Some(PreparedPlainAction::BasicAttack { use_magic, .. }) => {
                    self.drain_plain_default_attack_into(action.actor, action.target, use_magic, &mut updates);
                }
                Some(PreparedPlainAction::ForcedAttack { target, .. }) => {
                    self.drain_plain_berserk_forced_attack_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::Saitama { target }) => {
                    self.drain_plain_saitama_action_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::BuiltinSkill(_)) => {
                    unreachable!("builtin skill actions are handled before the default action branch")
                }
                None => {
                    self.effects.push(QueuedEffect::Damage {
                        caster: action.actor,
                        target: action.target,
                        amount: action.amount,
                    });
                    self.drain_effects_into(&mut updates);
                }
            }
            let post_damage_skill_plan =
                self.scheduler
                    .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_DAMAGE);
            self.drain_skill_hook_plan_into(&post_damage_skill_plan, &mut updates);
            let post_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_DAMAGE);
            self.drain_state_hook_plan_into(&post_damage_state_plan, &mut updates);
        }
        if matches!(prepared_plain_action, Some(PreparedPlainAction::ForcedAttack { .. })) {
            self.drain_plain_berserk_forced_action_state_into(action.actor, &mut updates);
        }
        if legacy_plain_action {
            self.recover_plain_actor_into(action.actor, &mut updates);
        }
        if state_intercepted_action {
            updates.add_newline();
        }
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
        self.drain_plain_update_end_into(&mut updates);
        #[cfg(not(feature = "no_debug"))]
        if debug_tick {
            let actor = self
                .entities
                .get(action.actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 debug tick actor: {}", action.actor.0));
            let bytes = (self.rng.i as i32 - tick_rng_before.0 as i32).rem_euclid(256);
            eprintln!(
                "[v2_tick_end] actor={} id={} mp_after={} hp_after={} rc4=({},{})->({},{}) bytes={} messages={:?}",
                actor.template.name,
                action.actor.0,
                actor.runtime.move_state.speed_points,
                actor.runtime.hp,
                tick_rng_before.0,
                tick_rng_before.1,
                self.rng.i,
                self.rng.j,
                bytes,
                updates
                    .updates
                    .iter()
                    .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                    .map(|update| update.message.as_ref())
                    .collect::<Vec<_>>(),
            );
        }
        self.finish_round(Some(action), updates)
    }

    fn finish_round(&mut self, action: Option<ActionPlan>, updates: RunUpdates) -> RoundOutcome {
        let frame = updates.had_updates().then_some(RuntimeFrame { updates });
        self.round += 1;
        let winner_team = self.world.sync_winner(&self.entities);
        #[cfg(not(feature = "no_debug"))]
        if let (Some(trace), Some(frame)) = (&mut self.trace, &frame) {
            trace.record_frame(self.round, frame, winner_team, Some(RngCheckpoint::from_rc4(&self.rng)));
        }
        RoundOutcome {
            action,
            frame,
            winner_team,
        }
    }

    fn roll_actor_smart(&mut self, actor: EntityIdx) -> bool {
        let smart_byte = self.rng.next_u8();
        let smart_roll = (smart_byte & 63) as i32;
        self.entities.get(actor).is_some_and(|entity| entity.runtime.wisdom > smart_roll)
    }

    #[cfg(not(feature = "no_debug"))]
    fn probe_plain_action_matches(&self, actor: EntityIdx) -> bool {
        std::env::var("TSWN_PROBE_ACTION")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false)
    }

    fn prepare_plain_action(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedPlainAction> {
        if self.has_plain_berserk_state(actor) {
            let target = self.select_plain_berserk_forced_attack_target(smart)?;
            let amount = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 forced-attack actor: {}", actor.0))
                .runtime
                .attack;
            return Some(PreparedPlainAction::ForcedAttack {
                target,
                amount: (f64::from(amount) * 1.2000000476837158).round() as i32,
            });
        }

        #[cfg(not(feature = "no_debug"))]
        let rng_before = (self.rng.i, self.rng.j);
        let req_mp_byte = self.rng.next_u8();
        let req_mp = (req_mp_byte & 15) as i32 + 8;
        let (mp_before, is_boss, actor_name) = {
            let actor_entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 default attack actor: {}", actor.0));
            (
                actor_entity.runtime.magic_point,
                actor_entity.runtime.flags.contains(PlayerKindFlags::BOSS),
                actor_entity.template.name.clone(),
            )
        };
        let can_scan_skills = mp_before >= req_mp;
        #[cfg(not(feature = "no_debug"))]
        let probe_action = self.probe_plain_action_matches(actor);
        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 action probe actor: {}", actor.0));
            eprintln!(
                "[action_probe:v2:mp] round={} actor={} name={} smart={} req_mp_byte={} req_mp={} mp_before={} \
                 can_scan={} rc4=({},{}) -> ({},{})",
                self.round + 1,
                actor.0,
                entity.template.name,
                smart,
                req_mp_byte,
                req_mp,
                mp_before,
                can_scan_skills,
                rng_before.0,
                rng_before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 action probe actor: {}", actor.0));
            eprintln!(
                "[possess_probe:v2:mp] round={} actor={} name={} smart={} req_mp_byte={} req_mp={} mp_before={} \
                 can_scan={} rc4=({},{}) -> ({},{})",
                self.round + 1,
                actor.0,
                entity.template.name,
                smart,
                req_mp_byte,
                req_mp,
                mp_before,
                can_scan_skills,
                rng_before.0,
                rng_before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        let prepared_skill = if can_scan_skills {
            let selected = if is_boss {
                for _ in 0..crate::player::boss::boss_action_prob_count(&actor_name) {
                    let _ = self.rng.r127();
                }
                None
            } else {
                self.scan_plain_action_skill_probabilities(actor, smart)
            };
            self.entities.get_mut(actor).unwrap().runtime.magic_point -= req_mp;
            selected
        } else {
            None
        };
        if let Some(prepared) = prepared_skill {
            #[cfg(not(feature = "no_debug"))]
            if probe_action {
                eprintln!(
                    "[action_probe:v2:selected] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
                    actor.0,
                    prepared.selected.skill.export_name(),
                    prepared.selected.fixed_lane,
                    prepared.targets,
                    self.rng.i,
                    self.rng.j,
                );
            }
            return Some(PreparedPlainAction::BuiltinSkill(prepared));
        }

        if is_boss {
            if self.saitama_boss_state(actor).is_some() {
                let target = self.select_plain_default_attack_target(actor, smart);
                return Some(PreparedPlainAction::Saitama { target });
            }
            let target = self.select_plain_default_attack_target(actor, smart)?;
            let amount = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 boss actor: {}", actor.0))
                .runtime
                .attack;
            return Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: false,
                amount,
            });
        }

        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            eprintln!(
                "[action_probe:v2:fallback] actor={} rc4=({}, {})",
                actor.0, self.rng.i, self.rng.j
            );
        }
        let target = self.select_plain_default_attack_target(actor, smart)?;
        let actor_entity = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default attack actor: {}", actor.0));
        let attack = actor_entity.runtime.attack;
        let magic = actor_entity.runtime.magic;
        let magic_cost = (magic - attack) >> 2;
        let use_magic = smart && magic > attack && actor_entity.runtime.magic_point >= magic_cost;
        if use_magic {
            self.entities.get_mut(actor).unwrap().runtime.magic_point -= magic_cost;
            Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: true,
                amount: magic,
            })
        } else {
            Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: false,
                amount: attack,
            })
        }
    }

    fn has_plain_berserk_state(&self, actor: EntityIdx) -> bool {
        self.entities.get(actor).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
        })
    }

    fn select_plain_berserk_forced_attack_target(&mut self, smart: bool) -> Option<EntityIdx> {
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return None;
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = self.rng.pick(&all_alive)?;
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return None;
        }

        let mut scored = selected
            .into_iter()
            .map(|target| {
                let attract = self
                    .entities
                    .get(target)
                    .unwrap_or_else(|| panic!("runtime_v2 forced-attack target disappeared: {}", target.0))
                    .runtime
                    .attract();
                (target, self.rng.rFFFF() as f64 * attract)
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.first().map(|(target, _)| *target)
    }

    fn drain_plain_berserk_forced_attack_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0]发起[狂暴攻击]",
            0,
        ));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 forced-attack actor: {}", actor.0))
            .runtime
            .get_at(false, &mut self.rng)
            * 1.2000000476837158;
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
    }

    fn drain_plain_berserk_forced_action_state_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (state_key, clear_state, actor_alive) = {
            let actor_entity = self
                .entities
                .get_mut(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 forced-attack actor: {}", actor.0));
            let Some(entry) = actor_entity
                .states
                .entries()
                .iter()
                .find(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
            else {
                return;
            };
            let state_key = entry.legacy_order_key;
            let entry = actor_entity
                .states
                .entry_mut(state_key)
                .expect("runtime_v2 berserk state disappeared during forced action");
            let StatePayload::Berserk { step } = &mut entry.payload else {
                unreachable!("runtime_v2 berserk state key changed payload during forced action");
            };
            *step -= 1;
            (state_key, *step <= 0, actor_entity.runtime.active())
        };

        if !clear_state {
            return;
        }
        if actor_alive {
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                "[1]从[狂暴]中解除",
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
        }
        let removed = self
            .entities
            .get_mut(actor)
            .expect("runtime_v2 forced-attack actor disappeared before state clear")
            .states
            .clear_legacy_key(state_key);
        debug_assert!(removed, "runtime_v2 berserk state disappeared before state clear");
    }

    fn scan_plain_action_skill_probabilities(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBuiltinSkillAction> {
        let active_order = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 action-scan actor: {}", actor.0))
            .template
            .skills
            .active_order()
            .to_vec();
        for fixed_lane in active_order {
            let (skill_id, level) = {
                let loadout = &self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime_v2 action-scan actor: {}", actor.0))
                    .template
                    .skills;
                let Some(skill_id) = loadout.skills().get(fixed_lane).copied() else {
                    panic!("runtime_v2 active skill order references missing fixed lane {fixed_lane}");
                };
                let level = loadout
                    .level_at(fixed_lane)
                    .unwrap_or_else(|| panic!("runtime_v2 active skill level missing for fixed lane {fixed_lane}"));
                (skill_id, level)
            };
            if level == 0 {
                continue;
            }
            let Some(builtin_skill) = self.builtin_active_skill(skill_id) else {
                continue;
            };
            if self.plain_action_skill_probability(actor, builtin_skill, level, smart) {
                let selected = SelectedBuiltinSkill {
                    skill: builtin_skill,
                    fixed_lane,
                };
                let targets = match builtin_skill {
                    BuiltinActiveSkill::Fire
                    | BuiltinActiveSkill::Thunder
                    | BuiltinActiveSkill::Absorb
                    | BuiltinActiveSkill::Poison
                    | BuiltinActiveSkill::Critical => self.select_plain_default_enemy_targets(actor, smart),
                    BuiltinActiveSkill::Berserk => self.select_plain_berserk_targets(actor, smart),
                    BuiltinActiveSkill::Quake => {
                        self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 6 } else { 5 })
                    }
                    BuiltinActiveSkill::Ice => self.select_plain_ice_targets(actor, smart),
                    BuiltinActiveSkill::Rapid => self.select_plain_rapid_targets(actor, smart),
                    BuiltinActiveSkill::Half => self.select_plain_half_targets(actor, smart),
                    BuiltinActiveSkill::Shadow => vec![actor],
                    BuiltinActiveSkill::Charm => self.select_plain_charm_targets(actor, smart),
                    BuiltinActiveSkill::Curse => self.select_plain_curse_targets(actor, smart),
                    BuiltinActiveSkill::Haste => self.select_plain_haste_targets(actor, smart),
                    BuiltinActiveSkill::Heal => self.select_plain_heal_targets(actor, smart),
                    BuiltinActiveSkill::Slow => self.select_plain_slow_targets(actor, smart),
                    BuiltinActiveSkill::Exchange => self.select_plain_exchange_targets(actor, smart),
                    BuiltinActiveSkill::Revive => self.select_plain_revive_targets(actor, smart),
                    BuiltinActiveSkill::Disperse => self.select_plain_disperse_targets(actor, smart),
                    BuiltinActiveSkill::Iron => vec![actor],
                    BuiltinActiveSkill::Clone => vec![actor],
                    BuiltinActiveSkill::Charge => vec![actor],
                    BuiltinActiveSkill::Accumulate => vec![actor],
                    BuiltinActiveSkill::Possess => self.select_plain_possess_targets(actor, smart),
                    _ => return None,
                };
                #[cfg(not(feature = "no_debug"))]
                if self.probe_plain_action_matches(actor) {
                    eprintln!(
                        "[action_probe:v2:targets] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
                        actor.0,
                        builtin_skill.export_name(),
                        fixed_lane,
                        targets,
                        self.rng.i,
                        self.rng.j,
                    );
                }
                if targets.is_empty() {
                    continue;
                }
                return Some(PreparedBuiltinSkillAction { selected, targets });
            }
        }
        None
    }

    fn builtin_active_skill(&self, skill_id: SkillId) -> Option<BuiltinActiveSkill> {
        BuiltinActiveSkill::from_export_name(&self.registry.skill(skill_id)?.export_name)
    }

    fn plain_action_skill_probability(
        &mut self,
        actor: EntityIdx,
        builtin_skill: BuiltinActiveSkill,
        level: u32,
        smart: bool,
    ) -> bool {
        #[cfg(not(feature = "no_debug"))]
        let probe_action = self.probe_plain_action_matches(actor);
        if builtin_skill == BuiltinActiveSkill::Charge {
            let actor_runtime = &self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 charge probability actor: {}", actor.0))
                .runtime;
            if actor_runtime.charge.active || (smart && actor_runtime.hp < 100) {
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=charge_gate active={} hp={} \
                         rc4=({}, {})",
                        actor.0,
                        builtin_skill.export_name(),
                        level,
                        smart,
                        actor_runtime.charge.active,
                        actor_runtime.hp,
                        self.rng.i,
                        self.rng.j,
                    );
                }
                return false;
            }
        }
        if builtin_skill == BuiltinActiveSkill::Absorb && smart {
            let actor_entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 absorb probability actor: {}", actor.0));
            if actor_entity.template.max_hp - actor_entity.runtime.hp < 32 {
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=absorb_low_missing_hp \
                         hp={} max_hp={} rc4=({}, {})",
                        actor.0,
                        builtin_skill.export_name(),
                        level,
                        smart,
                        actor_entity.runtime.hp,
                        actor_entity.template.max_hp,
                        self.rng.i,
                        self.rng.j,
                    );
                }
                return false;
            }
        }
        if builtin_skill == BuiltinActiveSkill::Iron
            && self
                .entities
                .get(actor)
                .and_then(|entity| entity.states.entry(PLAIN_IRON_STATE_KEY))
                .and_then(StateEntry::iron_value)
                .is_some_and(|(protect, step)| protect > 0 && step > 0)
        {
            return false;
        }
        if builtin_skill == BuiltinActiveSkill::Accumulate {
            let actor_runtime = &self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 accumulate probability actor: {}", actor.0))
                .runtime;
            if actor_runtime.accumulate.active || (smart && actor_runtime.hp < 120) {
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=accumulate_gate \
                         active={} hp={} rc4=({}, {})",
                        actor.0,
                        builtin_skill.export_name(),
                        level,
                        smart,
                        actor_runtime.accumulate.active,
                        actor_runtime.hp,
                        self.rng.i,
                        self.rng.j,
                    );
                }
                return false;
            }
        }
        // ShadowSkill 在 smart 模式且 HP < 80 时短路，不消耗概率字节。
        if builtin_skill == BuiltinActiveSkill::Shadow
            && smart
            && self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 shadow probability actor: {}", actor.0))
                .runtime
                .hp
                < 80
        {
            #[cfg(not(feature = "no_debug"))]
            if probe_action {
                eprintln!(
                    "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=shadow_low_hp rc4=({}, {})",
                    actor.0,
                    builtin_skill.export_name(),
                    level,
                    smart,
                    self.rng.i,
                    self.rng.j,
                );
            }
            return false;
        }
        #[cfg(not(feature = "no_debug"))]
        let before = (self.rng.i, self.rng.j);
        let roll = self.rng.r127();
        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            eprintln!(
                "[action_probe:v2:prob] actor={} skill={} level={} smart={} roll={} pass={} rc4=({},{}) -> ({},{})",
                actor.0,
                builtin_skill.export_name(),
                level,
                smart,
                roll,
                roll < level,
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if builtin_skill == BuiltinActiveSkill::Charm && std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:prob] actor={} level={} roll={} pass={} rc4=({},{}) -> ({},{})",
                actor.0,
                level,
                roll,
                roll < level,
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if builtin_skill == BuiltinActiveSkill::Possess && std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 possess probe actor: {}", actor.0));
            eprintln!(
                "[possess_probe:v2:prob] round={} actor={} name={} smart={} level={} roll={} pass={} \
                 rc4=({},{}) -> ({},{})",
                self.round + 1,
                actor.0,
                entity.template.name,
                smart,
                level,
                roll,
                roll < level,
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        roll < level
    }

    fn drain_plain_builtin_skill_into(
        &mut self,
        actor: EntityIdx,
        prepared: PreparedBuiltinSkillAction,
        updates: &mut RunUpdates,
    ) {
        match prepared.selected.skill {
            BuiltinActiveSkill::Fire => {
                let target = prepared.targets[0];
                self.drain_plain_fire_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Thunder => {
                let target = prepared.targets[0];
                self.drain_plain_thunder_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Quake => {
                self.drain_plain_quake_skill_into(actor, prepared.targets, updates);
            }
            BuiltinActiveSkill::Absorb => {
                let target = prepared.targets[0];
                self.drain_plain_absorb_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Poison => {
                let target = prepared.targets[0];
                self.drain_plain_poison_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Critical => {
                let target = prepared.targets[0];
                self.drain_plain_critical_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Berserk => {
                let target = prepared.targets[0];
                self.drain_plain_berserk_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Ice => {
                let target = prepared.targets[0];
                self.drain_plain_ice_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Rapid => {
                self.drain_plain_rapid_skill_into(actor, prepared.targets, updates);
            }
            BuiltinActiveSkill::Half => {
                let target = prepared.targets[0];
                self.drain_plain_half_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Curse => {
                let target = prepared.targets[0];
                self.drain_plain_curse_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Haste => {
                let target = prepared.targets[0];
                self.drain_plain_haste_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Heal => {
                let target = prepared.targets[0];
                self.drain_plain_heal_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Shadow => {
                self.drain_plain_shadow_skill_into(actor, prepared.selected.fixed_lane, updates);
            }
            BuiltinActiveSkill::Charm => {
                let target = prepared.targets[0];
                self.drain_plain_charm_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Slow => {
                let target = prepared.targets[0];
                self.drain_plain_slow_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Exchange => {
                let target = prepared.targets[0];
                self.drain_plain_exchange_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Revive => {
                let target = prepared.targets[0];
                self.drain_plain_revive_skill_into(actor, prepared.selected.fixed_lane, target, updates);
            }
            BuiltinActiveSkill::Disperse => {
                let target = prepared.targets[0];
                self.drain_plain_disperse_skill_into(actor, target, updates);
            }
            BuiltinActiveSkill::Iron => {
                self.drain_plain_iron_skill_into(actor, updates);
            }
            BuiltinActiveSkill::Clone => {
                self.drain_plain_clone_skill_into(actor, prepared.selected.fixed_lane, updates);
            }
            BuiltinActiveSkill::Charge => {
                self.drain_plain_charge_skill_into(actor, updates);
            }
            BuiltinActiveSkill::Accumulate => {
                self.drain_plain_accumulate_skill_into(actor, updates);
            }
            BuiltinActiveSkill::Possess => {
                let target = prepared.targets[0];
                self.drain_plain_possess_skill_into(actor, target, updates);
            }
            _ => unreachable!("only migrated builtin skills may produce PreparedPlainAction::BuiltinSkill"),
        }
    }

    fn select_plain_default_enemy_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 3 } else { 2 })
    }

    fn select_plain_default_enemy_targets_with_count(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        select_count: usize,
    ) -> Vec<EntityIdx> {
        if select_count == 0 {
            return Vec::new();
        }
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let ally_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, candidate)| (self.plain_effective_team(*candidate) == actor_team).then_some(index))
            .collect::<Vec<_>>();
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if ally_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &ally_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_default_enemy_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_default_enemy_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default enemy target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / rate_hi_hp(entity.runtime.hp)) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    fn drain_plain_fire_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.effects.push(QueuedEffect::FireAttack {
            caster: actor,
            target,
            fire_state_key: PLAIN_FIRE_STATE_KEY,
        });
        self.drain_effects_into(updates);
    }

    fn drain_plain_thunder_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[雷击术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let mut accuracy = 100
            + self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 thunder actor: {}", actor.0))
                .runtime
                .agility;
        let count = 3 + self.rng.r3() as usize;
        for _ in 0..count {
            let actor_active = self.entities.get(actor).is_some_and(EntityRecord::is_active);
            let target_alive = self.entities.get(target).is_some_and(|entity| entity.runtime.alive);
            if !actor_active || !target_alive {
                continue;
            }

            updates.add_newline();
            let (target_active, target_dodge) = {
                let target_entity = self
                    .entities
                    .get(target)
                    .unwrap_or_else(|| panic!("unknown runtime_v2 thunder target: {}", target.0));
                (
                    target_entity.is_active(),
                    target_entity.runtime.agility + target_entity.runtime.resistance,
                )
            };
            if target_active && PlayerRuntime::dodge(accuracy, target_dodge, &mut self.rng) {
                updates.add(RuntimeFrame::replay_update(
                    target.0 as usize,
                    actor.0 as usize,
                    "[0][回避]了攻击",
                    0,
                ));
                return;
            }

            accuracy -= 10;
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 thunder actor: {}", actor.0))
                .runtime
                .get_at(true, &mut self.rng)
                * 0.36000001430511475;
            let update_pos = updates.updates.len();
            self.drain_plain_defended_attack_with_atp_into(actor, target, true, atp, updates);
            if let Some(update) = updates.updates.get_mut(update_pos) {
                update.delay0 = 300;
            }
        }
    }

    fn drain_plain_quake_skill_into(&mut self, actor: EntityIdx, mut targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        if targets.is_empty() {
            return;
        }
        let round = if self.rng.c50() { 5 } else { 4 };
        targets.truncate(round.min(targets.len()));
        if targets.is_empty() {
            return;
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[地裂术]",
            actor.0 as usize,
            targets[0].0 as usize,
            1,
        ));
        let divisor = targets.len() as f64 + 0.6000000238418579;
        for target in targets {
            let atp = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 quake actor: {}", actor.0))
                .runtime
                .get_at(true, &mut self.rng)
                * 2.440000057220459
                / divisor;
            let target_alive = self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0);
            if !target_alive {
                continue;
            }

            updates.add_newline();
            let update_pos = updates.updates.len();
            self.drain_plain_attack_with_atp_into(actor, target, true, atp, updates);
            if let Some(update) = updates.updates.get_mut(update_pos) {
                update.delay0 = 300;
            }
            if self.world.sync_winner(&self.entities).is_some() {
                break;
            }
        }
    }

    fn drain_plain_absorb_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 absorb actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 1.2999999523162842;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发起[吸血攻击]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Absorb, updates);
    }

    fn drain_plain_poison_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 poison actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0][投毒]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Poison, updates);
    }

    fn drain_plain_critical_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let actor_runtime = &mut self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 critical actor: {}", actor.0))
            .runtime;
        let atp0 = actor_runtime.get_at(false, &mut self.rng) * 1.149999976158142;
        let atp1 = actor_runtime.get_at(false, &mut self.rng) * 1.2000000476837158;
        let atp2 = actor_runtime.get_at(false, &mut self.rng) * 1.25;
        let atp = atp0.max(atp1).max(atp2);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发动[会心一击]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
    }

    fn select_plain_berserk_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let ally_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| (self.plain_effective_team(*target) == actor_team).then_some(index))
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        let mut invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if ally_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &ally_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                !smart
                    || (!entity
                        .states
                        .entries()
                        .iter()
                        .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
                        && !entity.runtime.flags.contains(PlayerKindFlags::MINION))
            });
            if !valid {
                invalid_count += 1;
                continue;
            }
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_berserk_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_berserk_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let mut score = self.score_plain_default_enemy_target(target, smart);
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 berserk target: {}", target.0));
        if target_entity
            .states
            .entries()
            .iter()
            .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. } | StatePayload::Charm { .. }))
        {
            score /= 1.2000000476837158;
        }
        score
    }

    fn drain_plain_berserk_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 berserk actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[狂暴术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Berserk, updates);
    }

    fn select_plain_haste_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let candidates = self
            .world
            .team_roster(actor_team)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|target| self.entities.get(*target).is_some_and(|entity| entity.runtime.alive))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Vec::new();
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        let mut invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let Some(picked) = self.rng.pick(&candidates) else {
                return Vec::new();
            };
            let target = candidates[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                if !smart {
                    return true;
                }
                entity.runtime.hp >= 60
                    && entity
                        .states
                        .entry(PLAIN_HASTE_STATE_KEY)
                        .and_then(StateEntry::haste_value)
                        .is_none_or(|(_, step)| (step + 1) * 60 <= entity.runtime.hp)
                    && !entity.runtime.flags.contains(PlayerKindFlags::MINION)
            });
            if !valid {
                invalid_count += 1;
                continue;
            }
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_haste_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_haste_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if !smart {
            return self.rng.rFFFF() as f64;
        }
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 haste target: {}", target.0));
        let hp = entity.runtime.hp;
        let rate_hi_hp = if hp < 20 {
            30.0
        } else if hp > 300 {
            300.0
        } else {
            hp as f64
        };
        let mut score = rate_hi_hp * entity.runtime.attr_sum as f64;
        if entity.states.entry(PLAIN_HASTE_STATE_KEY).is_some() {
            score /= 4.0;
        }
        score
    }

    fn drain_plain_haste_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[加速术]",
            actor.0 as usize,
            target.0 as usize,
            60,
        ));
        let (charge_active, owner_speed) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 haste actor: {}", actor.0));
            (
                owner.runtime.at_boost_millionths >= 3_000_000,
                owner.states.effective_speed(owner.runtime.speed),
            )
        };
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 haste actor: {}", actor.0))
            .runtime
            .move_state
            .speed_points += owner_speed;

        let haste_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT)
            .expect("default runtime v2 profile must register core haste state");
        let haste_priority = self
            .registry
            .state(haste_state_id)
            .expect("default runtime v2 core haste state disappeared")
            .priority;
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 haste target: {}", target.0));
        if let Some((mut faster, mut step)) = target_entity.states.entry(PLAIN_HASTE_STATE_KEY).and_then(StateEntry::haste_value)
        {
            step += 2;
            if charge_active {
                faster += 2;
                step += 2;
            }
            assert!(
                target_entity
                    .states
                    .set_payload(PLAIN_HASTE_STATE_KEY, StatePayload::Haste { faster, step }),
                "runtime_v2 haste state disappeared during extension"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::haste(
                    PLAIN_HASTE_STATE_KEY,
                    haste_state_id,
                    if charge_active { 4 } else { 2 },
                    if charge_active { 5 } else { 3 },
                    haste_priority,
                )),
                "runtime_v2 haste state should be inserted"
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]进入[疾走]状态",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
    }

    fn drain_plain_iron_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 iron actor: {}", actor.0));
            (owner.runtime.magic, owner.runtime.at_boost_millionths >= 3_000_000)
        };
        let step = 3 + if charge_active { 4 } else { 0 };
        let protect = 110 + magic + if charge_active { 240 + magic * 4 } else { 0 };
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发动[铁壁]",
            actor.0 as usize,
            actor.0 as usize,
            60,
        ));

        let iron_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_IRON_STATE_EXPORT)
            .expect("default runtime v2 profile must register core iron state");
        let iron_priority = self
            .registry
            .state(iron_state_id)
            .expect("default runtime v2 core iron state disappeared")
            .priority;
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 iron actor: {}", actor.0));
        if owner.states.entry(PLAIN_IRON_STATE_KEY).is_some() {
            assert!(
                owner.states.set_payload(PLAIN_IRON_STATE_KEY, StatePayload::Iron { protect, step }),
                "runtime_v2 iron state disappeared during replacement"
            );
        } else {
            assert!(
                owner.states.add_entry(StateEntry::iron(
                    PLAIN_IRON_STATE_KEY,
                    iron_state_id,
                    protect,
                    step,
                    iron_priority,
                )),
                "runtime_v2 iron state should be inserted"
            );
        }
        owner.runtime.move_state.speed_points -= 256;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]防御力大幅上升",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    fn select_plain_rapid_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 5 } else { 3 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
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

        let mut scored = selected
            .into_iter()
            .map(|target| {
                let target_entity = self
                    .entities
                    .get(target)
                    .unwrap_or_else(|| panic!("runtime_v2 rapid target disappeared: {}", target.0));
                let score = if smart {
                    let hp = if target_entity.runtime.hp < 20 {
                        30
                    } else if target_entity.runtime.hp > 300 {
                        300
                    } else {
                        target_entity.runtime.hp
                    };
                    if self.world.alive_group_count() > 2 {
                        hp as f64 * self.world.alive_group_len_containing(target) as f64 * target_entity.runtime.attract()
                    } else {
                        (1.0 / hp as f64) * target_entity.runtime.atk_sum as f64 * target_entity.runtime.attract()
                    }
                } else {
                    self.rng.rFFFF() as f64 + target_entity.runtime.attract()
                };
                (target, score)
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn drain_plain_rapid_skill_into(&mut self, actor: EntityIdx, mut targets: Vec<EntityIdx>, updates: &mut RunUpdates) {
        if targets.is_empty() {
            return;
        }
        let rounds = if self.rng.c50() { 3.0 } else { 2.0 };
        targets.truncate(3);
        let mut hit_scores = vec![0.0f64; targets.len()];
        let mut position = 0usize;
        let mut round = 0.0f64;
        while round < rounds {
            let actor_active = self.entities.get(actor).is_some_and(|entity| entity.is_active());
            if !actor_active {
                return;
            }

            let target = targets[position];
            let target_dead = self.entities.get(target).map(|entity| !entity.runtime.alive).unwrap_or(true);
            if target_dead {
                round -= 0.5;
            } else {
                let atp = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("runtime_v2 rapid actor disappeared: {}", actor.0))
                    .runtime
                    .get_at(false, &mut self.rng)
                    * (0.75 - hit_scores[position] * 0.15000000596046448);
                hit_scores[position] += 1.0;
                updates.add(crate::engine::update::RunUpdate::new(
                    if round == 0.0 { "[0]发起攻击" } else { "[0][连击]" },
                    actor.0 as usize,
                    target.0 as usize,
                    if round == 0.0 { 0 } else { 1 },
                ));
                let damage = self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
                if damage <= 0 {
                    return;
                }
                updates.add_newline();
            }
            position = (position + self.rng.r3() as usize) % targets.len();
            round += 1.0;
        }
    }

    fn select_plain_half_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = !smart
                || self
                    .entities
                    .get(target)
                    .is_some_and(|entity| entity.runtime.hp > 160 && entity.runtime.hp < 400);
            if !valid {
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_half_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_half_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 half target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let base = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        base * entity.runtime.hp as f64
    }

    fn select_plain_curse_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = !smart
                || self.entities.get(target).is_some_and(|entity| {
                    entity.runtime.hp >= 80
                        && entity
                            .states
                            .entries()
                            .iter()
                            .find_map(|entry| match entry.payload {
                                StatePayload::Curse { prob, .. } => Some(prob),
                                _ => None,
                            })
                            .is_none_or(|prob| prob <= 32)
                });
            if !valid {
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_curse_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_curse_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 curse target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let base = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / rate_hi_hp(entity.runtime.hp)) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity
            .states
            .entries()
            .iter()
            .any(|entry| matches!(entry.payload, StatePayload::Curse { .. }))
        {
            base / 2.0
        } else {
            base
        }
    }

    fn drain_plain_curse_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 curse actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[诅咒]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Curse, updates);
    }

    fn drain_plain_half_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[瘟疫]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));

        let (owner_wisdom, owner_magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 half actor: {}", actor.0));
            (
                owner.runtime.wisdom,
                owner.runtime.magic,
                owner.runtime.at_boost_millionths >= 3_000_000,
            )
        };
        let (target_hp, target_resistance, target_agility, target_flags, target_name, target_active) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 half target: {}", target.0));
            (
                target_entity.runtime.hp,
                target_entity.runtime.resistance,
                target_entity.runtime.agility,
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.is_active(),
            )
        };
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "half");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        let chance = (owner_wisdom + ((360 - target_hp) / 3)).max(0);
        if immune
            || (target_active
                && !charge_active
                && PlayerRuntime::dodge(chance, target_resistance + target_agility, &mut self.rng))
        {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        let mut percent = ((owner_magic - (target_resistance / 2)) / 2) + 47;
        if charge_active {
            percent = owner_magic + 50;
        }
        percent = percent.min(99);
        let new_hp = ((target_hp as f64) * (100 - percent) as f64 / 100.0).ceil() as i32;
        let damage = (target_hp - new_hp).max(0);
        let mut update =
            crate::engine::update::RunUpdate::new("[1]体力减少[2]%", actor.0 as usize, target.0 as usize, damage as u32);
        update.param = Some(percent.max(0) as u32);
        updates.add(update);
        if damage <= 0 {
            return;
        }

        self.entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 half target disappeared: {}", target.0))
            .runtime
            .hp = new_hp;
        self.drain_plain_post_damage_skill_chain_into(target, damage, actor, updates);
    }

    fn select_plain_ice_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            if self.entities.get(target).is_none() {
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_ice_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_ice_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 ice target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let mut score = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / rate_hi_hp(entity.runtime.hp)) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity.states.ice_frozen_step(PLAIN_ICE_STATE_KEY).is_some() {
            score /= 2.0;
        }
        score
    }

    fn drain_plain_ice_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 ice actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * crate::player::skill::act::ice::ICE_DAMAGE_MULTIPLIER;
        let mut defend_value = RuntimeDefendValue::Atp {
            value: atp,
            caster: actor,
            target,
        };
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0]使用[冰冻术]",
            1,
        ));
        self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
        let Some(atp) = defend_value.atp() else {
            panic!("runtime_v2 PRE_DEFEND hooks must leave an atp value");
        };
        if atp == 0.0 {
            return;
        }
        if self.magic_attack_dodged(actor, target) {
            updates.add(RuntimeFrame::replay_update(
                target.0 as usize,
                actor.0 as usize,
                "[0][回避]了攻击",
                20,
            ));
            return;
        }

        let amount = (atp / self.entities.get(target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        let mut defend_value = RuntimeDefendValue::Damage {
            value: amount,
            caster: actor,
            target,
        };
        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
        let Some(amount) = defend_value.damage() else {
            panic!("runtime_v2 POST_DEFEND hooks must leave a damage value");
        };
        if self.apply_plain_attack_damage_into(actor, target, amount, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        } else if amount > 0 {
            self.apply_ice_on_damage(actor, target, updates);
        }
    }

    fn drain_plain_charge_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 charge owner: {}", actor.0));
        owner.activate_charge_runtime();
        owner.runtime.magic_point += 32;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]开始[蓄力]",
            actor.0 as usize,
            actor.0 as usize,
            1,
        ));
    }

    fn drain_plain_accumulate_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 accumulate owner: {}", actor.0));
        if !owner.activate_accumulate_runtime() {
            return;
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]开始[聚气]",
            actor.0 as usize,
            actor.0 as usize,
            1,
        ));
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]攻击力上升",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    fn drain_plain_clone_skill_into(&mut self, actor: EntityIdx, fixed_lane: usize, updates: &mut RunUpdates) {
        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 clone level missing for fixed lane {fixed_lane}"));
        let shadow_blueprint_slot = self.registry.entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT);
        let random_factor = (u32::from(self.rng.next_u8()) & 63) + 64;
        let mut decayed_level = ((current_level as f64) * random_factor as f64 / 128.0).ceil() as u32;
        let charge_active = self
            .entities
            .get(actor)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);

        if !charge_active {
            let owner = self
                .entities
                .get_mut(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 clone actor: {}", actor.0));
            let old_max_hp = owner.template.max_hp.max(1);
            let next_hp = (((owner.runtime.hp as f64) * 0.5).ceil() as i32).clamp(1, old_max_hp);
            let build = owner
                .template
                .clone_build
                .as_mut()
                .unwrap_or_else(|| panic!("runtime_v2 clone build data missing for entity {}", actor.0));
            build.decay_owner();
            let stats = build.derive_stats();
            owner.apply_derived_stats(stats);
            owner.runtime.hp = next_hp;
        }

        let (
            root_owner,
            root_name,
            owner_display_name,
            owner_team,
            owner_hp,
            owner_magic,
            mut clone_skills,
            clone_build,
            shadow_blueprint,
        ) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 clone actor: {}", actor.0));
            let root_owner = owner.runtime.root_owner;
            let root_name = self
                .entities
                .get(root_owner)
                .unwrap_or_else(|| panic!("unknown runtime_v2 clone root owner: {}", root_owner.0))
                .template
                .name
                .clone();
            let clone_build = owner
                .template
                .clone_build
                .as_ref()
                .unwrap_or_else(|| panic!("runtime_v2 clone build data missing for entity {}", actor.0))
                .child();
            let shadow_blueprint = shadow_blueprint_slot.and_then(|slot| match owner.slots.get(slot) {
                Some(SlotValue::PlayerTemplate(template)) => Some(template.as_ref().clone()),
                Some(_) => panic!("runtime_v2 core shadow blueprint slot has invalid value"),
                None => None,
            });
            (
                root_owner,
                root_name,
                owner.template.display_name.clone(),
                owner.runtime.team,
                owner.runtime.hp,
                owner.runtime.magic,
                owner.template.skills.clone(),
                clone_build,
                shadow_blueprint,
            )
        };
        clone_skills.reapply_clone_boosts();
        let clone_move_points = self.rng.r255() as i32 * 4 + 256;
        if owner_hp + owner_magic < self.rng.r255() as i32 {
            decayed_level = (decayed_level >> 1) + 1;
        }
        let cloned_clone_level = (decayed_level as f64).sqrt().ceil() as u32;
        assert!(
            clone_skills.set_level_at(fixed_lane, cloned_clone_level.max(1)),
            "runtime_v2 clone fixed lane disappeared while building child"
        );
        assert!(
            self.entities
                .get_mut(actor)
                .unwrap()
                .template
                .skills
                .set_level_at(fixed_lane, decayed_level.max(1)),
            "runtime_v2 clone fixed lane disappeared while updating owner"
        );

        let counter_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core minion counter slot");
        let next_minion_index = match self
            .entities
            .get(root_owner)
            .unwrap_or_else(|| panic!("unknown runtime_v2 clone root owner: {}", root_owner.0))
            .slots
            .get(counter_slot)
        {
            Some(SlotValue::U64(next)) => *next,
            Some(_) => panic!("runtime_v2 core minion counter slot has invalid value"),
            None => 0,
        };
        self.entities
            .get_mut(root_owner)
            .unwrap()
            .slots
            .set(counter_slot, SlotValue::U64(next_minion_index + 1))
            .expect("runtime_v2 core minion counter slot must exist");

        let clone_stats = clone_build.derive_stats();
        let next_entity = self.entities.len();
        let mut clone_template = PlayerTemplate::new(
            next_entity + 1,
            format!("{root_name}?{next_minion_index}"),
            owner_team,
            clone_stats.max_hp.max(1),
            clone_stats.attack.max(0),
        )
        .with_display_name(owner_display_name)
        .with_magic(clone_stats.magic.max(0))
        .with_magic_point((clone_stats.wisdom >> 1).max(0))
        .with_wisdom(clone_stats.wisdom.max(0))
        .with_speed(clone_stats.speed.max(0))
        .with_def_res(clone_stats.defense.max(0), clone_stats.resistance.max(0))
        .with_agility(clone_stats.agility.max(0))
        .with_at_boost_millionths(clone_stats.at_boost_millionths.max(0))
        .with_target_score_stats(
            clone_stats.attr_sum,
            clone_stats.atk_sum,
            f64::from_bits(clone_stats.attract_bits),
        )
        .with_speed_points(clone_move_points)
        .with_skill_loadout(clone_skills);
        clone_template.clone_build = Some(clone_build);
        let clone_idx = EntityIdx(next_entity.try_into().expect("runtime_v2 clone entity index overflow"));

        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[分身]",
            actor.0 as usize,
            actor.0 as usize,
            60,
        ));
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template: clone_template,
            message: "出现一个新的[1]".to_owned(),
        });
        self.drain_effects_into(updates);
        let clone_entity = self
            .entities
            .get_mut(clone_idx)
            .unwrap_or_else(|| panic!("runtime_v2 clone spawn missing entity {}", clone_idx.0));
        clone_entity.runtime.hp = owner_hp.max(1);
        if let (Some(slot), Some(template)) = (shadow_blueprint_slot, shadow_blueprint) {
            clone_entity
                .slots
                .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                .expect("runtime_v2 core shadow blueprint slot must exist");
        }
    }

    fn select_plain_exchange_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let (actor_team, actor_hp) = self
            .entities
            .get(actor)
            .map(|entity| (entity.runtime.team, entity.runtime.hp))
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange actor: {}", actor.0));
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                if smart {
                    entity.runtime.hp - actor_hp > 32
                } else {
                    entity.runtime.hp > actor_hp
                }
            });
            if !valid {
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_exchange_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_exchange_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        if smart {
            let base = if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            };
            base * entity.runtime.hp as f64
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    fn drain_plain_exchange_skill_into(
        &mut self,
        actor: EntityIdx,
        fixed_lane: usize,
        target: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 exchange level missing for fixed lane {fixed_lane}"));
        assert!(
            self.entities
                .get_mut(actor)
                .unwrap()
                .template
                .skills
                .set_level_at(fixed_lane, (current_level + 1) >> 1),
            "runtime_v2 exchange fixed lane disappeared during action"
        );
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[生命之轮]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));

        let (owner_magic, charge_active, owner_hp, owner_max_hp) = self
            .entities
            .get(actor)
            .map(|owner| {
                (
                    owner.runtime.magic,
                    owner.runtime.at_boost_millionths >= 3_000_000,
                    owner.runtime.hp,
                    owner.template.max_hp,
                )
            })
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange actor: {}", actor.0));
        let (target_flags, target_name, target_res, target_def, target_agl, target_hp, target_active) = self
            .entities
            .get(target)
            .map(|target_entity| {
                (
                    target_entity.runtime.flags,
                    target_entity.template.name.clone(),
                    target_entity.runtime.resistance,
                    target_entity.runtime.defense,
                    target_entity.runtime.agility,
                    target_entity.runtime.hp,
                    target_entity.is_active(),
                )
            })
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange target: {}", target.0));
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "exchange");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_EXCHANGE").is_some() {
            let owner = self.entities.get(actor).expect("runtime_v2 exchange owner missing for probe");
            let target_entity = self.entities.get(target).expect("runtime_v2 exchange target missing for probe");
            eprintln!(
                "[exchange_probe:v2:before] round={} owner={} target={} owner_hp={} target_hp={} owner_max_hp={} \
                 owner_magic={} owner_boost={} charge={} owner_move={} target_move={} target_active={} immune={} rc4=({}, {})",
                self.round + 1,
                owner.template.name,
                target_entity.template.name,
                owner_hp,
                target_hp,
                owner_max_hp,
                owner_magic,
                owner.runtime.at_boost(),
                charge_active,
                owner.runtime.move_state.speed_points,
                target_entity.runtime.move_state.speed_points,
                target_active,
                immune,
                self.rng.i,
                self.rng.j,
            );
        }
        if immune
            || (target_active
                && !charge_active
                && PlayerRuntime::dodge(owner_magic, target_res + target_def + target_agl, &mut self.rng))
        {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        if charge_active {
            let target_move_points = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 exchange target: {}", target.0))
                .runtime
                .move_state
                .speed_points;
            self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += target_move_points;
            self.entities.get_mut(target).unwrap().runtime.move_state.speed_points = 0;
        }

        self.entities.get_mut(actor).unwrap().runtime.hp = target_hp.min(owner_max_hp);
        self.entities.get_mut(target).unwrap().runtime.hp = owner_hp;
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]的体力值与[0]互换",
            actor.0 as usize,
            target.0 as usize,
            ((target_hp - owner_hp) * 2).max(0) as u32,
        ));
        if target_hp > owner_hp {
            self.drain_plain_post_damage_skill_chain_into(target, target_hp - owner_hp, actor, updates);
        }
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_EXCHANGE").is_some() {
            let owner = self.entities.get(actor).expect("runtime_v2 exchange owner missing after probe");
            let target_entity = self.entities.get(target).expect("runtime_v2 exchange target missing after probe");
            eprintln!(
                "[exchange_probe:v2:after] round={} owner_hp={} target_hp={} owner_move={} target_move={} rc4=({}, {})",
                self.round + 1,
                owner.runtime.hp,
                target_entity.runtime.hp,
                owner.runtime.move_state.speed_points,
                target_entity.runtime.move_state.speed_points,
                self.rng.i,
                self.rng.j,
            );
        }
    }

    fn clear_plain_hide_before_action(&mut self, actor: EntityIdx) {
        let Some(hide) = self.entities.get_mut(actor).and_then(|entity| entity.runtime.hide.take()) else {
            return;
        };
        let actor = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime_v2 hide owner disappeared while clearing: {}", actor.0));
        actor.runtime.attract_bits = hide.attract_bits;
        actor.runtime.agility = hide.agility;
        actor.runtime.defense = hide.defense;
        actor.runtime.resistance = hide.resistance;
    }

    fn drain_plain_post_damage_skill_chain_into(
        &mut self,
        target: EntityIdx,
        damage: i32,
        caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        #[derive(Debug, Clone, Copy)]
        enum PlainPostDamageSkill {
            Upgrade,
            Hide,
            Counter,
        }

        let plan = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 post-damage target: {}", target.0))
            .template
            .skills
            .skills()
            .iter()
            .copied()
            .enumerate()
            .filter_map(|(fixed_lane, skill_id)| {
                let export_name = self.registry.skill(skill_id)?.export_name.as_str();
                let skill = match export_name {
                    DEFAULT_CORE_UPGRADE_SKILL_EXPORT => PlainPostDamageSkill::Upgrade,
                    DEFAULT_CORE_HIDE_SKILL_EXPORT => PlainPostDamageSkill::Hide,
                    DEFAULT_CORE_COUNTER_SKILL_EXPORT => PlainPostDamageSkill::Counter,
                    _ => return None,
                };
                let level = self.entities.get(target)?.template.skills.level_at(fixed_lane)?;
                if level == 0 {
                    return None;
                }
                Some((skill, level))
            })
            .collect::<Vec<_>>();
        #[cfg(not(feature = "no_debug"))]
        let debug_counter = std::env::var_os("TSWN_PROBE_COUNTER").is_some();
        #[cfg(not(feature = "no_debug"))]
        if debug_counter {
            eprintln!(
                "[counter_probe:v2:plan] target={} caster={} damage={} updates_id={} plan={:?} rc4=({}, {})",
                target.0, caster.0, damage, updates.id, plan, self.rng.i, self.rng.j,
            );
        }

        for (skill, level) in plan {
            #[cfg(not(feature = "no_debug"))]
            let rng_before = (self.rng.i, self.rng.j);
            match skill {
                PlainPostDamageSkill::Upgrade => {
                    self.run_plain_upgrade_post_damage_into(target, level, damage, caster, updates);
                }
                PlainPostDamageSkill::Hide => {
                    self.run_plain_hide_post_damage_into(target, level, damage, caster, updates);
                }
                PlainPostDamageSkill::Counter => {
                    self.run_plain_counter_post_damage_into(target, level, damage, caster, updates);
                }
            }
            #[cfg(not(feature = "no_debug"))]
            if debug_counter {
                eprintln!(
                    "[counter_probe:v2:skill] target={} caster={} skill={:?} level={} updates_id={} rc4=({}, {}) -> ({}, {})",
                    target.0, caster.0, skill, level, updates.id, rng_before.0, rng_before.1, self.rng.i, self.rng.j,
                );
            }
        }
        if damage > 0 && self.lazy_boss_at_boost(target).is_some() {
            self.infect_with_lazy_into(target, caster, updates);
        }
    }

    fn run_plain_counter_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        if level == 0 {
            return;
        }
        let owner_wisdom = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 counter owner: {}", target.0))
            .runtime
            .wisdom
            .clamp(0, 127) as u32;
        let owner_ally_team = self.plain_effective_team(target);
        let caster_team = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime_v2 counter caster: {}", caster.0))
            .runtime
            .team;
        if owner_ally_team == caster_team && self.rng.r63() < owner_wisdom {
            return;
        }

        let counter = &mut self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 counter owner disappeared: {}", target.0))
            .runtime
            .counter;
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_COUNTER").is_some() {
            eprintln!(
                "[counter_probe:v2:state] target={} caster={} updates_id={} last_updates_id={:?} pending={} last_target={:?}",
                target.0,
                caster.0,
                updates.id,
                counter.last_updates_id,
                counter.pending,
                counter.last_target.map(|idx| idx.0),
            );
        }
        if counter.last_updates_id == Some(updates.id) {
            if counter.pending && Some(caster) != counter.last_target && self.rng.r127() < level {
                counter.last_target = Some(caster);
            }
            return;
        }

        counter.last_updates_id = Some(updates.id);
        if self.rng.r255() < level {
            counter.last_target = Some(caster);
            counter.pending = true;
            updates.on_update_end.push(target.0 as usize);
        } else {
            counter.pending = false;
            counter.last_target = None;
        }
    }

    fn drain_plain_update_end_into(&mut self, updates: &mut RunUpdates) {
        let mut guard = 0usize;
        while guard < 64 && !updates.on_update_end.is_empty() {
            let pending = std::mem::take(&mut updates.on_update_end);
            for actor in pending {
                let Ok(actor) = u32::try_from(actor) else {
                    continue;
                };
                self.run_plain_counter_update_end_into(EntityIdx(actor), updates);
            }
            guard += 1;
        }
    }

    fn run_plain_counter_update_end_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let counter_levels = self
            .entities
            .get(owner)
            .map(|entity| {
                entity
                    .template
                    .skills
                    .skills()
                    .iter()
                    .copied()
                    .enumerate()
                    .filter_map(|(fixed_lane, skill_id)| {
                        (self.registry.skill(skill_id)?.export_name == DEFAULT_CORE_COUNTER_SKILL_EXPORT)
                            .then(|| entity.template.skills.level_at(fixed_lane))
                            .flatten()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        for _level in counter_levels {
            let target = {
                let Some(owner_entity) = self.entities.get_mut(owner) else {
                    return;
                };
                let counter = &mut owner_entity.runtime.counter;
                if !counter.pending || counter.last_updates_id != Some(updates.id) {
                    continue;
                }
                counter.pending = false;
                counter.last_updates_id = None;
                counter.last_target.take()
            };
            let Some(target) = target else {
                continue;
            };
            if !self.entities.get(target).is_some_and(EntityRecord::is_active) {
                continue;
            }

            let atp = {
                let owner_runtime = &mut self
                    .entities
                    .get_mut(owner)
                    .unwrap_or_else(|| panic!("runtime_v2 counter owner disappeared: {}", owner.0))
                    .runtime;
                if !owner_runtime.mp_ready(&mut self.rng) {
                    continue;
                }
                owner_runtime.get_at(false, &mut self.rng)
            };
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                "[0]发起[反击][s_counter]",
                owner.0 as usize,
                target.0 as usize,
                1,
            ));
            self.drain_plain_attack_with_atp_into(owner, target, false, atp, updates);
        }
    }

    fn run_plain_upgrade_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        _caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let (already_active, alive, hp) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.upgrade_active, entity.runtime.alive, entity.runtime.hp))
            .unwrap_or_else(|| panic!("unknown runtime_v2 upgrade target: {}", target.0));
        if level == 0 || already_active || !alive || hp <= 0 {
            return;
        }
        let min_hp = 16 + level.saturating_sub(63) as i32;
        if hp >= min_hp + self.rng.r63() as i32 {
            return;
        }
        if self.rng.r63() >= level {
            return;
        }

        updates.add_newline();
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]做出[垂死]抗争",
            target.0 as usize,
            target.0 as usize,
            60,
        ));
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]所有属性上升",
            target.0 as usize,
            target.0 as usize,
            0,
        ));
        let target = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 upgrade target disappeared: {}", target.0));
        target.runtime.upgrade_active = true;
        target.runtime.move_state.speed_points += 400;
        target.runtime.attack += 30;
        target.runtime.defense += 30;
        target.runtime.agility += 30;
        target.runtime.magic += 30;
        target.runtime.resistance += 30;
        target.runtime.speed += 20;
        target.runtime.wisdom += 20;
    }

    fn run_plain_hide_post_damage_into(
        &mut self,
        target: EntityIdx,
        level: u32,
        _damage: i32,
        _caster: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let (already_active, owner_active) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hide.is_some(), entity.runtime.alive && entity.runtime.hp > 0))
            .unwrap_or_else(|| panic!("unknown runtime_v2 hide target: {}", target.0));
        if level == 0 || already_active || !owner_active {
            return;
        }
        let effective_team = self.plain_effective_team(target);
        let alive_allies = self.world.team_alive(effective_team).map_or(0, |team| {
            team.iter()
                .filter(|ally| {
                    self.entities
                        .get(**ally)
                        .is_some_and(|entity| entity.runtime.alive && entity.runtime.hp > 0)
                })
                .count()
        });
        if alive_allies <= 1 || self.rng.r63() >= level {
            return;
        }

        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 hide target disappeared: {}", target.0));
        target_entity.runtime.hide = Some(HideRuntime {
            level,
            attract_bits: target_entity.runtime.attract_bits,
            agility: target_entity.runtime.agility,
            defense: target_entity.runtime.defense,
            resistance: target_entity.runtime.resistance,
        });
        target_entity.runtime.attract_bits = (target_entity.runtime.attract() / 10.0).to_bits();
        if level > 63 {
            let boost = (level - 63) as i32;
            target_entity.runtime.agility += boost;
            target_entity.runtime.defense += boost;
            target_entity.runtime.resistance += boost;
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发动[隐匿]",
            target.0 as usize,
            target.0 as usize,
            10,
        ));
    }

    fn plain_effective_team(&self, actor: EntityIdx) -> usize {
        let actor_entity = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 effective-team actor: {}", actor.0));
        actor_entity
            .states
            .entries()
            .iter()
            .find_map(|entry| match entry.payload {
                StatePayload::Charm {
                    group_id,
                    effective_team_idx,
                    ..
                } => effective_team_idx.or_else(|| {
                    u32::try_from(group_id)
                        .ok()
                        .and_then(|group_entity| self.entities.get(EntityIdx(group_entity)))
                        .map(|entity| entity.runtime.team)
                }),
                _ => None,
            })
            .unwrap_or(actor_entity.runtime.team)
    }

    fn select_plain_charm_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        #[cfg(not(feature = "no_debug"))]
        let before = (self.rng.i, self.rng.j);
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        let mut candidates = Vec::new();
        let mut enemy_skip_indices = Vec::new();
        for (idx, target) in all_alive.iter().copied().enumerate() {
            if self.entities.get(target).is_some_and(|entity| entity.runtime.team == actor_team) {
                enemy_skip_indices.push(idx);
            } else {
                candidates.push(target);
            }
        }
        if candidates.is_empty() {
            return Vec::new();
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = !smart
                || self
                    .entities
                    .get(target)
                    .and_then(|entity| entity.states.entry(76))
                    .and_then(StateEntry::charm_value)
                    .is_none_or(|(_, _, _, _, step)| step <= 1);
            if !valid {
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_charm_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        let targets = scored.into_iter().map(|(target, _)| target).collect::<Vec<_>>();
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:select] actor={} smart={} candidates={:?} targets={:?} rc4=({},{}) -> ({},{})",
                actor.0,
                smart,
                candidates.iter().map(|target| target.0).collect::<Vec<_>>(),
                targets.iter().map(|target| target.0).collect::<Vec<_>>(),
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        targets
    }

    fn score_plain_charm_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 charm target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let mut score = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity.states.entry(76).and_then(StateEntry::charm_value).is_some()
            || entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
        {
            score /= 2.0;
        }
        score
    }

    fn drain_plain_charm_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:act_before] actor={} target={} rc4=({},{})",
                actor.0, target.0, self.rng.i, self.rng.j,
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[魅惑]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let (owner_magic, charge_active, caster_effective_team_idx) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 charm actor: {}", actor.0));
            (
                owner.runtime.magic,
                owner.runtime.at_boost_millionths >= 3_000_000,
                self.plain_effective_team(actor),
            )
        };
        let (target_flags, target_name, target_dodge, target_active) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 charm target: {}", target.0));
            (
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.runtime.agility + target_entity.runtime.resistance,
                target_entity.is_active(),
            )
        };
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "charm");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        if immune || (target_active && PlayerRuntime::dodge(owner_magic, target_dodge, &mut self.rng)) {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
                eprintln!(
                    "[charm_probe:v2:act_after] actor={} target={} dodged=true rc4=({},{})",
                    actor.0, target.0, self.rng.i, self.rng.j,
                );
            }
            return;
        }

        let existing = self
            .entities
            .get(target)
            .and_then(|entity| entity.states.entry(76))
            .and_then(StateEntry::charm_value);
        if let Some((mut group_id, effective_team_idx, mut source_team_idx, state_target, mut step)) = existing {
            let existing_team_idx = source_team_idx.or_else(|| {
                u32::try_from(group_id)
                    .ok()
                    .and_then(|group_entity| self.entities.get(EntityIdx(group_entity)))
                    .map(|entity| entity.runtime.team)
            });
            if existing_team_idx == Some(caster_effective_team_idx) {
                step += 1;
            } else {
                group_id = actor.0 as usize;
                source_team_idx = Some(caster_effective_team_idx);
            }
            if charge_active {
                step += 3;
            }
            assert!(
                self.entities.get_mut(target).unwrap().states.set_payload(
                    76,
                    StatePayload::Charm {
                        group_id,
                        effective_team_idx,
                        source_team_idx,
                        target: state_target,
                        step,
                    },
                ),
                "runtime_v2 charm state disappeared during recharm"
            );
        } else {
            let charm_state_id = self
                .registry
                .state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT)
                .expect("default runtime v2 profile must register core charm state");
            let charm_priority = self
                .registry
                .state(charm_state_id)
                .expect("default runtime v2 core charm state disappeared")
                .priority;
            assert!(
                self.entities.get_mut(target).unwrap().states.add_entry(StateEntry::charm(
                    76,
                    charm_state_id,
                    actor.0 as usize,
                    Some(caster_effective_team_idx),
                    Some(caster_effective_team_idx),
                    Some(target.0),
                    if charge_active { 4 } else { 1 },
                    charm_priority,
                )),
                "runtime_v2 charm state should be inserted"
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]被[魅惑]了",
            actor.0 as usize,
            target.0 as usize,
            120,
        ));
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:act_after] actor={} target={} dodged=false rc4=({},{})",
                actor.0, target.0, self.rng.i, self.rng.j,
            );
        }
    }

    fn select_plain_heal_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let candidates = self
            .world
            .team_roster(actor_team)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|target| self.entities.get(*target).is_some_and(|entity| entity.runtime.alive))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Vec::new();
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(picked) = self.rng.pick(&candidates) else {
                return Vec::new();
            };
            let target = candidates[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                if smart {
                    entity.runtime.hp + 80 < entity.template.max_hp
                } else {
                    entity.runtime.hp < entity.template.max_hp
                }
            });
            if !valid {
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

        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_heal_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_heal_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if !smart {
            return self.rng.rFFFF() as f64;
        }
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 heal target: {}", target.0));
        let negative_state_count = entity
            .states
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry.payload,
                    StatePayload::FireMagHalfSteps(_)
                        | StatePayload::Ice { .. }
                        | StatePayload::Curse { .. }
                        | StatePayload::Poison { .. }
                        | StatePayload::Berserk { .. }
                        | StatePayload::Charm { .. }
                        | StatePayload::Slow { .. }
                )
            })
            .count() as i32;
        let damaged = (entity.template.max_hp - entity.runtime.hp).max(0) + negative_state_count * 64;
        damaged as f64 * entity.runtime.attr_sum.max(1) as f64
    }

    fn drain_plain_heal_skill_into(&mut self, actor: EntityIdx, fixed_lane: usize, target: EntityIdx, updates: &mut RunUpdates) {
        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 heal level missing for fixed lane {fixed_lane}"));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 heal actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        let missing_hp = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 heal target: {}", target.0));
            (target_entity.template.max_hp - target_entity.runtime.hp).max(0)
        };
        if missing_hp <= 0 {
            return;
        }
        let heal = ((atp / 60.0).ceil() as i32).clamp(1, missing_hp);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[治愈魔法]",
            actor.0 as usize,
            target.0 as usize,
            heal as u32,
        ));

        let (had_berserk, had_charm, had_curse, had_ice, had_poison, had_slow) = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 heal target: {}", target.0));
            target_entity.runtime.hp = (target_entity.runtime.hp + heal).min(target_entity.template.max_hp);

            let mut had_berserk = false;
            let mut had_charm = false;
            let mut had_curse = false;
            let mut had_ice = false;
            let mut had_poison = false;
            let mut had_slow = false;
            let negative_keys = target_entity
                .states
                .entries()
                .iter()
                .filter_map(|entry| {
                    let negative = match entry.payload {
                        StatePayload::FireMagHalfSteps(_) => true,
                        StatePayload::Ice { .. } => {
                            had_ice = true;
                            true
                        }
                        StatePayload::Curse { .. } => {
                            had_curse = true;
                            true
                        }
                        StatePayload::Poison { .. } => {
                            had_poison = true;
                            true
                        }
                        StatePayload::Berserk { .. } => {
                            had_berserk = true;
                            true
                        }
                        StatePayload::Charm { .. } => {
                            had_charm = true;
                            true
                        }
                        StatePayload::Slow { .. } => {
                            had_slow = true;
                            true
                        }
                        _ => false,
                    };
                    negative.then_some(entry.legacy_order_key)
                })
                .collect::<Vec<_>>();
            for legacy_order_key in negative_keys {
                assert!(
                    target_entity.states.clear_legacy_key(legacy_order_key),
                    "runtime_v2 negative state disappeared during heal"
                );
            }

            if had_curse || had_ice || had_charm || had_slow {
                target_entity.runtime.atk_sum = target_entity.template.atk_sum;
                target_entity.runtime.speed = target_entity.states.effective_speed(target_entity.template.speed);
            }
            (had_berserk, had_charm, had_curse, had_ice, had_poison, had_slow)
        };

        let mut recover_update =
            crate::engine::update::RunUpdate::new("[1]回复体力[2]点", actor.0 as usize, target.0 as usize, 0);
        recover_update.param = Some(heal as u32);
        updates.add(recover_update);

        for (had_state, message) in [
            (had_berserk, "[1]从[狂暴]中解除"),
            (had_charm, "[1]从[魅惑]中解除"),
            (had_curse, "[1]从[诅咒]中解除"),
            (had_ice, "[1]从[冰冻]中解除"),
            (had_poison, "[1]从[中毒]中解除"),
            (had_slow, "[1]从[迟缓]中解除"),
        ] {
            if had_state {
                updates.add_newline();
                updates.add(RuntimeFrame::replay_update(actor.0 as usize, target.0 as usize, message, 0));
            }
        }

        let next_level = if current_level > 8 { current_level - 1 } else { current_level };
        assert!(
            self.entities.get_mut(actor).unwrap().template.skills.set_level_at(fixed_lane, next_level),
            "runtime_v2 heal fixed lane disappeared during action"
        );
    }

    fn select_plain_disperse_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            if self.entities.get(target).is_none() {
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
        let mut scored = selected
            .into_iter()
            .map(|target| {
                (
                    target,
                    score_disperse_target(&self.entities, &self.world, target, smart, &mut self.rng),
                )
            })
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn drain_plain_disperse_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.effects.push(QueuedEffect::DisperseAttack { caster: actor, target });
        self.drain_effects_into(updates);
    }

    fn select_plain_revive_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 revive actor: {}", actor.0))
            .runtime
            .team;
        let candidates = self.world.team_roster(actor_team).unwrap_or_default().to_vec();
        if candidates.is_empty() {
            return Vec::new();
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(picked) = self.rng.pick(&candidates) else {
                return Vec::new();
            };
            let target = candidates[picked];
            let valid = self
                .entities
                .get(target)
                .is_some_and(|entity| !entity.runtime.alive && !entity.runtime.flags.contains(PlayerKindFlags::MINION));
            if !valid {
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

        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_revive_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_revive_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if smart {
            self.entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 revive target: {}", target.0))
                .runtime
                .attr_sum as f64
        } else {
            self.rng.rFFFF() as f64
        }
    }

    fn drain_plain_revive_skill_into(
        &mut self,
        actor: EntityIdx,
        fixed_lane: usize,
        target: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 revive level missing for fixed lane {fixed_lane}"));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 revive actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        let max_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 revive target: {}", target.0))
            .template
            .max_hp;
        let heal = ((atp / 75.0).ceil() as i32).clamp(1, max_hp.max(1));

        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[苏生术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));

        let team = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 revive target: {}", target.0));
            if target_entity.runtime.alive {
                return;
            }
            target_entity.runtime.hp = heal;
            target_entity.runtime.alive = true;
            target_entity.runtime.team
        };
        self.world.revive_round_actor(target);
        self.world.revive_alive(target, team);

        updates.add(crate::engine::update::RunUpdate::new(
            "[1][复活]了",
            actor.0 as usize,
            target.0 as usize,
            (heal + 60) as u32,
        ));
        let mut recover_update =
            crate::engine::update::RunUpdate::new("[1]回复体力[2]点", actor.0 as usize, target.0 as usize, 0);
        recover_update.param = Some(heal as u32);
        updates.add(recover_update);

        assert!(
            self.entities
                .get_mut(actor)
                .unwrap()
                .template
                .skills
                .set_level_at(fixed_lane, (current_level + 1) >> 1),
            "runtime_v2 revive fixed lane disappeared during action"
        );
    }

    fn select_plain_slow_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                !smart
                    || (entity.runtime.hp >= 80
                        && entity.states.entry(78).and_then(StateEntry::slow_value).is_none_or(|step| step <= 1))
            });
            if !valid {
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_slow_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_slow_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 slow target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let mut score = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity.states.entry(78).and_then(StateEntry::slow_value).is_some() {
            score /= 2.0;
        }
        score
    }

    fn drain_plain_slow_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[减速术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let (owner_magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 slow actor: {}", actor.0));
            (owner.runtime.magic, owner.runtime.at_boost_millionths >= 3_000_000)
        };
        let (target_flags, target_name, target_resistance, target_active) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 slow target: {}", target.0));
            (
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.runtime.resistance,
                target_entity.is_active(),
            )
        };
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "slow");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        if immune || (target_active && PlayerRuntime::dodge(owner_magic, target_resistance, &mut self.rng)) {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        let slow_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_SLOW_STATE_EXPORT)
            .expect("default runtime v2 profile must register core slow state");
        let slow_priority = self
            .registry
            .state(slow_state_id)
            .expect("default runtime v2 core slow state disappeared")
            .priority;
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 slow target: {}", target.0));
        let reduce_move_point = target_entity.states.effective_speed(target_entity.runtime.speed) + 64;
        target_entity.runtime.move_state.speed_points -= reduce_move_point;
        let next_step = target_entity.states.entry(78).and_then(StateEntry::slow_value).map_or(2, |step| step + 2)
            + if charge_active { 4 } else { 0 };
        if target_entity.states.entry(78).is_some() {
            assert!(
                target_entity.states.set_payload(78, StatePayload::Slow { step: next_step }),
                "runtime_v2 slow state disappeared during extension"
            );
        } else {
            assert!(
                target_entity
                    .states
                    .add_entry(StateEntry::slow(78, slow_state_id, next_step, slow_priority)),
                "runtime_v2 slow state should be inserted"
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]进入[迟缓]状态",
            actor.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    fn drain_plain_shadow_skill_into(&mut self, actor: EntityIdx, fixed_lane: usize, updates: &mut RunUpdates) {
        let blueprint_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core shadow blueprint slot");
        let counter_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core minion counter slot");
        let mut shadow_template = match self.entities.get(actor).and_then(|entity| entity.slots.get(blueprint_slot)) {
            Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
            Some(_) => panic!("runtime_v2 core shadow blueprint slot has invalid value"),
            None => panic!("runtime_v2 core shadow blueprint missing for entity {}", actor.0),
        };
        let root_owner = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 shadow actor: {}", actor.0))
            .runtime
            .root_owner;
        let (root_name, next_minion_index) = {
            let root = self
                .entities
                .get(root_owner)
                .unwrap_or_else(|| panic!("unknown runtime_v2 shadow root owner: {}", root_owner.0));
            let next = match root.slots.get(counter_slot) {
                Some(SlotValue::U64(next)) => *next,
                Some(_) => panic!("runtime_v2 core minion counter slot has invalid value"),
                None => 0,
            };
            (root.template.name.clone(), next)
        };
        self.entities
            .get_mut(root_owner)
            .unwrap()
            .slots
            .set(counter_slot, SlotValue::U64(next_minion_index + 1))
            .expect("runtime_v2 core minion counter slot must exist");
        let next_entity = self.entities.len();
        shadow_template.id = next_entity + 1;
        shadow_template.name = format!("{root_name}?{next_minion_index}");
        shadow_template.move_state.speed_points = if self
            .entities
            .get(actor)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000)
        {
            2048
        } else {
            -2048
        };

        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            actor.0 as usize,
            "[0]使用[幻术]",
            60,
        ));
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template: shadow_template,
            message: "召唤出[1]".to_owned(),
        });
        self.drain_effects_into(updates);

        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 shadow level missing for fixed lane {fixed_lane}"));
        let next_level = current_level.saturating_mul(3).div_ceil(4).max(1);
        assert!(
            self.entities.get_mut(actor).unwrap().template.skills.set_level_at(fixed_lane, next_level),
            "runtime_v2 shadow fixed lane disappeared during action"
        );
    }

    fn select_plain_possess_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let has_enemy = all_alive
            .iter()
            .copied()
            .any(|target| self.entities.get(target).is_some_and(|entity| entity.runtime.team != actor_team));
        if !has_enemy {
            return Vec::new();
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        let invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_possess_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_possess_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 possess target: {}", target.0));
        if smart {
            let hp = if entity.runtime.hp < 20 {
                30.0
            } else if entity.runtime.hp > 300 {
                300.0
            } else {
                entity.runtime.hp as f64
            };
            if self.world.alive_group_count() > 2 {
                hp * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / hp) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    fn drain_plain_possess_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[附体]",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        let (caster_magic, target_flags, target_name, target_resistance, target_active) = {
            let caster = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 possess actor: {}", actor.0));
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 possess target: {}", target.0));
            (
                caster.runtime.magic,
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.runtime.resistance,
                target_entity.is_active(),
            )
        };
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            eprintln!(
                "[possess_probe:v2:act_before] actor={} target={} target_name={} flags={:?} rc4=({},{})",
                actor.0, target.0, target_name, target_flags, self.rng.i, self.rng.j,
            );
        }
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "berserk");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        let dodged = immune || (target_active && PlayerRuntime::dodge(caster_magic, target_resistance, &mut self.rng));
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            eprintln!(
                "[possess_probe:v2:act_after] actor={} target={} immune={} dodged={} rc4=({},{})",
                actor.0, target.0, immune, dodged, self.rng.i, self.rng.j,
            );
        }
        if dodged {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 possess target disappeared: {}", target.0));
        let next_step = target_entity
            .states
            .entry(10)
            .and_then(|entry| match entry.payload {
                StatePayload::Berserk { step } => Some(step + 4),
                _ => None,
            })
            .unwrap_or(4);
        if !target_entity.states.set_payload(10, StatePayload::Berserk { step: next_step }) {
            target_entity.states.add_entry(StateEntry::berserk(10, next_step));
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]进入[狂暴]状态",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime_v2 possess actor disappeared: {}", actor.0))
            .runtime
            .hp = 0;
        self.drain_plain_lethal_damage_into(actor, actor, updates);
    }

    fn select_plain_default_attack_target(&mut self, actor: EntityIdx, smart: bool) -> Option<EntityIdx> {
        self.entities.get(actor)?;
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return None;
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                self.entities
                    .get(*entity)
                    .is_some_and(|record| record.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            }?;
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return None;
        }

        let mut scored = Vec::with_capacity(selected.len());
        for target in selected {
            let target_entity = self.entities.get(target).unwrap();
            let score = if smart {
                let hp = if target_entity.runtime.hp < 20 {
                    30.0
                } else if target_entity.runtime.hp > 300 {
                    300.0
                } else {
                    target_entity.runtime.hp as f64
                };
                let alive_group_len = self.world.alive_group_len_containing(target) as f64;
                if self.world.alive_group_count() > 2 {
                    hp * alive_group_len * target_entity.runtime.attract()
                } else {
                    (1.0 / hp) * target_entity.runtime.atk_sum as f64 * target_entity.runtime.attract()
                }
            } else {
                self.rng.rFFFF() as f64 + target_entity.runtime.attract()
            };
            scored.push((target, score));
        }
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        #[cfg(not(feature = "no_debug"))]
        if std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false)
        {
            let entity_name = |entity: EntityIdx| {
                self.entities
                    .get(entity)
                    .map(|record| format!("{}#{}(hp={})", record.template.name, entity.0, record.runtime.hp))
                    .unwrap_or_else(|| format!("#{}", entity.0))
            };
            let ranked = scored
                .iter()
                .map(|(entity, score)| format!("{}:{score}", entity_name(*entity)))
                .collect::<Vec<_>>();
            eprintln!(
                "[probe_default_attack:v2] actor={}#{} smart={} effective_team={} rc4=({}, {}) all_alive={:?} ranked={:?}",
                self.entities.get(actor).unwrap().template.name,
                actor.0,
                smart,
                actor_team,
                self.rng.i,
                self.rng.j,
                all_alive.iter().copied().map(entity_name).collect::<Vec<_>>(),
                ranked,
            );
        }
        scored.first().map(|(target, _)| *target)
    }

    fn drain_plain_default_attack_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        updates: &mut RunUpdates,
    ) {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false);
        if let Some(at_boost) = self.lazy_boss_at_boost(actor)
            && self.has_lazy_infection(target)
            && self.rng.next_u8() < 128
        {
            self.emit_lazy_activity_into(actor, updates);
            self.set_lazy_boss_at_boost(actor, at_boost + 0.5);
            return;
        }
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0]发起攻击",
            0,
        ));
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:atp_before] actor={} target={} use_magic={} rc4=({}, {})",
                actor.0, target.0, use_magic, self.rng.i, self.rng.j,
            );
        }
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default attack actor: {}", actor.0))
            .runtime
            .get_at(use_magic, &mut self.rng)
            * self.lazy_boss_at_boost(actor).unwrap_or(1.0);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:atp_after] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        self.drain_plain_attack_with_atp_into(actor, target, use_magic, atp, updates);
    }

    fn saitama_boss_state(&self, actor: EntityIdx) -> Option<(i32, i32, usize, usize)> {
        self.entities.get(actor)?.states.entries().iter().find_map(|entry| {
            let StatePayload::SaitamaBoss {
                turns,
                damages,
                hitters,
                minions,
            } = &entry.payload
            else {
                return None;
            };
            Some((*turns, *damages, hitters.len(), minions.len()))
        })
    }

    fn drain_plain_saitama_action_into(
        &mut self,
        actor: EntityIdx,
        selected_target: Option<EntityIdx>,
        updates: &mut RunUpdates,
    ) {
        let (turns, damages, hitter_count, minion_count) = self
            .saitama_boss_state(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor lacks saitama state: {}", actor.0));
        let hunger_denominator = hitter_count as i32 + minion_count as i32 / 3 + 1;
        if damages / hunger_denominator.max(1) > 255 {
            let display_name = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
                .template
                .display_name
                .clone();
            let mut hungry_update =
                crate::engine::update::RunUpdate::new(format!("{display_name}觉得有点饿"), actor.0 as usize, actor.0 as usize, 0);
            hungry_update.delay1 = 2000;
            updates.add(hungry_update);
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                format!(" {display_name}离开了战场"),
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            let team = {
                let actor_entity = self
                    .entities
                    .get_mut(actor)
                    .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0));
                actor_entity.runtime.hp = 0;
                actor_entity.runtime.alive = false;
                actor_entity.runtime.team
            };
            self.world.mark_dead(actor, team);
            return;
        }

        if turns < 10 {
            let actor_entity = self
                .entities
                .get_mut(actor)
                .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0));
            let entry = actor_entity
                .states
                .entry_mut(PLAIN_SAITAMA_BOSS_STATE_KEY)
                .expect("runtime_v2 saitama state disappeared");
            let StatePayload::SaitamaBoss { turns, .. } = &mut entry.payload else {
                panic!("runtime_v2 saitama state key is occupied by another state");
            };
            *turns += 1;
            return;
        }

        let Some(target) = selected_target else {
            return;
        };
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
            .runtime
            .get_at(false, &mut self.rng)
            * 12.0;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发起攻击",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);

        let actor_team = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
            .runtime
            .team;
        let team_members = self
            .entities
            .iter()
            .filter_map(|(member, entity)| (entity.runtime.team == actor_team).then_some(member))
            .collect::<Vec<_>>();
        for member in team_members {
            self.entities
                .get_mut(member)
                .expect("runtime_v2 saitama team member disappeared")
                .runtime
                .move_state
                .speed_points = 0;
        }
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
            .runtime
            .move_state
            .speed_points = 1700;
    }

    fn drain_plain_attack_with_atp_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_with_atp_covid_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    fn drain_plain_attack_with_atp_and_on_damage_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_with_atp_covid_and_on_damage_into(actor, target, use_magic, atp, covid_source, on_damage, updates)
    }

    fn drain_plain_attack_with_atp_and_covid_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        updates: &mut RunUpdates,
    ) -> i32 {
        self.drain_plain_attack_with_atp_covid_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    fn drain_plain_attack_with_atp_covid_and_on_damage_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false);
        let mut defend_value = RuntimeDefendValue::Atp {
            value: atp,
            caster: actor,
            target,
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:pre_defend_before] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
        let Some(atp) = defend_value.atp() else {
            panic!("runtime_v2 PRE_DEFEND hooks must leave an atp value");
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:pre_defend_after] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        if atp == 0.0 {
            return 0;
        }

        let (accuracy, dodge_value, target_active) = {
            let actor_runtime = &self.entities.get(actor).unwrap().runtime;
            let target_entity = self.entities.get(target).unwrap();
            let target_runtime = &target_entity.runtime;
            (
                if use_magic {
                    actor_runtime.magic + actor_runtime.agility
                } else {
                    actor_runtime.attack + actor_runtime.agility
                },
                if use_magic {
                    target_runtime.resistance + target_runtime.agility
                } else {
                    target_runtime.defense + target_runtime.agility
                },
                target_entity.is_active(),
            )
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:dodge_before] actor={} target={} accuracy={} dodge={} active={} rc4=({}, {})",
                actor.0, target.0, accuracy, dodge_value, target_active, self.rng.i, self.rng.j,
            );
        }
        let dodged = target_active && PlayerRuntime::dodge(accuracy, dodge_value, &mut self.rng);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:dodge_after] actor={} target={} dodged={} rc4=({}, {})",
                actor.0, target.0, dodged, self.rng.i, self.rng.j,
            );
        }
        if dodged {
            updates.add(RuntimeFrame::replay_update(
                target.0 as usize,
                actor.0 as usize,
                "[0][回避]了攻击",
                20,
            ));
            return 0;
        }

        self.drain_plain_attack_after_dodge_into(actor, target, use_magic, atp, covid_source, on_damage, updates)
    }

    fn drain_plain_defended_attack_with_atp_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        let mut defend_value = RuntimeDefendValue::Atp {
            value: atp,
            caster: actor,
            target,
        };
        self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
        let Some(atp) = defend_value.atp() else {
            panic!("runtime_v2 PRE_DEFEND hooks must leave an atp value");
        };
        if atp == 0.0 {
            return 0;
        }
        self.drain_plain_attack_after_dodge_into(actor, target, use_magic, atp, covid_source, PlainAttackOnDamage::None, updates)
    }

    fn drain_plain_attack_after_dodge_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false);
        let defense = {
            let target_runtime = &self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 attack target: {}", target.0))
                .runtime;
            if use_magic {
                target_runtime.resistance + 64
            } else {
                target_runtime.defense + 64
            }
        };
        let amount = (atp / defense as f64).ceil() as i32;
        let mut defend_value = RuntimeDefendValue::Damage {
            value: amount,
            caster: actor,
            target,
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:post_defend_before] actor={} target={} damage={} rc4=({}, {})",
                actor.0, target.0, amount, self.rng.i, self.rng.j,
            );
        }
        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
        let Some(amount) = defend_value.damage() else {
            panic!("runtime_v2 POST_DEFEND hooks must leave a damage value");
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:post_defend_after] actor={} target={} damage={} rc4=({}, {})",
                actor.0, target.0, amount, self.rng.i, self.rng.j,
            );
        }
        if self.apply_plain_attack_damage_with_covid_and_on_damage_into(actor, target, amount, covid_source, on_damage, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
        amount
    }

    fn apply_plain_attack_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let covid_source = self.covid_boss_mutation(caster).map(|mutation| (caster, mutation));
        self.apply_plain_attack_damage_with_covid_into(caster, target, amount, covid_source, updates)
    }

    fn apply_plain_attack_damage_with_covid_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        covid_source: Option<(EntityIdx, i32)>,
        updates: &mut RunUpdates,
    ) -> bool {
        self.apply_plain_attack_damage_with_covid_and_on_damage_into(
            caster,
            target,
            amount,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    fn apply_plain_attack_damage_with_covid_and_on_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> bool {
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default attack target: {}", target.0));
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        if let Some((boss, mutation)) = covid_source {
            self.try_covid_spread_on_damage_into(boss, target, mutation, amount, updates);
        }
        if self.lazy_boss_at_boost(caster).is_some() {
            self.infect_with_lazy_into(caster, target, updates);
            if amount > 0 {
                self.set_lazy_boss_at_boost(caster, 1.0);
            }
        }
        match on_damage {
            PlainAttackOnDamage::None => {}
            PlainAttackOnDamage::Absorb => self.apply_absorb_on_damage(caster, amount, updates),
            PlainAttackOnDamage::Berserk => self.apply_berserk_on_damage(caster, target, amount, updates),
            PlainAttackOnDamage::Curse => self.apply_curse_on_damage(caster, target, amount, updates),
            PlainAttackOnDamage::Poison => self.apply_poison_on_damage(caster, target, amount, updates),
        }
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        killed
    }

    fn apply_absorb_on_damage(&mut self, caster: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        let owner = self
            .entities
            .get_mut(caster)
            .unwrap_or_else(|| panic!("unknown runtime_v2 absorb caster: {}", caster.0));
        if owner.runtime.hp <= 0 {
            return;
        }
        let healed = ((damage + 1) / 2).min(owner.template.max_hp - owner.runtime.hp);
        if healed > 0 {
            owner.runtime.hp = (owner.runtime.hp + healed).min(owner.template.max_hp);
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]回复体力[2]点",
            caster.0 as usize,
            caster.0 as usize,
            healed as u32,
        ));
    }

    fn apply_berserk_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        if self.entities.get(target).is_none_or(|entity| entity.runtime.hp <= 0) || self.status_immune(target, "berserk") {
            return;
        }
        let charge_active = self
            .entities
            .get(caster)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);
        let existing_key = self.entities.get(target).and_then(|entity| {
            entity
                .states
                .entries()
                .iter()
                .find(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
                .map(|entry| entry.legacy_order_key)
        });
        if let Some(state_key) = existing_key {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 berserk target: {}", target.0));
            let StatePayload::Berserk { step } = &mut target_entity
                .states
                .entry_mut(state_key)
                .expect("runtime_v2 berserk state disappeared during extension")
                .payload
            else {
                unreachable!("runtime_v2 berserk state key changed payload during extension");
            };
            *step += 1 + i32::from(charge_active);
            return;
        }

        assert!(
            self.entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 berserk target: {}", target.0))
                .states
                .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 1 + i32::from(charge_active),)),
            "runtime_v2 berserk state should be inserted"
        );
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]进入[狂暴]状态",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    fn apply_curse_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        let (target_hp, target_flags, charge_active, existing) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 curse target: {}", target.0));
            let existing = target_entity.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| match entry.payload {
                StatePayload::Curse { prob, multiply } => (prob, multiply),
                _ => panic!("runtime_v2 curse state key is occupied by another payload"),
            });
            (
                target_entity.runtime.hp,
                target_entity.runtime.flags,
                target_entity.runtime.at_boost_millionths >= 3_000_000,
                existing,
            )
        };
        if target_hp <= 0 || target_flags.intersects(PlayerKindFlags::BOSS | PlayerKindFlags::BOOST) {
            return;
        }

        let curse_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_CURSE_STATE_EXPORT)
            .expect("default runtime v2 profile must register curse state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 curse target: {}", target.0));
        let charge_prob = if charge_active { 10 } else { 0 };
        let charge_multiply = if charge_active { 1 } else { 0 };
        if let Some((prob, multiply)) = existing {
            assert!(
                target_entity.states.set_payload(
                    PLAIN_CURSE_STATE_KEY,
                    StatePayload::Curse {
                        prob: prob + 10 + charge_prob,
                        multiply: multiply + 1 + charge_multiply,
                    },
                ),
                "runtime_v2 curse state disappeared while stacking"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::curse(
                    PLAIN_CURSE_STATE_KEY,
                    curse_state,
                    42 + charge_prob,
                    2 + charge_multiply,
                    SkillPriority(10_000),
                )),
                "runtime_v2 curse state key should be vacant"
            );
            target_entity.runtime.atk_sum = target_entity.runtime.atk_sum.saturating_mul(4);
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]被[诅咒]了",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    fn apply_poison_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 4 {
            return;
        }
        let target_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 poison target: {}", target.0))
            .runtime
            .hp;
        if target_hp <= 0 || self.status_immune(target, "poison") {
            return;
        }

        let poison_atp = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime_v2 poison caster: {}", caster.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 1.2000000476837158;
        let poison_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_POISON_STATE_EXPORT)
            .expect("default runtime v2 profile must register poison state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 poison target: {}", target.0));
        let existing = target_entity.states.entry(PLAIN_POISON_STATE_KEY).map(|entry| match entry.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                ..
            } => (caster, target, f64::from_bits(atp_bits)),
            _ => panic!("runtime_v2 poison state key is occupied by another payload"),
        });
        if let Some((_, existing_target, existing_atp)) = existing {
            assert!(
                target_entity.states.set_payload(
                    PLAIN_POISON_STATE_KEY,
                    StatePayload::Poison {
                        caster: Some(caster.0),
                        target: existing_target.or(Some(target.0)),
                        atp_bits: (existing_atp + poison_atp).to_bits(),
                        count: 4,
                    },
                ),
                "runtime_v2 poison state disappeared while stacking"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::poison(
                    PLAIN_POISON_STATE_KEY,
                    poison_state,
                    Some(caster.0),
                    Some(target.0),
                    poison_atp,
                    4,
                    SkillPriority(150),
                )),
                "runtime_v2 poison state key should be vacant"
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1][中毒]",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    fn covid_boss_mutation(&self, boss: EntityIdx) -> Option<i32> {
        self.entities.get(boss)?.states.entries().iter().find_map(|entry| {
            let StatePayload::CovidBoss { mutation } = &entry.payload else {
                return None;
            };
            Some(*mutation)
        })
    }

    fn has_covid_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(&entry.payload, StatePayload::CovidInfection { .. }))
        })
    }

    fn lazy_boss_at_boost(&self, boss: EntityIdx) -> Option<f64> {
        self.entities.get(boss)?.states.entries().iter().find_map(|entry| {
            let StatePayload::LazyBoss { at_boost_bits } = &entry.payload else {
                return None;
            };
            Some(f64::from_bits(*at_boost_bits))
        })
    }

    fn set_lazy_boss_at_boost(&mut self, boss: EntityIdx, at_boost: f64) {
        let boss_entity = self
            .entities
            .get_mut(boss)
            .unwrap_or_else(|| panic!("unknown runtime_v2 lazy boss entity: {}", boss.0));
        assert!(
            boss_entity.states.set_payload(
                PLAIN_LAZY_BOSS_STATE_KEY,
                StatePayload::LazyBoss {
                    at_boost_bits: at_boost.to_bits(),
                },
            ),
            "runtime_v2 lazy boss state disappeared"
        );
    }

    fn has_lazy_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(&entry.payload, StatePayload::LazyInfection { .. }))
        })
    }

    fn infect_with_lazy_into(&mut self, boss: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        if target == boss || self.has_lazy_infection(target) {
            return false;
        }
        let state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT)
            .expect("default runtime v2 profile must register lazy infection state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 lazy infection target: {}", target.0));
        if !target_entity.states.add_entry(StateEntry::lazy_infection(
            PLAIN_LAZY_INFECTION_STATE_KEY,
            state_id,
            boss,
            SkillPriority(1000),
        )) {
            return false;
        }
        let boss_display = self
            .entities
            .get(boss)
            .unwrap_or_else(|| panic!("unknown runtime_v2 lazy boss entity: {}", boss.0))
            .template
            .display_name
            .clone();
        updates.add(crate::engine::update::RunUpdate::new(
            format!("[1]感染了{boss_display}"),
            boss.0 as usize,
            target.0 as usize,
            0,
        ));
        true
    }

    fn emit_lazy_activity_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let activity = match self.rng.next_u8() {
            0..=49 => "Steam",
            50..=99 => "守望先锋",
            100..=149 => "文明6",
            150..=189 => "英雄联盟",
            190..=229 => "微博",
            _ => "朋友圈",
        };
        let owner_name = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime_v2 lazy activity owner: {}", owner.0))
            .template
            .display_name
            .clone();
        updates.add(crate::engine::update::RunUpdate::new(
            format!("{owner_name}打开了{activity}, 这回合什么也没做"),
            owner.0 as usize,
            owner.0 as usize,
            0,
        ));
    }

    fn try_covid_spread_on_damage_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        damage: i32,
        updates: &mut RunUpdates,
    ) {
        if self.has_covid_infection(target) {
            return;
        }
        if i32::from(self.rng.next_u8() & 63) < damage {
            self.infect_with_covid_into(boss, target, mutation, updates);
        }
    }

    fn infect_with_covid_into(&mut self, boss: EntityIdx, target: EntityIdx, mutation: i32, updates: &mut RunUpdates) -> bool {
        if target == boss {
            return false;
        }
        let boss_display = self
            .entities
            .get(boss)
            .unwrap_or_else(|| panic!("unknown runtime_v2 covid boss entity: {}", boss.0))
            .template
            .display_name
            .clone();
        let infection_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT)
            .expect("default runtime v2 profile must register covid infection state");

        let infected = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 covid target entity: {}", target.0));
            if let Some(entry) = target_entity.states.entry_mut(PLAIN_COVID_INFECTION_STATE_KEY) {
                let StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                } = &mut entry.payload
                else {
                    panic!("runtime_v2 covid infection key is occupied by a different state");
                };
                if !*recovered || mutation_set.contains(&mutation) {
                    false
                } else {
                    *recovered = false;
                    entries.push(CovidInfectionEntry { boss, mutation, days: 0 });
                    mutation_set.push(mutation);
                    true
                }
            } else {
                target_entity.states.add_entry(StateEntry::covid_infection(
                    PLAIN_COVID_INFECTION_STATE_KEY,
                    infection_state,
                    boss,
                    mutation,
                    SkillPriority(1000),
                ))
            }
        };
        if !infected {
            return false;
        }

        updates.add(crate::engine::update::RunUpdate::new(
            format!("[1]感染了{boss_display}"),
            boss.0 as usize,
            target.0 as usize,
            0,
        ));
        let all_alive = self.world.flat_alive().to_vec();
        for entity_idx in all_alive {
            let delta = if entity_idx == target { 2048 } else { -256 };
            self.entities
                .get_mut(entity_idx)
                .unwrap_or_else(|| panic!("runtime_v2 covid alive entity disappeared: {}", entity_idx.0))
                .runtime
                .move_state
                .speed_points += delta;
        }
        true
    }

    fn drain_covid_pneumonia_into(&mut self, owner: EntityIdx, boss: EntityIdx, mutation: i32, updates: &mut RunUpdates) {
        if !self.entities.get(owner).is_some_and(|entity| entity.runtime.alive) {
            return;
        }
        let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
        let atp = self.entities.get(owner).unwrap().runtime.get_at(true, &mut self.rng);
        let defense = self.entities.get(owner).unwrap().runtime.magic_defense();
        let damage = ((atp + f64::from(mutation * 80)) / f64::from(defense)).ceil() as i32;
        if damage <= 0 {
            return;
        }

        updates.add(crate::engine::update::RunUpdate::new(
            format!(" {owner_name}肺炎发作"),
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        let old_hp = self.entities.get(owner).unwrap().runtime.hp;
        let killed = self.apply_plain_attack_damage_with_covid_into(boss, owner, damage, None, updates);
        let actual_damage = if killed { old_hp } else { damage };
        if killed {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
        }

        let boss_hp_full = {
            let boss_entity = self.entities.get(boss).unwrap();
            boss_entity.runtime.hp >= boss_entity.template.max_hp
        };
        let heal_amount = if boss_hp_full {
            ((damage >> 3) + 1).min(actual_damage)
        } else {
            (damage >> 1).min(actual_damage)
        };
        if heal_amount <= 0 {
            return;
        }
        let boss_entity = self.entities.get_mut(boss).unwrap();
        boss_entity.runtime.hp = (boss_entity.runtime.hp + heal_amount).min(boss_entity.template.max_hp);
        let boss_display = boss_entity.template.display_name.clone();
        updates.add(crate::engine::update::RunUpdate::new(
            format!("{boss_display}回复体力{heal_amount}点"),
            boss.0 as usize,
            boss.0 as usize,
            0,
        ));
    }

    fn drain_lazy_flare_into(&mut self, owner: EntityIdx, boss: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(owner).is_some_and(|entity| entity.runtime.alive)
            || !self.entities.get(boss).is_some_and(|entity| entity.runtime.alive)
        {
            return;
        }
        let boss_atp = self.entities.get(boss).unwrap().runtime.get_at(true, &mut self.rng);
        let target_defense = self.entities.get(owner).unwrap().runtime.magic_defense();
        let damage = (boss_atp / f64::from(target_defense)).ceil() as i32;
        if damage <= 0 {
            return;
        }
        let boss_display = self.entities.get(boss).unwrap().template.display_name.clone();
        let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
        updates.add(crate::engine::update::RunUpdate::new(
            format!(" {owner_name}{boss_display}发作"),
            boss.0 as usize,
            owner.0 as usize,
            0,
        ));
        if self.apply_plain_attack_damage_with_covid_into(boss, owner, damage, None, updates) {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
        }
    }

    fn drain_plain_lethal_damage_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let die_message = if self
            .entities
            .get(target)
            .is_some_and(|entity| entity.runtime.flags.contains(PlayerKindFlags::MINION))
        {
            "[1]消失了"
        } else {
            "[1]被击倒了"
        };
        updates.add_newline();
        updates.add(crate::engine::update::RunUpdate::new(
            die_message,
            caster.0 as usize,
            target.0 as usize,
            50,
        ));
        self.drain_die_hooks_into(target, updates);

        let (hp, team) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hp, entity.runtime.team))
            .unwrap_or_else(|| panic!("runtime_v2 lethal target disappeared: {}", target.0));
        if hp > 0 {
            return;
        }

        self.entities.get_mut(target).unwrap().runtime.alive = false;
        self.world.mark_dead(target, team);
        self.cleanup_linked_minions_for_owner(target, updates);
        self.drain_kill_hooks_into(caster, target, updates);
    }

    fn recover_plain_actor_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let recover_threshold = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 recovery actor: {}", actor.0))
            .runtime
            .wisdom
            + 64;
        if (self.rng.r127() as i32) < recover_threshold {
            self.entities.get_mut(actor).unwrap().runtime.magic_point += 16;
        }
        updates.add_newline();
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
                QueuedEffect::ReflectedAttack {
                    caster,
                    target,
                    atp_bits,
                } => {
                    self.ensure_effect_entity("reflected attack", "caster", caster);
                    self.ensure_effect_entity("reflected attack", "target", target);
                    self.drain_plain_attack_with_atp_into(caster, target, true, f64::from_bits(atp_bits), updates);
                    self.entities
                        .get_mut(caster)
                        .expect("runtime_v2 reflected attack caster disappeared")
                        .runtime
                        .move_state
                        .speed_points -= 480;
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
                QueuedEffect::FireAttack {
                    caster,
                    target,
                    fire_state_key,
                } => {
                    self.ensure_effect_entity("fire-attack", "caster", caster);
                    self.ensure_effect_entity("fire-attack", "target", target);
                    let fire_mag = self.entities.get(target).unwrap().states.fire_mag(fire_state_key);
                    let atp = self.entities.get(caster).unwrap().runtime.get_at(true, &mut self.rng);
                    let mut defend_value = RuntimeDefendValue::Atp {
                        value: atp * (1.5 + fire_mag),
                        caster,
                        target,
                    };
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        "[0]使用[火球术]",
                        1,
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
                        if self.apply_damage_into(caster, target, amount, updates) {
                            self.drain_lethal_damage_hooks_into(caster, target, updates);
                        } else if amount > 0 {
                            self.apply_fire_on_damage(target, fire_state_key);
                        }
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
                QueuedEffect::CovidContact {
                    owner,
                    candidate,
                    boss,
                    mutation,
                } => {
                    self.ensure_effect_entity("covid-contact", "owner", owner);
                    self.ensure_effect_entity("covid-contact", "candidate", candidate);
                    self.ensure_effect_entity("covid-contact", "boss", boss);
                    let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
                    let candidate_entity = self.entities.get(candidate).unwrap();
                    let candidate_name = candidate_entity.template.display_name.clone();
                    let threshold = candidate_entity.runtime.wisdom >> 1;
                    updates.add(crate::engine::update::RunUpdate::new(
                        format!("{owner_name}和{candidate_name}近距离接触"),
                        owner.0 as usize,
                        candidate.0 as usize,
                        0,
                    ));
                    if i32::from(self.rng.next_u8()) < threshold {
                        updates.add(crate::engine::update::RunUpdate::new(
                            format!("但{candidate_name}没被感染"),
                            owner.0 as usize,
                            candidate.0 as usize,
                            0,
                        ));
                    } else {
                        self.infect_with_covid_into(boss, candidate, mutation, updates);
                    }
                }
                QueuedEffect::CovidAttack {
                    owner,
                    candidate,
                    boss,
                    mutation,
                } => {
                    self.ensure_effect_entity("covid-attack", "owner", owner);
                    self.ensure_effect_entity("covid-attack", "candidate", candidate);
                    self.ensure_effect_entity("covid-attack", "boss", boss);
                    updates.add(RuntimeFrame::replay_update(
                        owner.0 as usize,
                        candidate.0 as usize,
                        "[0]发起攻击",
                        0,
                    ));
                    let atp = self.entities.get(owner).unwrap().runtime.get_at(false, &mut self.rng);
                    self.drain_plain_attack_with_atp_and_covid_into(
                        owner,
                        candidate,
                        false,
                        atp,
                        Some((boss, mutation)),
                        updates,
                    );
                }
                QueuedEffect::CovidPneumonia { owner, boss, mutation } => {
                    self.ensure_effect_entity("covid-pneumonia", "owner", owner);
                    self.ensure_effect_entity("covid-pneumonia", "boss", boss);
                    self.drain_covid_pneumonia_into(owner, boss, mutation, updates);
                }
                QueuedEffect::LazyFlare { owner, boss } => {
                    self.ensure_effect_entity("lazy-flare", "owner", owner);
                    self.ensure_effect_entity("lazy-flare", "boss", boss);
                    self.drain_lazy_flare_into(owner, boss, updates);
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
                QueuedEffect::AddBerserkState {
                    target,
                    legacy_order_key,
                    step,
                } => {
                    self.ensure_effect_entity("add-berserk-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 add-berserk-state target entity: {}", target.0);
                    };
                    let next_step = target_entity
                        .states
                        .entry(legacy_order_key)
                        .and_then(|entry| match entry.payload {
                            StatePayload::Berserk { step: existing_step } => Some(existing_step + step),
                            _ => None,
                        })
                        .unwrap_or(step);
                    if !target_entity
                        .states
                        .set_payload(legacy_order_key, StatePayload::Berserk { step: next_step })
                    {
                        target_entity.states.add_entry(StateEntry::berserk(legacy_order_key, next_step));
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
                    self.world.mark_dead(target, team);
                    updates.add(RuntimeFrame::remove_update(caster.0 as usize, target.0 as usize));
                    self.cleanup_linked_minions_for_owner(target, updates);
                }
                QueuedEffect::Merge { caster, target } => {
                    self.ensure_effect_entity("merge", "caster", caster);
                    self.ensure_effect_entity("merge", "target", target);
                    let target_skills = self.entities.get(target).unwrap().template.skills.clone();
                    let target_build = self.entities.get(target).unwrap().template.clone_build.clone();
                    let target_magic_point = self.entities.get(target).unwrap().runtime.magic_point;
                    let target_move_points = self.entities.get(target).unwrap().runtime.move_state.speed_points;
                    let (merged, transfer_magic_point, transfer_move_points) = {
                        let Some(caster_entity) = self.entities.get_mut(caster) else {
                            panic!("unknown runtime_v2 merge caster entity: {}", caster.0);
                        };
                        let merged_attrs = match (caster_entity.template.clone_build.as_mut(), target_build.as_ref()) {
                            (Some(owner_build), Some(target_build)) => owner_build.merge_attrs_from(target_build),
                            _ => false,
                        };
                        if merged_attrs {
                            let stats = caster_entity
                                .template
                                .clone_build
                                .as_ref()
                                .expect("runtime_v2 merge owner build disappeared")
                                .derive_stats();
                            caster_entity.apply_derived_stats(stats);
                        }
                        let merged_skills = caster_entity
                            .template
                            .skills
                            .merge_fixed_lanes_from(&target_skills, caster_entity.runtime.policies.merge);
                        let transfer_magic_point = target_magic_point > caster_entity.runtime.magic_point;
                        if transfer_magic_point {
                            caster_entity.runtime.magic_point = target_magic_point;
                        }
                        let transfer_move_points = target_move_points > caster_entity.runtime.move_state.speed_points;
                        if transfer_move_points {
                            caster_entity.runtime.move_state.speed_points += target_move_points;
                        }
                        (merged_attrs || merged_skills, transfer_magic_point, transfer_move_points)
                    };
                    if transfer_magic_point || transfer_move_points {
                        let target_entity = self
                            .entities
                            .get_mut(target)
                            .unwrap_or_else(|| panic!("unknown runtime_v2 merge target entity: {}", target.0));
                        if transfer_magic_point {
                            target_entity.runtime.magic_point = 0;
                        }
                        if transfer_move_points {
                            target_entity.runtime.move_state.speed_points = 0;
                        }
                    }
                    #[cfg(not(feature = "no_debug"))]
                    if std::env::var_os("TSWN_PROBE_KILL").is_some() {
                        let caster_entity = self
                            .entities
                            .get(caster)
                            .unwrap_or_else(|| panic!("runtime_v2 merge probe caster disappeared: {}", caster.0));
                        let find_fixed_lane = |loadout: &SkillLoadout, fixed_lane_key: usize| {
                            (0..loadout.len()).find_map(|lane| {
                                (loadout.fixed_lane_key_at(lane) == Some(fixed_lane_key)).then(|| (lane, loadout.level_at(lane)))
                            })
                        };
                        eprintln!(
                            "[kill_probe:v2:merge_effect] caster={} merged={} transfer_mp={} transfer_move={} \
                             attack={} magic={} speed={} agility={} mp={} move={} owner_key1={:?} target_key1={:?}",
                            caster.0,
                            merged,
                            transfer_magic_point,
                            transfer_move_points,
                            caster_entity.runtime.attack,
                            caster_entity.runtime.magic,
                            caster_entity.runtime.speed,
                            caster_entity.runtime.agility,
                            caster_entity.runtime.magic_point,
                            caster_entity.runtime.move_state.speed_points,
                            find_fixed_lane(&caster_entity.template.skills, 1),
                            find_fixed_lane(&target_skills, 1),
                        );
                    }
                    if merged {
                        updates.add_newline();
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
        if self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0) {
            return;
        }
        self.drain_kill_hooks_into(caster, target, updates);
    }

    fn drain_pre_defend_hooks_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        if self.drain_plain_protect_pre_defend_into(target, updates, defend_value) {
            return;
        }
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, target, ProcMask::PRE_DEFEND);
        self.drain_skill_hook_plan_with_defend_value_into(&skill_plan, updates, defend_value);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::PRE_DEFEND);
        self.drain_state_hook_plan_with_defend_value_into(&state_plan, updates, defend_value);
    }

    fn plain_protect_level(&self, owner: EntityIdx, fallback: u32) -> u32 {
        let Some(owner) = self.entities.get(owner) else {
            return fallback;
        };
        owner
            .template
            .skills
            .skills()
            .iter()
            .copied()
            .enumerate()
            .find_map(|(fixed_lane, skill_id)| {
                (self.registry.skill(skill_id)?.export_name == DEFAULT_CORE_PROTECT_SKILL_EXPORT)
                    .then(|| owner.template.skills.level_at(fixed_lane))
                    .flatten()
            })
            .unwrap_or(fallback)
    }

    fn drain_plain_protect_post_action_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let mut plan =
            self.scheduler
                .skill_post_action_hook_plan(&self.entities, &self.registry, owner, SkillPostActionPhase::Early);
        plan.entries.retain(|entry| {
            self.registry
                .skill(entry.skill_id)
                .is_some_and(|skill| skill.export_name == DEFAULT_CORE_PROTECT_SKILL_EXPORT)
        });
        if !plan.entries.is_empty() {
            self.drain_skill_hook_plan_into(&plan, updates);
        }
    }

    fn drain_plain_protect_pre_defend_into(
        &mut self,
        target: EntityIdx,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) -> bool {
        let Some(incoming_atp) = defend_value.atp() else {
            return false;
        };
        let caster = defend_value.caster();
        let target_team = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("runtime_v2 protect target disappeared: {}", target.0))
            .runtime
            .team;

        loop {
            let link_count = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("runtime_v2 protect target disappeared: {}", target.0))
                .runtime
                .protect_from
                .len();
            let link_index = match link_count {
                0 => return false,
                1 => 0,
                count => self.rng.next_i32(count as i32) as usize,
            };
            let link = self
                .entities
                .get(target)
                .unwrap()
                .runtime
                .protect_from
                .get(link_index)
                .cloned()
                .unwrap_or_else(|| panic!("runtime_v2 protect link index disappeared: {link_index}"));
            let level = self.plain_protect_level(link.owner, link.level);
            let same_group = self
                .entities
                .get(link.owner)
                .is_some_and(|_| self.plain_effective_team(link.owner) == target_team);
            let trigger_ok = same_group && self.rng.r127() < level;
            let protector_ready = trigger_ok
                && self
                    .entities
                    .get_mut(link.owner)
                    .is_some_and(|protector| protector.runtime.mp_ready(&mut self.rng));

            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_PROBE_PROTECT").is_some() {
                eprintln!(
                    "[protect_probe:v2] target={} protector={} link_index={} links={} same_group={} level={} trigger_ok={} protector_ready={} rc4=({}, {})",
                    target.0,
                    link.owner.0,
                    link_index,
                    link_count,
                    same_group,
                    level,
                    trigger_ok,
                    protector_ready,
                    self.rng.i,
                    self.rng.j,
                );
            }

            if trigger_ok && protector_ready {
                self.drain_plain_protect_post_action_into(link.owner, updates);
                updates.add(crate::engine::update::RunUpdate::new(
                    "[0][守护][1]",
                    link.owner.0 as usize,
                    target.0 as usize,
                    40,
                ));

                let mut redirected_atp = RuntimeDefendValue::Atp {
                    value: incoming_atp,
                    caster,
                    target: link.owner,
                };
                self.drain_pre_defend_hooks_into(link.owner, updates, &mut redirected_atp);
                let redirected_atp = redirected_atp.atp().expect("runtime_v2 protect pre-defend hooks must leave an atp value");
                if redirected_atp == 0.0 {
                    defend_value.set_atp(0.0);
                    return true;
                }

                let defense = {
                    let protector = self
                        .entities
                        .get(link.owner)
                        .unwrap_or_else(|| panic!("runtime_v2 protector disappeared: {}", link.owner.0));
                    protector.runtime.defense + 64
                };
                let redirected_damage = (redirected_atp * 0.5 / defense as f64).floor() as i32;
                let mut redirected_damage_value = RuntimeDefendValue::Damage {
                    value: redirected_damage,
                    caster,
                    target: link.owner,
                };
                self.drain_post_defend_hooks_into(link.owner, updates, &mut redirected_damage_value);
                let redirected_damage = redirected_damage_value
                    .damage()
                    .expect("runtime_v2 protect post-defend hooks must leave a damage value");
                if self.apply_plain_attack_damage_into(caster, link.owner, redirected_damage, updates) {
                    self.drain_plain_lethal_damage_into(caster, link.owner, updates);
                }
                defend_value.set_atp(0.0);
                return true;
            }

            self.entities.get_mut(target).unwrap().runtime.protect_from.remove(link_index);
            if let Some(protector) = self.entities.get_mut(link.owner)
                && protector.runtime.protect_to == Some(target)
            {
                protector.runtime.protect_to = None;
            }
        }
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
        self.apply_runtime_shield_post_defend(target, defend_value);
    }

    fn apply_runtime_shield_post_defend(&mut self, target: EntityIdx, defend_value: &mut RuntimeDefendValue) {
        let Some(damage) = defend_value.damage() else {
            return;
        };
        if damage <= 0 {
            return;
        }
        let target = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 shield target disappeared: {}", target.0));
        if target.runtime.shield <= 0 {
            return;
        }
        if damage > target.runtime.shield {
            target.runtime.shield = 0;
        } else {
            target.runtime.shield -= damage;
            defend_value.set_damage(0);
        }
    }

    fn drain_die_hooks_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) {
        let die_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, target, ProcMask::DIE);
        self.drain_skill_hook_plan_into(&die_skill_plan, updates);
        if self.entities.get(target).is_some_and(|entity| entity.runtime.hp > 0) {
            return;
        }
        let die_state_plan = self.scheduler.state_hook_plan(&self.entities, target, ProcMask::DIE);
        self.drain_state_hook_plan_into(&die_state_plan, updates);
    }

    fn drain_kill_hooks_into(&mut self, caster: EntityIdx, killed_target: EntityIdx, updates: &mut RunUpdates) {
        let kill_skill_plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, caster, ProcMask::KILL);
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_KILL").is_some() {
            let caster_entity = self
                .entities
                .get(caster)
                .unwrap_or_else(|| panic!("runtime_v2 kill probe caster disappeared: {}", caster.0));
            let entries = kill_skill_plan
                .entries
                .iter()
                .map(|entry| {
                    let export_name = self
                        .registry
                        .skill(entry.skill_id)
                        .map(|spec| spec.export_name.as_str())
                        .unwrap_or("<missing>");
                    (
                        entry.fixed_lane,
                        export_name,
                        caster_entity.template.skills.level_at(entry.fixed_lane),
                    )
                })
                .collect::<Vec<_>>();
            eprintln!(
                "[kill_probe:v2:plan] caster={} name={} target={} entries={entries:?} rc4=({}, {})",
                caster.0, caster_entity.template.name, killed_target.0, self.rng.i, self.rng.j,
            );
        }
        self.drain_skill_hook_plan_with_selected_target_into(&kill_skill_plan, updates, Some(killed_target));
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
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        if killed {
            self.world.mark_dead(target, team);
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
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        if killed {
            self.world.mark_dead(target, team);
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
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        if amount > 0 {
            self.apply_disperse_hit_into(caster, target, updates);
        }
        if killed {
            let Some(target_entity) = self.entities.get_mut(target) else {
                panic!("unknown runtime_v2 disperse damage target entity: {}", target.0);
            };
            target_entity.runtime.alive = false;
            self.world.mark_dead(target, team);
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
        if !target_entity.is_active() {
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

    fn apply_ice_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 ice target entity: {}", target.0);
        };
        if target_entity.runtime.hp <= 0 || !target_entity.runtime.alive || self.ice_immune(target) {
            return;
        }
        let charge_active = self
            .entities
            .get(caster)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);
        let frozen_step = 1024 + if charge_active { 2048 } else { 0 };
        self.entities
            .get_mut(target)
            .unwrap()
            .states
            .add_ice_frozen_step(PLAIN_ICE_STATE_KEY, frozen_step);
        updates.add(RuntimeFrame::replay_update(
            caster.0 as usize,
            target.0 as usize,
            "[1]被[冰冻]了",
            40,
        ));
    }

    fn status_immune(&mut self, target: EntityIdx, status: &'static str) -> bool {
        let Some(target_entity) = self.entities.get(target) else {
            panic!("unknown runtime_v2 {status} immune target entity: {}", target.0);
        };
        if target_entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_entity.template.name, status);
            return (self.rng.next_u8() as i32) < threshold;
        }
        if target_entity.runtime.flags.contains(PlayerKindFlags::BOOST) {
            return self.rng.r127() < crate::player::boost_value(&target_entity.template.name);
        }
        false
    }

    fn ice_immune(&mut self, target: EntityIdx) -> bool { self.status_immune(target, "ice") }

    fn fire_immune(&mut self, target: EntityIdx) -> bool { self.status_immune(target, "fire") }

    fn kill_entity_without_damage_into(&mut self, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        let Some(target_entity) = self.entities.get_mut(target) else {
            panic!("unknown runtime_v2 self-death target entity: {}", target.0);
        };
        let killed = target_entity.runtime.alive;
        target_entity.runtime.hp = 0;
        target_entity.runtime.alive = false;
        let team = target_entity.runtime.team;
        if killed {
            self.world.mark_dead(target, team);
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
            self.world.mark_dead(minion, team);
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                "[1]消失了",
                owner.0 as usize,
                minion.0 as usize,
                50,
            ));
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

    mod plain_attack_skill_tests;
    mod plain_status_skill_tests;

    fn normalized_rng_checkpoint(i: u32, j: u32) -> crate::runtime_v2::oracle::NormalizedRngCheckpoint {
        crate::runtime_v2::oracle::NormalizedRngCheckpoint {
            i,
            j,
            #[cfg(not(feature = "no_debug"))]
            byte_count: 0,
        }
    }

    fn assert_rng_state_eq(actual: &RC4, expected: &RC4) {
        assert_eq!(actual.i, expected.i);
        assert_eq!(actual.j, expected.j);
        assert_eq!(actual.main_val, expected.main_val);
    }

    fn plain_large_expected_round(
        round: u64,
        winner_team: Option<usize>,
        score: u64,
        rng_i: u32,
        rng_j: u32,
        hp: [i32; 2],
        alive: [bool; 2],
        action: [usize; 2],
    ) -> NormalizedOutcome {
        let team_alive = vec![
            alive[0].then_some(0).into_iter().collect::<Vec<_>>(),
            alive[1].then_some(1).into_iter().collect::<Vec<_>>(),
        ];
        let flat_alive = alive
            .iter()
            .enumerate()
            .filter_map(|(idx, is_alive)| is_alive.then_some(idx))
            .collect::<Vec<_>>();
        let round_order = flat_alive.clone();
        let alive_group_count = team_alive.iter().filter(|team| !team.is_empty()).count();
        NormalizedOutcome {
            winner_team,
            round,
            total_score: score,
            rng: normalized_rng_checkpoint(rng_i, rng_j),
            entity_ids: vec![1, 2],
            teams: vec![0, 1],
            hp: hp.to_vec(),
            magic_point: vec![28, 29],
            defense: vec![58, 52],
            resistance: vec![49, 57],
            alive: alive.to_vec(),
            round_order,
            flat_alive,
            team_alive,
            alive_group_count,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round,
                actor: action[0],
                target: action[1],
                amount: score as i32,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: action[0],
                target: action[1],
                targets: Vec::new(),
                param: None,
                score: score as u32,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        }
    }

    #[test]
    fn minimal_1v1_template_builds_runtime() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(runtime.world.winner_team(), None);
        assert!(runtime.effects.is_empty());
        assert!(runtime.slots.is_empty());
        assert_eq!(runtime.validate_ready(), Ok(()));
    }

    #[test]
    fn runtime_ready_validation_reports_entity_and_template_skill_sources() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill("custom", "missing", "custom.missing", TargetPolicy::Enemy, SkillPriority(0))
            .expect("skill should register");
        let template_slot = builder
            .reserve_template_slot("custom", "spawn", "custom.spawn")
            .expect("template slot should reserve");
        let registry = builder.build();
        let mut template =
            PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill])], registry);
        template
            .slots
            .set(
                template_slot,
                SlotValue::PlayerTemplate(Box::new(PlayerTemplate::new(2, "spawn", 0, 5, 1).with_skills([skill]))),
            )
            .expect("template slot should accept player template");
        let mut runtime = CombatRuntime::from_template(template);

        let error = runtime.validate_ready().expect_err("missing handler should reject runtime");
        assert_eq!(
            error,
            RuntimeV2ReadyError {
                missing_skill_handlers: vec![RuntimeV2MissingSkillHandler {
                    skill_id: skill,
                    export_name: Some("custom.missing".to_owned()),
                    sources: vec![
                        RuntimeV2SkillSource::Entity(EntityIdx(0)),
                        RuntimeV2SkillSource::TemplateSlot(template_slot),
                    ],
                }],
            }
        );
        assert_eq!(
            error.to_string(),
            "runtime v2 missing skill handlers: custom.missing (id 0) used by entity 0, template slot 0"
        );

        runtime.set_skill_handler(skill, skill_noop);
        assert_eq!(runtime.validate_ready(), Ok(()));
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
        runner.runtime_mut().set_skill_handler(summon, skill_noop);

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
        let config = CustomRuntimeV2ImportConfig::new(registry, bed2, summon)
            .with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
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
            })
            .with_skill_handler(summon, skill_noop)
            .with_skill_handler(fire, skill_noop)
            .with_skill_handler(explode, skill_noop)
            .with_skill_handler(possess, skill_noop)
            .with_skill_handler(zombie_heal, skill_noop);
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
        let overlays = config
            .bed2_minion_overlays
            .expect("default custom profile should install bed2 minion overlay import");
        assert_eq!(
            config
                .registry
                .skill_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT)
                .map(|spec| spec.id),
            Some(summon)
        );
        let fire = config
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT)
            .expect("default profile should register summon fire export");
        let explode = config
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT)
            .expect("default profile should register summon explode export");
        let possess = config
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT)
            .expect("default profile should register minion possess export");
        assert_eq!(config.registry.skill(possess).unwrap().hook_mask, ProcMask::NONE);
        assert_eq!(config.registry.player_kind(bed2).unwrap().export_name, "custom.bed2");
        assert_eq!(overlays.summon.template_slot, TemplateSlotId(0));
        assert_eq!(
            config.registry.player_kind(overlays.summon.summon_kind).unwrap().export_name,
            DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT
        );
        assert_eq!(
            config.registry.player_kind(overlays.shadow.shadow_kind).unwrap().export_name,
            DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT
        );
        assert_eq!(
            config.registry.player_kind(overlays.zombie.zombie_kind).unwrap().export_name,
            DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT
        );

        let raw_input = "plain@red\n\
alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";
        let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw_input.to_owned(), config)
            .expect("default custom profile should construct mixed runner");
        let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

        assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
        assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
        assert_eq!(
            runner.runtime().entities.get(EntityIdx(1)).unwrap().template.skills.skills(),
            &[summon]
        );
        let SlotValue::PlayerTemplate(summon_template) = runner
            .runtime()
            .template_slots
            .get(overlays.summon.template_slot)
            .expect("default profile should populate summon template slot")
        else {
            panic!("default profile summon overlay slot should hold PlayerTemplate");
        };
        assert_eq!(summon_template.kind, overlays.summon.summon_kind);
        assert_eq!(summon_template.max_hp, 123);
        assert_eq!(summon_template.policy_overrides.inherit_owner_def_res, Some(true));
        assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
        assert_eq!(summon_template.skills.active_order(), &[1, 0]);

        let SlotValue::PlayerTemplate(shadow_template) = runner
            .runtime()
            .template_slots
            .get(overlays.shadow.template_slot)
            .expect("default profile should populate shadow template slot")
        else {
            panic!("default profile shadow overlay slot should hold PlayerTemplate");
        };
        assert_eq!(shadow_template.kind, overlays.shadow.shadow_kind);
        assert_eq!(shadow_template.max_hp, 88);
        assert_eq!(shadow_template.skills.skills(), &[possess]);

        let SlotValue::PlayerTemplate(zombie_template) = runner
            .runtime()
            .template_slots
            .get(overlays.zombie.template_slot)
            .expect("default profile should populate zombie template slot")
        else {
            panic!("default profile zombie overlay slot should hold PlayerTemplate");
        };
        assert_eq!(zombie_template.kind, overlays.zombie.zombie_kind);
        assert_eq!(zombie_template.max_hp, 77);
        assert!(zombie_template.skills.skills().is_empty());
        let zombie_heal = runner
            .runtime()
            .registry
            .skill_id_by_export_name("custom.minion.heal")
            .expect("default profile should register minion heal export");
        assert!(runner.runtime().skill_handlers.get(zombie_heal).is_none());
    }

    #[test]
    fn default_profile_imports_plain_defend_skill_level_from_legacy_loadout() {
        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let defend = config
            .registry
            .skill_id_by_export_name(DEFAULT_CORE_DEFEND_SKILL_EXPORT)
            .expect("default profile should register core defend skill");
        let raw = "left@red\n\nright@blue\n";
        let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
            .expect("plain raw should construct runtime v2 runner");
        let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("plain raw should construct legacy runner");
        let snapshot = legacy
            .storage
            .get_player(&1)
            .expect("right legacy player should exist")
            .skill_loadout_snapshot();
        let defend_kind = std::any::type_name::<crate::player::skill::defend::DefendSkill>();
        let expected_level = snapshot
            .entries
            .iter()
            .find(|entry| entry.runtime_kind == defend_kind)
            .map(|entry| entry.level)
            .expect("right legacy player should have DefendSkill");
        let right = runner.runtime().entities.get(EntityIdx(1)).expect("right runtime v2 player should exist");

        let defend_lane = right
            .template
            .skills
            .skills()
            .iter()
            .position(|skill| *skill == defend)
            .unwrap_or_else(|| panic!("runtime v2 loadout should contain DefendSkill; legacy snapshot: {snapshot:?}"));
        assert_eq!(right.template.skills.level_at(defend_lane), Some(expected_level));
    }

    #[test]
    fn default_profile_imports_plain_merge_kill_hook_from_legacy_loadout() {
        let raw = "我力 7#W2ib8D@仙蛊屋+123\n\
                   万我 68#huMG43@仙蛊屋+123\n\n\
                   Dianmu YKFMWRPXIMCQ@nan+234\n\
                   Freddy FVNXBNVTWJEA@nan+234\n\n\
                   seed:第十八届武术大赛小组赛第8组:307-3@!\n";
        let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("large_51 raw should construct legacy runner");
        let snapshot = legacy
            .storage
            .get_player(&0)
            .expect("large_51 merge owner should exist")
            .skill_loadout_snapshot();
        let merge_kind = std::any::type_name::<crate::player::skill::merge::MergeSkill>();
        let expected_level = snapshot
            .entries
            .iter()
            .find(|entry| entry.runtime_kind == merge_kind)
            .map(|entry| entry.level)
            .expect("large_51 owner should have MergeSkill");

        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let merge = config
            .registry
            .skill_id_by_export_name(DEFAULT_CORE_MERGE_SKILL_EXPORT)
            .expect("default profile should register core merge skill");
        assert_eq!(config.registry.skill(merge).unwrap().hook_mask, ProcMask::KILL);
        let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
            .expect("large_51 raw should construct runtime v2 runner");
        let owner = runner
            .runtime()
            .entities
            .get(EntityIdx(0))
            .expect("large_51 runtime v2 owner should exist");
        let merge_lane = owner
            .template
            .skills
            .skills()
            .iter()
            .position(|skill| *skill == merge)
            .unwrap_or_else(|| panic!("runtime v2 loadout should contain MergeSkill; legacy snapshot: {snapshot:?}"));
        assert_eq!(owner.template.skills.level_at(merge_lane), Some(expected_level));
        assert!(runner.runtime().skill_handlers.get(merge).is_some());

        let plan = runner.runtime().scheduler.skill_hook_plan(
            &runner.runtime().entities,
            &runner.runtime().registry,
            EntityIdx(0),
            ProcMask::KILL,
        );
        assert!(plan.entries.iter().any(|entry| entry.skill_id == merge && entry.fixed_lane == merge_lane));
    }

    #[test]
    fn builtin_active_skill_semantic_exports_round_trip_legacy_keys() {
        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");

        for skill in BuiltinActiveSkill::ALL {
            assert_eq!(BuiltinActiveSkill::from_legacy_key(skill.legacy_key()), Some(skill));
            assert_eq!(BuiltinActiveSkill::from_export_name(skill.export_name()), Some(skill));
            let registered = config
                .registry
                .skill_id_by_export_name(skill.export_name())
                .unwrap_or_else(|| panic!("default profile should register {}", skill.export_name()));
            assert_eq!(config.registry.skill(registered).unwrap().export_name, skill.export_name());
        }

        assert_eq!(BuiltinActiveSkill::from_legacy_key(BuiltinActiveSkill::ALL.len()), None);
        assert_eq!(BuiltinActiveSkill::from_export_name("core.skill.24"), None);
    }

    #[test]
    fn default_profile_imports_and_executes_plain_shadow_blueprint() {
        let raw = "我力 7#W2ib8D@仙蛊屋+123\n\
                   万我 68#huMG43@仙蛊屋+123\n\n\
                   Dianmu YKFMWRPXIMCQ@nan+234\n\
                   Freddy FVNXBNVTWJEA@nan+234\n\n\
                   seed:第十八届武术大赛小组赛第8组:307-3@!\n";
        let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("large_51 raw should construct legacy runner");
        let legacy_owner = legacy.storage.get_player(&0).expect("large_51 shadow owner should exist");
        let snapshot = legacy_owner.skill_loadout_snapshot();
        let shadow_kind = std::any::type_name::<crate::player::skill::act::shadow::ShadowSkill>();
        let expected_level = snapshot
            .entries
            .iter()
            .find(|entry| entry.runtime_kind == shadow_kind)
            .map(|entry| entry.level)
            .expect("large_51 owner should have ShadowSkill");
        let legacy_shadow = crate::player::skill::act::shadow::build_shadow_minion(0, &legacy.storage);
        let legacy_shadow_status = legacy_shadow.get_status();
        let legacy_shadow_snapshot = legacy_shadow.skill_loadout_snapshot();
        let possess_kind = std::any::type_name::<crate::player::skill::act::possess::PossessSkill>();
        let expected_possess_level = legacy_shadow_snapshot
            .entries
            .iter()
            .find(|entry| entry.runtime_kind == possess_kind)
            .map(|entry| entry.level)
            .expect("large_51 shadow should have PossessSkill");

        let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
        let shadow = config
            .registry
            .skill_id_by_export_name(BuiltinActiveSkill::Shadow.export_name())
            .expect("default profile should register core shadow skill");
        let possess = config
            .registry
            .skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT)
            .expect("default profile should register minion possess skill");
        assert_eq!(config.registry.skill(possess).unwrap().hook_mask, ProcMask::NONE);
        let blueprint_slot = config
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT)
            .expect("default profile should register core shadow blueprint slot");
        let counter_slot = config
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default profile should register core minion counter slot");
        let mut runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
            .expect("large_51 raw should construct runtime v2 runner");
        let owner = runner.runtime().entities.get(EntityIdx(0)).expect("runtime v2 shadow owner should exist");
        let shadow_lane = owner
            .template
            .skills
            .skills()
            .iter()
            .position(|skill| *skill == shadow)
            .expect("runtime v2 owner should import ShadowSkill");
        assert_eq!(owner.template.skills.level_at(shadow_lane), Some(expected_level));
        let SlotValue::PlayerTemplate(blueprint) = owner
            .slots
            .get(blueprint_slot)
            .expect("runtime v2 owner should store a per-owner shadow blueprint")
        else {
            panic!("runtime v2 shadow blueprint slot should hold PlayerTemplate");
        };
        assert_eq!(blueprint.name, legacy_shadow.id_name());
        assert_eq!(blueprint.display_name, legacy_shadow.display_name());
        assert_eq!(blueprint.max_hp, legacy_shadow_status.max_hp);
        assert_eq!(blueprint.attack, legacy_shadow_status.attack);
        assert_eq!(blueprint.magic_point, legacy_shadow_status.magic_point);
        assert_eq!(blueprint.move_state.speed_points, legacy_shadow.move_point());
        let possess_lane = blueprint
            .skills
            .skills()
            .iter()
            .position(|skill| *skill == possess)
            .expect("runtime v2 shadow blueprint should import PossessSkill");
        assert_eq!(blueprint.skills.level_at(possess_lane), Some(expected_possess_level));
        assert!(blueprint.skills.active_order().contains(&possess_lane));
        let blueprint_skills = blueprint.skills.clone();

        let initial_entity_count = runner.runtime().entities.len();
        let owner_name = owner.template.name.clone();
        let round = runner.run_round_normalized();

        assert_eq!(
            round.frames.iter().map(|frame| frame.message.as_str()).collect::<Vec<_>>(),
            vec!["[0]使用[幻术]", "召唤出[1]", "\n"]
        );
        let owner = runner.runtime().entities.get(EntityIdx(0)).unwrap();
        assert_eq!(
            owner.template.skills.level_at(shadow_lane),
            Some(expected_level.saturating_mul(3).div_ceil(4).max(1))
        );
        assert_eq!(owner.slots.get(counter_slot), Some(&SlotValue::U64(1)));
        let spawned_idx = EntityIdx(initial_entity_count.try_into().unwrap());
        let spawned = runner
            .runtime()
            .entities
            .get(spawned_idx)
            .expect("ShadowSkill should spawn one shadow entity");
        assert_eq!(spawned.template.name, format!("{owner_name}?0"));
        assert_eq!(spawned.template.display_name, "幻影");
        assert_eq!(spawned.runtime.owner, EntityIdx(0));
        assert_eq!(spawned.runtime.root_owner, EntityIdx(0));
        assert_eq!(spawned.runtime.magic_point, legacy_shadow_status.magic_point);
        assert_eq!(spawned.template.skills.skills(), blueprint_skills.skills());
        assert_eq!(spawned.template.skills.level_at(possess_lane), Some(expected_possess_level));
        assert!(
            runner
                .runtime()
                .scheduler
                .skill_hook_plan(
                    &runner.runtime().entities,
                    &runner.runtime().registry,
                    spawned_idx,
                    ProcMask::PRE_ACTION,
                )
                .entries
                .is_empty()
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
            round_order: vec![0, 1],
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
    #[ignore = "self-referential v2 prefix golden; use legacy/v2 parity report until full plain-player loadout converges"]
    fn runtime_v2_runner_large_prefix_normalized_run_matches_golden() {
        let raw_input =
            "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

        let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let run = runner.run_until_winner_normalized_rounds(4);

        assert_eq!(run.winner_team, None);
        assert!(run.guard_exhausted);
        assert_eq!(run.total_score, 188);
        assert_eq!(run.rounds.len(), 4);
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
            NormalizedOutcome {
                winner_team: None,
                round: 4,
                total_score: 37,
                rng: normalized_rng_checkpoint(229, 251),
                entity_ids: vec![1, 2],
                teams: vec![0, 1],
                hp: vec![276, 208],
                magic_point: vec![28, 29],
                defense: vec![58, 52],
                resistance: vec![49, 57],
                alive: vec![true, true],
                round_order: vec![0, 1],
                flat_alive: vec![0, 1],
                team_alive: vec![vec![0], vec![1]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 4,
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
        ];

        for (expected, actual) in expected_rounds.iter().zip(&run.rounds) {
            assert_eq!(strict_diff(expected, actual), Ok(()));
        }
    }

    #[test]
    #[ignore = "self-referential v2 terminal golden; use legacy/v2 parity report until full plain-player loadout converges"]
    fn runtime_v2_runner_large_full_normalized_run_matches_golden() {
        let raw_input =
            "虚空托腮 IVHEWTNEA@TigerStar\n\n进口牢货.不可磨灭的回忆之殇 8}i%Yh&<@幻景殇\nseed:2026-03-07 22:54 #013595@!";

        let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let run = runner.run_until_winner_normalized_rounds(32);

        assert_eq!(run.winner_team, Some(0));
        assert!(!run.guard_exhausted);
        assert_eq!(run.total_score, 527);
        assert_eq!(run.rounds.len(), 11);

        let expected_rounds = [
            plain_large_expected_round(1, None, 57, 226, 30, [350, 265], [true, true], [0, 1]),
            plain_large_expected_round(2, None, 37, 227, 87, [313, 265], [true, true], [1, 0]),
            plain_large_expected_round(3, None, 57, 228, 178, [313, 208], [true, true], [0, 1]),
            plain_large_expected_round(4, None, 37, 229, 251, [276, 208], [true, true], [1, 0]),
            plain_large_expected_round(5, None, 57, 230, 218, [276, 151], [true, true], [0, 1]),
            plain_large_expected_round(6, None, 37, 231, 61, [239, 151], [true, true], [1, 0]),
            plain_large_expected_round(7, None, 57, 232, 135, [239, 94], [true, true], [0, 1]),
            plain_large_expected_round(8, None, 37, 233, 250, [202, 94], [true, true], [1, 0]),
            plain_large_expected_round(9, None, 57, 234, 242, [202, 37], [true, true], [0, 1]),
            plain_large_expected_round(10, None, 37, 235, 149, [165, 37], [true, true], [1, 0]),
            plain_large_expected_round(11, Some(0), 57, 236, 3, [165, 0], [true, false], [0, 1]),
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
    #[ignore = "self-referential v2 prefix golden; use legacy/v2 parity report until full plain-player loadout converges"]
    fn runtime_v2_runner_fight_multi_prefix_normalized_run_matches_golden() {
        let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

        let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let run = runner.run_until_winner_normalized_rounds(4);

        assert_eq!(run.winner_team, None);
        assert!(run.guard_exhausted);
        assert_eq!(run.total_score, 187);
        assert_eq!(run.rounds.len(), 4);
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
            NormalizedOutcome {
                winner_team: None,
                round: 4,
                total_score: 58,
                rng: normalized_rng_checkpoint(218, 78),
                entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
                teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
                hp: vec![342, 331, 387, 150, 359, 331, 324, 327, 332, 335],
                magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
                defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
                resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
                alive: vec![true, true, true, true, true, true, true, true, true, true],
                round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
                flat_alive: vec![6, 9, 5, 7, 8, 3, 4, 1, 2, 0],
                team_alive: vec![vec![6, 9, 5, 7, 8], vec![3, 4, 1, 2, 0]],
                alive_group_count: 2,
                actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                    round: 4,
                    actor: 5,
                    target: 3,
                    amount: 58,
                }],
                frames: vec![NormalizedUpdateFrame {
                    message: "[0]攻击[1]".to_owned(),
                    caster: 5,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 58,
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
    fn runtime_v2_runner_fight_multi_reports_real_legacy_divergence() {
        let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

        let mut legacy_runner =
            crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy fight_multi should construct");
        let legacy = normalize_legacy_run(&mut legacy_runner, 256);
        let (mut v2_runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let v2 = v2_runner.run_until_winner_normalized_rounds(256);

        assert_eq!(legacy.rounds.len(), 84);
        assert_eq!(legacy.total_score, 6766);
        assert!(matches!(strict_diff_runs(&legacy, &v2), Err(StrictRunDiff::Round { .. })));
    }

    #[test]
    #[ignore = "self-referential v2 terminal golden; use legacy/v2 parity report until behavior converges"]
    fn runtime_v2_runner_fight_multi_full_terminal_normalized_run_matches_golden() {
        let raw_input = "测707640862046T，烦恼立刻消失@爱\n坚持 E6b10FVHvKDO@Afterglow\nInfluence #MEZC2wa@Unbound\n耀眼之星 /JxrJYwouGw/@新纪元\n随之任之 #iWZYBGuwxX@🥒\n\n真夜霞 #FBNWDPBPW@无惨\n虚空托腮 UMOXFIARH@TigerStar\nFengshen ONVWTGMPNCKV@nan\nBoundless_Ocean,Vast_Skies #l6RZxopUn@Shabby_fish\nSpearmaster ZbblyZQQwr@RainWorld_XIV\nseed:1376-2-15@!";

        let (mut runner, _) = mixed_raw_runner_for_plain_fixture(raw_input);
        let run = runner.run_until_winner_normalized_rounds(256);

        assert_eq!(run.winner_team, Some(1));
        assert!(!run.guard_exhausted);
        assert_eq!(run.total_score, 3211);
        assert_eq!(run.rounds.len(), 88);

        let expected_checkpoints = [
            (
                1,
                None,
                50,
                215,
                67,
                [342, 331, 387, 267, 359, 331, 344, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [6, 3, 50],
            ),
            (
                2,
                None,
                20,
                216,
                115,
                [342, 331, 387, 267, 359, 331, 324, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [3, 6, 20],
            ),
            (
                3,
                None,
                59,
                217,
                59,
                [342, 331, 387, 208, 359, 331, 324, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [9, 3, 59],
            ),
            (
                4,
                None,
                58,
                218,
                78,
                [342, 331, 387, 150, 359, 331, 324, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [5, 3, 58],
            ),
            (
                5,
                None,
                48,
                219,
                152,
                [342, 331, 387, 150, 359, 331, 276, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [4, 6, 48],
            ),
            (
                6,
                None,
                47,
                220,
                55,
                [342, 331, 387, 150, 359, 331, 229, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [1, 6, 47],
            ),
            (
                7,
                None,
                37,
                221,
                90,
                [342, 331, 387, 113, 359, 331, 229, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [7, 3, 37],
            ),
            (
                8,
                None,
                5,
                222,
                110,
                [342, 331, 387, 108, 359, 331, 229, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [8, 3, 5],
            ),
            (
                9,
                None,
                61,
                223,
                126,
                [342, 331, 387, 108, 359, 331, 168, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [2, 6, 61],
            ),
            (
                10,
                None,
                20,
                224,
                62,
                [342, 331, 387, 108, 359, 331, 148, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [0, 6, 20],
            ),
            (
                11,
                None,
                50,
                225,
                11,
                [342, 331, 387, 58, 359, 331, 148, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [6, 3, 50],
            ),
            (
                12,
                None,
                20,
                226,
                147,
                [342, 331, 387, 58, 359, 331, 128, 327, 332, 335],
                [true, true, true, true, true, true, true, true, true, true],
                [3, 6, 20],
            ),
            (
                13,
                None,
                59,
                227,
                2,
                [342, 331, 387, 0, 359, 331, 128, 327, 332, 335],
                [true, true, true, false, true, true, true, true, true, true],
                [9, 3, 59],
            ),
            (
                14,
                None,
                58,
                228,
                44,
                [342, 331, 387, 0, 301, 331, 128, 327, 332, 335],
                [true, true, true, false, true, true, true, true, true, true],
                [5, 4, 58],
            ),
            (
                15,
                None,
                48,
                229,
                76,
                [342, 331, 387, 0, 301, 331, 80, 327, 332, 335],
                [true, true, true, false, true, true, true, true, true, true],
                [4, 6, 48],
            ),
            (
                16,
                None,
                47,
                230,
                217,
                [342, 331, 387, 0, 301, 331, 33, 327, 332, 335],
                [true, true, true, false, true, true, true, true, true, true],
                [1, 6, 47],
            ),
            (
                17,
                None,
                37,
                231,
                11,
                [342, 331, 387, 0, 264, 331, 33, 327, 332, 335],
                [true, true, true, false, true, true, true, true, true, true],
                [7, 4, 37],
            ),
            (
                18,
                None,
                5,
                232,
                198,
                [342, 331, 387, 0, 259, 331, 33, 327, 332, 335],
                [true, true, true, false, true, true, true, true, true, true],
                [8, 4, 5],
            ),
            (
                19,
                None,
                61,
                233,
                235,
                [342, 331, 387, 0, 259, 331, 0, 327, 332, 335],
                [true, true, true, false, true, true, false, true, true, true],
                [2, 6, 61],
            ),
            (
                20,
                None,
                20,
                234,
                89,
                [342, 331, 387, 0, 259, 331, 0, 327, 332, 315],
                [true, true, true, false, true, true, false, true, true, true],
                [0, 9, 20],
            ),
            (
                21,
                None,
                59,
                235,
                126,
                [342, 331, 387, 0, 200, 331, 0, 327, 332, 315],
                [true, true, true, false, true, true, false, true, true, true],
                [9, 4, 59],
            ),
            (
                22,
                None,
                58,
                236,
                92,
                [342, 331, 387, 0, 142, 331, 0, 327, 332, 315],
                [true, true, true, false, true, true, false, true, true, true],
                [5, 4, 58],
            ),
            (
                23,
                None,
                48,
                237,
                176,
                [342, 331, 387, 0, 142, 331, 0, 327, 332, 267],
                [true, true, true, false, true, true, false, true, true, true],
                [4, 9, 48],
            ),
            (
                24,
                None,
                47,
                238,
                119,
                [342, 331, 387, 0, 142, 331, 0, 327, 332, 220],
                [true, true, true, false, true, true, false, true, true, true],
                [1, 9, 47],
            ),
            (
                25,
                None,
                37,
                239,
                54,
                [342, 331, 387, 0, 105, 331, 0, 327, 332, 220],
                [true, true, true, false, true, true, false, true, true, true],
                [7, 4, 37],
            ),
            (
                26,
                None,
                5,
                240,
                168,
                [342, 331, 387, 0, 100, 331, 0, 327, 332, 220],
                [true, true, true, false, true, true, false, true, true, true],
                [8, 4, 5],
            ),
            (
                27,
                None,
                61,
                241,
                169,
                [342, 331, 387, 0, 100, 331, 0, 327, 332, 159],
                [true, true, true, false, true, true, false, true, true, true],
                [2, 9, 61],
            ),
            (
                28,
                None,
                20,
                242,
                84,
                [342, 331, 387, 0, 100, 331, 0, 327, 332, 139],
                [true, true, true, false, true, true, false, true, true, true],
                [0, 9, 20],
            ),
            (
                29,
                None,
                59,
                243,
                173,
                [342, 331, 387, 0, 41, 331, 0, 327, 332, 139],
                [true, true, true, false, true, true, false, true, true, true],
                [9, 4, 59],
            ),
            (
                30,
                None,
                58,
                244,
                241,
                [342, 331, 387, 0, 0, 331, 0, 327, 332, 139],
                [true, true, true, false, false, true, false, true, true, true],
                [5, 4, 58],
            ),
            (
                31,
                None,
                47,
                245,
                186,
                [342, 331, 387, 0, 0, 331, 0, 327, 332, 92],
                [true, true, true, false, false, true, false, true, true, true],
                [1, 9, 47],
            ),
            (
                32,
                None,
                37,
                246,
                197,
                [342, 294, 387, 0, 0, 331, 0, 327, 332, 92],
                [true, true, true, false, false, true, false, true, true, true],
                [7, 1, 37],
            ),
            (
                33,
                None,
                5,
                247,
                40,
                [342, 289, 387, 0, 0, 331, 0, 327, 332, 92],
                [true, true, true, false, false, true, false, true, true, true],
                [8, 1, 5],
            ),
            (
                34,
                None,
                61,
                248,
                9,
                [342, 289, 387, 0, 0, 331, 0, 327, 332, 31],
                [true, true, true, false, false, true, false, true, true, true],
                [2, 9, 61],
            ),
            (
                35,
                None,
                20,
                249,
                136,
                [342, 289, 387, 0, 0, 331, 0, 327, 332, 11],
                [true, true, true, false, false, true, false, true, true, true],
                [0, 9, 20],
            ),
            (
                36,
                None,
                59,
                250,
                211,
                [342, 230, 387, 0, 0, 331, 0, 327, 332, 11],
                [true, true, true, false, false, true, false, true, true, true],
                [9, 1, 59],
            ),
            (
                37,
                None,
                58,
                251,
                207,
                [342, 172, 387, 0, 0, 331, 0, 327, 332, 11],
                [true, true, true, false, false, true, false, true, true, true],
                [5, 1, 58],
            ),
            (
                38,
                None,
                47,
                252,
                245,
                [342, 172, 387, 0, 0, 331, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [1, 9, 47],
            ),
            (
                39,
                None,
                37,
                253,
                82,
                [342, 135, 387, 0, 0, 331, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [7, 1, 37],
            ),
            (
                40,
                None,
                5,
                254,
                138,
                [342, 130, 387, 0, 0, 331, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [8, 1, 5],
            ),
            (
                41,
                None,
                61,
                255,
                140,
                [342, 130, 387, 0, 0, 270, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [2, 5, 61],
            ),
            (
                42,
                None,
                20,
                0,
                50,
                [342, 130, 387, 0, 0, 250, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [0, 5, 20],
            ),
            (
                43,
                None,
                58,
                1,
                43,
                [342, 72, 387, 0, 0, 250, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [5, 1, 58],
            ),
            (
                44,
                None,
                47,
                2,
                154,
                [342, 72, 387, 0, 0, 203, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [1, 5, 47],
            ),
            (
                45,
                None,
                37,
                3,
                188,
                [342, 35, 387, 0, 0, 203, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [7, 1, 37],
            ),
            (
                46,
                None,
                5,
                4,
                57,
                [342, 30, 387, 0, 0, 203, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [8, 1, 5],
            ),
            (
                47,
                None,
                61,
                5,
                240,
                [342, 30, 387, 0, 0, 142, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [2, 5, 61],
            ),
            (
                48,
                None,
                20,
                6,
                222,
                [342, 30, 387, 0, 0, 122, 0, 327, 332, 0],
                [true, true, true, false, false, true, false, true, true, false],
                [0, 5, 20],
            ),
            (
                49,
                None,
                58,
                7,
                58,
                [342, 0, 387, 0, 0, 122, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [5, 1, 58],
            ),
            (
                50,
                None,
                37,
                8,
                46,
                [342, 0, 350, 0, 0, 122, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [7, 2, 37],
            ),
            (
                51,
                None,
                5,
                9,
                15,
                [342, 0, 345, 0, 0, 122, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [8, 2, 5],
            ),
            (
                52,
                None,
                61,
                10,
                42,
                [342, 0, 345, 0, 0, 61, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [2, 5, 61],
            ),
            (
                53,
                None,
                20,
                11,
                92,
                [342, 0, 345, 0, 0, 41, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [0, 5, 20],
            ),
            (
                54,
                None,
                58,
                12,
                105,
                [342, 0, 287, 0, 0, 41, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [5, 2, 58],
            ),
            (
                55,
                None,
                37,
                13,
                205,
                [342, 0, 250, 0, 0, 41, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [7, 2, 37],
            ),
            (
                56,
                None,
                5,
                14,
                86,
                [342, 0, 245, 0, 0, 41, 0, 327, 332, 0],
                [true, false, true, false, false, true, false, true, true, false],
                [8, 2, 5],
            ),
            (
                57,
                None,
                61,
                15,
                55,
                [342, 0, 245, 0, 0, 0, 0, 327, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [2, 5, 61],
            ),
            (
                58,
                None,
                20,
                16,
                186,
                [342, 0, 245, 0, 0, 0, 0, 307, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [0, 7, 20],
            ),
            (
                59,
                None,
                37,
                17,
                58,
                [342, 0, 208, 0, 0, 0, 0, 307, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [7, 2, 37],
            ),
            (
                60,
                None,
                5,
                18,
                239,
                [342, 0, 203, 0, 0, 0, 0, 307, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [8, 2, 5],
            ),
            (
                61,
                None,
                61,
                19,
                84,
                [342, 0, 203, 0, 0, 0, 0, 246, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [2, 7, 61],
            ),
            (
                62,
                None,
                20,
                20,
                196,
                [342, 0, 203, 0, 0, 0, 0, 226, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [0, 7, 20],
            ),
            (
                63,
                None,
                37,
                21,
                219,
                [342, 0, 166, 0, 0, 0, 0, 226, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [7, 2, 37],
            ),
            (
                64,
                None,
                5,
                22,
                22,
                [342, 0, 161, 0, 0, 0, 0, 226, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [8, 2, 5],
            ),
            (
                65,
                None,
                61,
                23,
                250,
                [342, 0, 161, 0, 0, 0, 0, 165, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [2, 7, 61],
            ),
            (
                66,
                None,
                20,
                24,
                57,
                [342, 0, 161, 0, 0, 0, 0, 145, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [0, 7, 20],
            ),
            (
                67,
                None,
                37,
                25,
                36,
                [342, 0, 124, 0, 0, 0, 0, 145, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [7, 2, 37],
            ),
            (
                68,
                None,
                5,
                26,
                112,
                [342, 0, 119, 0, 0, 0, 0, 145, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [8, 2, 5],
            ),
            (
                69,
                None,
                61,
                27,
                215,
                [342, 0, 119, 0, 0, 0, 0, 84, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [2, 7, 61],
            ),
            (
                70,
                None,
                20,
                28,
                239,
                [342, 0, 119, 0, 0, 0, 0, 64, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [0, 7, 20],
            ),
            (
                71,
                None,
                37,
                29,
                214,
                [342, 0, 82, 0, 0, 0, 0, 64, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [7, 2, 37],
            ),
            (
                72,
                None,
                5,
                30,
                1,
                [342, 0, 77, 0, 0, 0, 0, 64, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [8, 2, 5],
            ),
            (
                73,
                None,
                61,
                31,
                56,
                [342, 0, 77, 0, 0, 0, 0, 3, 332, 0],
                [true, false, true, false, false, false, false, true, true, false],
                [2, 7, 61],
            ),
            (
                74,
                None,
                20,
                32,
                161,
                [342, 0, 77, 0, 0, 0, 0, 0, 332, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [0, 7, 20],
            ),
            (
                75,
                None,
                5,
                33,
                208,
                [342, 0, 72, 0, 0, 0, 0, 0, 332, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [8, 2, 5],
            ),
            (
                76,
                None,
                61,
                34,
                164,
                [342, 0, 72, 0, 0, 0, 0, 0, 271, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [2, 8, 61],
            ),
            (
                77,
                None,
                20,
                35,
                128,
                [342, 0, 72, 0, 0, 0, 0, 0, 251, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [0, 8, 20],
            ),
            (
                78,
                None,
                5,
                36,
                107,
                [342, 0, 67, 0, 0, 0, 0, 0, 251, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [8, 2, 5],
            ),
            (
                79,
                None,
                61,
                37,
                115,
                [342, 0, 67, 0, 0, 0, 0, 0, 190, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [2, 8, 61],
            ),
            (
                80,
                None,
                20,
                38,
                6,
                [342, 0, 67, 0, 0, 0, 0, 0, 170, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [0, 8, 20],
            ),
            (
                81,
                None,
                5,
                39,
                57,
                [342, 0, 62, 0, 0, 0, 0, 0, 170, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [8, 2, 5],
            ),
            (
                82,
                None,
                61,
                40,
                156,
                [342, 0, 62, 0, 0, 0, 0, 0, 109, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [2, 8, 61],
            ),
            (
                83,
                None,
                20,
                41,
                137,
                [342, 0, 62, 0, 0, 0, 0, 0, 89, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [0, 8, 20],
            ),
            (
                84,
                None,
                5,
                42,
                164,
                [342, 0, 57, 0, 0, 0, 0, 0, 89, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [8, 2, 5],
            ),
            (
                85,
                None,
                61,
                43,
                157,
                [342, 0, 57, 0, 0, 0, 0, 0, 28, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [2, 8, 61],
            ),
            (
                86,
                None,
                20,
                44,
                199,
                [342, 0, 57, 0, 0, 0, 0, 0, 8, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [0, 8, 20],
            ),
            (
                87,
                None,
                5,
                45,
                5,
                [342, 0, 52, 0, 0, 0, 0, 0, 8, 0],
                [true, false, true, false, false, false, false, false, true, false],
                [8, 2, 5],
            ),
            (
                88,
                Some(1),
                61,
                46,
                249,
                [342, 0, 52, 0, 0, 0, 0, 0, 0, 0],
                [true, false, true, false, false, false, false, false, false, false],
                [2, 8, 61],
            ),
        ];
        assert_eq!(run.rounds.len(), expected_checkpoints.len());
        for (
            round,
            (expected_round, expected_winner, expected_score, rng_i, rng_j, expected_hp, expected_alive, expected_action),
        ) in run.rounds.iter().zip(expected_checkpoints)
        {
            assert_eq!(round.round, expected_round);
            assert_eq!(round.winner_team, expected_winner);
            assert_eq!(round.total_score, expected_score);
            assert_eq!(round.rng, normalized_rng_checkpoint(rng_i, rng_j));
            assert_eq!(round.hp, expected_hp);
            assert_eq!(round.alive, expected_alive);
            assert_eq!(round.actions.len(), 1);
            assert_eq!(round.frames.len(), 1);
            let action = round.actions.first().unwrap();
            let frame = round.frames.first().unwrap();
            assert_eq!([action.actor, action.target, action.amount as usize], expected_action);
            assert_eq!([frame.caster, frame.target, frame.score as usize], expected_action);
        }

        let expected_terminal = NormalizedOutcome {
            winner_team: Some(1),
            round: 88,
            total_score: 61,
            rng: normalized_rng_checkpoint(46, 249),
            entity_ids: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            teams: vec![1, 1, 1, 1, 1, 0, 0, 0, 0, 0],
            hp: vec![342, 0, 52, 0, 0, 0, 0, 0, 0, 0],
            magic_point: vec![30, 28, 23, 30, 26, 24, 26, 25, 23, 25],
            defense: vec![51, 37, 51, 39, 53, 55, 52, 53, 45, 54],
            resistance: vec![44, 52, 60, 56, 56, 59, 36, 59, 58, 56],
            alive: vec![true, false, true, false, false, false, false, false, false, false],
            round_order: vec![6, 3, 9, 5, 4, 1, 7, 8, 2, 0],
            flat_alive: vec![2, 0],
            team_alive: vec![vec![], vec![2, 0]],
            alive_group_count: 1,
            actions: vec![crate::runtime_v2::oracle::NormalizedActionBoundary {
                round: 88,
                actor: 2,
                target: 8,
                amount: 61,
            }],
            frames: vec![NormalizedUpdateFrame {
                message: "[0]攻击[1]".to_owned(),
                caster: 2,
                target: 8,
                targets: Vec::new(),
                param: None,
                score: 61,
                delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                update_type: crate::engine::update::UpdateType::None,
            }],
        };

        assert_eq!(strict_diff(&expected_terminal, run.rounds.last().unwrap()), Ok(()));
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
        runner.runtime_mut().set_skill_handler(summon, skill_noop);
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
            round_order: vec![0, 1],
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
                round_order: vec![0],
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
    fn finish_round_advances_round_without_action_or_frame() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

        let outcome = runtime.finish_round(None, RunUpdates::new());

        assert!(outcome.action.is_none());
        assert!(outcome.frame.is_none());
        assert_eq!(outcome.winner_team, None);
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
    fn flush_effects_passes_killed_target_to_kill_skill_hooks() {
        let mut builder = ExtensionRegistryBuilder::default();
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
                PlayerTemplate::new(2, "first-target", 1, 10, 3),
                PlayerTemplate::new(3, "killed-target", 1, 3, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(kill_skill, skill_marks_selected_target);
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(2),
            amount: 3,
        });

        let frame = runtime.flush_effects().expect("lethal damage should emit kill hook");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[1].message, "selected target");
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[1].score, 2);
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
    fn possess_skill_berserks_target_and_removes_shadow_caster() {
        let mut builder = ExtensionRegistryBuilder::default();
        let possess_skill = builder
            .register_skill_with_hooks(
                "custom",
                "possess",
                "custom.minion.possess",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
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
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3),
                PlayerTemplate::with_kind(2, "owner?0", shadow_kind, 0, 5, 1).with_skills([possess_skill]),
                PlayerTemplate::new(3, "target", 1, 20, 1),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.owner = EntityIdx(0);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.root_owner = EntityIdx(0);
        runtime.set_skill_handler(possess_skill, run_possess_skill);

        let plan = runtime
            .scheduler
            .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(1), ProcMask::PRE_ACTION);
        let mut updates = RunUpdates::new();
        runtime.drain_skill_hook_plan_with_selected_target_into(&plan, &mut updates, Some(EntityIdx(2)));
        let frame = RuntimeFrame { updates };

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]使用[附体]");
        assert_eq!(frame.updates.updates[0].caster, 1);
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[1].message, "[1]进入[狂暴]状态");
        assert_eq!(frame.updates.updates[1].caster, 1);
        assert_eq!(frame.updates.updates[1].target, 2);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].caster, 1);
        assert_eq!(frame.updates.updates[2].target, 1);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(2))
                .unwrap()
                .states
                .entry(10)
                .map(|entry| entry.payload.clone()),
            Some(StatePayload::Berserk { step: 4 })
        );
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 0);
        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2)]);
    }

    #[test]
    fn possess_skill_extends_existing_berserk_state() {
        let mut builder = ExtensionRegistryBuilder::default();
        let possess_skill = builder
            .register_skill_with_hooks(
                "custom",
                "possess",
                "custom.minion.possess",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("possess skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "shadow", 0, 5, 1).with_skills([possess_skill]),
                PlayerTemplate::new(2, "target", 1, 20, 1),
            ],
            registry,
        ));
        runtime
            .entities
            .get_mut(EntityIdx(1))
            .unwrap()
            .states
            .add_entry(StateEntry::berserk(10, 2));
        runtime.set_skill_handler(possess_skill, run_possess_skill);

        let plan = runtime
            .scheduler
            .skill_hook_plan(&runtime.entities, &runtime.registry, EntityIdx(0), ProcMask::PRE_ACTION);
        let mut updates = RunUpdates::new();
        runtime.drain_skill_hook_plan_with_selected_target_into(&plan, &mut updates, Some(EntityIdx(1)));

        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(10)
                .map(|entry| entry.payload.clone()),
            Some(StatePayload::Berserk { step: 6 })
        );
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
    fn zombie_style_minion_handler_uses_lethal_damage_killed_target() {
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
        let payload = PlayerTemplate::with_kind(4, "owner?zombie", zombie_kind, 0, 4, 1);
        let mut template = PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "owner", 0, 20, 3).with_skills([zombie_skill]),
                PlayerTemplate::new(2, "first-target", 1, 20, 1),
                PlayerTemplate::new(3, "killed-target", 1, 3, 1),
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
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(2),
            amount: 3,
        });

        let frame = runtime.flush_effects().expect("lethal damage should drive zombie-style minion spawn");

        assert_eq!(frame.updates.updates.len(), 4);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].target, 2);
        assert_eq!(frame.updates.updates[1].message, "\n");
        assert_eq!(frame.updates.updates[2].message, "[0][召唤亡灵]");
        assert_eq!(frame.updates.updates[2].target, 2);
        assert_eq!(frame.updates.updates[3].message, "[2]变成了[1]");
        assert_eq!(frame.updates.updates[3].target, 3);
        assert_eq!(frame.updates.updates[3].targets.as_slice(), &[2]);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(counter_slot),
            Some(&SlotValue::U64(1))
        );
        let zombie = runtime.entities.get(EntityIdx(3)).expect("zombie minion should spawn");
        assert_eq!(zombie.template.name, "owner?0");
        assert_eq!(zombie.template.kind, zombie_kind);
        assert_eq!(zombie.runtime.owner, EntityIdx(0));
        assert_eq!(zombie.runtime.root_owner, EntityIdx(0));
        assert!(!runtime.world.flat_alive().contains(&EntityIdx(2)));
        assert!(runtime.world.flat_alive().contains(&EntityIdx(3)));
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
        assert_eq!(runtime.world.round_order(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(0), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(1)]);
        assert_eq!(runtime.world.alive_group_count(), 1);
        assert_eq!(frame.updates.updates.len(), 5);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(
            frame.updates.updates[1].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[2].target, 2);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].score, 50);
        assert_eq!(
            frame.updates.updates[3].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[4].target, 3);
        assert_eq!(frame.updates.updates[4].message, "[1]消失了");
        assert_eq!(frame.updates.updates[4].score, 50);
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
        assert_eq!(frame.updates.updates.len(), 5);
        assert_eq!(frame.updates.updates[0].target, 0);
        assert_eq!(frame.updates.updates[0].message, "[1]消失了");
        assert_eq!(
            frame.updates.updates[1].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[2].target, 2);
        assert_eq!(frame.updates.updates[2].message, "[1]消失了");
        assert_eq!(frame.updates.updates[2].score, 50);
        assert_eq!(
            frame.updates.updates[3].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[4].target, 3);
        assert_eq!(frame.updates.updates[4].message, "[1]消失了");
        assert_eq!(frame.updates.updates[4].score, 50);
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
            total_score: 110,
            rng: crate::runtime_v2::oracle::NormalizedRngCheckpoint::default(),
            entity_ids: vec![1, 2, 3, 4],
            teams: vec![0, 1, 0, 0],
            hp: vec![0, 10, 0, 0],
            magic_point: vec![0, 0, 0, 0],
            defense: vec![0, 0, 0, 0],
            resistance: vec![0, 0, 0, 0],
            alive: vec![false, true, false, false],
            round_order: vec![1],
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
                    message: "\n".to_owned(),
                    caster: 0,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: 0,
                    delay1: 0,
                    update_type: crate::engine::update::UpdateType::NextLine,
                },
                NormalizedUpdateFrame {
                    message: "[1]消失了".to_owned(),
                    caster: 0,
                    target: 2,
                    targets: Vec::new(),
                    param: None,
                    score: 50,
                    delay0: crate::engine::update::DEFAULT_DELAY0_MS,
                    delay1: crate::engine::update::DEFAULT_DELAY1_MS,
                    update_type: crate::engine::update::UpdateType::None,
                },
                NormalizedUpdateFrame {
                    message: "\n".to_owned(),
                    caster: 0,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: 0,
                    delay1: 0,
                    update_type: crate::engine::update::UpdateType::NextLine,
                },
                NormalizedUpdateFrame {
                    message: "[1]消失了".to_owned(),
                    caster: 0,
                    target: 3,
                    targets: Vec::new(),
                    param: None,
                    score: 50,
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
                PlayerTemplate::with_kind(1, "merge-owner", merge_kind, 0, 10, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)])),
                PlayerTemplate::new(2, "merge-target", 1, 10, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
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
                    message: "\n".to_owned(),
                    caster: 0,
                    target: 0,
                    targets: Vec::new(),
                    param: None,
                    score: 0,
                    delay0: 0,
                    delay1: 0,
                    update_type: crate::engine::update::UpdateType::NextLine,
                },
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

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[2]);
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

    fn skill_marks_selected_target(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        let target = context.selected_target().expect("skill should receive selected target");
        context.add_update(crate::engine::update::RunUpdate::new(
            "selected target",
            entry.owner.0 as usize,
            target.0 as usize,
            target.0,
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
    fn plain_revive_without_valid_target_continues_to_clone() {
        let mut builder = ExtensionRegistryBuilder::default();
        let revive = builder
            .register_skill(
                "core",
                "revive",
                BuiltinActiveSkill::Revive.export_name(),
                TargetPolicy::Ally,
                SkillPriority(16),
            )
            .expect("revive skill should register");
        let clone = builder
            .register_skill(
                "core",
                "clone",
                BuiltinActiveSkill::Clone.export_name(),
                TargetPolicy::None,
                SkillPriority(23),
            )
            .expect("clone skill should register");
        let registry = builder.build();
        let loadout = SkillLoadout::from_skill_levels([(revive, 128), (clone, 128)]).with_active_order(vec![0, 1]);
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3).with_skill_loadout(loadout),
                PlayerTemplate::new(2, "ally", 0, 100, 3),
                PlayerTemplate::new(3, "enemy", 1, 100, 3),
            ],
            registry,
        ));

        let prepared = runtime
            .scan_plain_action_skill_probabilities(EntityIdx(0), true)
            .expect("clone should be selected after revive finds no target");

        assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Clone);
        assert_eq!(prepared.selected.fixed_lane, 1);
        assert_eq!(prepared.targets, vec![EntityIdx(0)]);
    }

    #[test]
    fn plain_charge_selects_self_executes_and_ticks_in_late_post_action() {
        let mut builder = ExtensionRegistryBuilder::default();
        let charge = builder
            .register_skill_with_hooks_and_post_action_phase(
                "core",
                "charge",
                BuiltinActiveSkill::Charge.export_name(),
                ProcMask::POST_ACTION,
                TargetPolicy::None,
                SkillPriority(19),
                SkillPostActionPhase::Late,
            )
            .expect("charge skill should register");
        let registry = builder.build();
        let loadout = SkillLoadout::from_skill_levels([(charge, 128)]);
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3).with_skill_loadout(loadout),
                PlayerTemplate::new(2, "enemy", 1, 100, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(charge, run_charge_post_action_skill);

        let prepared = runtime
            .scan_plain_action_skill_probabilities(EntityIdx(0), false)
            .expect("charge should be selected when probability passes");
        assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Charge);
        assert_eq!(prepared.targets, vec![EntityIdx(0)]);

        let mut updates = RunUpdates::new();
        runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();
        assert_eq!(updates.updates[0].message, "[0]开始[蓄力]");
        assert_eq!(owner.runtime.magic_point, 32);
        assert!(owner.runtime.charge.active);
        assert_eq!(owner.runtime.charge.step, 2);
        assert_eq!(owner.runtime.at_boost_millionths, 3_000_000);

        let plan = runtime.scheduler.skill_post_action_hook_plan(
            &runtime.entities,
            &runtime.registry,
            EntityIdx(0),
            SkillPostActionPhase::Late,
        );
        runtime.drain_skill_hook_plan_into(&plan, &mut updates);
        let owner = runtime.entities.get(EntityIdx(0)).unwrap();
        assert!(owner.runtime.charge.active);
        assert_eq!(owner.runtime.charge.step, 1);
    }

    #[test]
    fn plain_reraise_revives_halves_level_and_stops_kill_hooks() {
        let mut builder = ExtensionRegistryBuilder::default();
        let kill = builder
            .register_skill_with_hooks(
                "custom",
                "kill-marker",
                "custom.kill_marker",
                ProcMask::KILL,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("kill marker should register");
        let reraise = builder
            .register_skill_with_hooks(
                "core",
                "reraise",
                DEFAULT_CORE_RERAISE_SKILL_EXPORT,
                ProcMask::DIE,
                TargetPolicy::None,
                SkillPriority(10),
            )
            .expect("reraise skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "killer", 0, 100, 3).with_skills([kill]),
                PlayerTemplate::new(2, "target", 1, 100, 3).with_skill_loadout(SkillLoadout::from_skill_levels([(reraise, 128)])),
            ],
            registry,
        ));
        runtime.set_skill_handler(kill, skill_marks_selected_target);
        runtime.set_skill_handler(reraise, run_reraise_die_skill);
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 0;

        let mut updates = RunUpdates::new();
        runtime.drain_plain_lethal_damage_into(EntityIdx(0), EntityIdx(1), &mut updates);

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert!(target.runtime.alive);
        assert!((1..=16).contains(&target.runtime.hp));
        assert_eq!(target.template.skills.level_at(0), Some(64));
        assert!(runtime.world.flat_alive().contains(&EntityIdx(1)));
        assert_eq!(
            updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| update.message.as_ref())
                .collect::<Vec<_>>(),
            vec!["[1]被击倒了", "[0]使用[护身符]抵挡了一次死亡", "[1]回复体力[2]点"]
        );
        assert!(!updates.updates.iter().any(|update| update.message == "selected target"));
    }

    #[test]
    fn plain_revive_selects_dead_non_minion_ally_by_attr_sum() {
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
                PlayerTemplate::new(1, "caster", 0, 100, 3),
                PlayerTemplate::new(2, "weak-dead", 0, 100, 3).with_target_score_stats(10, 0, 1.0),
                PlayerTemplate::new(3, "strong-dead", 0, 100, 3).with_target_score_stats(100, 0, 1.0),
                PlayerTemplate::with_kind(4, "minion-dead", minion_kind, 0, 100, 3).with_target_score_stats(1_000, 0, 1.0),
                PlayerTemplate::new(5, "enemy", 1, 100, 3),
            ],
            registry,
        ));
        for target in [EntityIdx(1), EntityIdx(2), EntityIdx(3)] {
            let entity = runtime.entities.get_mut(target).unwrap();
            entity.runtime.hp = 0;
            entity.runtime.alive = false;
            assert!(runtime.world.mark_dead(target, 0));
        }

        let selected = runtime.select_plain_revive_targets(EntityIdx(0), true);

        assert_eq!(selected.first(), Some(&EntityIdx(2)));
        assert!(selected.contains(&EntityIdx(1)));
        assert!(!selected.contains(&EntityIdx(3)));
    }

    #[test]
    fn plain_revive_restores_world_emits_legacy_updates_and_halves_level() {
        let mut builder = ExtensionRegistryBuilder::default();
        let revive = builder
            .register_skill(
                "core",
                "revive",
                BuiltinActiveSkill::Revive.export_name(),
                TargetPolicy::Ally,
                SkillPriority(16),
            )
            .expect("revive skill should register");
        let registry = builder.build();
        let loadout = SkillLoadout::from_skill_levels([(revive, 19)]);
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(150).with_skill_loadout(loadout),
                PlayerTemplate::new(2, "target", 0, 200, 3),
                PlayerTemplate::new(3, "enemy", 1, 100, 3),
            ],
            registry,
        ));
        {
            let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
            target.runtime.hp = 0;
            target.runtime.alive = false;
        }
        assert!(runtime.world.mark_dead(EntityIdx(1), 0));
        let mut expected_rng = runtime.rng.clone();
        let expected_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        let expected_heal = ((expected_atp / 75.0).ceil() as i32).clamp(1, 200);
        let mut updates = RunUpdates::new();

        runtime.drain_plain_revive_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert!(target.runtime.alive);
        assert_eq!(target.runtime.hp, expected_heal);
        assert!(runtime.world.round_order().contains(&EntityIdx(1)));
        assert_eq!(runtime.world.team_alive(0), Some(&[EntityIdx(0), EntityIdx(1)][..]));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().template.skills.level_at(0),
            Some(10)
        );
        assert_eq!(
            updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score, update.param))
                .collect::<Vec<_>>(),
            vec![
                ("[0]使用[苏生术]", 1, None),
                ("[1][复活]了", (expected_heal + 60) as u32, None),
                ("[1]回复体力[2]点", 0, Some(expected_heal as u32)),
            ]
        );
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn plain_revive_random_score_consumes_legacy_rffff() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "target", 0, 100, 3),
        ]));
        let mut expected_rng = runtime.rng.clone();
        let expected = expected_rng.rFFFF() as f64;

        let actual = runtime.score_plain_revive_target(EntityIdx(1), false);

        assert_eq!(actual, expected);
        assert_eq!(runtime.rng.i, expected_rng.i);
        assert_eq!(runtime.rng.j, expected_rng.j);
        assert_eq!(runtime.rng.main_val, expected_rng.main_val);
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
        assert_eq!(runtime.world.alive_group_count(), 1);
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
    fn owner_spawn_ignores_blueprint_team_and_does_not_create_enemy() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            ExtensionRegistry::default(),
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(2, "enemy", 1, 8, 4),
        });
        runtime.flush_effects().expect("spawn should emit update");

        let spawned = runtime.entities.get(EntityIdx(1)).expect("spawned entity should exist");
        assert_eq!(spawned.template.team, 0);
        assert_eq!(spawned.runtime.team, 0);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.sync_winner(&runtime.entities), Some(0));
        assert_eq!(
            runtime.scheduler.select_minimal_action(&mut runtime.world, &runtime.entities),
            None
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
            state: state.clone(),
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
                PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)])),
                PlayerTemplate::new(2, "right", 1, 10, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("merge should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[2]);
        assert_eq!(
            frame.updates.updates[0].update_type,
            crate::engine::update::UpdateType::NextLine
        );
        assert_eq!(frame.updates.updates[1].message, "[0][吞噬]了[1]");
        assert_eq!(frame.updates.updates[1].score, 60);
        assert_eq!(frame.updates.updates[2].message, "[0]属性上升");
        assert_eq!(frame.updates.updates[2].score, 0);
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
                PlayerTemplate::with_kind(1, "left", merge_kind, 0, 10, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_a, 1)]).with_fixed_lane_keys([1])),
                PlayerTemplate::new(2, "right", 1, 10, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(skill_b, 2), (skill_c, 3)])),
            ],
            registry,
        ));
        runtime.effects.push(QueuedEffect::Merge {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        runtime.flush_effects().expect("merge should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill_a]);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.levels(), &[3]);
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

    #[test]
    fn plain_absorb_smart_low_missing_hp_skips_probability_rng() {
        let registry = ExtensionRegistryBuilder::default().build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "actor", 0, 100, 3),
                PlayerTemplate::new(2, "target", 1, 100, 3),
            ],
            registry,
        ));
        let expected_rng = runtime.rng.clone();

        assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Absorb, 128, true));
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }

    #[test]
    fn plain_accumulate_gates_skip_probability_rng() {
        let registry = ExtensionRegistryBuilder::default().build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "actor", 0, 200, 3),
                PlayerTemplate::new(2, "target", 1, 100, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 119;
        let expected_rng = runtime.rng.clone();

        assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Accumulate, 128, true));
        assert_rng_state_eq(&runtime.rng, &expected_rng);

        {
            let actor = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            actor.runtime.hp = actor.template.max_hp;
            assert!(actor.activate_accumulate_runtime());
        }
        assert!(!runtime.plain_action_skill_probability(EntityIdx(0), BuiltinActiveSkill::Accumulate, 128, false));
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }

    #[test]
    fn plain_curse_empty_smart_targets_still_consume_sampling_rng() {
        let registry = ExtensionRegistryBuilder::default().build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "actor", 0, 100, 3),
                PlayerTemplate::new(2, "low-hp-target", 1, 100, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 79;
        let all_alive = runtime.world.flat_alive().to_vec();
        let mut expected_rng = runtime.rng.clone();
        for _ in 0..7 {
            assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0]), Some(1));
        }

        assert!(runtime.select_plain_curse_targets(EntityIdx(0), true).is_empty());
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }

    #[test]
    fn reflect_failed_level_roll_only_consumes_r255() {
        let mut builder = ExtensionRegistryBuilder::default();
        let reflect = builder
            .register_skill_with_hooks(
                "core",
                "reflect",
                DEFAULT_CORE_REFLECT_SKILL_EXPORT,
                ProcMask::PRE_DEFEND,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .expect("reflect skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3),
                PlayerTemplate::new(2, "reflector", 1, 100, 3)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(reflect, 1)])),
            ],
            registry,
        ));
        runtime.set_skill_handler(reflect, run_reflect_pre_defend_skill);
        let mut expected_rng = runtime.rng.clone();
        expected_rng.r255();
        let mut updates = RunUpdates::new();
        let mut defend_value = RuntimeDefendValue::Atp {
            value: 50.0,
            caster: EntityIdx(0),
            target: EntityIdx(1),
        };

        runtime.drain_pre_defend_hooks_into(EntityIdx(1), &mut updates, &mut defend_value);

        assert_eq!(defend_value.atp(), Some(50.0));
        assert!(updates.updates.is_empty());
        assert!(runtime.effects.is_empty());
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }

    #[test]
    fn reflected_attack_applies_damage_before_move_penalty_finishes() {
        let registry = ExtensionRegistryBuilder::default().build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "reflector", 0, 100, 3)
                    .with_magic(10_000)
                    .with_wisdom(10_000)
                    .with_speed_points(1_000),
                PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16),
            ],
            registry,
        ));
        while {
            let mut probe = runtime.rng.clone();
            probe.next_u8() <= 7
        } {
            runtime.rng.next_u8();
        }
        runtime.effects.push(QueuedEffect::ReflectedAttack {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            atp_bits: 50.0_f64.to_bits(),
        });

        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
            1_000
        );
        let frame = runtime.flush_effects().expect("reflected attack should emit damage");

        assert!(
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp < 1_000,
            "reflected damage must resolve before the queued effect completes"
        );
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 520);
        assert!(
            frame
                .updates
                .updates
                .iter()
                .any(|update| update.caster == 0 && update.target == 1 && update.score > 0)
        );
    }

    #[test]
    fn plain_curse_skill_applies_state_after_damage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let curse_state = builder
            .register_state(
                "core",
                "curse",
                DEFAULT_CORE_CURSE_STATE_EXPORT,
                ProcMask::POST_DEFEND,
                SkillPriority(10_000),
            )
            .expect("curse state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3).with_magic(80).with_wisdom(64),
                PlayerTemplate::new(2, "target", 1, 1_000, 3)
                    .with_def_res(0, 16)
                    .with_target_score_stats(0, 7, 1.0),
            ],
            registry,
        ));
        runtime.set_state_handler(curse_state, run_curse_post_defend_state);
        while {
            let mut probe = runtime.rng.clone();
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut probe);
            PlayerRuntime::dodge(
                runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
                runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
                &mut probe,
            )
        } {
            runtime.rng.next_u8();
        }
        let mut updates = RunUpdates::new();

        runtime.drain_plain_curse_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert!((1..1_000).contains(&target.runtime.hp));
        assert_eq!(target.runtime.atk_sum, 28);
        assert_eq!(
            target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
            Some(StatePayload::Curse { prob: 42, multiply: 2 })
        );
        assert_eq!(updates.updates.first().unwrap().message, "[0]使用[诅咒]");
        assert!(updates.updates[1].message.starts_with("[1]受到[2]点伤害"));
        assert_eq!(updates.updates.last().unwrap().message, "[1]被[诅咒]了");
    }

    #[test]
    fn plain_curse_on_damage_stacks_charge_bonus_without_reapplying_atk_sum() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_state(
                "core",
                "curse",
                DEFAULT_CORE_CURSE_STATE_EXPORT,
                ProcMask::POST_DEFEND,
                SkillPriority(10_000),
            )
            .expect("curse state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3),
                PlayerTemplate::new(2, "charged-target", 1, 100, 3)
                    .with_at_boost_millionths(3_000_000)
                    .with_target_score_stats(0, 7, 1.0),
            ],
            registry,
        ));
        let mut updates = RunUpdates::new();

        runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);
        runtime.apply_curse_on_damage(EntityIdx(0), EntityIdx(1), 1, &mut updates);

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.atk_sum, 28);
        assert_eq!(
            target.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
            Some(StatePayload::Curse { prob: 72, multiply: 5 })
        );
        assert_eq!(
            updates
                .updates
                .iter()
                .map(|update| (update.message.as_ref(), update.score))
                .collect::<Vec<_>>(),
            vec![("[1]被[诅咒]了", 60), ("[1]被[诅咒]了", 60)]
        );
    }

    #[test]
    fn plain_heal_clears_negative_states_restores_derived_stats_and_decays_level() {
        let mut builder = ExtensionRegistryBuilder::default();
        let heal = builder
            .register_skill(
                "core",
                "heal",
                BuiltinActiveSkill::Heal.export_name(),
                TargetPolicy::Ally,
                SkillPriority(15),
            )
            .expect("heal skill should register");
        let curse = builder
            .register_state(
                "core",
                "curse",
                DEFAULT_CORE_CURSE_STATE_EXPORT,
                ProcMask::POST_DEFEND,
                SkillPriority(10_000),
            )
            .expect("curse state should register");
        let poison = builder
            .register_state("core", "poison", "core.state.poison", ProcMask::POST_ACTION, SkillPriority(150))
            .expect("poison state should register");
        let haste = builder
            .register_state("core", "haste", "core.state.haste", ProcMask::POST_ACTION, SkillPriority(210))
            .expect("haste state should register");
        let charm = builder
            .register_state(
                "core",
                "charm",
                DEFAULT_CORE_CHARM_STATE_EXPORT,
                ProcMask::POST_ACTION,
                SkillPriority(210),
            )
            .expect("charm state should register");
        let slow = builder
            .register_state(
                "core",
                "slow",
                DEFAULT_CORE_SLOW_STATE_EXPORT,
                ProcMask::POST_ACTION,
                SkillPriority(210),
            )
            .expect("slow state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "healer", 0, 1_000, 3)
                    .with_magic(6_000)
                    .with_wisdom(128)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(heal, 9)])),
                PlayerTemplate::new(2, "target", 0, 1_000, 3)
                    .with_speed(40)
                    .with_target_score_stats(10, 7, 1.0),
            ],
            registry,
        ));
        {
            let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
            target.runtime.hp = 500;
            target.runtime.atk_sum = 28;
            target.runtime.speed = 40;
            target.states.add_entry(StateEntry::fire_mag(0, 2));
            target.states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 2));
            target
                .states
                .add_entry(StateEntry::curse(PLAIN_CURSE_STATE_KEY, curse, 42, 2, SkillPriority(10_000)));
            target
                .states
                .add_entry(StateEntry::poison(75, poison, Some(0), Some(1), 10.0, 2, SkillPriority(150)));
            target.states.add_entry(StateEntry::haste(77, haste, 2, 3, SkillPriority(210)));
            target.states.add_entry(StateEntry::berserk(10, 2));
            target.states.add_entry(StateEntry::charm(
                76,
                charm,
                0,
                Some(0),
                Some(0),
                Some(1),
                2,
                SkillPriority(210),
            ));
            target.states.add_entry(StateEntry::slow(78, slow, 2, SkillPriority(210)));
        }
        let mut updates = RunUpdates::new();

        runtime.drain_plain_heal_skill_into(EntityIdx(0), 0, EntityIdx(1), &mut updates);

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert!(target.runtime.hp > 500);
        assert_eq!(target.runtime.atk_sum, 7);
        assert_eq!(target.runtime.speed, 80);
        assert_eq!(target.states.entry(77).and_then(StateEntry::haste_value), Some((2, 3)));
        assert_eq!(target.states.entries().len(), 1);
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().template.skills.level_at(0), Some(8));
        assert_eq!(
            updates
                .updates
                .iter()
                .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                .map(|update| update.message.as_ref())
                .collect::<Vec<_>>(),
            vec![
                "[0]使用[治愈魔法]",
                "[1]回复体力[2]点",
                "[1]从[狂暴]中解除",
                "[1]从[魅惑]中解除",
                "[1]从[诅咒]中解除",
                "[1]从[冰冻]中解除",
                "[1]从[中毒]中解除",
                "[1]从[迟缓]中解除",
            ]
        );
    }

    #[test]
    fn plain_disperse_uses_builtin_static_dispatch_without_handler() {
        let mut builder = ExtensionRegistryBuilder::default();
        let disperse = builder
            .register_skill(
                "core",
                "disperse",
                BuiltinActiveSkill::Disperse.export_name(),
                TargetPolicy::Enemy,
                SkillPriority(17),
            )
            .expect("disperse skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3)
                    .with_magic(1_000)
                    .with_wisdom(128)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(disperse, 128)])),
                PlayerTemplate::new(2, "target", 1, 1_000, 3).with_def_res(0, 16),
            ],
            registry,
        ));

        let prepared = runtime
            .scan_plain_action_skill_probabilities(EntityIdx(0), false)
            .expect("disperse should be selected");
        assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Disperse);
        assert_eq!(prepared.targets, vec![EntityIdx(1)]);

        let mut updates = RunUpdates::new();
        runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

        assert_eq!(updates.updates.first().unwrap().message, "[0]使用[净化]");
    }

    #[test]
    fn plain_default_enemy_target_selection_matches_legacy_rng_for_single_enemy() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "ally", 0, 100, 3),
            PlayerTemplate::new(3, "enemy", 1, 100, 3),
        ]));
        let all_alive = runtime.world.flat_alive().to_vec();
        let mut expected_rng = runtime.rng.clone();
        for _ in 0..4 {
            assert_eq!(expected_rng.pick_skip_range(&all_alive, &[0, 1]), Some(2));
        }
        let _ = expected_rng.rFFFF();

        let selected = runtime.select_plain_default_enemy_targets(EntityIdx(0), false);

        assert_eq!(selected, vec![EntityIdx(2)]);
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }

    #[test]
    fn plain_fire_uses_builtin_static_dispatch_and_stacks_fire_mag() {
        let mut builder = ExtensionRegistryBuilder::default();
        let fire = builder
            .register_skill(
                "core",
                "fire",
                BuiltinActiveSkill::Fire.export_name(),
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("fire skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3)
                    .with_magic(1_000_000)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(fire, 128)])),
                PlayerTemplate::new(2, "target", 1, 100_000, 3).with_def_res(0, 0),
            ],
            registry,
        ));

        let prepared = runtime
            .scan_plain_action_skill_probabilities(EntityIdx(0), false)
            .expect("fire should be selected");
        assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Fire);
        assert_eq!(prepared.targets, vec![EntityIdx(1)]);

        let mut expected_rng = runtime.rng.clone();
        let first_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.5;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng,
        ));
        let first_damage = (first_atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        let mut updates = RunUpdates::new();

        runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

        assert_rng_state_eq(&runtime.rng, &expected_rng);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100_000 - first_damage);
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(PLAIN_FIRE_STATE_KEY),
            0.5
        );
        assert_eq!(
            updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
            vec!["[0]使用[火球术]", "[0]攻击[1]"]
        );

        let mut expected_rng = runtime.rng.clone();
        let second_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 2.0;
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng,
        ));
        let second_damage =
            (second_atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;

        runtime.drain_plain_fire_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

        assert_rng_state_eq(&runtime.rng, &expected_rng);
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp,
            100_000 - first_damage - second_damage
        );
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().states.fire_mag(PLAIN_FIRE_STATE_KEY),
            1.0
        );
    }

    #[test]
    fn plain_poison_static_dispatch_applies_threshold_and_stacking_semantics() {
        let mut builder = ExtensionRegistryBuilder::default();
        let poison = builder
            .register_skill(
                "core",
                "poison",
                BuiltinActiveSkill::Poison.export_name(),
                TargetPolicy::Enemy,
                SkillPriority(5),
            )
            .expect("poison skill should register");
        let poison_state = builder
            .register_state(
                "core",
                "poison",
                DEFAULT_CORE_POISON_STATE_EXPORT,
                ProcMask::POST_ACTION,
                SkillPriority(150),
            )
            .expect("poison state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "caster", 0, 100, 3)
                    .with_magic(1_000_000)
                    .with_skill_loadout(SkillLoadout::from_skill_levels([(poison, 128)])),
                PlayerTemplate::new(2, "target", 1, 100_000, 3).with_def_res(0, 0),
            ],
            registry,
        ));
        let mut updates = RunUpdates::new();
        let threshold_rng = runtime.rng.clone();

        runtime.apply_poison_on_damage(EntityIdx(0), EntityIdx(1), 4, &mut updates);

        assert_rng_state_eq(&runtime.rng, &threshold_rng);
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().states.entry(PLAIN_POISON_STATE_KEY),
            None
        );
        assert!(updates.updates.is_empty());

        let prepared = runtime
            .scan_plain_action_skill_probabilities(EntityIdx(0), false)
            .expect("poison should be selected");
        assert_eq!(prepared.selected.skill, BuiltinActiveSkill::Poison);
        assert_eq!(prepared.targets, vec![EntityIdx(1)]);

        let mut expected_rng = runtime.rng.clone();
        let attack_atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng);
        assert!(!PlayerRuntime::dodge(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy(),
            runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge(),
            &mut expected_rng,
        ));
        let damage = (attack_atp / runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_defense() as f64).ceil() as i32;
        assert!(damage > 4);
        let first_poison_atp =
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2000000476837158;

        runtime.drain_plain_builtin_skill_into(EntityIdx(0), prepared, &mut updates);

        assert_rng_state_eq(&runtime.rng, &expected_rng);
        assert_eq!(
            runtime
                .entities
                .get(EntityIdx(1))
                .unwrap()
                .states
                .entry(PLAIN_POISON_STATE_KEY)
                .and_then(StateEntry::poison_value),
            Some((Some(0), Some(1), first_poison_atp, 4))
        );
        assert_eq!(
            updates.updates.iter().map(|update| update.message.as_ref()).collect::<Vec<_>>(),
            vec!["[0][投毒]", "[1]受到[2]点伤害[s_dmg160]", "[1][中毒]"]
        );

        let mut expected_rng = runtime.rng.clone();
        let second_poison_atp =
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 1.2000000476837158;

        runtime.apply_poison_on_damage(EntityIdx(0), EntityIdx(1), 5, &mut updates);

        assert_rng_state_eq(&runtime.rng, &expected_rng);
        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(
            target.states.entry(PLAIN_POISON_STATE_KEY).and_then(StateEntry::poison_value),
            Some((Some(0), Some(1), first_poison_atp + second_poison_atp, 4))
        );
        assert_eq!(
            target.states.entry(PLAIN_POISON_STATE_KEY).and_then(|entry| entry.extension_state_id),
            Some(poison_state)
        );
        assert_eq!(updates.updates.last().unwrap().message, "[1][中毒]");
    }
}

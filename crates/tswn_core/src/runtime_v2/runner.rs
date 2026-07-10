use super::*;

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
    pub runtime: CombatRuntime,
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
        is_magic: bool,
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

    pub fn is_magic(self) -> Option<bool> {
        match self {
            Self::Atp { is_magic, .. } => Some(is_magic),
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
        let (raw_groups, seed) = PreparedBattleInit::split_namerena_raw(raw_input);
        let mut runner = Self::from_custom_mixed_roster(&raw_groups, config)?;
        PreparedBattleInit::from_groups(&raw_groups, &seed, &runner.runtime.registry)?.apply(&mut runner.runtime)?;
        runner.validate_ready()?;
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

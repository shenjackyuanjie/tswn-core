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
        let summon_blueprint_slot = self
            .runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT);
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
            let imported_summon_skill = self
                .runtime
                .registry
                .skill_id_by_export_name(BuiltinActiveSkill::Summon.export_name())
                .is_some_and(|summon_skill| skills.skills().contains(&summon_skill));
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
            let summon_blueprint = imported_summon_skill.then(|| {
                let slot = summon_blueprint_slot
                    .expect("runtime v2 registry importing core summon skill must reserve core summon blueprint slot");
                let summon_player_kind = self
                    .runtime
                    .registry
                    .player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT)
                    .expect("runtime v2 registry importing core summon skill must register core summon kind");
                let summon =
                    crate::player::skill::act::summon::build_summon_minion(legacy_player_id, &legacy_runner.storage, true);
                let summon_snapshot = summon.skill_loadout_snapshot();
                let summon_skills = import_plain_legacy_skill_loadout(&self.runtime.registry, &summon_snapshot);
                let mut template = Self::template_from_legacy_player(&summon, 0, team, summon_skills);
                template.kind = summon_player_kind;
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
            if let Some((slot, template)) = summon_blueprint {
                entity
                    .slots
                    .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                    .expect("runtime_v2 core summon blueprint slot must exist");
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

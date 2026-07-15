use super::prepared_init::PreparedBattleSeed;
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
    pub input_groups: Vec<Vec<EntityIdx>>,
    prepared_seed: Option<PreparedBattleSeed>,
}

#[derive(Debug, Clone)]
pub struct RuntimeV2RunSummary {
    pub rounds: Vec<RoundOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
}

/// 不保留逐回合帧的批量对局结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeV2CompletionSummary {
    pub rounds: usize,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
}

/// 可反复按不同 seed 构造 Runtime v2 对局的模板。
#[derive(Debug, Clone)]
pub struct PreparedRuntimeV2Runner {
    prototype: RuntimeV2Runner,
    battle_roster: PreparedBattleRoster,
    skill_import: PlainLegacySkillImportMap,
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
        let input_groups = Self::input_groups_from_templates(&template.players);
        Self {
            runtime: CombatRuntime::from_template(template),
            input_groups,
            prepared_seed: None,
        }
    }

    fn input_groups_from_templates(players: &[PlayerTemplate]) -> Vec<Vec<EntityIdx>> {
        let team_count = players.iter().map(|player| player.team).max().map_or(0, |team| team + 1);
        let mut groups = vec![Vec::new(); team_count];
        for (index, player) in players.iter().enumerate() {
            groups[player.team].push(EntityIdx(index.try_into().expect("runtime v2 input entity index overflow")));
        }
        groups.into_iter().filter(|group| !group.is_empty()).collect()
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
        Self::from_custom_mixed_namerena_raw_with_eval_rq(raw_input, crate::player::eval_name::DEFAULT_EVAL_RQ, config)
    }

    pub fn from_custom_mixed_namerena_raw_with_eval_rq(
        raw_input: String,
        eval_rq: f64,
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        let (raw_groups, seed) = PreparedBattleInit::split_namerena_raw(raw_input);
        let mut runner = Self::from_custom_mixed_roster(&raw_groups, config)?;
        let init = PreparedBattleInit::from_groups_with_eval_rq(&raw_groups, &seed, eval_rq, &runner.runtime.registry)?;
        runner.input_groups = init.input_groups().to_vec();
        init.apply(&mut runner.runtime)?;
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

    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    pub fn input_group_won(&self, group_index: usize) -> bool {
        let Some(winner_team) = self.runtime.world.winner_team() else {
            return false;
        };
        let Some(winner_roster) = self.runtime.world.team_roster(winner_team) else {
            return false;
        };
        self.input_groups
            .get(group_index)
            .is_some_and(|group| group.iter().any(|entity| winner_roster.contains(entity)))
    }

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

    fn run_round_unchecked_no_capture(&mut self) -> RoundOutcome { self.runtime.run_minimal_round_no_capture() }

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

    /// 跑到胜者产生或达到 guard；不收集逐回合结果，供批量评分/胜率热路径使用。
    pub fn run_to_completion(&mut self, max_rounds: usize) -> RuntimeV2CompletionSummary {
        self.assert_ready();
        self.run_to_completion_prevalidated(max_rounds)
    }

    /// 跳过 immutable handler readiness 扫描并跑到完成，供已在构造/复位时验证的批量 runner 使用。
    pub fn run_to_completion_prevalidated(&mut self, max_rounds: usize) -> RuntimeV2CompletionSummary {
        let mut rounds = 0usize;
        let mut winner_team = self.runtime.world.sync_winner_from_alive_views();
        while winner_team.is_none() && rounds < max_rounds {
            winner_team = self.run_round_unchecked_no_capture().winner_team;
            rounds += 1;
        }
        RuntimeV2CompletionSummary {
            rounds,
            winner_team,
            guard_exhausted: winner_team.is_none() && rounds == max_rounds,
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

impl PreparedRuntimeV2Runner {
    pub fn from_custom_mixed_roster(
        raw_groups: &[Vec<String>],
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        Self::from_custom_mixed_roster_with_eval_rq(raw_groups, crate::player::eval_name::DEFAULT_EVAL_RQ, config)
    }

    pub fn from_custom_mixed_roster_with_eval_rq(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        config: CustomRuntimeV2ImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeV2ImportError> {
        let skill_import = PlainLegacySkillImportMap::new(&config.registry);
        let battle_roster = PreparedBattleRoster::from_groups_with_eval_rq_and_skill_import(
            raw_groups,
            eval_rq,
            &config.registry,
            &skill_import,
        )?;
        let mut prototype = RuntimeV2Runner::from_custom_mixed_roster(raw_groups, config)?;
        let base_init = battle_roster.with_seed(&[]);
        prototype.input_groups = base_init.input_groups().to_vec();
        base_init.apply(&mut prototype.runtime)?;
        prototype.validate_ready()?;
        prototype.runtime.entities.mark_battle_baseline();
        prototype.runtime.slots.mark_battle_baseline();
        Ok(Self {
            prototype,
            battle_roster,
            skill_import,
        })
    }

    pub fn new_with_seed(&self, seed: &[String]) -> Result<RuntimeV2Runner, CustomRuntimeV2ImportError> {
        let mut runner = self.new_reusable_runner();
        self.reset_with_seed(&mut runner, seed)?;
        Ok(runner)
    }

    /// 为单个批量 worker 创建可反复复位的 runner 缓冲区。
    pub fn new_reusable_runner(&self) -> RuntimeV2Runner { self.prototype.clone() }

    pub fn reset_with_seed(&self, runner: &mut RuntimeV2Runner, seed: &[String]) -> Result<(), CustomRuntimeV2ImportError> {
        self.reset_mutable_battle_state(runner);
        let (mut seed_state, needs_refill) = match runner.prepared_seed.take() {
            Some(seed_state) => (seed_state, true),
            None => (self.battle_roster.seed_state(seed), false),
        };
        if needs_refill {
            self.battle_roster.refill_seed_state(seed, &mut seed_state);
        }

        seed_state.swap_input_groups(&mut runner.input_groups);
        let result = seed_state.apply_reusing(&mut runner.runtime).map_err(Into::into);
        runner.prepared_seed = Some(seed_state);
        result
    }

    /// 复用同一 runtime/registry 形状，以一份新 roster 构造对局。
    ///
    /// profile 评分每轮玩家名字会变化，但实体数量、custom kind 和全局模板槽形状不变；
    /// 该入口避免为每轮重新注册整套 Runtime v2 profile。
    pub fn new_from_groups_with_seed_and_eval_rq(
        &self,
        raw_groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
    ) -> Result<RuntimeV2Runner, CustomRuntimeV2ImportError> {
        let mut runner = self.new_reusable_runner();
        self.reset_from_groups_with_seed_and_eval_rq(&mut runner, raw_groups, seed, eval_rq)?;
        Ok(runner)
    }

    pub fn reset_from_groups_with_seed_and_eval_rq(
        &self,
        runner: &mut RuntimeV2Runner,
        raw_groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
    ) -> Result<(), CustomRuntimeV2ImportError> {
        let battle_roster = PreparedBattleRoster::from_groups_with_eval_rq_and_skill_import(
            raw_groups,
            eval_rq,
            &self.prototype.runtime.registry,
            &self.skill_import,
        )?;
        self.reset_with_init(runner, battle_roster.into_with_seed(seed))
    }

    pub(crate) fn reset_score_groups_with_seed_and_eval_rq(
        &self,
        runner: &mut RuntimeV2Runner,
        raw_groups: &[Vec<String>],
        profile_player_ids: &[crate::player::PlrId],
        profile_team: &str,
        profile_team_rng: &crate::rc4::RC4,
        skill_buffers: &mut [SkillLoadout],
        identity_buffers: &mut [ScoreIdentityBuffer],
        round_scratch: &mut ScoreRoundScratch,
        roster_buffers: &mut ScoreRosterBuffers,
        seed: &[String],
        eval_rq: f64,
    ) -> Result<(), CustomRuntimeV2ImportError> {
        assert_eq!(
            skill_buffers.len(),
            profile_player_ids.len(),
            "score 技能缓冲区数量必须与动态 profile 数量一致"
        );
        assert_eq!(
            identity_buffers.len(),
            profile_player_ids.len(),
            "score 身份缓冲区数量必须与动态 profile 数量一致"
        );
        for ((&id, skill_buffer), identity_buffer) in
            profile_player_ids.iter().zip(skill_buffers.iter_mut()).zip(identity_buffers.iter_mut())
        {
            let entity = runner
                .runtime
                .entities
                .get_mut(EntityIdx(id as u32))
                .expect("score 动态 profile 实体必须存在");
            std::mem::swap(&mut entity.template.skills, skill_buffer);
            std::mem::swap(&mut entity.template.name, &mut identity_buffer.name);
            std::mem::swap(&mut entity.template.id_key_name, &mut identity_buffer.id_key_name);
            std::mem::swap(&mut entity.template.clan_name, &mut identity_buffer.clan_name);
            std::mem::swap(&mut entity.template.display_name, &mut identity_buffer.display_name);
        }
        let battle_roster = PreparedBattleRoster::from_score_groups_with_cached_targets(
            raw_groups,
            eval_rq,
            &self.prototype.runtime.registry,
            &self.skill_import,
            profile_player_ids,
            profile_team,
            profile_team_rng,
            &self.battle_roster,
            skill_buffers,
            identity_buffers,
            round_scratch,
            roster_buffers,
        );
        let battle_roster = match battle_roster {
            Ok(roster) => roster,
            Err(error) => {
                for ((&id, skill_buffer), identity_buffer) in
                    profile_player_ids.iter().zip(skill_buffers.iter_mut()).zip(identity_buffers.iter_mut())
                {
                    let entity = runner
                        .runtime
                        .entities
                        .get_mut(EntityIdx(id as u32))
                        .expect("score 动态 profile 实体必须存在");
                    std::mem::swap(&mut entity.template.skills, skill_buffer);
                    std::mem::swap(&mut entity.template.name, &mut identity_buffer.name);
                    std::mem::swap(&mut entity.template.id_key_name, &mut identity_buffer.id_key_name);
                    std::mem::swap(&mut entity.template.clan_name, &mut identity_buffer.clan_name);
                    std::mem::swap(&mut entity.template.display_name, &mut identity_buffer.display_name);
                }
                return Err(error.into());
            }
        };
        let init = match runner.prepared_seed.take() {
            Some(seed_state) => battle_roster.into_with_reused_seed(seed, seed_state),
            None => battle_roster.into_with_seed(seed),
        };
        let fixed_count = profile_player_ids.first().copied().unwrap_or(runner.runtime.entities.len());
        self.reset_with_score_init(runner, init, fixed_count, roster_buffers)
    }

    fn reset_with_init(&self, runner: &mut RuntimeV2Runner, init: PreparedBattleInit) -> Result<(), CustomRuntimeV2ImportError> {
        runner.runtime.entities.clone_from(&self.prototype.runtime.entities);
        self.finish_reset_with_init(runner, init, None)
    }

    fn reset_with_score_init(
        &self,
        runner: &mut RuntimeV2Runner,
        init: PreparedBattleInit,
        fixed_count: usize,
        roster_buffers: &mut ScoreRosterBuffers,
    ) -> Result<(), CustomRuntimeV2ImportError> {
        runner
            .runtime
            .entities
            .reset_score_battle_state_from(&self.prototype.runtime.entities, fixed_count);
        self.finish_reset_with_init(runner, init, Some(roster_buffers))
    }

    fn finish_reset_with_init(
        &self,
        runner: &mut RuntimeV2Runner,
        init: PreparedBattleInit,
        roster_buffers: Option<&mut ScoreRosterBuffers>,
    ) -> Result<(), CustomRuntimeV2ImportError> {
        self.reset_shared_battle_state(runner);
        runner.prepared_seed = None;
        Self::clone_input_groups_reusing(&mut runner.input_groups, init.input_groups());
        let (seed_state, recycled_roster_buffers) = init.apply_and_recover_seed(&mut runner.runtime)?;
        if let Some(roster_buffers) = roster_buffers {
            *roster_buffers = recycled_roster_buffers.expect("score 初始化必须交还 roster 缓冲区");
        } else {
            debug_assert!(recycled_roster_buffers.is_none());
        }
        runner.prepared_seed = Some(seed_state);
        runner.validate_ready()?;
        Ok(())
    }

    fn reset_mutable_battle_state(&self, runner: &mut RuntimeV2Runner) {
        runner.runtime.entities.reset_battle_state_from(&self.prototype.runtime.entities);
        self.reset_shared_battle_state(runner);
    }

    fn reset_shared_battle_state(&self, runner: &mut RuntimeV2Runner) {
        runner.runtime.scheduler.clone_from(&self.prototype.runtime.scheduler);
        runner.runtime.effects.clear();
        runner.runtime.scratch.clear();
        runner.runtime.slots.reset_battle_state_from(&self.prototype.runtime.slots);
        #[cfg(not(feature = "no_debug"))]
        runner.runtime.trace.clone_from(&self.prototype.runtime.trace);
        runner.runtime.round = 0;
    }

    fn clone_input_groups_reusing(target: &mut Vec<Vec<EntityIdx>>, source: &[Vec<EntityIdx>]) {
        target.truncate(source.len());
        target.resize_with(source.len(), Vec::new);
        for (target, source) in target.iter_mut().zip(source) {
            target.clear();
            target.extend_from_slice(source);
        }
    }

    pub fn input_groups(&self) -> Vec<Vec<EntityIdx>> { self.battle_roster.input_groups() }
}

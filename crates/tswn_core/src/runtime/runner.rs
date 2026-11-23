use super::prepared_init::PreparedBattleSeed;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
/// 组合低层实体模板、注册表与模板槽；技能实现通过 Runtime 的 handler 入口绑定。
pub struct PreparedCombatTemplate {
    pub players: Vec<PlayerTemplate>,
    pub registry: ExtensionRegistry,
    pub slots: TemplateSlotStorage,
}

impl PreparedCombatTemplate {
    /// 用默认注册表构造准备模板。
    pub fn new(players: Vec<PlayerTemplate>) -> Self { Self::with_registry(players, ExtensionRegistry::default()) }

    /// 用给定注册表构造准备模板，并派生模板槽存储。
    pub fn with_registry(players: Vec<PlayerTemplate>, registry: ExtensionRegistry) -> Self {
        let slots = TemplateSlotStorage::from_registry(&registry);
        Self {
            players,
            registry,
            slots,
        }
    }

    /// 构造 1v1 最小模板，只设置体力与攻击，用于单独验证阶段机制。
    pub fn minimal_1v1(left_hp: i32, right_hp: i32, attack: i32) -> Self {
        Self::new(vec![
            PlayerTemplate::new(1, "left", 0, left_hp, attack),
            PlayerTemplate::new(2, "right", 1, right_hp, attack),
        ])
    }
}

#[derive(Debug, Clone)]
/// 一场可变对局，拥有 [`CombatRuntime`] 和输入队伍映射。
///
/// 普通 UI 接入使用 [`crate::cli_api::battle::BattleSession`]，直接复用其终止策略。
/// 高级调用者可访问 runtime，按机制入口协调 arena / world / handler。
pub struct RuntimeRunner {
    pub runtime: CombatRuntime,
    pub input_groups: Vec<Vec<EntityIdx>>,
    prepared_seed: Option<PreparedBattleSeed>,
}

#[derive(Debug, Clone)]
/// 保存逐回合 outcome 的运行结果；`guard_exhausted` 表示达到调用方给定的回合保护。
pub struct RuntimeRunSummary {
    pub rounds: Vec<RoundOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
}

/// 不保留逐回合帧的批量对局结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCompletionSummary {
    pub rounds: usize,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
}

/// 固化阵容、名字评估参数和导入配置，可反复按不同 seed 创建独立对局。
///
/// 准备阶段完成名字构建、handler 校验与场前基线封存；`new_with_seed` 克隆原型，
/// 批量调用则让每个 worker 持有一个 `new_reusable_runner` 并逐局复位。
/// 模板本身不随战斗改变，不记录之前局的胜者、召唤物或随机流位置。
/// 准备输入须至少含一个非 seed 玩家；空阵容没有可用的初始化密钥，不属于此入口的支持域。
///
/// seed 是完整文本材料，常用形式为 `seed:值@!`。初始化将它与玩家身份键排序去重，
/// 用回车连接后派生随机流；空 seed 仍由阵容决定随机流，重复材料或材料排列不改变密钥。
/// 对局内随机方法和实体遍历顺序仍参与确定性契约，不能仅凭相同 seed 跨规则版本复现。
#[derive(Debug, Clone)]
pub struct PreparedRuntimeRunner {
    prototype: RuntimeRunner,
    pub(super) battle_roster: PreparedBattleRoster,
    skill_import: BuiltinSkillImportMap,
}

impl RuntimeRunSummary {
    /// 最后一个回合的 outcome。
    pub fn last_outcome(&self) -> Option<&RoundOutcome> { self.rounds.last() }
}

/// 保存逐回合规范化快照的运行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeNormalizedRun {
    pub rounds: Vec<NormalizedOutcome>,
    pub winner_team: Option<usize>,
    pub guard_exhausted: bool,
    pub total_score: u64,
}

impl RuntimeNormalizedRun {
    /// 最后一个回合的规范化快照。
    pub fn last_outcome(&self) -> Option<&NormalizedOutcome> { self.rounds.last() }
}

/// 缺失 handler 的技能来源：实体装备或模板槽。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSkillSource {
    Entity(EntityIdx),
    TemplateSlot(TemplateSlotId),
}

/// 一个缺少 handler 的技能及其全部使用位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMissingSkillHandler {
    pub skill_id: SkillId,
    pub export_name: Option<String>,
    pub sources: Vec<RuntimeSkillSource>,
}

/// 就绪校验失败：列出全部缺失的 skill handler。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReadyError {
    pub missing_skill_handlers: Vec<RuntimeMissingSkillHandler>,
}

/// 使用内置主 Runtime profile 构造对局时的错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeBuildError {
    Profile(DefaultCustomRuntimeProfileError),
    Import(CustomRuntimeImportError),
}

impl std::fmt::Display for RuntimeBuildError {
    /// 显示构造错误：区分默认 profile 构建失败与阵容导入失败，均附内部错误的调试信息。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Profile(error) => write!(f, "default runtime profile failed: {error:?}"),
            Self::Import(error) => write!(f, "runtime import failed: {error:?}"),
        }
    }
}

impl std::error::Error for RuntimeBuildError {}

impl From<DefaultCustomRuntimeProfileError> for RuntimeBuildError {
    /// 把默认 profile 错误转换为 [`RuntimeBuildError::Profile`] 变体。
    fn from(error: DefaultCustomRuntimeProfileError) -> Self { Self::Profile(error) }
}

impl From<CustomRuntimeImportError> for RuntimeBuildError {
    /// 把导入错误转换为 [`RuntimeBuildError::Import`] 变体。
    fn from(error: CustomRuntimeImportError) -> Self { Self::Import(error) }
}

impl std::fmt::Display for RuntimeReadyError {
    /// 显示就绪校验错误：逐个列出缺失 skill handler 的导出名/ID 及其使用来源。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("runtime missing skill handlers: ")?;
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
                    RuntimeSkillSource::Entity(entity) => write!(f, "entity {}", entity.0)?,
                    RuntimeSkillSource::TemplateSlot(slot) => write!(f, "template slot {}", slot.0)?,
                }
            }
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeReadyError {}

/// 伤害类型标记，对应 0.5.5 中传给 `a3` 的 `T.ad()` / `T.oH()` / `T.mE()`。
///
/// 青雉的 POST_DEFEND 吸收会区分冰冻与物理伤害，其余 Boss 防御只看数值；
/// 其余攻击全部使用默认类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageKind {
    /// `T.ad()`：普通攻击与绝大多数技能。
    #[default]
    Default,
    /// `T.oH()`：`SklAttack` 的物理分支。
    Physical,
    /// `T.mE()`：冰冻术与冰河时代。
    Ice,
}

/// 防御 hook 的受击值：PRE_DEFEND 存攻击威力，POST_DEFEND 存整数伤害。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeDefendValue {
    Atp {
        value: f64,
        caster: EntityIdx,
        target: EntityIdx,
        is_magic: bool,
        kind: DamageKind,
    },
    Damage {
        value: i32,
        caster: EntityIdx,
        target: EntityIdx,
        kind: DamageKind,
    },
}

impl RuntimeDefendValue {
    /// 取攻击威力；当前为伤害阶段时返回 None。
    pub fn atp(self) -> Option<f64> {
        match self {
            Self::Atp { value, .. } => Some(value),
            Self::Damage { .. } => None,
        }
    }

    /// 是否魔法攻击；当前为伤害阶段时返回 None。
    pub fn is_magic(self) -> Option<bool> {
        match self {
            Self::Atp { is_magic, .. } => Some(is_magic),
            Self::Damage { .. } => None,
        }
    }

    /// 改写攻击威力；当前为伤害阶段时 panic。
    pub fn set_atp(&mut self, atp: f64) {
        match self {
            Self::Atp { value, .. } => *value = atp,
            Self::Damage { .. } => panic!("runtime defend value is damage, not atp"),
        }
    }

    /// 取整数伤害；当前为威力阶段时返回 None。
    pub fn damage(self) -> Option<i32> {
        match self {
            Self::Atp { .. } => None,
            Self::Damage { value, .. } => Some(value),
        }
    }

    /// 改写整数伤害；当前为威力阶段时 panic。
    pub fn set_damage(&mut self, damage: i32) {
        match self {
            Self::Atp { .. } => panic!("runtime defend value is atp, not damage"),
            Self::Damage { value, .. } => *value = damage,
        }
    }

    /// 受击值中的攻击者下标。
    pub fn caster(self) -> EntityIdx {
        match self {
            Self::Atp { caster, .. } | Self::Damage { caster, .. } => caster,
        }
    }

    /// 受击值中的目标下标。
    pub fn target(self) -> EntityIdx {
        match self {
            Self::Atp { target, .. } | Self::Damage { target, .. } => target,
        }
    }

    /// 受击值携带的伤害类型。
    pub fn kind(self) -> DamageKind {
        match self {
            Self::Atp { kind, .. } | Self::Damage { kind, .. } => kind,
        }
    }
}

impl RuntimeRunner {
    /// 使用内置 profile 从 namerena raw 构造主 Runtime 对局。
    pub fn new_from_namerena_raw(raw_input: String) -> Result<Self, RuntimeBuildError> {
        Self::new_from_namerena_raw_with_eval_rq(raw_input, crate::namerena::eval_name::DEFAULT_EVAL_RQ)
    }

    /// 使用内置 profile 和显式 `eval_rq` 从 namerena raw 构造主 Runtime 对局。
    pub fn new_from_namerena_raw_with_eval_rq(raw_input: String, eval_rq: f64) -> Result<Self, RuntimeBuildError> {
        let config = default_custom_runtime_import_config()?;
        Self::from_custom_mixed_namerena_raw_with_eval_rq(raw_input, eval_rq, config).map_err(Into::into)
    }

    /// 从准备模板构造一场对局，输入组按模板队伍推导。
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let input_groups = Self::input_groups_from_templates(&template.players);
        Self {
            runtime: CombatRuntime::from_template(template),
            input_groups,
            prepared_seed: None,
        }
    }

    /// 由模板列表推导输入组：按队伍分桶实体下标，空队丢弃。
    fn input_groups_from_templates(players: &[PlayerTemplate]) -> Vec<Vec<EntityIdx>> {
        let team_count = players.iter().map(|player| player.team).max().map_or(0, |team| team + 1);
        let mut groups = vec![Vec::new(); team_count];
        for (index, player) in players.iter().enumerate() {
            groups[player.team].push(EntityIdx(index.try_into().expect("runtime input entity index overflow")));
        }
        groups.into_iter().filter(|group| !group.is_empty()).collect()
    }

    /// 按自定义 bed2 roster 与导入配置构造对局，并安装配置内的 handler。
    pub fn from_custom_bed2_roster(
        raw_groups: &[Vec<String>],
        config: CustomRuntimeImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeImportError> {
        let CustomRuntimeImportConfig {
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
                    .map_err(CustomRuntimeImportError::Bed2MinionOverlay)
            }
            None => Self::from_bed2_roster(raw_groups, registry, bed2_kind, bed2_summon_skill)
                .map_err(CustomRuntimeImportError::Bed2Roster),
        }?;
        runner.install_skill_handler_bindings(skill_handlers);
        runner.install_state_handler_bindings(state_handlers);
        runner.validate_ready()?;
        Ok(runner)
    }

    /// 按混合 roster 与导入配置构造对局，并安装配置内的 handler。
    pub fn from_custom_mixed_roster(
        raw_groups: &[Vec<String>],
        config: CustomRuntimeImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeImportError> {
        let CustomRuntimeImportConfig {
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
                    .map_err(CustomRuntimeImportError::Bed2MinionOverlay)
            }
            None => Self::from_mixed_roster(raw_groups, registry, bed2_kind, bed2_summon_skill)
                .map_err(CustomRuntimeImportError::MixedRoster),
        }?;
        runner.install_skill_handler_bindings(skill_handlers);
        runner.install_state_handler_bindings(state_handlers);
        runner.validate_ready()?;
        Ok(runner)
    }

    /// 按默认 eval_rq 从 namerena 文本构造混合对局。
    pub fn from_custom_mixed_namerena_raw(
        raw_input: String,
        config: CustomRuntimeImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeImportError> {
        Self::from_custom_mixed_namerena_raw_with_eval_rq(raw_input, crate::namerena::eval_name::DEFAULT_EVAL_RQ, config)
    }

    /// 按指定 eval_rq 从 namerena 文本构造混合对局。
    pub fn from_custom_mixed_namerena_raw_with_eval_rq(
        raw_input: String,
        eval_rq: f64,
        config: CustomRuntimeImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeImportError> {
        let (raw_groups, seed) = PreparedBattleInit::split_namerena_raw(raw_input);
        let mut runner = Self::from_custom_mixed_roster(&raw_groups, config)?;
        let init = PreparedBattleInit::from_groups_with_eval_rq(&raw_groups, &seed, eval_rq, &runner.runtime.registry)?;
        runner.input_groups = init.input_groups().to_vec();
        init.apply(&mut runner.runtime)?;
        runner.validate_ready()?;
        Ok(runner)
    }

    /// 从 bed2 roster 构造对局。
    pub fn from_bed2_roster(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        kind: PlayerKindId,
        summon_skill: SkillId,
    ) -> Result<Self, CustomBed2RosterImportError> {
        let template = CustomBed2Import::roster_into_prepared_template(raw_groups, registry, kind, summon_skill)?;
        Ok(Self::from_template(template))
    }

    /// 从 bed2 roster 构造对局，并叠加召唤物模板配置。
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

    /// 从 bed2 roster 构造对局，并叠加幻影模板配置。
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

    /// 从 bed2 roster 构造对局，并叠加亡灵模板配置。
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

    /// 从 bed2 roster 构造对局，并叠加一组使魔模板配置。
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

    /// 从混合 roster 构造对局。
    pub fn from_mixed_roster(
        raw_groups: &[Vec<String>],
        registry: ExtensionRegistry,
        bed2_kind: PlayerKindId,
        bed2_summon_skill: SkillId,
    ) -> Result<Self, CustomMixedRosterImportError> {
        let template = CustomBed2Import::mixed_roster_into_prepared_template(raw_groups, registry, bed2_kind, bed2_summon_skill)?;
        Ok(Self::from_template(template))
    }

    /// 从混合 roster 构造对局，并叠加召唤物模板配置。
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

    /// 从混合 roster 构造对局，并叠加幻影模板配置。
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

    /// 从混合 roster 构造对局，并叠加亡灵模板配置。
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

    /// 从混合 roster 构造对局，并叠加一组使魔模板配置。
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

    /// 只读访问内部 CombatRuntime。
    pub fn runtime(&self) -> &CombatRuntime { &self.runtime }

    /// 高级机制入口；直接修改字段可能破坏世界视图、缓存与默认导出支持域。
    pub fn runtime_mut(&mut self) -> &mut CombatRuntime { &mut self.runtime }

    /// 输入组对应的实体下标。
    pub fn input_groups(&self) -> &[Vec<EntityIdx>] { &self.input_groups }

    /// 指定输入组是否包含最终胜者队伍的成员。
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

    /// 检查当前实体与模板槽使用的技能是否已安装 handler。
    pub fn validate_ready(&self) -> Result<(), RuntimeReadyError> { self.runtime.validate_ready() }

    /// 批量安装 skill handler 绑定，连同各自声明的能力一起写入。
    fn install_skill_handler_bindings(&mut self, bindings: Vec<RuntimeSkillHandlerBinding>) {
        for binding in bindings {
            self.runtime
                .set_skill_handler_with_capabilities(binding.skill_id, binding.handler, &binding.capabilities);
        }
    }

    /// 批量安装 state handler 绑定，连同各自声明的能力一起写入。
    fn install_state_handler_bindings(&mut self, bindings: Vec<RuntimeStateHandlerBinding>) {
        for binding in bindings {
            self.runtime
                .set_state_handler_with_capabilities(binding.state_id, binding.handler, &binding.capabilities);
        }
    }

    /// 就绪校验；未就绪时 panic 并附完整错误文本。
    fn assert_ready(&self) {
        if let Err(error) = self.validate_ready() {
            panic!("{error}");
        }
    }

    /// 跳过就绪校验，直接推进一个最小回合。
    fn run_round_unchecked(&mut self) -> RoundOutcome { self.runtime.run_minimal_round() }

    /// 推进一个 Runtime 回合；可能没有可见帧。未就绪时会 panic，高层会话会先校验。
    pub fn run_round(&mut self) -> RoundOutcome {
        self.assert_ready();
        self.run_round_unchecked()
    }

    /// 推进一个 Runtime 回合并返回规范化快照。
    pub fn run_round_normalized(&mut self) -> NormalizedOutcome {
        let outcome = self.run_round();
        NormalizedOutcome::from_runtime(&self.runtime, &outcome)
    }

    /// 最多推进指定回合数并保存 outcome；大量胜率计算应使用不保留帧的 completion 入口。
    pub fn run_until_winner(&mut self, max_rounds: usize) -> RuntimeRunSummary {
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
        RuntimeRunSummary {
            guard_exhausted: winner_team.is_none() && rounds.len() == max_rounds,
            rounds,
            winner_team,
        }
    }

    /// 跑到胜者产生或达到 guard；不收集逐回合结果，供批量评分/胜率热路径使用。
    pub fn run_to_completion(&mut self, max_rounds: usize) -> RuntimeCompletionSummary {
        self.assert_ready();
        self.run_to_completion_prevalidated(max_rounds)
    }

    /// 跳过 handler 就绪扫描并跑到完成，供已在构造或复位时验证的批量 runner 使用。
    ///
    /// 调用方需保持 handler 绑定有效，并通过机制入口同步维护实体与存活视图；
    /// 此路径直接按存活视图判胜，不重新扫描实体表。`max_rounds == 0` 只检查现有胜者，
    /// 无胜者时报告保护上限耗尽；本方法不生成展示帧，也不保留逐回合 outcome。
    pub fn run_to_completion_prevalidated(&mut self, max_rounds: usize) -> RuntimeCompletionSummary {
        let mut rounds = 0usize;
        let mut winner_team = self.runtime.world.sync_winner_from_alive_views();
        while winner_team.is_none() && rounds < max_rounds {
            winner_team = self.runtime.run_active_round_no_capture();
            rounds += 1;
        }
        RuntimeCompletionSummary {
            rounds,
            winner_team,
            guard_exhausted: winner_team.is_none() && rounds == max_rounds,
        }
    }

    /// 跑到胜者或上限，返回逐回合摘要与终局规范化快照。
    pub fn run_until_winner_normalized(&mut self, max_rounds: usize) -> (RuntimeRunSummary, NormalizedOutcome) {
        let summary = self.run_until_winner(max_rounds);
        let final_outcome = summary.last_outcome().cloned().unwrap_or(RoundOutcome {
            action: None,
            frame: None,
            winner_team: summary.winner_team,
        });
        let normalized = NormalizedOutcome::from_runtime(&self.runtime, &final_outcome);
        (summary, normalized)
    }

    /// 跑到胜者或上限，保留逐回合规范化快照与累计得分。
    pub fn run_until_winner_normalized_rounds(&mut self, max_rounds: usize) -> RuntimeNormalizedRun {
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
        RuntimeNormalizedRun {
            guard_exhausted: winner_team.is_none() && rounds.len() == max_rounds,
            rounds,
            winner_team,
            total_score,
        }
    }
}

impl PreparedRuntimeRunner {
    /// 按默认 eval_rq 从混合 roster 预构建可复用阵容。
    pub fn from_custom_mixed_roster(
        raw_groups: &[Vec<String>],
        config: CustomRuntimeImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeImportError> {
        Self::from_custom_mixed_roster_with_eval_rq(raw_groups, crate::namerena::eval_name::DEFAULT_EVAL_RQ, config)
    }

    /// 按指定 eval_rq 从混合 roster 预构建可复用阵容。
    pub fn from_custom_mixed_roster_with_eval_rq(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        config: CustomRuntimeImportConfig<'_>,
    ) -> Result<Self, CustomRuntimeImportError> {
        Self::from_custom_mixed_roster_with_options(raw_groups, eval_rq, config, true)
    }

    pub(crate) fn from_custom_mixed_roster_with_options(
        raw_groups: &[Vec<String>],
        eval_rq: f64,
        config: CustomRuntimeImportConfig<'_>,
        team_upgrades: bool,
    ) -> Result<Self, CustomRuntimeImportError> {
        let skill_import = BuiltinSkillImportMap::new(&config.registry);
        let battle_roster = PreparedBattleRoster::from_groups_with_team_upgrades(
            raw_groups,
            eval_rq,
            &config.registry,
            &skill_import,
            team_upgrades,
        )?;
        let mut prototype = RuntimeRunner::from_custom_mixed_roster(raw_groups, config)?;
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

    /// 创建独立拥有可变状态的对局；返回值不借用 `self` 或 seed 文本。
    ///
    /// 创建不会推进战斗。seed 只改变初始化随机流、行动顺序、运行时队伍编号与初始移动点，
    /// 阵容身份及名字评估口径沿用准备模板；输入组索引仍用于关联原始队伍。
    pub fn new_with_seed(&self, seed: &[String]) -> Result<RuntimeRunner, CustomRuntimeImportError> {
        let mut runner = self.new_reusable_runner();
        self.reset_with_seed(&mut runner, seed)?;
        Ok(runner)
    }

    /// 为单个批量 worker 克隆场前原型，独占后续战斗与复位所用的缓冲。
    ///
    /// 返回值已带空 seed 的初始状态；需要特定 seed 时，在推进前调用
    /// [`Self::reset_with_seed`]。不要让多个 worker 共享同一个可变 runner。
    pub fn new_reusable_runner(&self) -> RuntimeRunner { self.prototype.clone() }

    /// 从同一准备模板恢复下一局，保留可复用容器容量并应用本场 seed。
    ///
    /// 清除上一局生成的实体、状态、效果、胜者、调度和回合计数，恢复技能与槽基线。
    /// `runner` 应来自此模板；注册表、handler 与模板配置不在此入口重装，调用方
    /// 不应替换它们。失败不保证保留旧对局，调用方可从模板重新创建 runner。
    pub fn reset_with_seed(&self, runner: &mut RuntimeRunner, seed: &[String]) -> Result<(), CustomRuntimeImportError> {
        self.reset_with_seed_state(runner, |roster, state| roster.refill_seed_state(seed, state))
    }

    pub(super) fn reset_with_seed_rng(&self, runner: &mut RuntimeRunner, rng: RC4) -> Result<(), CustomRuntimeImportError> {
        self.reset_with_seed_state(runner, |roster, state| roster.refill_seed_state_with_rng(state, rng))
    }

    /// 用回调填充 seed 状态并复位 runner：先恢复可变战斗状态，再套用 seed 与输入组。
    fn reset_with_seed_state(
        &self,
        runner: &mut RuntimeRunner,
        refill: impl FnOnce(&PreparedBattleRoster, &mut PreparedBattleSeed),
    ) -> Result<(), CustomRuntimeImportError> {
        self.reset_mutable_battle_state(runner);
        let seed_state = runner.prepared_seed.get_or_insert_with(|| self.battle_roster.new_seed_state());
        refill(&self.battle_roster, seed_state);

        seed_state.swap_input_groups(&mut runner.input_groups);
        seed_state.apply_swapping_views(&mut runner.runtime).map_err(Into::into)
    }

    /// 复用同一 runtime/registry 形状，以一份新 roster 构造对局。
    ///
    /// profile 评分每轮玩家名字会变化，但实体数量、custom kind 和全局模板槽形状不变；
    /// 该入口避免为每轮重新注册整套 Runtime profile。
    pub fn new_from_groups_with_seed_and_eval_rq(
        &self,
        raw_groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
    ) -> Result<RuntimeRunner, CustomRuntimeImportError> {
        let mut runner = self.new_reusable_runner();
        self.reset_from_groups_with_seed_and_eval_rq(&mut runner, raw_groups, seed, eval_rq)?;
        Ok(runner)
    }

    /// 用一份新 roster 与 seed 复位已有 runner，复用其注册表与缓冲。
    pub fn reset_from_groups_with_seed_and_eval_rq(
        &self,
        runner: &mut RuntimeRunner,
        raw_groups: &[Vec<String>],
        seed: &[String],
        eval_rq: f64,
    ) -> Result<(), CustomRuntimeImportError> {
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
        runner: &mut RuntimeRunner,
        raw_groups: &[Vec<String>],
        profile_player_ids: &[crate::runtime::PlrId],
        profile_team: &str,
        profile_team_rng: &crate::rc4::RC4,
        skill_buffers: &mut [SkillLoadout],
        identity_buffers: &mut [ScoreIdentityBuffer],
        round_scratch: &mut ScoreRoundScratch,
        roster_buffers: &mut ScoreRosterBuffers,
        seed: &[String],
        eval_rq: f64,
    ) -> Result<(), CustomRuntimeImportError> {
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

    /// 用准备初始化数据复位 runner：实体整体克隆原型，再走统一收尾。
    fn reset_with_init(&self, runner: &mut RuntimeRunner, init: PreparedBattleInit) -> Result<(), CustomRuntimeImportError> {
        runner.runtime.entities.clone_from(&self.prototype.runtime.entities);
        self.finish_reset_with_init(runner, init, None)
    }

    /// 用 score 初始化数据复位：可复用 score 缓冲时只恢复固定实体，否则回退完整复位。
    fn reset_with_score_init(
        &self,
        runner: &mut RuntimeRunner,
        init: PreparedBattleInit,
        fixed_count: usize,
        roster_buffers: &mut ScoreRosterBuffers,
    ) -> Result<(), CustomRuntimeImportError> {
        // 完整 roster 回退不清理实体的动态状态和槽位，须先恢复原型，
        // 避免保留已被截掉的召唤物 ID，以及同局状态和蓝图。
        if !init.has_score_reuse_buffers() {
            return self.reset_with_init(runner, init);
        }
        runner
            .runtime
            .entities
            .reset_score_battle_state_from(&self.prototype.runtime.entities, fixed_count);
        self.finish_reset_with_init(runner, init, Some(roster_buffers))
    }

    /// 复位的统一收尾：恢复共享状态、套用初始化数据、按需回收 score 缓冲，并做就绪校验。
    fn finish_reset_with_init(
        &self,
        runner: &mut RuntimeRunner,
        init: PreparedBattleInit,
        roster_buffers: Option<&mut ScoreRosterBuffers>,
    ) -> Result<(), CustomRuntimeImportError> {
        self.reset_shared_battle_state(runner);
        runner.prepared_seed = None;
        Self::clone_input_groups_reusing(&mut runner.input_groups, init.input_groups());
        let (seed_state, recycled_roster_buffers) = init.apply_and_recover_seed(&mut runner.runtime)?;
        if let Some(roster_buffers) = roster_buffers {
            // 动态 profile 与固定 target 同 clan 等场景会回退完整 roster 构造。
            // 该路径不持有 score 专用缓冲；保留调用方的空缓冲即可，下轮仍会走
            // 完整构造，不能把这项性能优化当成初始化正确性的前置条件。
            if let Some(recycled_roster_buffers) = recycled_roster_buffers {
                *roster_buffers = recycled_roster_buffers;
            }
        } else {
            debug_assert!(recycled_roster_buffers.is_none());
        }
        runner.prepared_seed = Some(seed_state);
        runner.validate_ready()?;
        Ok(())
    }

    /// 只恢复每局可变的实体战斗状态，再走共享状态复位。
    fn reset_mutable_battle_state(&self, runner: &mut RuntimeRunner) {
        runner.runtime.entities.reset_battle_state_from(&self.prototype.runtime.entities);
        self.reset_shared_battle_state(runner);
    }

    /// 复位跨实体共享状态：调度器、效果队列、scratch、槽位、外部脚本与回合计数。
    fn reset_shared_battle_state(&self, runner: &mut RuntimeRunner) {
        runner.runtime.scheduler.clone_from(&self.prototype.runtime.scheduler);
        runner.runtime.effects.clear();
        runner.runtime.scratch.clear();
        // 复用对局恢复原始 Boss 槽，不能携带上场新增实体的科研状态。
        runner.runtime.squall_bosses.clone_from(&self.prototype.runtime.squall_bosses);
        runner.runtime.slots.reset_battle_state_from(&self.prototype.runtime.slots);
        // 外部脚本与受控队伍属于单场设置，下一场必须从零开始，避免暂停或残留指令跨局泄漏。
        runner.runtime.control.reset();
        #[cfg(not(feature = "no_debug"))]
        runner.runtime.trace.clone_from(&self.prototype.runtime.trace);
        runner.runtime.round = 0;
    }

    /// 复用目标向量容量，从来源整体复制输入组。
    fn clone_input_groups_reusing(target: &mut Vec<Vec<EntityIdx>>, source: &[Vec<EntityIdx>]) {
        target.truncate(source.len());
        target.resize_with(source.len(), Vec::new);
        for (target, source) in target.iter_mut().zip(source) {
            target.clear();
            target.extend_from_slice(source);
        }
    }

    /// 准备阵容的输入组下标。
    pub fn input_groups(&self) -> Vec<Vec<EntityIdx>> { self.battle_roster.input_groups() }
}

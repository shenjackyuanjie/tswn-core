//! 扩展注册表：描述名字、ID、hook 元数据与策略，handler 函数另行绑定。
//!
//! [`ExtensionRegistryBuilder`] 按注册顺序分配各类别的 ID，`build` 后得到
//! [`ExtensionRegistry`]。ID 依赖所属 registry，查找和 storage 使用同一份注册表。
//! namespace / name 用于分类，export_name 用于导入和查找；重复名会返回
//! [`ExtensionError`]。默认规则的稳定导出名称由 Runtime profile 维护。
//!
//! # 注册与函数绑定
//!
//! [`TswnExtension`] 声明名称、版本、能力和注册步骤；实际函数仍需通过
//! [`super::CombatRuntime::set_skill_handler`] 等方法安装。capabilities 也需传给
//! `set_*_handler_with_capabilities`，完成实际的上下文授权。
//! 这些 API 面向同进程 Rust 扩展，通过明确注册和函数绑定组合规则。
//!
//! [`ProcMask`] 指定参与的生命周期阶段；[`SkillPriority`] 越小越先执行。
//! 内置主动技能可使用 `ProcMask::NONE`，主体行为由专用行动分支触发。
//! [`PlayerKindPolicies`] 决定归属解析、伤害共享、吞噬映射与主人防抗继承。
//!
//! # 最小技能 hook
//!
//! 下面在低层最小模板中注册行动前恢复。它演示注册、loadout、绑定、验证和推进，
//! 普通名字推导、DIY 解析与模型词表可沿各自入口继续接入新技能。
//!
//! ```
//! use tswn_core::runtime::{
//!     ExtensionRegistryBuilder, PlayerTemplate, PreparedCombatTemplate, ProcMask,
//!     QueuedEffect, RuntimeRunner, SkillContext, SkillHookPlanEntry, SkillPriority,
//!     TargetPolicy,
//! };
//!
//! fn recover(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
//!     let owner = context.owner_idx();
//!     context.push_nested(QueuedEffect::Heal {
//!         caster: owner,
//!         target: owner,
//!         amount: context.skill_level(entry) as i32,
//!     });
//! }
//!
//! let mut builder = ExtensionRegistryBuilder::default();
//! let skill = builder.register_skill_with_hooks(
//!     "demo", "recover", "demo.skill.recover",
//!     ProcMask::PRE_ACTION, TargetPolicy::None, SkillPriority(0),
//! )?;
//! let players = vec![
//!     PlayerTemplate::new(1, "left", 0, 20, 4).with_skills([skill]),
//!     PlayerTemplate::new(2, "right", 1, 20, 4),
//! ];
//! let mut runner = RuntimeRunner::from_template(
//!     PreparedCombatTemplate::with_registry(players, builder.build()),
//! );
//! runner.runtime_mut().set_skill_handler(skill, recover);
//! runner.validate_ready()?;
//! let outcome = runner.run_round();
//! assert!(outcome.action.is_some());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::runtime::profile::{
    BuiltinActiveSkill, DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT, DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT,
    DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT,
};

/// 玩家类型 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct PlayerKindId(pub u32);

/// 技能 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkillId(pub u32);

/// 状态 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateId(pub u32);

/// 模板槽 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateSlotId(pub u32);

/// 对局级槽 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BattleSlotId(pub u32);

/// 实体槽 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntitySlotId(pub u32);

/// 效果 handler ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectHandlerId(pub u32);

/// 回放渲染器 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplayRendererId(pub u32);

/// 展示渲染器 ID；由注册表按注册次序分配，仅在所属注册表内有效。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShowRendererId(pub u32);

/// 扩展的语义化版本号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ExtensionVersion {
    /// 构造版本号。
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self { Self { major, minor, patch } }
}

/// handler 需要的上下文能力；实际授权由 set_*_handler_with_capabilities 完成。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensionCapability {
    ReadAllies,
    ReadEnemies,
    ReadBattleSlots,
    ReadTemplateSlots,
    MutateEntitySlots,
}

/// hook 执行优先级；数值越小越先执行，同值按注册次序。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkillPriority(pub i32);

/// 注册次序，用于同优先级内的稳定排序。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationOrder(pub u32);

/// 生命周期 hook 掩码；各位相互独立，可用位或组合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcMask(pub u64);

impl ProcMask {
    /// 空掩码，不参与任何 hook。
    pub const NONE: Self = Self(0);
    /// 行动前 hook。
    pub const PRE_ACTION: Self = Self(1 << 0);
    /// 行动后 hook。
    pub const POST_ACTION: Self = Self(1 << 1);
    /// 伤害结算前 hook。
    pub const PRE_DAMAGE: Self = Self(1 << 2);
    /// 伤害结算后 hook。
    pub const POST_DAMAGE: Self = Self(1 << 3);
    /// 防御判定前 hook。
    pub const PRE_DEFEND: Self = Self(1 << 4);
    /// 防御判定后 hook。
    pub const POST_DEFEND: Self = Self(1 << 5);
    /// 死亡 hook。
    pub const DIE: Self = Self(1 << 6);
    /// 击杀 hook。
    pub const KILL: Self = Self(1 << 7);

    /// 两个掩码是否有共同阶段。
    pub const fn intersects(self, rhs: Self) -> bool { (self.0 & rhs.0) != 0 }

    /// 掩码是否为空。
    pub const fn is_empty(self) -> bool { self.0 == 0 }
}

impl Default for ProcMask {
    /// 默认 hook 掩码：空掩码，不参与任何阶段。
    fn default() -> Self { Self::NONE }
}

impl std::ops::BitOr for ProcMask {
    /// 位或结果类型：ProcMask 自身。
    type Output = Self;

    /// 合并两个掩码的 hook 阶段。
    fn bitor(self, rhs: Self) -> Self::Output { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for ProcMask {
    /// 就地并入另一掩码的 hook 阶段。
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

/// 技能的目标策略：无目标、敌方、友方或任意。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TargetPolicy {
    #[default]
    None,
    Enemy,
    Ally,
    Any,
}

/// POST_ACTION 阶段的执行时机：早（默认）或晚。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillPostActionPhase {
    #[default]
    Early,
    Late,
}

/// 玩家类型标志位；各位相互独立，可用位或组合。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct PlayerKindFlags(pub u64);

impl PlayerKindFlags {
    /// 无标志。
    pub const NONE: Self = Self(0);
    /// Boss 角色。
    pub const BOSS: Self = Self(1 << 0);
    /// 使魔（召唤物谱系）。
    pub const MINION: Self = Self(1 << 1);
    /// 召唤物。
    pub const SUMMON: Self = Self(1 << 2);
    /// bed2 自定义阵容成员。
    pub const BED2: Self = Self(1 << 3);
    /// 加成（boost）角色。
    pub const BOOST: Self = Self(1 << 4);
    /// 参与战斗结算的使魔。
    pub const COMBAT_MINION: Self = Self(1 << 5);

    /// 是否包含全部指定标志。
    pub const fn contains(self, rhs: Self) -> bool { (self.0 & rhs.0) == rhs.0 }
    /// 是否有任一指定标志。
    pub const fn intersects(self, rhs: Self) -> bool { (self.0 & rhs.0) != 0 }
}

impl std::ops::BitOr for PlayerKindFlags {
    /// 位或结果类型：PlayerKindFlags 自身。
    type Output = Self;

    /// 合并两组玩家类型标志。
    fn bitor(self, rhs: Self) -> Self::Output { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for PlayerKindFlags {
    /// 就地并入另一组玩家类型标志。
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

/// 归属解析策略：效果归属自身还是根所有者。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OwnerResolutionPolicy {
    #[default]
    SelfEntity,
    RootOwner,
}

/// 伤害共享策略：不共享、共享给所有者或共享给召唤物。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DamageSharePolicy {
    #[default]
    None,
    ShareToOwner,
    ShareToSummons,
}

/// 吞噬（合并）策略：合并槽位映射与资源转移方式。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MergePolicy {
    None,
    #[default]
    FixedLane,
    /// 固定槽位合并技能，但只获得目标一半的 MP 差值与行动点（bed2 召唤物玩法）。
    FixedLaneHalfResources,
    DropUnmappedSkills,
}

/// 玩家类型的策略组合：归属、伤害共享、吞噬与主防抗继承。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlayerKindPolicies {
    pub owner_resolution: OwnerResolutionPolicy,
    pub damage_share: DamageSharePolicy,
    pub merge: MergePolicy,
    pub inherit_owner_def_res: bool,
}

/// 玩家类型的注册信息：ID、命名、导出名、标志与策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerKindSpec {
    pub id: PlayerKindId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub flags: PlayerKindFlags,
    pub policies: PlayerKindPolicies,
}

/// 技能的注册信息：ID、命名、导出名、hook 掩码、目标策略与优先级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSpec {
    pub id: SkillId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub hook_mask: ProcMask,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
    pub post_action_phase: SkillPostActionPhase,
    pub registration_order: RegistrationOrder,
}

/// 状态的注册信息：ID、命名、导出名、hook 掩码与优先级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSpec {
    pub id: StateId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub hook_mask: ProcMask,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

/// 模板槽的注册信息：ID、命名与导出名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSlotSpec {
    pub id: TemplateSlotId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

/// 对局级槽的注册信息：ID、命名与导出名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleSlotSpec {
    pub id: BattleSlotId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

/// 实体槽的注册信息：ID、命名与导出名。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySlotSpec {
    pub id: EntitySlotId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

/// 效果 handler 的注册信息：ID、命名、导出名与优先级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectHandlerSpec {
    pub id: EffectHandlerId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

/// 回放渲染器的注册信息：ID、命名、导出名与优先级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRendererSpec {
    pub id: ReplayRendererId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

/// 展示渲染器的注册信息：ID、命名、导出名与优先级。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowRendererSpec {
    pub id: ShowRendererId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

/// 已安装扩展的记录：名称、版本与声明的能力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtensionSpec {
    pub name: String,
    pub version: ExtensionVersion,
    pub capabilities: Vec<ExtensionCapability>,
}

/// 注册错误：扩展名、类别内名称或导出名重复。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionError {
    DuplicateExtensionName { name: String },
    DuplicateName { namespace: String, name: String },
    DuplicateExportName { export_name: String },
}

impl Display for ExtensionError {
    /// 显示注册错误：区分扩展名重复、类别内名称重复与导出名重复三类。
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtensionError::DuplicateExtensionName { name } => {
                write!(f, "duplicate extension name: {name}")
            }
            ExtensionError::DuplicateName { namespace, name } => {
                write!(f, "duplicate extension name: {namespace}::{name}")
            }
            ExtensionError::DuplicateExportName { export_name } => {
                write!(f, "duplicate extension export name: {export_name}")
            }
        }
    }
}

impl std::error::Error for ExtensionError {}

/// 同进程扩展的描述和元数据注册协议；handler 与实际能力通过 Runtime 绑定。
pub trait TswnExtension {
    /// 扩展的唯一名称，重复安装会报错。
    fn name(&self) -> &'static str;

    /// 扩展的描述性版本，供配置与审计读取。
    fn version(&self) -> ExtensionVersion;

    /// 声明使用的上下文能力；实际 handler 绑定仍需显式传递。
    fn capabilities(&self) -> &'static [ExtensionCapability] { &[] }

    /// 将技能、状态和槽等注册到 builder；失败时调用方重新构造完整配置。
    fn register(&self, registry: &mut ExtensionRegistryBuilder) -> Result<(), ExtensionError>;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
/// 构造期注册器；各类别 ID 依赖注册次序，完成后用 [`Self::build`] 冻结。
pub struct ExtensionRegistryBuilder {
    installed_extensions: Vec<InstalledExtensionSpec>,
    player_kinds: Vec<PlayerKindSpec>,
    skills: Vec<SkillSpec>,
    states: Vec<StateSpec>,
    template_slots: Vec<TemplateSlotSpec>,
    battle_slots: Vec<BattleSlotSpec>,
    entity_slots: Vec<EntitySlotSpec>,
    effect_handlers: Vec<EffectHandlerSpec>,
    replay_renderers: Vec<ReplayRendererSpec>,
    show_renderers: Vec<ShowRendererSpec>,
    player_kind_names: HashMap<(String, String), PlayerKindId>,
    skill_names: HashMap<(String, String), SkillId>,
    state_names: HashMap<(String, String), StateId>,
    template_slot_names: HashMap<(String, String), TemplateSlotId>,
    battle_slot_names: HashMap<(String, String), BattleSlotId>,
    entity_slot_names: HashMap<(String, String), EntitySlotId>,
    effect_handler_names: HashMap<(String, String), EffectHandlerId>,
    replay_renderer_names: HashMap<(String, String), ReplayRendererId>,
    show_renderer_names: HashMap<(String, String), ShowRendererId>,
    extension_names: HashMap<String, ()>,
    export_names: HashMap<String, ()>,
    next_registration_order: u32,
}

impl ExtensionRegistryBuilder {
    /// 安装一个扩展：调用其注册步骤并记录名称、版本与能力。
    pub fn install_extension(&mut self, extension: &impl TswnExtension) -> Result<(), ExtensionError> {
        let name = extension.name().to_owned();
        if self.extension_names.contains_key(&name) {
            return Err(ExtensionError::DuplicateExtensionName { name });
        }

        extension.register(self)?;
        self.extension_names.insert(name.clone(), ());
        self.installed_extensions.push(InstalledExtensionSpec {
            name,
            version: extension.version(),
            capabilities: extension.capabilities().to_vec(),
        });
        Ok(())
    }

    /// 注册玩家类型，使用默认标志与策略。
    pub fn register_player_kind(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
    ) -> Result<PlayerKindId, ExtensionError> {
        self.register_player_kind_with_policies(
            namespace,
            name,
            export_name,
            PlayerKindFlags::default(),
            PlayerKindPolicies::default(),
        )
    }

    /// 注册玩家类型并指定标志与策略。
    pub fn register_player_kind_with_policies(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        flags: PlayerKindFlags,
        policies: PlayerKindPolicies,
    ) -> Result<PlayerKindId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.player_kind_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = PlayerKindId(self.player_kinds.len() as u32);
        let spec = PlayerKindSpec {
            id,
            namespace,
            name,
            export_name,
            flags,
            policies,
        };
        self.player_kind_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.player_kinds.push(spec);
        Ok(id)
    }

    /// 注册技能，默认挂到 PRE_ACTION hook。
    pub fn register_skill(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        target_policy: TargetPolicy,
        priority: SkillPriority,
    ) -> Result<SkillId, ExtensionError> {
        self.register_skill_with_hooks(namespace, name, export_name, ProcMask::PRE_ACTION, target_policy, priority)
    }

    /// 注册技能并指定 hook 掩码。
    pub fn register_skill_with_hooks(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        hook_mask: ProcMask,
        target_policy: TargetPolicy,
        priority: SkillPriority,
    ) -> Result<SkillId, ExtensionError> {
        self.register_skill_with_hooks_and_post_action_phase(
            namespace,
            name,
            export_name,
            hook_mask,
            target_policy,
            priority,
            SkillPostActionPhase::Early,
        )
    }

    /// 注册技能并指定 hook 掩码与 POST_ACTION 执行时机。
    pub fn register_skill_with_hooks_and_post_action_phase(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        hook_mask: ProcMask,
        target_policy: TargetPolicy,
        priority: SkillPriority,
        post_action_phase: SkillPostActionPhase,
    ) -> Result<SkillId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.skill_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = SkillId(self.skills.len() as u32);
        let spec = SkillSpec {
            id,
            namespace,
            name,
            export_name,
            hook_mask,
            target_policy,
            priority,
            post_action_phase,
            registration_order: self.next_order(),
        };
        self.skill_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.skills.push(spec);
        Ok(id)
    }

    /// 注册状态并指定 hook 掩码与优先级。
    pub fn register_state(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        hook_mask: ProcMask,
        priority: SkillPriority,
    ) -> Result<StateId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.state_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = StateId(self.states.len() as u32);
        let spec = StateSpec {
            id,
            namespace,
            name,
            export_name,
            hook_mask,
            priority,
            registration_order: self.next_order(),
        };
        self.state_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.states.push(spec);
        Ok(id)
    }

    /// 预留一个模板槽。
    pub fn reserve_template_slot(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
    ) -> Result<TemplateSlotId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.template_slot_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = TemplateSlotId(self.template_slots.len() as u32);
        let spec = TemplateSlotSpec {
            id,
            namespace,
            name,
            export_name,
        };
        self.template_slot_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.template_slots.push(spec);
        Ok(id)
    }

    /// 预留一个对局级槽。
    pub fn reserve_battle_slot(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
    ) -> Result<BattleSlotId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.battle_slot_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = BattleSlotId(self.battle_slots.len() as u32);
        let spec = BattleSlotSpec {
            id,
            namespace,
            name,
            export_name,
        };
        self.battle_slot_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.battle_slots.push(spec);
        Ok(id)
    }

    /// 预留一个实体槽。
    pub fn reserve_entity_slot(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
    ) -> Result<EntitySlotId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.entity_slot_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = EntitySlotId(self.entity_slots.len() as u32);
        let spec = EntitySlotSpec {
            id,
            namespace,
            name,
            export_name,
        };
        self.entity_slot_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.entity_slots.push(spec);
        Ok(id)
    }

    /// 注册一个效果 handler。
    pub fn register_effect_handler(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        priority: SkillPriority,
    ) -> Result<EffectHandlerId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.effect_handler_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = EffectHandlerId(self.effect_handlers.len() as u32);
        let spec = EffectHandlerSpec {
            id,
            namespace,
            name,
            export_name,
            priority,
            registration_order: self.next_order(),
        };
        self.effect_handler_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.effect_handlers.push(spec);
        Ok(id)
    }

    /// 注册一个回放渲染器。
    pub fn register_replay_renderer(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        priority: SkillPriority,
    ) -> Result<ReplayRendererId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.replay_renderer_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = ReplayRendererId(self.replay_renderers.len() as u32);
        let spec = ReplayRendererSpec {
            id,
            namespace,
            name,
            export_name,
            priority,
            registration_order: self.next_order(),
        };
        self.replay_renderer_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.replay_renderers.push(spec);
        Ok(id)
    }

    /// 注册一个展示渲染器。
    pub fn register_show_renderer(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
        priority: SkillPriority,
    ) -> Result<ShowRendererId, ExtensionError> {
        let namespace = namespace.into();
        let name = name.into();
        let export_name = export_name.into();
        let name_key = (namespace.clone(), name.clone());

        if self.show_renderer_names.contains_key(&name_key) {
            return Err(ExtensionError::DuplicateName { namespace, name });
        }
        if self.export_names.contains_key(&export_name) {
            return Err(ExtensionError::DuplicateExportName { export_name });
        }

        let id = ShowRendererId(self.show_renderers.len() as u32);
        let spec = ShowRendererSpec {
            id,
            namespace,
            name,
            export_name,
            priority,
            registration_order: self.next_order(),
        };
        self.show_renderer_names.insert(name_key, id);
        self.export_names.insert(spec.export_name.clone(), ());
        self.show_renderers.push(spec);
        Ok(id)
    }

    /// 冻结注册表：解析内置主动技能映射与蓝图槽 ID。
    pub fn build(self) -> ExtensionRegistry {
        let builtin_active_skills = self
            .skills
            .iter()
            .map(|skill| BuiltinActiveSkill::from_export_name(&skill.export_name))
            .collect();
        let seed_blueprint_slots = [
            DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT,
            DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT,
            DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT,
        ]
        .map(|name| self.entity_slots.iter().find(|spec| spec.export_name == name).map(|spec| spec.id));
        ExtensionRegistry {
            seed_blueprint_slots,
            installed_extensions: self.installed_extensions,
            player_kinds: self.player_kinds,
            skills: self.skills,
            states: self.states,
            template_slots: self.template_slots,
            battle_slots: self.battle_slots,
            entity_slots: self.entity_slots,
            effect_handlers: self.effect_handlers,
            replay_renderers: self.replay_renderers,
            show_renderers: self.show_renderers,
            builtin_active_skills,
        }
    }

    /// 分配下一个注册序（单调递增，溢出回绕），供各类 spec 登记使用。
    fn next_order(&mut self) -> RegistrationOrder {
        let order = RegistrationOrder(self.next_registration_order);
        self.next_registration_order = self.next_registration_order.wrapping_add(1);
        order
    }
}

/// 冻结后的扩展注册表：按 ID 索引各类别 spec，并提供导出名查询。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtensionRegistry {
    installed_extensions: Vec<InstalledExtensionSpec>,
    player_kinds: Vec<PlayerKindSpec>,
    skills: Vec<SkillSpec>,
    states: Vec<StateSpec>,
    template_slots: Vec<TemplateSlotSpec>,
    battle_slots: Vec<BattleSlotSpec>,
    entity_slots: Vec<EntitySlotSpec>,
    effect_handlers: Vec<EffectHandlerSpec>,
    replay_renderers: Vec<ReplayRendererSpec>,
    show_renderers: Vec<ShowRendererSpec>,
    /// 构造期解析好的内置主动技能，战斗热路径按 `SkillId` 直接索引。
    builtin_active_skills: Vec<Option<BuiltinActiveSkill>>,
    /// 每局会同步队伍编号的内置蓝图槽；注册表构造后 ID 固定。
    seed_blueprint_slots: [Option<EntitySlotId>; 3],
}

impl ExtensionRegistry {
    pub fn seed_blueprint_slots(&self) -> [Option<EntitySlotId>; 3] { self.seed_blueprint_slots }

    /// 已安装扩展列表，按安装顺序排列。
    pub fn installed_extensions(&self) -> &[InstalledExtensionSpec] { &self.installed_extensions }

    /// 按 ID 查玩家类型；越界返回 None。
    pub fn player_kind(&self, id: PlayerKindId) -> Option<&PlayerKindSpec> { self.player_kinds.get(id.0 as usize) }

    /// 全部玩家类型，按注册顺序排列。
    pub fn player_kinds(&self) -> &[PlayerKindSpec] { &self.player_kinds }

    /// 按导出名查玩家类型。
    pub fn player_kind_by_export_name(&self, export_name: &str) -> Option<&PlayerKindSpec> {
        self.player_kinds.iter().find(|spec| spec.export_name == export_name)
    }

    /// 按导出名查玩家类型 ID。
    pub fn player_kind_id_by_export_name(&self, export_name: &str) -> Option<PlayerKindId> {
        self.player_kind_by_export_name(export_name).map(|spec| spec.id)
    }

    /// 按 ID 查技能；越界返回 None。
    pub fn skill(&self, id: SkillId) -> Option<&SkillSpec> { self.skills.get(id.0 as usize) }

    pub fn builtin_active_skill(&self, id: SkillId) -> Option<BuiltinActiveSkill> {
        self.builtin_active_skills.get(id.0 as usize).copied().flatten()
    }

    /// 全部技能，按注册顺序排列。
    pub fn skills(&self) -> &[SkillSpec] { &self.skills }

    /// 按 namespace 与 name 查技能。
    pub fn skill_by_name(&self, namespace: &str, name: &str) -> Option<&SkillSpec> {
        self.skills.iter().find(|spec| spec.namespace == namespace && spec.name == name)
    }

    /// 按 namespace 与 name 查技能 ID。
    pub fn skill_id_by_name(&self, namespace: &str, name: &str) -> Option<SkillId> {
        self.skill_by_name(namespace, name).map(|spec| spec.id)
    }

    /// 按导出名查技能。
    pub fn skill_by_export_name(&self, export_name: &str) -> Option<&SkillSpec> {
        self.skills.iter().find(|spec| spec.export_name == export_name)
    }

    /// 按导出名查技能 ID。
    pub fn skill_id_by_export_name(&self, export_name: &str) -> Option<SkillId> {
        self.skill_by_export_name(export_name).map(|spec| spec.id)
    }

    /// 按 ID 查状态；越界返回 None。
    pub fn state(&self, id: StateId) -> Option<&StateSpec> { self.states.get(id.0 as usize) }

    /// 全部状态，按注册顺序排列。
    pub fn states(&self) -> &[StateSpec] { &self.states }

    /// 按导出名查状态。
    pub fn state_by_export_name(&self, export_name: &str) -> Option<&StateSpec> {
        self.states.iter().find(|spec| spec.export_name == export_name)
    }

    /// 按导出名查状态 ID。
    pub fn state_id_by_export_name(&self, export_name: &str) -> Option<StateId> {
        self.state_by_export_name(export_name).map(|spec| spec.id)
    }

    /// 按 ID 查模板槽；越界返回 None。
    pub fn template_slot(&self, id: TemplateSlotId) -> Option<&TemplateSlotSpec> { self.template_slots.get(id.0 as usize) }

    /// 全部模板槽，按注册顺序排列。
    pub fn template_slots(&self) -> &[TemplateSlotSpec] { &self.template_slots }

    /// 按 ID 查对局级槽；越界返回 None。
    pub fn battle_slot(&self, id: BattleSlotId) -> Option<&BattleSlotSpec> { self.battle_slots.get(id.0 as usize) }

    /// 全部对局级槽，按注册顺序排列。
    pub fn battle_slots(&self) -> &[BattleSlotSpec] { &self.battle_slots }

    /// 按 ID 查实体槽；越界返回 None。
    pub fn entity_slot(&self, id: EntitySlotId) -> Option<&EntitySlotSpec> { self.entity_slots.get(id.0 as usize) }

    /// 按导出名查实体槽。
    pub fn entity_slot_by_export_name(&self, export_name: &str) -> Option<&EntitySlotSpec> {
        self.entity_slots.iter().find(|spec| spec.export_name == export_name)
    }

    /// 按导出名查实体槽 ID。
    pub fn entity_slot_id_by_export_name(&self, export_name: &str) -> Option<EntitySlotId> {
        self.entity_slot_by_export_name(export_name).map(|spec| spec.id)
    }

    /// 全部实体槽，按注册顺序排列。
    pub fn entity_slots(&self) -> &[EntitySlotSpec] { &self.entity_slots }

    /// 按 ID 查效果 handler；越界返回 None。
    pub fn effect_handler(&self, id: EffectHandlerId) -> Option<&EffectHandlerSpec> { self.effect_handlers.get(id.0 as usize) }

    /// 全部效果 handler，按注册顺序排列。
    pub fn effect_handlers(&self) -> &[EffectHandlerSpec] { &self.effect_handlers }

    /// 按 ID 查回放渲染器；越界返回 None。
    pub fn replay_renderer(&self, id: ReplayRendererId) -> Option<&ReplayRendererSpec> {
        self.replay_renderers.get(id.0 as usize)
    }

    /// 全部回放渲染器，按注册顺序排列。
    pub fn replay_renderers(&self) -> &[ReplayRendererSpec] { &self.replay_renderers }

    /// 按 ID 查展示渲染器；越界返回 None。
    pub fn show_renderer(&self, id: ShowRendererId) -> Option<&ShowRendererSpec> { self.show_renderers.get(id.0 as usize) }

    /// 全部展示渲染器，按注册顺序排列。
    pub fn show_renderers(&self) -> &[ShowRendererSpec] { &self.show_renderers }

    /// 按优先级与注册次序排序的技能列表。
    pub fn skills_in_hook_order(&self) -> Vec<&SkillSpec> {
        let mut specs: Vec<&SkillSpec> = self.skills.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    /// 按优先级与注册次序排序的状态列表。
    pub fn states_in_hook_order(&self) -> Vec<&StateSpec> {
        let mut specs: Vec<&StateSpec> = self.states.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    /// 按优先级与注册次序排序的效果 handler 链。
    pub fn effect_handlers_in_chain_order(&self) -> Vec<&EffectHandlerSpec> {
        let mut specs: Vec<&EffectHandlerSpec> = self.effect_handlers.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    /// 按优先级与注册次序排序的回放渲染器链。
    pub fn replay_renderers_in_order(&self) -> Vec<&ReplayRendererSpec> {
        let mut specs: Vec<&ReplayRendererSpec> = self.replay_renderers.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    /// 按优先级与注册次序排序的展示渲染器链。
    pub fn show_renderers_in_order(&self) -> Vec<&ShowRendererSpec> {
        let mut specs: Vec<&ShowRendererSpec> = self.show_renderers.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }
}

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::runtime::profile::BuiltinActiveSkill;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerKindId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkillId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StateId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TemplateSlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BattleSlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntitySlotId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EffectHandlerId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplayRendererId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShowRendererId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExtensionVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ExtensionVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self { Self { major, minor, patch } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtensionCapability {
    ReadAllies,
    ReadEnemies,
    ReadBattleSlots,
    ReadTemplateSlots,
    MutateEntitySlots,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SkillPriority(pub i32);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistrationOrder(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProcMask(pub u64);

impl ProcMask {
    pub const NONE: Self = Self(0);
    pub const PRE_ACTION: Self = Self(1 << 0);
    pub const POST_ACTION: Self = Self(1 << 1);
    pub const PRE_DAMAGE: Self = Self(1 << 2);
    pub const POST_DAMAGE: Self = Self(1 << 3);
    pub const PRE_DEFEND: Self = Self(1 << 4);
    pub const POST_DEFEND: Self = Self(1 << 5);
    pub const DIE: Self = Self(1 << 6);
    pub const KILL: Self = Self(1 << 7);

    pub const fn intersects(self, rhs: Self) -> bool { (self.0 & rhs.0) != 0 }

    pub const fn is_empty(self) -> bool { self.0 == 0 }
}

impl Default for ProcMask {
    fn default() -> Self { Self::NONE }
}

impl std::ops::BitOr for ProcMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for ProcMask {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TargetPolicy {
    #[default]
    None,
    Enemy,
    Ally,
    Any,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SkillPostActionPhase {
    #[default]
    Early,
    Late,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlayerKindFlags(pub u64);

impl PlayerKindFlags {
    pub const NONE: Self = Self(0);
    pub const BOSS: Self = Self(1 << 0);
    pub const MINION: Self = Self(1 << 1);
    pub const SUMMON: Self = Self(1 << 2);
    pub const BED2: Self = Self(1 << 3);
    pub const BOOST: Self = Self(1 << 4);
    pub const COMBAT_MINION: Self = Self(1 << 5);

    pub const fn contains(self, rhs: Self) -> bool { (self.0 & rhs.0) == rhs.0 }
    pub const fn intersects(self, rhs: Self) -> bool { (self.0 & rhs.0) != 0 }
}

impl std::ops::BitOr for PlayerKindFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for PlayerKindFlags {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OwnerResolutionPolicy {
    #[default]
    SelfEntity,
    RootOwner,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum DamageSharePolicy {
    #[default]
    None,
    ShareToOwner,
    ShareToSummons,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MergePolicy {
    None,
    #[default]
    FixedLane,
    DropUnmappedSkills,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerKindPolicies {
    pub owner_resolution: OwnerResolutionPolicy,
    pub damage_share: DamageSharePolicy,
    pub merge: MergePolicy,
    pub inherit_owner_def_res: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerKindSpec {
    pub id: PlayerKindId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub flags: PlayerKindFlags,
    pub policies: PlayerKindPolicies,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateSlotSpec {
    pub id: TemplateSlotId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleSlotSpec {
    pub id: BattleSlotId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitySlotSpec {
    pub id: EntitySlotId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectHandlerSpec {
    pub id: EffectHandlerId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayRendererSpec {
    pub id: ReplayRendererId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowRendererSpec {
    pub id: ShowRendererId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledExtensionSpec {
    pub name: String,
    pub version: ExtensionVersion,
    pub capabilities: Vec<ExtensionCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtensionError {
    DuplicateExtensionName { name: String },
    DuplicateName { namespace: String, name: String },
    DuplicateExportName { export_name: String },
}

impl Display for ExtensionError {
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

pub trait TswnExtension {
    fn name(&self) -> &'static str;

    fn version(&self) -> ExtensionVersion;

    fn capabilities(&self) -> &'static [ExtensionCapability] { &[] }

    fn register(&self, registry: &mut ExtensionRegistryBuilder) -> Result<(), ExtensionError>;
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
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

    pub fn build(self) -> ExtensionRegistry {
        let builtin_active_skills = self
            .skills
            .iter()
            .map(|skill| BuiltinActiveSkill::from_export_name(&skill.export_name))
            .collect();
        ExtensionRegistry {
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

    fn next_order(&mut self) -> RegistrationOrder {
        let order = RegistrationOrder(self.next_registration_order);
        self.next_registration_order = self.next_registration_order.wrapping_add(1);
        order
    }
}

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
}

impl ExtensionRegistry {
    pub fn installed_extensions(&self) -> &[InstalledExtensionSpec] { &self.installed_extensions }

    pub fn player_kind(&self, id: PlayerKindId) -> Option<&PlayerKindSpec> { self.player_kinds.get(id.0 as usize) }

    pub fn player_kinds(&self) -> &[PlayerKindSpec] { &self.player_kinds }

    pub fn player_kind_by_export_name(&self, export_name: &str) -> Option<&PlayerKindSpec> {
        self.player_kinds.iter().find(|spec| spec.export_name == export_name)
    }

    pub fn player_kind_id_by_export_name(&self, export_name: &str) -> Option<PlayerKindId> {
        self.player_kind_by_export_name(export_name).map(|spec| spec.id)
    }

    pub fn skill(&self, id: SkillId) -> Option<&SkillSpec> { self.skills.get(id.0 as usize) }

    pub(crate) fn builtin_active_skill(&self, id: SkillId) -> Option<BuiltinActiveSkill> {
        self.builtin_active_skills.get(id.0 as usize).copied().flatten()
    }

    pub fn skills(&self) -> &[SkillSpec] { &self.skills }

    pub fn skill_by_name(&self, namespace: &str, name: &str) -> Option<&SkillSpec> {
        self.skills.iter().find(|spec| spec.namespace == namespace && spec.name == name)
    }

    pub fn skill_id_by_name(&self, namespace: &str, name: &str) -> Option<SkillId> {
        self.skill_by_name(namespace, name).map(|spec| spec.id)
    }

    pub fn skill_by_export_name(&self, export_name: &str) -> Option<&SkillSpec> {
        self.skills.iter().find(|spec| spec.export_name == export_name)
    }

    pub fn skill_id_by_export_name(&self, export_name: &str) -> Option<SkillId> {
        self.skill_by_export_name(export_name).map(|spec| spec.id)
    }

    pub fn state(&self, id: StateId) -> Option<&StateSpec> { self.states.get(id.0 as usize) }

    pub fn states(&self) -> &[StateSpec] { &self.states }

    pub fn state_by_export_name(&self, export_name: &str) -> Option<&StateSpec> {
        self.states.iter().find(|spec| spec.export_name == export_name)
    }

    pub fn state_id_by_export_name(&self, export_name: &str) -> Option<StateId> {
        self.state_by_export_name(export_name).map(|spec| spec.id)
    }

    pub fn template_slot(&self, id: TemplateSlotId) -> Option<&TemplateSlotSpec> { self.template_slots.get(id.0 as usize) }

    pub fn template_slots(&self) -> &[TemplateSlotSpec] { &self.template_slots }

    pub fn battle_slot(&self, id: BattleSlotId) -> Option<&BattleSlotSpec> { self.battle_slots.get(id.0 as usize) }

    pub fn battle_slots(&self) -> &[BattleSlotSpec] { &self.battle_slots }

    pub fn entity_slot(&self, id: EntitySlotId) -> Option<&EntitySlotSpec> { self.entity_slots.get(id.0 as usize) }

    pub fn entity_slot_by_export_name(&self, export_name: &str) -> Option<&EntitySlotSpec> {
        self.entity_slots.iter().find(|spec| spec.export_name == export_name)
    }

    pub fn entity_slot_id_by_export_name(&self, export_name: &str) -> Option<EntitySlotId> {
        self.entity_slot_by_export_name(export_name).map(|spec| spec.id)
    }

    pub fn entity_slots(&self) -> &[EntitySlotSpec] { &self.entity_slots }

    pub fn effect_handler(&self, id: EffectHandlerId) -> Option<&EffectHandlerSpec> { self.effect_handlers.get(id.0 as usize) }

    pub fn effect_handlers(&self) -> &[EffectHandlerSpec] { &self.effect_handlers }

    pub fn replay_renderer(&self, id: ReplayRendererId) -> Option<&ReplayRendererSpec> {
        self.replay_renderers.get(id.0 as usize)
    }

    pub fn replay_renderers(&self) -> &[ReplayRendererSpec] { &self.replay_renderers }

    pub fn show_renderer(&self, id: ShowRendererId) -> Option<&ShowRendererSpec> { self.show_renderers.get(id.0 as usize) }

    pub fn show_renderers(&self) -> &[ShowRendererSpec] { &self.show_renderers }

    pub fn skills_in_hook_order(&self) -> Vec<&SkillSpec> {
        let mut specs: Vec<&SkillSpec> = self.skills.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    pub fn states_in_hook_order(&self) -> Vec<&StateSpec> {
        let mut specs: Vec<&StateSpec> = self.states.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    pub fn effect_handlers_in_chain_order(&self) -> Vec<&EffectHandlerSpec> {
        let mut specs: Vec<&EffectHandlerSpec> = self.effect_handlers.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    pub fn replay_renderers_in_order(&self) -> Vec<&ReplayRendererSpec> {
        let mut specs: Vec<&ReplayRendererSpec> = self.replay_renderers.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }

    pub fn show_renderers_in_order(&self) -> Vec<&ShowRendererSpec> {
        let mut specs: Vec<&ShowRendererSpec> = self.show_renderers.iter().collect();
        specs.sort_by_key(|spec| (spec.priority, spec.registration_order));
        specs
    }
}

#[cfg(test)]
mod tests;

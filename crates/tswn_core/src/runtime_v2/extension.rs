use std::collections::HashMap;
use std::fmt::{Display, Formatter};

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
    #[default]
    None,
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
}

impl ExtensionRegistry {
    pub fn installed_extensions(&self) -> &[InstalledExtensionSpec] { &self.installed_extensions }

    pub fn player_kind(&self, id: PlayerKindId) -> Option<&PlayerKindSpec> { self.player_kinds.get(id.0 as usize) }

    pub fn player_kinds(&self) -> &[PlayerKindSpec] { &self.player_kinds }

    pub fn skill(&self, id: SkillId) -> Option<&SkillSpec> { self.skills.get(id.0 as usize) }

    pub fn skills(&self) -> &[SkillSpec] { &self.skills }

    pub fn state(&self, id: StateId) -> Option<&StateSpec> { self.states.get(id.0 as usize) }

    pub fn states(&self) -> &[StateSpec] { &self.states }

    pub fn template_slot(&self, id: TemplateSlotId) -> Option<&TemplateSlotSpec> { self.template_slots.get(id.0 as usize) }

    pub fn template_slots(&self) -> &[TemplateSlotSpec] { &self.template_slots }

    pub fn battle_slot(&self, id: BattleSlotId) -> Option<&BattleSlotSpec> { self.battle_slots.get(id.0 as usize) }

    pub fn battle_slots(&self) -> &[BattleSlotSpec] { &self.battle_slots }

    pub fn entity_slot(&self, id: EntitySlotId) -> Option<&EntitySlotSpec> { self.entity_slots.get(id.0 as usize) }

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
mod tests {
    use super::*;

    struct FixtureExtension;

    impl TswnExtension for FixtureExtension {
        fn name(&self) -> &'static str { "fixture" }

        fn version(&self) -> ExtensionVersion { ExtensionVersion::new(1, 2, 3) }

        fn capabilities(&self) -> &'static [ExtensionCapability] {
            &[ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots]
        }

        fn register(&self, registry: &mut ExtensionRegistryBuilder) -> Result<(), ExtensionError> {
            registry.register_player_kind("fixture", "kind", "fixture.kind")?;
            registry.register_skill("fixture", "skill", "fixture.skill", TargetPolicy::Enemy, SkillPriority(3))?;
            registry.reserve_entity_slot("fixture", "flags", "fixture.flags")?;
            Ok(())
        }
    }

    #[test]
    fn registry_allocates_stable_player_kind_ids_in_registration_order() {
        let mut builder = ExtensionRegistryBuilder::default();

        let alpha = builder
            .register_player_kind("custom", "alpha", "custom.alpha")
            .expect("alpha should register");
        let beta = builder
            .register_player_kind("custom", "beta", "custom.beta")
            .expect("beta should register");

        assert_eq!(alpha, PlayerKindId(0));
        assert_eq!(beta, PlayerKindId(1));

        let registry = builder.build();
        assert_eq!(registry.player_kind(alpha).unwrap().name, "alpha");
        assert_eq!(registry.player_kind(beta).unwrap().export_name, "custom.beta");
        assert_eq!(registry.player_kind(alpha).unwrap().flags, PlayerKindFlags::NONE);
        assert_eq!(registry.player_kind(alpha).unwrap().policies, PlayerKindPolicies::default());
        assert_eq!(registry.player_kinds().len(), 2);
    }

    #[test]
    fn registry_stores_player_kind_policy_flags_without_kind_explosion() {
        let mut builder = ExtensionRegistryBuilder::default();

        let bed2 = builder
            .register_player_kind_with_policies(
                "custom",
                "bed2",
                "custom.bed2",
                PlayerKindFlags::BED2 | PlayerKindFlags::SUMMON,
                PlayerKindPolicies {
                    owner_resolution: OwnerResolutionPolicy::RootOwner,
                    damage_share: DamageSharePolicy::ShareToOwner,
                    merge: MergePolicy::FixedLane,
                    inherit_owner_def_res: true,
                },
            )
            .expect("bed2 kind should register");

        let registry = builder.build();
        let spec = registry.player_kind(bed2).expect("bed2 kind should exist");
        assert!(spec.flags.contains(PlayerKindFlags::BED2));
        assert!(spec.flags.contains(PlayerKindFlags::SUMMON));
        assert_eq!(spec.policies.owner_resolution, OwnerResolutionPolicy::RootOwner);
        assert_eq!(spec.policies.damage_share, DamageSharePolicy::ShareToOwner);
        assert_eq!(spec.policies.merge, MergePolicy::FixedLane);
        assert!(spec.policies.inherit_owner_def_res);
        assert_eq!(registry.player_kinds().len(), 1);
    }

    #[test]
    fn registry_allows_same_local_name_in_different_namespaces() {
        let mut builder = ExtensionRegistryBuilder::default();

        let custom = builder
            .register_player_kind("custom", "boss", "custom.boss")
            .expect("custom boss should register");
        let fixture = builder
            .register_player_kind("fixture", "boss", "fixture.boss")
            .expect("fixture boss should register");

        assert_eq!(custom, PlayerKindId(0));
        assert_eq!(fixture, PlayerKindId(1));
    }

    #[test]
    fn registry_rejects_duplicate_player_kind_name_in_namespace() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_player_kind("custom", "boss", "custom.boss")
            .expect("first boss should register");

        assert_eq!(
            builder.register_player_kind("custom", "boss", "custom.boss.v2"),
            Err(ExtensionError::DuplicateName {
                namespace: "custom".to_owned(),
                name: "boss".to_owned(),
            })
        );
    }

    #[test]
    fn registry_rejects_duplicate_export_name() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_player_kind("custom", "boss", "custom.boss")
            .expect("first boss should register");

        assert_eq!(
            builder.register_player_kind("fixture", "boss", "custom.boss"),
            Err(ExtensionError::DuplicateExportName {
                export_name: "custom.boss".to_owned(),
            })
        );
    }

    #[test]
    fn registry_stores_skill_and_state_specs() {
        let mut builder = ExtensionRegistryBuilder::default();

        let skill = builder
            .register_skill("custom", "fire", "custom.fire", TargetPolicy::Enemy, SkillPriority(10))
            .expect("skill should register");
        let state = builder
            .register_state(
                "custom",
                "burning",
                "custom.burning",
                ProcMask::POST_ACTION | ProcMask::POST_DAMAGE,
                SkillPriority(5),
            )
            .expect("state should register");

        assert_eq!(skill, SkillId(0));
        assert_eq!(state, StateId(0));

        let registry = builder.build();
        assert_eq!(registry.skill(skill).unwrap().target_policy, TargetPolicy::Enemy);
        assert_eq!(
            registry.state(state).unwrap().hook_mask,
            ProcMask::POST_ACTION | ProcMask::POST_DAMAGE
        );
    }

    #[test]
    fn registry_orders_skill_and_state_specs_by_priority_then_registration() {
        let mut builder = ExtensionRegistryBuilder::default();

        let late_skill = builder
            .register_skill("custom", "late", "custom.late", TargetPolicy::Enemy, SkillPriority(10))
            .expect("late skill should register");
        let early_skill = builder
            .register_skill("custom", "early", "custom.early", TargetPolicy::Enemy, SkillPriority(1))
            .expect("early skill should register");
        let tie_skill = builder
            .register_skill("custom", "tie", "custom.tie", TargetPolicy::Enemy, SkillPriority(10))
            .expect("tie skill should register");

        let late_state = builder
            .register_state(
                "custom",
                "late-state",
                "custom.late_state",
                ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("late state should register");
        let early_state = builder
            .register_state(
                "custom",
                "early-state",
                "custom.early_state",
                ProcMask::POST_ACTION,
                SkillPriority(1),
            )
            .expect("early state should register");
        let tie_state = builder
            .register_state(
                "custom",
                "tie-state",
                "custom.tie_state",
                ProcMask::POST_ACTION,
                SkillPriority(10),
            )
            .expect("tie state should register");

        let registry = builder.build();

        assert_eq!(
            registry.skills_in_hook_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
            vec![early_skill, late_skill, tie_skill]
        );
        assert_eq!(
            registry.states_in_hook_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
            vec![early_state, late_state, tie_state]
        );
    }

    #[test]
    fn registry_rejects_skill_and_state_name_and_export_collisions() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_skill("custom", "fire", "custom.fire", TargetPolicy::Enemy, SkillPriority(0))
            .expect("first skill should register");

        assert_eq!(
            builder.register_skill("custom", "fire", "custom.fire.v2", TargetPolicy::Enemy, SkillPriority(0)),
            Err(ExtensionError::DuplicateName {
                namespace: "custom".to_owned(),
                name: "fire".to_owned(),
            })
        );
        assert_eq!(
            builder.register_state("custom", "burning", "custom.fire", ProcMask::POST_ACTION, SkillPriority(0)),
            Err(ExtensionError::DuplicateExportName {
                export_name: "custom.fire".to_owned(),
            })
        );
    }

    #[test]
    fn registry_stores_template_battle_and_entity_slot_specs() {
        let mut builder = ExtensionRegistryBuilder::default();

        let template = builder
            .reserve_template_slot("custom", "template-config", "custom.template_config")
            .expect("template slot should reserve");
        let battle = builder
            .reserve_battle_slot("custom", "battle-cache", "custom.battle_cache")
            .expect("battle slot should reserve");
        let entity = builder
            .reserve_entity_slot("custom", "entity-flags", "custom.entity_flags")
            .expect("entity slot should reserve");

        assert_eq!(template, TemplateSlotId(0));
        assert_eq!(battle, BattleSlotId(0));
        assert_eq!(entity, EntitySlotId(0));

        let registry = builder.build();
        assert_eq!(registry.template_slot(template).unwrap().name, "template-config");
        assert_eq!(registry.battle_slot(battle).unwrap().export_name, "custom.battle_cache");
        assert_eq!(registry.entity_slot(entity).unwrap().namespace, "custom");
    }

    #[test]
    fn registry_rejects_slot_name_and_export_collisions() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .reserve_template_slot("custom", "config", "custom.config")
            .expect("template slot should reserve");

        assert_eq!(
            builder.reserve_template_slot("custom", "config", "custom.config.v2"),
            Err(ExtensionError::DuplicateName {
                namespace: "custom".to_owned(),
                name: "config".to_owned(),
            })
        );
        assert_eq!(
            builder.reserve_battle_slot("custom", "cache", "custom.config"),
            Err(ExtensionError::DuplicateExportName {
                export_name: "custom.config".to_owned(),
            })
        );
    }

    #[test]
    fn registry_stores_effect_replay_and_show_renderer_specs() {
        let mut builder = ExtensionRegistryBuilder::default();

        let effect = builder
            .register_effect_handler("custom", "summon-damage", "custom.summon_damage", SkillPriority(10))
            .expect("effect handler should register");
        let replay = builder
            .register_replay_renderer("custom", "hp-marker", "custom.hp_marker.replay", SkillPriority(5))
            .expect("replay renderer should register");
        let show = builder
            .register_show_renderer("custom", "hp-marker", "custom.hp_marker.show", SkillPriority(5))
            .expect("show renderer should register");

        assert_eq!(effect, EffectHandlerId(0));
        assert_eq!(replay, ReplayRendererId(0));
        assert_eq!(show, ShowRendererId(0));

        let registry = builder.build();
        assert_eq!(registry.effect_handler(effect).unwrap().name, "summon-damage");
        assert_eq!(registry.replay_renderer(replay).unwrap().export_name, "custom.hp_marker.replay");
        assert_eq!(registry.show_renderer(show).unwrap().namespace, "custom");
    }

    #[test]
    fn registry_orders_effect_and_renderer_specs_by_priority_then_registration() {
        let mut builder = ExtensionRegistryBuilder::default();

        let late_effect = builder
            .register_effect_handler("custom", "late-effect", "custom.late_effect", SkillPriority(10))
            .expect("late effect should register");
        let early_effect = builder
            .register_effect_handler("custom", "early-effect", "custom.early_effect", SkillPriority(1))
            .expect("early effect should register");
        let tie_effect = builder
            .register_effect_handler("custom", "tie-effect", "custom.tie_effect", SkillPriority(10))
            .expect("tie effect should register");

        let late_replay = builder
            .register_replay_renderer("custom", "late-replay", "custom.late_replay", SkillPriority(10))
            .expect("late replay should register");
        let early_replay = builder
            .register_replay_renderer("custom", "early-replay", "custom.early_replay", SkillPriority(1))
            .expect("early replay should register");
        let late_show = builder
            .register_show_renderer("custom", "late-show", "custom.late_show", SkillPriority(10))
            .expect("late show should register");
        let early_show = builder
            .register_show_renderer("custom", "early-show", "custom.early_show", SkillPriority(1))
            .expect("early show should register");

        let registry = builder.build();

        assert_eq!(
            registry
                .effect_handlers_in_chain_order()
                .into_iter()
                .map(|spec| spec.id)
                .collect::<Vec<_>>(),
            vec![early_effect, late_effect, tie_effect]
        );
        assert_eq!(
            registry.replay_renderers_in_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
            vec![early_replay, late_replay]
        );
        assert_eq!(
            registry.show_renderers_in_order().into_iter().map(|spec| spec.id).collect::<Vec<_>>(),
            vec![early_show, late_show]
        );
    }

    #[test]
    fn registry_rejects_renderer_name_and_export_collisions() {
        let mut builder = ExtensionRegistryBuilder::default();
        builder
            .register_replay_renderer("custom", "hp", "custom.hp.replay", SkillPriority(0))
            .expect("replay renderer should register");

        assert_eq!(
            builder.register_replay_renderer("custom", "hp", "custom.hp.replay.v2", SkillPriority(0)),
            Err(ExtensionError::DuplicateName {
                namespace: "custom".to_owned(),
                name: "hp".to_owned(),
            })
        );
        assert_eq!(
            builder.register_show_renderer("custom", "hp", "custom.hp.replay", SkillPriority(0)),
            Err(ExtensionError::DuplicateExportName {
                export_name: "custom.hp.replay".to_owned(),
            })
        );
    }

    #[test]
    fn registry_runs_extension_registration_flow() {
        let mut builder = ExtensionRegistryBuilder::default();

        builder.install_extension(&FixtureExtension).expect("fixture extension should install");

        let registry = builder.build();
        assert_eq!(
            registry.installed_extensions(),
            &[InstalledExtensionSpec {
                name: "fixture".to_owned(),
                version: ExtensionVersion::new(1, 2, 3),
                capabilities: vec![ExtensionCapability::ReadAllies, ExtensionCapability::MutateEntitySlots],
            }]
        );
        assert_eq!(registry.player_kinds()[0].export_name, "fixture.kind");
        assert_eq!(registry.skills()[0].priority, SkillPriority(3));
        assert_eq!(registry.entity_slots()[0].name, "flags");
    }

    #[test]
    fn registry_rejects_duplicate_extension_names() {
        let mut builder = ExtensionRegistryBuilder::default();

        builder.install_extension(&FixtureExtension).expect("first fixture should install");

        assert_eq!(
            builder.install_extension(&FixtureExtension),
            Err(ExtensionError::DuplicateExtensionName {
                name: "fixture".to_owned(),
            })
        );
    }
}

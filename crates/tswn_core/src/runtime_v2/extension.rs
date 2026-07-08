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
    pub const DIE: Self = Self(1 << 4);
    pub const KILL: Self = Self(1 << 5);
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerKindSpec {
    pub id: PlayerKindId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSpec {
    pub id: SkillId,
    pub namespace: String,
    pub name: String,
    pub export_name: String,
    pub target_policy: TargetPolicy,
    pub priority: SkillPriority,
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
pub enum ExtensionError {
    DuplicateName { namespace: String, name: String },
    DuplicateExportName { export_name: String },
}

impl Display for ExtensionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtensionRegistryBuilder {
    player_kinds: Vec<PlayerKindSpec>,
    skills: Vec<SkillSpec>,
    states: Vec<StateSpec>,
    template_slots: Vec<TemplateSlotSpec>,
    battle_slots: Vec<BattleSlotSpec>,
    entity_slots: Vec<EntitySlotSpec>,
    player_kind_names: HashMap<(String, String), PlayerKindId>,
    skill_names: HashMap<(String, String), SkillId>,
    state_names: HashMap<(String, String), StateId>,
    template_slot_names: HashMap<(String, String), TemplateSlotId>,
    battle_slot_names: HashMap<(String, String), BattleSlotId>,
    entity_slot_names: HashMap<(String, String), EntitySlotId>,
    export_names: HashMap<String, ()>,
    next_registration_order: u32,
}

impl ExtensionRegistryBuilder {
    pub fn register_player_kind(
        &mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        export_name: impl Into<String>,
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
            target_policy,
            priority,
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

    pub fn build(self) -> ExtensionRegistry {
        ExtensionRegistry {
            player_kinds: self.player_kinds,
            skills: self.skills,
            states: self.states,
            template_slots: self.template_slots,
            battle_slots: self.battle_slots,
            entity_slots: self.entity_slots,
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
    player_kinds: Vec<PlayerKindSpec>,
    skills: Vec<SkillSpec>,
    states: Vec<StateSpec>,
    template_slots: Vec<TemplateSlotSpec>,
    battle_slots: Vec<BattleSlotSpec>,
    entity_slots: Vec<EntitySlotSpec>,
}

impl ExtensionRegistry {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(registry.player_kinds().len(), 2);
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
}

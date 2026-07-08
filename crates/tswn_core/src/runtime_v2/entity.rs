use crate::runtime_v2::extension::{
    PlayerKindFlags, PlayerKindId, PlayerKindPolicies, ProcMask, RegistrationOrder, SkillId, SkillPriority, StateId,
};
use crate::runtime_v2::{EntitySlotStorage, ExtensionRegistry};
use smallvec::SmallVec;
use std::collections::HashMap;

use crate::player::PlrId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTemplate {
    pub id: PlrId,
    pub name: String,
    pub kind: PlayerKindId,
    pub skills: SkillLoadout,
    pub team: usize,
    pub max_hp: i32,
    pub attack: i32,
}

impl PlayerTemplate {
    pub const DEFAULT_KIND: PlayerKindId = PlayerKindId(u32::MAX);

    pub fn new(id: PlrId, name: impl Into<String>, team: usize, max_hp: i32, attack: i32) -> Self {
        Self::with_kind(id, name, Self::DEFAULT_KIND, team, max_hp, attack)
    }

    pub fn with_kind(id: PlrId, name: impl Into<String>, kind: PlayerKindId, team: usize, max_hp: i32, attack: i32) -> Self {
        assert!(max_hp > 0, "runtime_v2 player max_hp must be positive");
        assert!(attack >= 0, "runtime_v2 player attack must be non-negative");
        Self {
            id,
            name: name.into(),
            kind,
            skills: SkillLoadout::default(),
            team,
            max_hp,
            attack,
        }
    }

    pub fn with_skill_loadout(mut self, skills: SkillLoadout) -> Self {
        self.skills = skills;
        self
    }

    pub fn with_skills(self, skills: impl IntoIterator<Item = SkillId>) -> Self {
        self.with_skill_loadout(SkillLoadout::from_skills(skills))
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkillLoadout {
    skills: SmallVec<[SkillId; 8]>,
}

impl SkillLoadout {
    pub fn from_skills(skills: impl IntoIterator<Item = SkillId>) -> Self {
        Self {
            skills: skills.into_iter().collect(),
        }
    }

    pub fn skills(&self) -> &[SkillId] { &self.skills }

    pub fn is_empty(&self) -> bool { self.skills.is_empty() }

    pub fn len(&self) -> usize { self.skills.len() }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MoveState {
    pub speed_points: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRuntime {
    pub hp: i32,
    pub alive: bool,
    pub kind: PlayerKindId,
    pub owner: EntityIdx,
    pub root_owner: EntityIdx,
    pub team: usize,
    pub flags: PlayerKindFlags,
    pub policies: PlayerKindPolicies,
    pub move_state: MoveState,
}

impl PlayerRuntime {
    pub fn from_template(
        template: &PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: EntityIdx,
        root_owner: EntityIdx,
    ) -> Self {
        let (flags, policies) = registry
            .player_kind(template.kind)
            .map_or((PlayerKindFlags::NONE, PlayerKindPolicies::default()), |kind| {
                (kind.flags, kind.policies)
            });
        Self {
            hp: template.max_hp,
            alive: true,
            kind: template.kind,
            owner,
            root_owner,
            team: template.team,
            flags,
            policies,
            move_state: MoveState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRecord {
    pub template: PlayerTemplate,
    pub runtime: PlayerRuntime,
    pub states: StateStore,
    pub slots: EntitySlotStorage,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct EntityArena {
    entities: Vec<EntityRecord>,
}

impl EntityArena {
    pub fn from_templates(players: Vec<PlayerTemplate>) -> Self {
        Self::from_templates_with_registry(players, &ExtensionRegistry::default())
    }

    pub fn from_templates_with_registry(players: Vec<PlayerTemplate>, registry: &ExtensionRegistry) -> Self {
        let entities = players
            .into_iter()
            .enumerate()
            .map(|(idx, template)| {
                let owner = EntityIdx(idx as u32);
                let runtime = PlayerRuntime::from_template(&template, registry, owner, owner);
                EntityRecord {
                    template,
                    runtime,
                    states: StateStore::default(),
                    slots: EntitySlotStorage::from_registry(registry),
                }
            })
            .collect();
        Self { entities }
    }

    pub fn len(&self) -> usize { self.entities.len() }

    pub fn is_empty(&self) -> bool { self.entities.is_empty() }

    pub fn get(&self, idx: EntityIdx) -> Option<&EntityRecord> { self.entities.get(idx.0 as usize) }

    pub fn get_mut(&mut self, idx: EntityIdx) -> Option<&mut EntityRecord> { self.entities.get_mut(idx.0 as usize) }

    pub fn spawn_from_template(&mut self, template: PlayerTemplate, registry: &ExtensionRegistry) -> EntityIdx {
        self.spawn_from_template_with_owner(template, registry, None, None)
    }

    pub fn spawn_from_template_with_owner(
        &mut self,
        template: PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: Option<EntityIdx>,
        root_owner: Option<EntityIdx>,
    ) -> EntityIdx {
        let idx = EntityIdx(self.entities.len().try_into().expect("runtime_v2 entity index overflow"));
        let owner = owner.unwrap_or(idx);
        let root_owner = root_owner.unwrap_or(owner);
        let runtime = PlayerRuntime::from_template(&template, registry, owner, root_owner);
        self.entities.push(EntityRecord {
            template,
            runtime,
            states: StateStore::default(),
            slots: EntitySlotStorage::from_registry(registry),
        });
        idx
    }

    pub fn iter(&self) -> impl Iterator<Item = (EntityIdx, &EntityRecord)> {
        self.entities.iter().enumerate().map(|(idx, entity)| (EntityIdx(idx as u32), entity))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityIdx(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateEntry {
    pub legacy_order_key: u32,
    pub extension_state_id: Option<StateId>,
    pub hook_mask: ProcMask,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
}

impl StateEntry {
    pub fn legacy(legacy_order_key: u32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StateStore {
    entries: SmallVec<[StateEntry; 8]>,
    hook_mask: ProcMask,
    generation: u32,
    index: HashMap<u32, usize>,
}

impl StateStore {
    pub fn entries(&self) -> &[StateEntry] { &self.entries }

    pub fn hook_mask(&self) -> ProcMask { self.hook_mask }

    pub fn generation(&self) -> u32 { self.generation }

    pub fn entry(&self, legacy_order_key: u32) -> Option<&StateEntry> {
        self.index.get(&legacy_order_key).and_then(|idx| self.entries.get(*idx))
    }

    pub fn add_legacy_key(&mut self, legacy_order_key: u32) -> bool { self.add_entry(StateEntry::legacy(legacy_order_key)) }

    pub fn add_entry(&mut self, entry: StateEntry) -> bool {
        if self.index.contains_key(&entry.legacy_order_key) {
            return false;
        }

        self.index.insert(entry.legacy_order_key, self.entries.len());
        self.hook_mask |= entry.hook_mask;
        self.entries.push(entry);
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn clear_legacy_key(&mut self, legacy_order_key: u32) -> bool {
        let Some(idx) = self.index.get(&legacy_order_key).copied() else {
            return false;
        };

        self.entries.remove(idx);
        self.rebuild_index();
        self.rebuild_hook_mask();
        self.generation = self.generation.wrapping_add(1);
        true
    }

    pub fn entries_in_hook_order(&self) -> Vec<&StateEntry> {
        let mut entries: Vec<&StateEntry> = self.entries.iter().collect();
        entries.sort_by_key(|entry| (entry.priority, entry.registration_order));
        entries
    }

    fn rebuild_index(&mut self) {
        self.index.clear();
        for (idx, entry) in self.entries.iter().enumerate() {
            self.index.insert(entry.legacy_order_key, idx);
        }
    }

    fn rebuild_hook_mask(&mut self) {
        self.hook_mask = self.entries.iter().fold(ProcMask::default(), |mask, entry| mask | entry.hook_mask);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_records_start_with_empty_state_store() {
        let arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3)]);

        assert!(arena.get(EntityIdx(0)).unwrap().states.entries().is_empty());
        assert_eq!(arena.get(EntityIdx(0)).unwrap().states.hook_mask(), ProcMask::default());
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.kind, PlayerTemplate::DEFAULT_KIND);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.owner, EntityIdx(0));
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.root_owner, EntityIdx(0));
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.team, 0);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.flags, PlayerKindFlags::NONE);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.policies, PlayerKindPolicies::default());
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.move_state, MoveState::default());
        assert!(arena.get(EntityIdx(0)).unwrap().template.skills.is_empty());
    }

    #[test]
    fn player_template_stores_registered_skill_loadout() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill(
                "custom",
                "fire",
                "custom.fire",
                crate::runtime_v2::TargetPolicy::Enemy,
                SkillPriority(3),
            )
            .expect("skill should register");
        let registry = builder.build();
        let template = PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]);

        let arena = EntityArena::from_templates_with_registry(vec![template], &registry);

        assert_eq!(arena.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill]);
    }

    #[test]
    fn entity_arena_preserves_skill_loadout_when_spawning() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill(
                "custom",
                "summon-skill",
                "custom.summon_skill",
                crate::runtime_v2::TargetPolicy::Enemy,
                SkillPriority(1),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);

        let spawned = arena.spawn_from_template(PlayerTemplate::new(2, "spawned", 1, 7, 2).with_skills([skill]), &registry);

        assert_eq!(arena.get(spawned).unwrap().template.skills.skills(), &[skill]);
        assert_eq!(arena.get(spawned).unwrap().runtime.owner, spawned);
        assert_eq!(arena.get(spawned).unwrap().runtime.root_owner, spawned);
    }

    #[test]
    fn entity_records_copy_registered_player_kind_flags_into_runtime() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
                crate::runtime_v2::PlayerKindPolicies::default(),
            )
            .expect("kind should register");
        let registry = builder.build();

        let arena =
            EntityArena::from_templates_with_registry(vec![PlayerTemplate::with_kind(1, "summon", kind, 0, 10, 3)], &registry);

        let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;
        assert_eq!(runtime.kind, kind);
        assert_eq!(runtime.team, 0);
        assert!(runtime.flags.contains(PlayerKindFlags::SUMMON));
        assert!(runtime.flags.contains(PlayerKindFlags::MINION));
        assert_eq!(runtime.policies, crate::runtime_v2::PlayerKindPolicies::default());
    }

    #[test]
    fn entity_records_copy_registered_player_kind_policies_into_runtime() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let policies = crate::runtime_v2::PlayerKindPolicies {
            owner_resolution: crate::runtime_v2::OwnerResolutionPolicy::RootOwner,
            damage_share: crate::runtime_v2::DamageSharePolicy::ShareToOwner,
            merge: crate::runtime_v2::MergePolicy::FixedLane,
        };
        let kind = builder
            .register_player_kind_with_policies("custom", "summon", "custom.summon", PlayerKindFlags::SUMMON, policies)
            .expect("kind should register");
        let registry = builder.build();

        let arena =
            EntityArena::from_templates_with_registry(vec![PlayerTemplate::with_kind(1, "summon", kind, 0, 10, 3)], &registry);

        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.policies, policies);
    }

    #[test]
    fn entity_arena_spawns_with_owner_and_root_owner_metadata() {
        let registry = ExtensionRegistry::default();
        let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "owner", 0, 10, 3)], &registry);

        let spawned = arena.spawn_from_template_with_owner(
            PlayerTemplate::new(2, "spawned", 1, 7, 2),
            &registry,
            Some(EntityIdx(0)),
            Some(EntityIdx(0)),
        );

        let runtime = &arena.get(spawned).unwrap().runtime;
        assert_eq!(runtime.owner, EntityIdx(0));
        assert_eq!(runtime.root_owner, EntityIdx(0));
        assert_eq!(runtime.team, 1);
    }

    #[test]
    fn entity_records_reserve_registered_entity_slots() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let slot = builder
            .reserve_entity_slot("custom", "flag", "custom.flag")
            .expect("entity slot should reserve");
        let registry = builder.build();
        let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);

        arena
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(slot, crate::runtime_v2::SlotValue::Bool(true))
            .unwrap();

        assert_eq!(
            arena.get(EntityIdx(0)).unwrap().slots.get(slot),
            Some(&crate::runtime_v2::SlotValue::Bool(true))
        );
    }

    #[test]
    fn entity_arena_spawns_new_entity_without_reusing_indices() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let slot = builder
            .reserve_entity_slot("custom", "flag", "custom.flag")
            .expect("entity slot should reserve");
        let registry = builder.build();
        let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);

        let spawned = arena.spawn_from_template(PlayerTemplate::new(2, "spawned", 1, 7, 2), &registry);

        assert_eq!(spawned, EntityIdx(1));
        assert_eq!(arena.len(), 2);
        assert_eq!(arena.get(spawned).unwrap().template.name, "spawned");
        assert_eq!(arena.get(spawned).unwrap().runtime.hp, 7);
        assert_eq!(arena.get(spawned).unwrap().slots.get(slot), None);
    }

    #[test]
    fn state_store_updates_legacy_keys_and_generation() {
        let mut store = StateStore::default();

        assert!(store.add_legacy_key(11));
        assert!(!store.add_legacy_key(11));
        assert_eq!(store.generation(), 1);
        assert_eq!(store.entries(), &[StateEntry::legacy(11)]);
        assert_eq!(store.entry(11), Some(&StateEntry::legacy(11)));

        assert!(store.clear_legacy_key(11));
        assert!(!store.clear_legacy_key(11));
        assert_eq!(store.generation(), 2);
        assert!(store.entries().is_empty());
        assert_eq!(store.entry(11), None);
    }

    #[test]
    fn state_store_rebuilds_dense_index_after_clear() {
        let mut store = StateStore::default();
        store.add_legacy_key(11);
        store.add_legacy_key(22);
        store.add_legacy_key(33);

        assert!(store.clear_legacy_key(22));

        assert_eq!(store.entry(11), Some(&StateEntry::legacy(11)));
        assert_eq!(store.entry(22), None);
        assert_eq!(store.entry(33), Some(&StateEntry::legacy(33)));
    }

    #[test]
    fn state_store_tracks_v2_state_entry_metadata_and_hook_mask() {
        let mut store = StateStore::default();
        let entry = StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(StateId(3)),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_DAMAGE,
            priority: SkillPriority(9),
            registration_order: RegistrationOrder(4),
        };

        assert!(store.add_entry(entry));
        assert!(!store.add_entry(entry));
        assert_eq!(store.entries(), &[entry]);
        assert_eq!(store.entry(42), Some(&entry));
        assert_eq!(store.hook_mask(), ProcMask::PRE_ACTION | ProcMask::POST_DAMAGE);

        assert!(store.clear_legacy_key(42));
        assert!(store.entries().is_empty());
        assert_eq!(store.hook_mask(), ProcMask::default());
    }

    #[test]
    fn state_store_orders_entries_by_priority_then_registration() {
        let mut store = StateStore::default();
        let late = StateEntry {
            legacy_order_key: 11,
            extension_state_id: Some(StateId(1)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(10),
            registration_order: RegistrationOrder(1),
        };
        let early = StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(StateId(2)),
            hook_mask: ProcMask::PRE_ACTION,
            priority: SkillPriority(1),
            registration_order: RegistrationOrder(2),
        };
        let tie = StateEntry {
            legacy_order_key: 33,
            extension_state_id: Some(StateId(3)),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(10),
            registration_order: RegistrationOrder(3),
        };

        store.add_entry(late);
        store.add_entry(early);
        store.add_entry(tie);

        assert_eq!(
            store
                .entries_in_hook_order()
                .into_iter()
                .map(|entry| entry.legacy_order_key)
                .collect::<Vec<_>>(),
            vec![22, 11, 33]
        );
        assert_eq!(
            store.hook_mask(),
            ProcMask::PRE_ACTION | ProcMask::POST_ACTION | ProcMask::POST_DAMAGE
        );
    }
}

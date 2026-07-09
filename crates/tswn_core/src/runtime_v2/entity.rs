use crate::runtime_v2::extension::{
    DamageSharePolicy, MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, ProcMask,
    RegistrationOrder, SkillId, SkillPriority, StateId,
};
use crate::runtime_v2::{EntitySlotStorage, ExtensionRegistry};
use smallvec::SmallVec;
use std::collections::HashMap;

use crate::player::PlrId;
use crate::rc4::RC4;

const DEFAULT_AT_BOOST_MILLIONTHS: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTemplate {
    pub id: PlrId,
    pub name: String,
    pub kind: PlayerKindId,
    pub skills: SkillLoadout,
    pub team: usize,
    pub max_hp: i32,
    pub attack: i32,
    pub magic: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_millionths: i64,
    pub move_state: MoveState,
    pub policy_overrides: PlayerPolicyOverrides,
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
            magic: 0,
            defense: 0,
            resistance: 0,
            agility: 0,
            at_boost_millionths: DEFAULT_AT_BOOST_MILLIONTHS,
            move_state: MoveState::default(),
            policy_overrides: PlayerPolicyOverrides::default(),
        }
    }

    pub fn with_magic(mut self, magic: i32) -> Self {
        assert!(magic >= 0, "runtime_v2 player magic must be non-negative");
        self.magic = magic;
        self
    }

    pub fn with_at_boost_millionths(mut self, at_boost_millionths: i64) -> Self {
        assert!(
            at_boost_millionths >= 0,
            "runtime_v2 player at_boost_millionths must be non-negative"
        );
        self.at_boost_millionths = at_boost_millionths;
        self
    }

    pub fn with_def_res(mut self, defense: i32, resistance: i32) -> Self {
        assert!(defense >= 0, "runtime_v2 player defense must be non-negative");
        assert!(resistance >= 0, "runtime_v2 player resistance must be non-negative");
        self.defense = defense;
        self.resistance = resistance;
        self
    }

    pub fn with_agility(mut self, agility: i32) -> Self {
        assert!(agility >= 0, "runtime_v2 player agility must be non-negative");
        self.agility = agility;
        self
    }

    pub fn with_skill_loadout(mut self, skills: SkillLoadout) -> Self {
        self.skills = skills;
        self
    }

    pub fn with_skills(self, skills: impl IntoIterator<Item = SkillId>) -> Self {
        self.with_skill_loadout(SkillLoadout::from_skills(skills))
    }

    pub fn with_move_state(mut self, move_state: MoveState) -> Self {
        self.move_state = move_state;
        self
    }

    pub fn with_speed_points(mut self, speed_points: i32) -> Self {
        self.move_state.speed_points = speed_points;
        self
    }

    pub fn with_policy_overrides(mut self, policy_overrides: PlayerPolicyOverrides) -> Self {
        self.policy_overrides = policy_overrides;
        self
    }

    pub fn with_damage_share_policy(mut self, damage_share: DamageSharePolicy) -> Self {
        self.policy_overrides.damage_share = Some(damage_share);
        self
    }

    pub fn effective_policies(&self, registry: &ExtensionRegistry) -> PlayerKindPolicies {
        let policies = registry
            .player_kind(self.kind)
            .map_or(PlayerKindPolicies::default(), |kind| kind.policies);
        self.policy_overrides.apply_to(policies)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SkillLoadout {
    skills: SmallVec<[SkillId; 8]>,
    active_order: SmallVec<[usize; 8]>,
}

impl SkillLoadout {
    pub fn from_skills(skills: impl IntoIterator<Item = SkillId>) -> Self {
        let skills = skills.into_iter().collect::<SmallVec<[SkillId; 8]>>();
        let active_order = (0..skills.len()).collect();
        Self { skills, active_order }
    }

    pub fn skills(&self) -> &[SkillId] { &self.skills }

    pub fn active_order(&self) -> &[usize] { &self.active_order }

    pub fn is_empty(&self) -> bool { self.skills.is_empty() }

    pub fn len(&self) -> usize { self.skills.len() }

    pub fn with_active_order(mut self, active_order: impl IntoIterator<Item = usize>) -> Self {
        self.active_order = active_order.into_iter().collect();
        assert!(
            self.active_order.iter().all(|idx| *idx < self.skills.len()),
            "runtime_v2 skill active order must reference existing fixed lanes"
        );
        self
    }

    pub fn merge_fixed_lanes_from(&mut self, source: &Self, policy: MergePolicy) -> bool {
        let drop_unmapped = match policy {
            MergePolicy::None => return false,
            MergePolicy::FixedLane => false,
            MergePolicy::DropUnmappedSkills => true,
        };
        let mut changed = false;
        for (idx, source_skill) in source.skills.iter().copied().enumerate() {
            if let Some(target_skill) = self.skills.get_mut(idx) {
                if *target_skill != source_skill {
                    *target_skill = source_skill;
                    changed = true;
                }
            } else if !drop_unmapped {
                self.skills.push(source_skill);
                self.active_order.push(idx);
                changed = true;
            }
        }
        self.active_order.retain(|idx| *idx < self.skills.len());
        changed
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MoveState {
    pub speed_points: i32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PlayerPolicyOverrides {
    pub owner_resolution: Option<OwnerResolutionPolicy>,
    pub damage_share: Option<DamageSharePolicy>,
    pub merge: Option<MergePolicy>,
    pub inherit_owner_def_res: Option<bool>,
}

impl PlayerPolicyOverrides {
    pub fn apply_to(self, mut policies: PlayerKindPolicies) -> PlayerKindPolicies {
        if let Some(owner_resolution) = self.owner_resolution {
            policies.owner_resolution = owner_resolution;
        }
        if let Some(damage_share) = self.damage_share {
            policies.damage_share = damage_share;
        }
        if let Some(merge) = self.merge {
            policies.merge = merge;
        }
        if let Some(inherit_owner_def_res) = self.inherit_owner_def_res {
            policies.inherit_owner_def_res = inherit_owner_def_res;
        }
        policies
    }

    pub fn with_damage_share(mut self, damage_share: DamageSharePolicy) -> Self {
        self.damage_share = Some(damage_share);
        self
    }

    pub fn with_inherit_owner_def_res(mut self, inherit_owner_def_res: bool) -> Self {
        self.inherit_owner_def_res = Some(inherit_owner_def_res);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRuntime {
    pub hp: i32,
    pub alive: bool,
    pub attack: i32,
    pub magic: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_millionths: i64,
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
                (kind.flags, template.policy_overrides.apply_to(kind.policies))
            });
        Self {
            hp: template.max_hp,
            alive: true,
            attack: template.attack,
            magic: template.magic,
            defense: template.defense,
            resistance: template.resistance,
            agility: template.agility,
            at_boost_millionths: template.at_boost_millionths,
            kind: template.kind,
            owner,
            root_owner,
            team: template.team,
            flags,
            policies,
            move_state: template.move_state,
        }
    }

    pub fn at_boost(&self) -> f64 { self.at_boost_millionths as f64 / DEFAULT_AT_BOOST_MILLIONTHS as f64 }

    pub fn get_at(&self, use_mag: bool, randomer: &mut RC4) -> f64 {
        let atk = if use_mag { self.magic } else { self.attack };
        let a = {
            let mut temp = [
                randomer.r127() as i32,
                randomer.r127() as i32,
                randomer.r127() as i32,
                atk + 64,
                atk,
            ];
            temp.sort_unstable();
            temp[2] as f64
        };
        let b = {
            let mut temp = [randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64];
            temp.sort_unstable();
            temp[1] as f64
        };
        a * b * self.at_boost()
    }

    pub fn magic_defense(&self) -> i32 { self.resistance + 64 }

    pub fn magic_accuracy(&self) -> i32 { self.magic + self.agility }

    pub fn magic_dodge(&self) -> i32 { self.resistance + self.agility }

    pub fn dodge(accuracy: i32, dodge_value: i32, randomer: &mut RC4) -> bool {
        let chance = {
            let temp = 24 + dodge_value - accuracy;
            if temp < 7 {
                7
            } else if temp > 64 {
                temp / 4 + 48
            } else {
                temp
            }
        };

        randomer.next_u8() as i32 <= chance
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
        mut template: PlayerTemplate,
        registry: &ExtensionRegistry,
        owner: Option<EntityIdx>,
        root_owner: Option<EntityIdx>,
    ) -> EntityIdx {
        let idx = EntityIdx(self.entities.len().try_into().expect("runtime_v2 entity index overflow"));
        let owner = owner.unwrap_or(idx);
        let root_owner = root_owner.unwrap_or(owner);
        if let Some(owner_idx) = Some(owner).filter(|owner_idx| *owner_idx != idx) {
            let owner_entity = self
                .entities
                .get(owner_idx.0 as usize)
                .unwrap_or_else(|| panic!("unknown runtime_v2 spawn owner entity: {}", owner_idx.0));
            let policies = template.effective_policies(registry);
            if policies.inherit_owner_def_res {
                template.defense = owner_entity.runtime.defense;
                template.resistance = owner_entity.runtime.resistance;
            }
        }
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
    pub payload: StatePayload,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum StatePayload {
    #[default]
    None,
    FireMagHalfSteps(i32),
    ShieldValue(i32),
    Curse {
        prob: i32,
        multiply: i32,
    },
    Poison {
        caster: Option<u32>,
        target: Option<u32>,
        atp_bits: u64,
        count: i32,
    },
    Haste {
        faster: i32,
        step: i32,
    },
    Charm {
        group_id: usize,
        effective_team_idx: Option<usize>,
        source_team_idx: Option<usize>,
        target: Option<u32>,
        step: i32,
    },
    Slow {
        step: i32,
    },
    Iron {
        protect: i32,
        step: i32,
    },
}

impl StateEntry {
    pub fn legacy(legacy_order_key: u32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::None,
        }
    }

    pub fn fire_mag(legacy_order_key: u32, half_steps: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::FireMagHalfSteps(half_steps),
        }
    }

    pub fn shield(legacy_order_key: u32, state_id: StateId, shield: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::ShieldValue(shield),
        }
    }

    pub fn curse(legacy_order_key: u32, state_id: StateId, prob: i32, multiply: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Curse { prob, multiply },
        }
    }

    pub fn poison(
        legacy_order_key: u32,
        state_id: StateId,
        caster: Option<u32>,
        target: Option<u32>,
        atp: f64,
        count: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Poison {
                caster,
                target,
                atp_bits: atp.to_bits(),
                count,
            },
        }
    }

    pub fn haste(legacy_order_key: u32, state_id: StateId, faster: i32, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Haste { faster, step },
        }
    }

    pub fn charm(
        legacy_order_key: u32,
        state_id: StateId,
        group_id: usize,
        effective_team_idx: Option<usize>,
        source_team_idx: Option<usize>,
        target: Option<u32>,
        step: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            },
        }
    }

    pub fn slow(legacy_order_key: u32, state_id: StateId, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Slow { step },
        }
    }

    pub fn iron(legacy_order_key: u32, state_id: StateId, protect: i32, step: i32, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Iron { protect, step },
        }
    }

    pub fn fire_mag_value(&self) -> Option<f64> {
        match self.payload {
            StatePayload::FireMagHalfSteps(half_steps) => Some(f64::from(half_steps) * 0.5),
            StatePayload::None
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. } => None,
        }
    }

    pub fn shield_value(&self) -> Option<i32> {
        match self.payload {
            StatePayload::ShieldValue(shield) => Some(shield),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. } => None,
        }
    }

    pub fn haste_value(&self) -> Option<(i32, i32)> {
        match self.payload {
            StatePayload::Haste { faster, step } => Some((faster, step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. } => None,
        }
    }

    pub fn poison_value(&self) -> Option<(Option<u32>, Option<u32>, f64, i32)> {
        match self.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                count,
            } => Some((caster, target, f64::from_bits(atp_bits), count)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. } => None,
        }
    }

    pub fn charm_value(&self) -> Option<(usize, Option<usize>, Option<usize>, Option<u32>, i32)> {
        match self.payload {
            StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            } => Some((group_id, effective_team_idx, source_team_idx, target, step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. } => None,
        }
    }

    pub fn slow_value(&self) -> Option<i32> {
        match self.payload {
            StatePayload::Slow { step } => Some(step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Iron { .. } => None,
        }
    }

    pub fn iron_value(&self) -> Option<(i32, i32)> {
        match self.payload {
            StatePayload::Iron { protect, step } => Some((protect, step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. } => None,
            StatePayload::Poison { .. } | StatePayload::Haste { .. } | StatePayload::Charm { .. } | StatePayload::Slow { .. } => {
                None
            }
        }
    }

    pub fn priority_for_hook(&self, hook: ProcMask) -> SkillPriority {
        match self.payload {
            StatePayload::Poison { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(150),
            StatePayload::Haste { .. } | StatePayload::Charm { .. } | StatePayload::Slow { .. }
                if hook.intersects(ProcMask::POST_ACTION) =>
            {
                SkillPriority(210)
            }
            StatePayload::Iron { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(210),
            _ => self.priority,
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

    pub fn entry_mut(&mut self, legacy_order_key: u32) -> Option<&mut StateEntry> {
        let idx = self.index.get(&legacy_order_key).copied()?;
        self.entries.get_mut(idx)
    }

    pub fn fire_mag(&self, legacy_order_key: u32) -> f64 {
        self.entry(legacy_order_key).and_then(StateEntry::fire_mag_value).unwrap_or(0.0)
    }

    pub fn add_fire_mag_half_step(&mut self, legacy_order_key: u32) {
        if let Some(idx) = self.index.get(&legacy_order_key).copied()
            && let Some(entry) = self.entries.get_mut(idx)
        {
            match &mut entry.payload {
                StatePayload::FireMagHalfSteps(half_steps) => {
                    *half_steps += 1;
                }
                StatePayload::None | StatePayload::ShieldValue(_) | StatePayload::Curse { .. } | StatePayload::Iron { .. } => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
                StatePayload::Poison { .. }
                | StatePayload::Haste { .. }
                | StatePayload::Charm { .. }
                | StatePayload::Slow { .. } => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
            }
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        self.add_entry(StateEntry::fire_mag(legacy_order_key, 1));
    }

    pub fn set_shield_value(&mut self, legacy_order_key: u32, shield: i32) -> bool {
        self.set_payload(legacy_order_key, StatePayload::ShieldValue(shield.max(0)))
    }

    pub fn set_payload(&mut self, legacy_order_key: u32, payload: StatePayload) -> bool {
        let Some(entry) = self.entry_mut(legacy_order_key) else {
            return false;
        };
        entry.payload = payload;
        self.generation = self.generation.wrapping_add(1);
        true
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

    pub fn entries_in_hook_order_for(&self, hook: ProcMask) -> Vec<&StateEntry> {
        let mut entries: Vec<&StateEntry> = self.entries.iter().filter(|entry| entry.hook_mask.intersects(hook)).collect();
        entries.sort_by_key(|entry| (entry.priority_for_hook(hook), entry.registration_order));
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
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.attack, 3);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.magic, 0);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.agility, 0);
        assert_eq!(
            arena.get(EntityIdx(0)).unwrap().runtime.at_boost_millionths,
            DEFAULT_AT_BOOST_MILLIONTHS
        );
        assert!(arena.get(EntityIdx(0)).unwrap().template.skills.is_empty());
    }

    #[test]
    fn player_runtime_get_at_matches_legacy_rng_formula_for_magic() {
        let arena = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3)
                .with_magic(80)
                .with_at_boost_millionths(1_500_000),
        ]);
        let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;
        let mut rng = RC4::default();
        let mut expected_rng = RC4::default();

        let a = {
            let mut temp = [
                expected_rng.r127() as i32,
                expected_rng.r127() as i32,
                expected_rng.r127() as i32,
                80 + 64,
                80,
            ];
            temp.sort_unstable();
            temp[2] as f64
        };
        let b = {
            let mut temp = [expected_rng.r63() as i32 + 64, expected_rng.r63() as i32 + 64, 80 + 64];
            temp.sort_unstable();
            temp[1] as f64
        };
        let expected = a * b * 1.5;

        assert_eq!(runtime.get_at(true, &mut rng), expected);
        assert_eq!(rng.i, expected_rng.i);
        assert_eq!(rng.j, expected_rng.j);
        assert_eq!(rng.main_val, expected_rng.main_val);
    }

    #[test]
    fn player_runtime_dodge_matches_legacy_rng_formula() {
        let cases = [(64, 64), (200, 0), (0, 256), (80, 512)];

        for (accuracy, dodge_value) in cases {
            let mut runtime_rng = RC4::default();
            let mut legacy_rng = RC4::default();

            for _ in 0..8 {
                assert_eq!(
                    PlayerRuntime::dodge(accuracy, dodge_value, &mut runtime_rng),
                    crate::player::Player::dodge(accuracy, dodge_value, &mut legacy_rng)
                );
                assert_eq!(runtime_rng.i, legacy_rng.i);
                assert_eq!(runtime_rng.j, legacy_rng.j);
                assert_eq!(runtime_rng.main_val, legacy_rng.main_val);
            }
        }
    }

    #[test]
    fn player_template_carries_move_state_into_runtime() {
        let template = PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048);

        let arena = EntityArena::from_templates(vec![template.clone()]);

        assert_eq!(arena.get(EntityIdx(0)).unwrap().template.move_state, template.move_state);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.move_state, template.move_state);
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
    fn skill_loadout_tracks_fixed_lanes_and_active_order_separately() {
        let loadout = SkillLoadout::from_skills([SkillId(1), SkillId(2), SkillId(3)]).with_active_order([2, 0, 1]);

        assert_eq!(loadout.skills(), &[SkillId(1), SkillId(2), SkillId(3)]);
        assert_eq!(loadout.active_order(), &[2, 0, 1]);
    }

    #[test]
    fn skill_loadout_merges_fixed_lanes_and_appends_unmapped_skills() {
        let mut target = SkillLoadout::from_skills([SkillId(1), SkillId(2)]);
        let source = SkillLoadout::from_skills([SkillId(1), SkillId(3), SkillId(4)]);

        assert!(target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));

        assert_eq!(target.skills(), &[SkillId(1), SkillId(3), SkillId(4)]);
        assert_eq!(target.active_order(), &[0, 1, 2]);
    }

    #[test]
    fn skill_loadout_drops_unmapped_merge_skills() {
        let mut target = SkillLoadout::from_skills([SkillId(1), SkillId(2)]);
        let source = SkillLoadout::from_skills([SkillId(3), SkillId(4), SkillId(5)]);

        assert!(target.merge_fixed_lanes_from(&source, MergePolicy::DropUnmappedSkills));

        assert_eq!(target.skills(), &[SkillId(3), SkillId(4)]);
    }

    #[test]
    fn skill_loadout_ignores_none_merge_policy() {
        let mut target = SkillLoadout::from_skills([SkillId(1)]);
        let source = SkillLoadout::from_skills([SkillId(2)]);

        assert!(!target.merge_fixed_lanes_from(&source, MergePolicy::None));

        assert_eq!(target.skills(), &[SkillId(1)]);
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
    fn entity_arena_preserves_move_state_when_spawning() {
        let registry = ExtensionRegistry::default();
        let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);
        let payload = PlayerTemplate::new(2, "spawned", 1, 7, 2).with_speed_points(-2048);

        let spawned = arena.spawn_from_template(payload.clone(), &registry);

        assert_eq!(arena.get(spawned).unwrap().template.move_state, payload.move_state);
        assert_eq!(arena.get(spawned).unwrap().runtime.move_state, payload.move_state);
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
            inherit_owner_def_res: false,
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
    fn player_template_policy_overrides_update_runtime_policy_fields() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let policies = crate::runtime_v2::PlayerKindPolicies {
            owner_resolution: crate::runtime_v2::OwnerResolutionPolicy::RootOwner,
            damage_share: crate::runtime_v2::DamageSharePolicy::ShareToOwner,
            merge: crate::runtime_v2::MergePolicy::FixedLane,
            inherit_owner_def_res: true,
        };
        let kind = builder
            .register_player_kind_with_policies("custom", "summon", "custom.summon", PlayerKindFlags::SUMMON, policies)
            .expect("kind should register");
        let registry = builder.build();

        let arena = EntityArena::from_templates_with_registry(
            vec![
                PlayerTemplate::with_kind(1, "summon", kind, 0, 10, 3)
                    .with_damage_share_policy(crate::runtime_v2::DamageSharePolicy::None),
            ],
            &registry,
        );

        let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;
        assert_eq!(
            runtime.policies.owner_resolution,
            crate::runtime_v2::OwnerResolutionPolicy::RootOwner
        );
        assert_eq!(runtime.policies.damage_share, crate::runtime_v2::DamageSharePolicy::None);
        assert_eq!(runtime.policies.merge, crate::runtime_v2::MergePolicy::FixedLane);
        assert!(runtime.policies.inherit_owner_def_res);
    }

    #[test]
    fn entity_arena_uses_policy_overrides_when_spawning() {
        let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
        let kind = builder
            .register_player_kind_with_policies(
                "custom",
                "summon",
                "custom.summon",
                PlayerKindFlags::SUMMON,
                crate::runtime_v2::PlayerKindPolicies {
                    owner_resolution: crate::runtime_v2::OwnerResolutionPolicy::SelfEntity,
                    damage_share: crate::runtime_v2::DamageSharePolicy::None,
                    merge: crate::runtime_v2::MergePolicy::None,
                    inherit_owner_def_res: false,
                },
            )
            .expect("kind should register");
        let registry = builder.build();
        let mut arena = EntityArena::from_templates_with_registry(
            vec![PlayerTemplate::new(1, "owner", 0, 10, 3).with_def_res(77, 88)],
            &registry,
        );

        let spawned = arena.spawn_from_template_with_owner(
            PlayerTemplate::with_kind(2, "summon", kind, 0, 7, 2)
                .with_policy_overrides(PlayerPolicyOverrides::default().with_inherit_owner_def_res(true)),
            &registry,
            Some(EntityIdx(0)),
            Some(EntityIdx(0)),
        );

        assert_eq!(arena.get(spawned).unwrap().template.defense, 77);
        assert_eq!(arena.get(spawned).unwrap().template.resistance, 88);
        assert!(arena.get(spawned).unwrap().runtime.policies.inherit_owner_def_res);
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
            payload: StatePayload::None,
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
            payload: StatePayload::None,
        };
        let early = StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(StateId(2)),
            hook_mask: ProcMask::PRE_ACTION,
            priority: SkillPriority(1),
            registration_order: RegistrationOrder(2),
            payload: StatePayload::None,
        };
        let tie = StateEntry {
            legacy_order_key: 33,
            extension_state_id: Some(StateId(3)),
            hook_mask: ProcMask::POST_DAMAGE,
            priority: SkillPriority(10),
            registration_order: RegistrationOrder(3),
            payload: StatePayload::None,
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
            store
                .entries_in_hook_order_for(ProcMask::POST_ACTION)
                .into_iter()
                .map(|entry| entry.legacy_order_key)
                .collect::<Vec<_>>(),
            vec![11]
        );
        assert_eq!(
            store.hook_mask(),
            ProcMask::PRE_ACTION | ProcMask::POST_ACTION | ProcMask::POST_DAMAGE
        );
    }

    #[test]
    fn state_store_uses_hook_specific_priority_for_iron() {
        let mut store = StateStore::default();
        store.add_entry(StateEntry::iron(11, StateId(1), 500, 3, SkillPriority(10)));
        store.add_entry(StateEntry {
            legacy_order_key: 22,
            extension_state_id: Some(StateId(2)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(100),
            registration_order: RegistrationOrder(1),
            payload: StatePayload::None,
        });

        assert_eq!(
            store
                .entries_in_hook_order_for(ProcMask::POST_DEFEND)
                .into_iter()
                .map(|entry| (entry.legacy_order_key, entry.priority_for_hook(ProcMask::POST_DEFEND)))
                .collect::<Vec<_>>(),
            [(11, SkillPriority(10))].into_iter().collect::<Vec<_>>()
        );
        assert_eq!(
            store
                .entries_in_hook_order_for(ProcMask::POST_ACTION)
                .into_iter()
                .map(|entry| (entry.legacy_order_key, entry.priority_for_hook(ProcMask::POST_ACTION)))
                .collect::<Vec<_>>(),
            vec![(22, SkillPriority(100)), (11, SkillPriority(210))]
        );
    }

    #[test]
    fn state_store_tracks_fire_mag_payload_as_half_steps() {
        let mut store = StateStore::default();

        assert_eq!(store.fire_mag(91), 0.0);
        assert!(store.add_entry(StateEntry::fire_mag(91, 3)));

        assert_eq!(store.entry(91).and_then(StateEntry::fire_mag_value), Some(1.5));
        assert_eq!(store.fire_mag(91), 1.5);
    }

    #[test]
    fn state_store_adds_or_increments_fire_mag_half_steps() {
        let mut store = StateStore::default();

        store.add_fire_mag_half_step(91);
        assert_eq!(store.fire_mag(91), 0.5);
        assert_eq!(store.generation(), 1);

        store.add_fire_mag_half_step(91);
        assert_eq!(store.fire_mag(91), 1.0);
        assert_eq!(store.generation(), 2);

        assert!(store.add_legacy_key(22));
        store.add_fire_mag_half_step(22);
        assert_eq!(store.fire_mag(22), 0.5);
    }
}

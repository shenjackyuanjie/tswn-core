use crate::runtime_v2::extension::{
    DamageSharePolicy, MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, ProcMask,
    RegistrationOrder, SkillId, SkillPriority, StateId,
};
use crate::runtime_v2::{EntitySlotStorage, ExtensionRegistry};
use smallvec::SmallVec;
use std::collections::HashMap;

use crate::player::{MOVE_POINT_THRESHOLD, PlayerStatus, PlrId, skill::SkillBoost};
use crate::rc4::RC4;

const DEFAULT_AT_BOOST_MILLIONTHS: i64 = 1_000_000;
const CLONE_ATTR_DECAY: f64 = 0.7799999713897705;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CloneStatAdjustments {
    max_hp: i32,
    attack: i32,
    magic: i32,
    wisdom: i32,
    speed: i32,
    defense: i32,
    resistance: i32,
    agility: i32,
    at_boost_millionths: i64,
    attr_sum: i64,
    atk_sum: i32,
    attract_delta_bits: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CloneDerivedStats {
    pub max_hp: i32,
    pub attack: i32,
    pub magic: i32,
    pub wisdom: i32,
    pub speed: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_millionths: i64,
    pub attr_sum: u32,
    pub atk_sum: i32,
    pub attract_bits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloneBuildData {
    attrs: [u32; 8],
    weapon_attr_bonus: [i32; 8],
    name_factor_bits: u64,
    adjustments: CloneStatAdjustments,
}

impl CloneBuildData {
    pub fn from_legacy(attrs: [u32; 8], weapon_attr_bonus: [i32; 8], name_factor: f64, status: &PlayerStatus) -> Self {
        let raw = Self::derive_raw(attrs, name_factor);
        Self {
            attrs,
            weapon_attr_bonus,
            name_factor_bits: name_factor.to_bits(),
            adjustments: CloneStatAdjustments {
                max_hp: status.max_hp - raw.max_hp,
                attack: status.attack - raw.attack,
                magic: status.magic - raw.magic,
                wisdom: status.wisdom - raw.wisdom,
                speed: status.speed - raw.speed,
                defense: status.defense - raw.defense,
                resistance: status.resistance - raw.resistance,
                agility: status.agility - raw.agility,
                at_boost_millionths: (status.at_boost * 1_000_000.0).round() as i64 - raw.at_boost_millionths,
                attr_sum: i64::from(status.attr_sum) - i64::from(raw.attr_sum),
                atk_sum: status.atk_sum - raw.atk_sum,
                attract_delta_bits: (status.attract - f64::from_bits(raw.attract_bits)).to_bits(),
            },
        }
    }

    pub fn decay_owner(&mut self) {
        for attr in &mut self.attrs[..7] {
            *attr = ((*attr as f64) * CLONE_ATTR_DECAY).ceil() as u32;
        }
        self.attrs[7] = ((self.attrs[7] as f64) * 0.5).ceil() as u32;
    }

    pub fn child(&self) -> Self {
        let mut child = self.clone();
        for (attr, bonus) in child.attrs.iter_mut().zip(child.weapon_attr_bonus) {
            *attr = (*attr as i32 + bonus) as u32;
        }
        child
    }

    pub fn merge_attrs_from(&mut self, source: &Self) -> bool {
        let mut changed = false;
        for (owner_attr, source_attr) in self.attrs.iter_mut().zip(source.attrs) {
            if source_attr > *owner_attr {
                *owner_attr = source_attr;
                changed = true;
            }
        }
        changed
    }

    pub fn derive_stats(&self) -> CloneDerivedStats {
        let raw = Self::derive_raw(self.attrs, f64::from_bits(self.name_factor_bits));
        let attr_sum = i64::from(raw.attr_sum) + self.adjustments.attr_sum;
        let attract = f64::from_bits(raw.attract_bits) + f64::from_bits(self.adjustments.attract_delta_bits);
        CloneDerivedStats {
            max_hp: raw.max_hp + self.adjustments.max_hp,
            attack: raw.attack + self.adjustments.attack,
            magic: raw.magic + self.adjustments.magic,
            wisdom: raw.wisdom + self.adjustments.wisdom,
            speed: raw.speed + self.adjustments.speed,
            defense: raw.defense + self.adjustments.defense,
            resistance: raw.resistance + self.adjustments.resistance,
            agility: raw.agility + self.adjustments.agility,
            at_boost_millionths: raw.at_boost_millionths + self.adjustments.at_boost_millionths,
            attr_sum: attr_sum.try_into().expect("runtime_v2 clone attr_sum became negative"),
            atk_sum: raw.atk_sum + self.adjustments.atk_sum,
            attract_bits: attract.to_bits(),
        }
    }

    fn derive_raw(attrs: [u32; 8], name_factor: f64) -> CloneDerivedStats {
        let scale = |value: u32, divisor: f64| ((value as f64) * (1.0 - name_factor / divisor)).round() as i32;
        let attr_sum = attrs[..7].iter().sum();
        let atk_sum = (attrs[0] as i32 - attrs[1] as i32 + attrs[2] as i32 + attrs[4] as i32 - attrs[5] as i32) * 2
            + attrs[3] as i32
            + attrs[6] as i32;
        CloneDerivedStats {
            max_hp: attrs[7] as i32,
            attack: scale(attrs[0], 128.0),
            magic: scale(attrs[4], 128.0),
            wisdom: scale(attrs[6], 80.0),
            speed: scale(attrs[2], 128.0) + 160,
            defense: scale(attrs[1], 128.0),
            resistance: scale(attrs[5], 128.0),
            agility: scale(attrs[3], 128.0),
            at_boost_millionths: DEFAULT_AT_BOOST_MILLIONTHS,
            attr_sum,
            atk_sum,
            attract_bits: 32768.0_f64.to_bits(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTemplate {
    pub id: PlrId,
    pub name: String,
    pub display_name: String,
    pub kind: PlayerKindId,
    pub skills: SkillLoadout,
    pub team: usize,
    pub max_hp: i32,
    pub attack: i32,
    pub magic: i32,
    pub magic_point: i32,
    pub wisdom: i32,
    pub speed: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_millionths: i64,
    pub attr_sum: u32,
    pub atk_sum: i32,
    pub attract_bits: u64,
    pub move_state: MoveState,
    pub policy_overrides: PlayerPolicyOverrides,
    pub clone_build: Option<CloneBuildData>,
}

impl PlayerTemplate {
    pub const DEFAULT_KIND: PlayerKindId = PlayerKindId(u32::MAX);

    pub fn new(id: PlrId, name: impl Into<String>, team: usize, max_hp: i32, attack: i32) -> Self {
        Self::with_kind(id, name, Self::DEFAULT_KIND, team, max_hp, attack)
    }

    pub fn with_kind(id: PlrId, name: impl Into<String>, kind: PlayerKindId, team: usize, max_hp: i32, attack: i32) -> Self {
        assert!(max_hp > 0, "runtime_v2 player max_hp must be positive");
        assert!(attack >= 0, "runtime_v2 player attack must be non-negative");
        let name = name.into();
        Self {
            id,
            display_name: name.clone(),
            name,
            kind,
            skills: SkillLoadout::default(),
            team,
            max_hp,
            attack,
            magic: 0,
            magic_point: 0,
            wisdom: 0,
            speed: 0,
            defense: 0,
            resistance: 0,
            agility: 0,
            at_boost_millionths: DEFAULT_AT_BOOST_MILLIONTHS,
            attr_sum: 0,
            atk_sum: attack,
            attract_bits: 32768.0_f64.to_bits(),
            move_state: MoveState::default(),
            policy_overrides: PlayerPolicyOverrides::default(),
            clone_build: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    pub fn with_magic(mut self, magic: i32) -> Self {
        assert!(magic >= 0, "runtime_v2 player magic must be non-negative");
        self.magic = magic;
        self
    }

    pub fn with_magic_point(mut self, magic_point: i32) -> Self {
        self.magic_point = magic_point;
        self
    }

    pub fn with_wisdom(mut self, wisdom: i32) -> Self {
        assert!(wisdom >= 0, "runtime_v2 player wisdom must be non-negative");
        self.wisdom = wisdom;
        self
    }

    pub fn with_speed(mut self, speed: i32) -> Self {
        assert!(speed >= 0, "runtime_v2 player speed must be non-negative");
        self.speed = speed;
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

    pub fn with_target_score_stats(mut self, attr_sum: u32, atk_sum: i32, attract: f64) -> Self {
        assert!(attract.is_finite(), "runtime_v2 player attract must be finite");
        self.attr_sum = attr_sum;
        self.atk_sum = atk_sum;
        self.attract_bits = attract.to_bits();
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
    levels: SmallVec<[u32; 8]>,
    boosts: SmallVec<[Option<SkillBoost>; 8]>,
    fixed_lane_keys: SmallVec<[usize; 8]>,
    active_order: SmallVec<[usize; 8]>,
    pre_action_order: SmallVec<[usize; 8]>,
}

impl SkillLoadout {
    pub fn from_skills(skills: impl IntoIterator<Item = SkillId>) -> Self {
        let skills = skills.into_iter().collect::<SmallVec<[SkillId; 8]>>();
        let levels = std::iter::repeat_n(1, skills.len()).collect();
        let boosts = std::iter::repeat_n(None, skills.len()).collect();
        let fixed_lane_keys = (0..skills.len()).collect();
        let active_order = (0..skills.len()).collect();
        Self {
            skills,
            levels,
            boosts,
            fixed_lane_keys,
            active_order,
            pre_action_order: SmallVec::new(),
        }
    }

    pub fn from_skill_levels(skills: impl IntoIterator<Item = (SkillId, u32)>) -> Self {
        let (skills, levels): (SmallVec<[SkillId; 8]>, SmallVec<[u32; 8]>) = skills.into_iter().unzip();
        let boosts = std::iter::repeat_n(None, skills.len()).collect();
        let fixed_lane_keys = (0..skills.len()).collect();
        let active_order = (0..skills.len()).collect();
        Self {
            skills,
            levels,
            boosts,
            fixed_lane_keys,
            active_order,
            pre_action_order: SmallVec::new(),
        }
    }

    pub fn from_skill_levels_and_boosts(skills: impl IntoIterator<Item = (SkillId, u32, Option<SkillBoost>)>) -> Self {
        let mut skill_ids = SmallVec::new();
        let mut levels = SmallVec::new();
        let mut boosts = SmallVec::new();
        for (skill_id, level, boost) in skills {
            skill_ids.push(skill_id);
            levels.push(level);
            boosts.push(boost);
        }
        let fixed_lane_keys = (0..skill_ids.len()).collect();
        let active_order = (0..skill_ids.len()).collect();
        Self {
            skills: skill_ids,
            levels,
            boosts,
            fixed_lane_keys,
            active_order,
            pre_action_order: SmallVec::new(),
        }
    }

    pub fn skills(&self) -> &[SkillId] { &self.skills }

    pub fn levels(&self) -> &[u32] { &self.levels }

    pub fn level_at(&self, fixed_lane: usize) -> Option<u32> { self.levels.get(fixed_lane).copied() }

    pub fn boost_at(&self, fixed_lane: usize) -> Option<&SkillBoost> { self.boosts.get(fixed_lane).and_then(Option::as_ref) }

    pub fn fixed_lane_key_at(&self, fixed_lane: usize) -> Option<usize> { self.fixed_lane_keys.get(fixed_lane).copied() }

    pub fn set_level_at(&mut self, fixed_lane: usize, level: u32) -> bool {
        let Some(current) = self.levels.get_mut(fixed_lane) else {
            return false;
        };
        *current = level;
        true
    }

    pub fn active_order(&self) -> &[usize] { &self.active_order }

    pub fn pre_action_order(&self) -> &[usize] { &self.pre_action_order }

    pub fn is_empty(&self) -> bool { self.skills.is_empty() }

    pub fn len(&self) -> usize { self.skills.len() }

    pub fn reapply_clone_boosts(&mut self) {
        for (level, boost) in self.levels.iter_mut().zip(&self.boosts) {
            let Some(boost) = boost else {
                continue;
            };
            if *level >= boost.final_level() {
                continue;
            }
            *level = match boost {
                SkillBoost::Normal(_) => *level,
                SkillBoost::LastBoost(_) => level.saturating_mul(2),
                SkillBoost::SlotBoost { boost, .. } => level.saturating_add((*boost).min(*level)),
            };
        }
    }

    pub fn with_active_order(mut self, active_order: impl IntoIterator<Item = usize>) -> Self {
        self.active_order = active_order.into_iter().collect();
        assert!(
            self.active_order.iter().all(|idx| *idx < self.skills.len()),
            "runtime_v2 skill active order must reference existing fixed lanes"
        );
        self
    }

    pub fn with_pre_action_order(mut self, pre_action_order: impl IntoIterator<Item = usize>) -> Self {
        self.pre_action_order = pre_action_order.into_iter().collect();
        assert!(
            self.pre_action_order.iter().all(|idx| *idx < self.skills.len()),
            "runtime_v2 skill pre-action order must reference existing fixed lanes"
        );
        self
    }

    pub fn ensure_pre_action_lane(&mut self, fixed_lane: usize) {
        assert!(
            fixed_lane < self.skills.len(),
            "runtime_v2 skill pre-action order must reference existing fixed lanes"
        );
        if !self.pre_action_order.contains(&fixed_lane) {
            self.pre_action_order.push(fixed_lane);
        }
    }

    pub fn remove_pre_action_lane(&mut self, fixed_lane: usize) { self.pre_action_order.retain(|lane| *lane != fixed_lane); }

    pub fn with_fixed_lane_keys(mut self, fixed_lane_keys: impl IntoIterator<Item = usize>) -> Self {
        self.fixed_lane_keys = fixed_lane_keys.into_iter().collect();
        assert_eq!(
            self.fixed_lane_keys.len(),
            self.skills.len(),
            "runtime_v2 fixed lane keys must match skill loadout length"
        );
        self
    }

    pub fn merge_fixed_lanes_from(&mut self, source: &Self, policy: MergePolicy) -> bool {
        match policy {
            MergePolicy::None => return false,
            MergePolicy::FixedLane | MergePolicy::DropUnmappedSkills => {}
        }
        let mut changed = false;
        for owner_idx in 0..self.levels.len() {
            let fixed_lane_key = self.fixed_lane_keys[owner_idx];
            let Some(source_idx) = source.fixed_lane_keys.iter().position(|source_key| *source_key == fixed_lane_key) else {
                continue;
            };
            let owner_level = &mut self.levels[owner_idx];
            let source_level = source.levels[source_idx];
            if source_level > *owner_level {
                let was_zero = *owner_level == 0;
                *owner_level = source_level;
                if was_zero && !self.active_order.contains(&owner_idx) {
                    self.active_order.push(owner_idx);
                }
                changed = true;
            }
        }
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
pub struct ProtectLinkRuntime {
    pub owner: EntityIdx,
    pub level: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HideRuntime {
    pub level: u32,
    pub attract_bits: u64,
    pub agility: i32,
    pub defense: i32,
    pub resistance: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssassinateRuntime {
    pub fixed_lane: usize,
    pub target: EntityIdx,
    pub break_on_damage: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CounterRuntime {
    pub pending: bool,
    pub last_target: Option<EntityIdx>,
    pub last_updates_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRuntime {
    pub hp: i32,
    pub alive: bool,
    pub attack: i32,
    pub magic: i32,
    pub magic_point: i32,
    pub wisdom: i32,
    pub speed: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_millionths: i64,
    pub attr_sum: u32,
    pub atk_sum: i32,
    pub attract_bits: u64,
    pub kind: PlayerKindId,
    pub owner: EntityIdx,
    pub root_owner: EntityIdx,
    pub team: usize,
    pub flags: PlayerKindFlags,
    pub policies: PlayerKindPolicies,
    pub move_state: MoveState,
    pub charge: ChargeRuntime,
    pub accumulate: AccumulateRuntime,
    pub shield: i32,
    pub protect_to: Option<EntityIdx>,
    pub protect_from: Vec<ProtectLinkRuntime>,
    pub upgrade_active: bool,
    pub hide: Option<HideRuntime>,
    pub assassinate: Option<AssassinateRuntime>,
    pub counter: CounterRuntime,
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
            magic_point: template.magic_point,
            wisdom: template.wisdom,
            speed: template.speed,
            defense: template.defense,
            resistance: template.resistance,
            agility: template.agility,
            at_boost_millionths: template.at_boost_millionths,
            attr_sum: template.attr_sum,
            atk_sum: template.atk_sum,
            attract_bits: template.attract_bits,
            kind: template.kind,
            owner,
            root_owner,
            team: template.team,
            flags,
            policies,
            move_state: template.move_state,
            charge: ChargeRuntime::default(),
            accumulate: AccumulateRuntime::default(),
            shield: 0,
            protect_to: None,
            protect_from: Vec::new(),
            upgrade_active: false,
            hide: None,
            assassinate: None,
            counter: CounterRuntime::default(),
        }
    }

    pub fn at_boost(&self) -> f64 { self.at_boost_millionths as f64 / DEFAULT_AT_BOOST_MILLIONTHS as f64 }

    pub fn attract(&self) -> f64 { f64::from_bits(self.attract_bits) }

    pub fn active(&self) -> bool { self.alive && self.hp > 0 }

    pub fn mp_ready(&mut self, randomer: &mut RC4) -> bool {
        if !self.active() {
            return false;
        }
        let require_mp = randomer.r3x3() as i32;
        if self.magic_point < require_mp {
            return false;
        }
        self.magic_point -= require_mp;
        true
    }

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ChargeRuntime {
    pub active: bool,
    pub post_action_active: bool,
    pub step: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccumulateRuntime {
    pub active: bool,
    pub acc_bits: u64,
    pub charge_bonus_bits: u64,
}

impl Default for AccumulateRuntime {
    fn default() -> Self {
        Self {
            active: false,
            acc_bits: 1.7000000476837158_f64.to_bits(),
            charge_bonus_bits: 0.0_f64.to_bits(),
        }
    }
}

impl AccumulateRuntime {
    pub fn acc(self) -> f64 { f64::from_bits(self.acc_bits) }

    pub fn charge_bonus(self) -> f64 { f64::from_bits(self.charge_bonus_bits) }

    fn set_acc(&mut self, acc: f64) { self.acc_bits = acc.to_bits(); }

    fn set_charge_bonus(&mut self, charge_bonus: f64) { self.charge_bonus_bits = charge_bonus.to_bits(); }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRecord {
    pub template: PlayerTemplate,
    pub runtime: PlayerRuntime,
    pub states: StateStore,
    pub slots: EntitySlotStorage,
}

impl EntityRecord {
    #[inline]
    pub fn is_active(&self) -> bool { self.runtime.active() && !self.states.is_frozen() }

    pub fn apply_derived_stats(&mut self, stats: CloneDerivedStats) {
        self.template.max_hp = stats.max_hp.max(1);
        self.template.attack = stats.attack.max(0);
        self.template.magic = stats.magic.max(0);
        self.template.wisdom = stats.wisdom.max(0);
        self.template.speed = stats.speed.max(0);
        self.template.defense = stats.defense.max(0);
        self.template.resistance = stats.resistance.max(0);
        self.template.agility = stats.agility.max(0);
        self.template.at_boost_millionths = stats.at_boost_millionths.max(0);
        self.template.attr_sum = stats.attr_sum;
        self.template.atk_sum = stats.atk_sum;
        self.template.attract_bits = stats.attract_bits;
        self.runtime.attack = self.template.attack;
        self.runtime.magic = self.template.magic;
        self.runtime.wisdom = self.template.wisdom;
        self.runtime.speed = self.template.speed;
        self.runtime.defense = self.template.defense;
        self.runtime.resistance = self.template.resistance;
        self.runtime.agility = self.template.agility;
        self.runtime.attr_sum = self.template.attr_sum;
        self.runtime.atk_sum = self.template.atk_sum;
        self.runtime.attract_bits = self.template.attract_bits;
        self.refresh_runtime_at_boost();
    }

    pub fn activate_charge_runtime(&mut self) {
        self.runtime.charge.step += 2;
        self.runtime.charge.active = true;
        self.runtime.charge.post_action_active = true;
        self.refresh_runtime_at_boost();
    }

    pub fn tick_charge_post_action(&mut self) -> bool {
        if !self.runtime.charge.post_action_active {
            return false;
        }

        self.runtime.charge.step -= 1;
        if self.runtime.charge.step <= 0 {
            self.runtime.charge.active = false;
            self.runtime.charge.post_action_active = false;
            self.refresh_runtime_at_boost();
        }
        true
    }

    pub fn clear_charge_runtime(&mut self) -> bool {
        if !self.runtime.charge.active {
            return false;
        }

        self.runtime.charge.active = false;
        self.runtime.charge.post_action_active = false;
        self.refresh_runtime_at_boost();
        true
    }

    pub fn activate_accumulate_runtime(&mut self) -> bool {
        if self.runtime.accumulate.active {
            return false;
        }

        let charge_active = self.runtime.at_boost_millionths >= 3_000_000;
        self.runtime.accumulate.active = true;
        self.runtime.accumulate.set_charge_bonus(if charge_active { 1.0 } else { 0.0 });
        if charge_active {
            self.runtime.move_state.speed_points += 500;
        }
        self.refresh_runtime_at_boost();
        self.runtime.move_state.speed_points += 400;
        true
    }

    pub fn clear_accumulate_runtime(&mut self) -> bool {
        if !self.runtime.accumulate.active {
            return false;
        }

        self.runtime.accumulate.active = false;
        self.runtime.accumulate.set_acc(1.600000023841858);
        self.runtime.accumulate.set_charge_bonus(0.0);
        self.refresh_runtime_at_boost();
        true
    }

    pub fn clear_positive_runtime_messages(&mut self) -> Vec<(i32, &'static str)> {
        let mut messages = Vec::new();
        if self.clear_accumulate_runtime() {
            messages.push((100, "[1]的[聚气]被打消了"));
        }
        if self.clear_charge_runtime() {
            messages.push((200, "[1]的[蓄力]被中止了"));
        }
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    pub fn clear_positive_messages(&mut self) -> Vec<(i32, &'static str)> {
        let mut messages = self.clear_positive_runtime_messages();
        messages.extend(self.states.clear_positive_states_with_ordered_messages(self.runtime.alive));
        messages.sort_unstable_by_key(|(priority, _)| *priority);
        messages
    }

    fn refresh_runtime_at_boost(&mut self) {
        let mut at_boost = self.template.at_boost_millionths as f64 / DEFAULT_AT_BOOST_MILLIONTHS as f64;
        if self.runtime.charge.active {
            at_boost *= 3.0;
        }
        if self.runtime.accumulate.active {
            at_boost *= self.runtime.accumulate.acc() + self.runtime.accumulate.charge_bonus();
        }
        self.runtime.at_boost_millionths = (at_boost * DEFAULT_AT_BOOST_MILLIONTHS as f64).round() as i64;
    }
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
            // Legacy `queue_spawn(owner, child)` assigns the child to the
            // owner's current world group. Blueprint teams are import-time
            // placeholders and must not decide runtime ownership.
            template.team = owner_entity.runtime.team;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEntry {
    pub legacy_order_key: u32,
    pub extension_state_id: Option<StateId>,
    pub hook_mask: ProcMask,
    pub priority: SkillPriority,
    pub registration_order: RegistrationOrder,
    pub payload: StatePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CovidInfectionEntry {
    pub boss: EntityIdx,
    pub mutation: i32,
    pub days: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum StatePayload {
    #[default]
    None,
    FireMagHalfSteps(i32),
    Ice {
        frozen_step: i32,
    },
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
    Berserk {
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
    CovidBoss {
        mutation: i32,
    },
    CovidInfection {
        entries: SmallVec<[CovidInfectionEntry; 2]>,
        mutation_set: SmallVec<[i32; 4]>,
        recovered: bool,
    },
    SaitamaBoss {
        turns: i32,
        damages: i32,
        hitters: SmallVec<[EntityIdx; 8]>,
        minions: SmallVec<[EntityIdx; 8]>,
    },
    LazyBoss {
        at_boost_bits: u64,
    },
    LazyInfection {
        boss: EntityIdx,
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

    pub fn ice(legacy_order_key: u32, frozen_step: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Ice { frozen_step },
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

    pub fn berserk(legacy_order_key: u32, step: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::default(),
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::Berserk { step },
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

    pub fn covid_boss(legacy_order_key: u32, mutation: i32) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::CovidBoss { mutation },
        }
    }

    pub fn covid_infection(
        legacy_order_key: u32,
        state_id: StateId,
        boss: EntityIdx,
        mutation: i32,
        priority: SkillPriority,
    ) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::CovidInfection {
                entries: SmallVec::from_slice(&[CovidInfectionEntry { boss, mutation, days: 0 }]),
                mutation_set: SmallVec::from_slice(&[mutation]),
                recovered: false,
            },
        }
    }

    pub fn lazy_boss(legacy_order_key: u32, at_boost: f64) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority::default(),
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::LazyBoss {
                at_boost_bits: at_boost.to_bits(),
            },
        }
    }

    pub fn saitama_boss(legacy_order_key: u32, state_id: StateId, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::POST_DEFEND,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::SaitamaBoss {
                turns: 0,
                damages: 0,
                hitters: SmallVec::new(),
                minions: SmallVec::new(),
            },
        }
    }

    pub fn lazy_infection(legacy_order_key: u32, state_id: StateId, boss: EntityIdx, priority: SkillPriority) -> Self {
        Self {
            legacy_order_key,
            extension_state_id: Some(state_id),
            hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_ACTION,
            priority,
            registration_order: RegistrationOrder::default(),
            payload: StatePayload::LazyInfection { boss },
        }
    }

    pub fn fire_mag_value(&self) -> Option<f64> {
        match &self.payload {
            StatePayload::FireMagHalfSteps(half_steps) => Some(f64::from(*half_steps) * 0.5),
            StatePayload::None
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn shield_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::ShieldValue(shield) => Some(*shield),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn haste_value(&self) -> Option<(i32, i32)> {
        match &self.payload {
            StatePayload::Haste { faster, step } => Some((*faster, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn poison_value(&self) -> Option<(Option<u32>, Option<u32>, f64, i32)> {
        match &self.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                count,
            } => Some((*caster, *target, f64::from_bits(*atp_bits), *count)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn charm_value(&self) -> Option<(usize, Option<usize>, Option<usize>, Option<u32>, i32)> {
        match &self.payload {
            StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            } => Some((*group_id, *effective_team_idx, *source_team_idx, *target, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn slow_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::Slow { step } => Some(*step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn ice_value(&self) -> Option<i32> {
        match &self.payload {
            StatePayload::Ice { frozen_step } => Some(*frozen_step),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::Iron { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn iron_value(&self) -> Option<(i32, i32)> {
        match &self.payload {
            StatePayload::Iron { protect, step } => Some((*protect, *step)),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. } => None,
            StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn priority_for_hook(&self, hook: ProcMask) -> SkillPriority {
        match &self.payload {
            StatePayload::Poison { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(150),
            StatePayload::Haste { .. } | StatePayload::Charm { .. } | StatePayload::Slow { .. }
                if hook.intersects(ProcMask::POST_ACTION) =>
            {
                SkillPriority(210)
            }
            StatePayload::Iron { .. } if hook.intersects(ProcMask::POST_ACTION) => SkillPriority(210),
            StatePayload::CovidInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            StatePayload::LazyInfection { .. } if hook.intersects(ProcMask::PRE_ACTION | ProcMask::POST_ACTION) => {
                SkillPriority(1000)
            }
            _ => self.priority,
        }
    }

    pub fn positive_clear_message(&self, owner_alive: bool) -> Option<(i32, &'static str)> {
        match &self.payload {
            StatePayload::Haste { .. } if owner_alive => Some((300, "[1]从[疾走]中解除")),
            StatePayload::Iron { .. } => Some((400, "[1]的[铁壁]被打消了")),
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::ShieldValue(_)
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Haste { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => None,
        }
    }

    pub fn is_positive_state(&self) -> bool {
        match &self.payload {
            StatePayload::ShieldValue(shield) => *shield > 0,
            StatePayload::Haste { .. } => true,
            StatePayload::Iron { step, .. } => *step > 0,
            StatePayload::None
            | StatePayload::FireMagHalfSteps(_)
            | StatePayload::Ice { .. }
            | StatePayload::Curse { .. }
            | StatePayload::Poison { .. }
            | StatePayload::Berserk { .. }
            | StatePayload::Charm { .. }
            | StatePayload::Slow { .. }
            | StatePayload::CovidBoss { .. }
            | StatePayload::CovidInfection { .. }
            | StatePayload::SaitamaBoss { .. }
            | StatePayload::LazyBoss { .. }
            | StatePayload::LazyInfection { .. } => false,
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

    pub fn is_frozen(&self) -> bool { self.entries.iter().any(|entry| matches!(&entry.payload, StatePayload::Ice { .. })) }

    pub fn effective_speed(&self, base_speed: i32) -> i32 {
        let mut speed = base_speed;
        for entry in &self.entries {
            match &entry.payload {
                StatePayload::Haste { faster, .. } => speed *= *faster,
                StatePayload::Slow { .. } => speed /= 2,
                StatePayload::LazyInfection { .. } => speed /= 2,
                _ => {}
            }
        }
        speed
    }

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

    pub fn ice_frozen_step(&self, legacy_order_key: u32) -> Option<i32> {
        self.entry(legacy_order_key).and_then(StateEntry::ice_value)
    }

    pub fn add_ice_frozen_step(&mut self, legacy_order_key: u32, frozen_step: i32) {
        if let Some(entry) = self.entry_mut(legacy_order_key) {
            let current = match &entry.payload {
                StatePayload::Ice { frozen_step } => *frozen_step,
                _ => 0,
            };
            entry.payload = StatePayload::Ice {
                frozen_step: current + frozen_step,
            };
            self.generation = self.generation.wrapping_add(1);
            return;
        }

        self.add_entry(StateEntry::ice(legacy_order_key, frozen_step));
    }

    pub fn apply_ice_pre_step(&mut self, step: i32, move_points: i32) -> (i32, bool) {
        let Some((legacy_order_key, frozen_step)) = self.entries.iter_mut().find_map(|entry| {
            if let StatePayload::Ice { frozen_step } = &mut entry.payload {
                Some((entry.legacy_order_key, frozen_step))
            } else {
                None
            }
        }) else {
            return (step, false);
        };

        if *frozen_step > 0 {
            if step != 0 {
                *frozen_step -= step;
                self.generation = self.generation.wrapping_add(1);
            }
            return (0, false);
        }
        if step + move_points >= MOVE_POINT_THRESHOLD {
            assert!(
                self.clear_legacy_key(legacy_order_key),
                "runtime_v2 ice state disappeared during pre-step"
            );
            return (0, true);
        }
        (step, false)
    }

    pub fn add_fire_mag_half_step(&mut self, legacy_order_key: u32) {
        if let Some(idx) = self.index.get(&legacy_order_key).copied()
            && let Some(entry) = self.entries.get_mut(idx)
        {
            match &mut entry.payload {
                StatePayload::FireMagHalfSteps(half_steps) => {
                    *half_steps += 1;
                }
                StatePayload::None
                | StatePayload::Ice { .. }
                | StatePayload::ShieldValue(_)
                | StatePayload::Curse { .. }
                | StatePayload::Iron { .. }
                | StatePayload::CovidBoss { .. }
                | StatePayload::CovidInfection { .. }
                | StatePayload::SaitamaBoss { .. }
                | StatePayload::LazyBoss { .. }
                | StatePayload::LazyInfection { .. } => {
                    entry.payload = StatePayload::FireMagHalfSteps(1);
                }
                StatePayload::Poison { .. }
                | StatePayload::Haste { .. }
                | StatePayload::Berserk { .. }
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

    pub fn clear_positive_states_with_ordered_messages(&mut self, owner_alive: bool) -> Vec<(i32, &'static str)> {
        let mut messages = Vec::new();
        let mut to_remove = Vec::new();

        for entry in &self.entries {
            if entry.is_positive_state() {
                if let Some(message) = entry.positive_clear_message(owner_alive) {
                    messages.push((message.0, entry.registration_order, entry.legacy_order_key, message.1));
                }
                to_remove.push(entry.legacy_order_key);
            }
        }

        messages.sort_unstable_by(|(priority_a, order_a, key_a, _), (priority_b, order_b, key_b, _)| {
            priority_a
                .cmp(priority_b)
                .then_with(|| order_a.cmp(order_b))
                .then_with(|| key_a.cmp(key_b))
        });
        for legacy_order_key in to_remove {
            self.clear_legacy_key(legacy_order_key);
        }
        messages.into_iter().map(|(priority, _, _, message)| (priority, message)).collect()
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
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.magic_point, 0);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.wisdom, 0);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.agility, 0);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.attr_sum, 0);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.atk_sum, 3);
        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.attract(), 32768.0);
        assert_eq!(
            arena.get(EntityIdx(0)).unwrap().runtime.at_boost_millionths,
            DEFAULT_AT_BOOST_MILLIONTHS
        );
        assert!(arena.get(EntityIdx(0)).unwrap().template.skills.is_empty());
    }

    #[test]
    fn player_template_carries_magic_point_into_runtime() {
        let arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_magic_point(96)]);

        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.magic_point, 96);
    }

    #[test]
    fn player_template_carries_wisdom_into_runtime() {
        let arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_wisdom(77)]);

        assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.wisdom, 77);
    }

    #[test]
    fn player_template_carries_target_score_stats_into_runtime() {
        let arena = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3).with_target_score_stats(42, 17, 1234.5),
        ]);
        let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;

        assert_eq!(runtime.attr_sum, 42);
        assert_eq!(runtime.atk_sum, 17);
        assert_eq!(runtime.attract(), 1234.5);
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
    fn skill_loadout_merges_levels_by_fixed_lane_without_replacing_skill_ids() {
        let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 0), (SkillId(2), 4)])
            .with_fixed_lane_keys([0, 2])
            .with_active_order([1]);
        let source =
            SkillLoadout::from_skill_levels([(SkillId(3), 9), (SkillId(4), 7), (SkillId(5), 11)]).with_fixed_lane_keys([0, 2, 4]);

        assert!(target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));

        assert_eq!(target.skills(), &[SkillId(1), SkillId(2)]);
        assert_eq!(target.levels(), &[9, 7]);
        assert_eq!(target.active_order(), &[1, 0]);
    }

    #[test]
    fn skill_loadout_ignores_unmapped_source_lanes() {
        let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 1), (SkillId(2), 2)]).with_fixed_lane_keys([0, 2]);
        let source = SkillLoadout::from_skill_levels([(SkillId(3), 9), (SkillId(4), 8)]).with_fixed_lane_keys([0, 7]);

        assert!(target.merge_fixed_lanes_from(&source, MergePolicy::DropUnmappedSkills));

        assert_eq!(target.skills(), &[SkillId(1), SkillId(2)]);
        assert_eq!(target.levels(), &[9, 2]);
    }

    #[test]
    fn skill_loadout_merge_reports_no_change_when_source_level_is_not_higher() {
        let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 5)]);
        let source = SkillLoadout::from_skill_levels([(SkillId(2), 5)]);

        assert!(!target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));

        assert_eq!(target.skills(), &[SkillId(1)]);
        assert_eq!(target.levels(), &[5]);
    }

    #[test]
    fn skill_loadout_none_merge_policy_keeps_levels_unchanged() {
        let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 1)]);
        let source = SkillLoadout::from_skill_levels([(SkillId(2), 9)]);

        assert!(!target.merge_fixed_lanes_from(&source, MergePolicy::None));
        assert_eq!(target.skills(), &[SkillId(1)]);
        assert_eq!(target.levels(), &[1]);
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
        assert_eq!(runtime.team, 0);
        assert_eq!(arena.get(spawned).unwrap().template.team, 0);
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

        assert!(store.add_entry(entry.clone()));
        assert!(!store.add_entry(entry.clone()));
        assert_eq!(store.entries(), &[entry.clone()]);
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

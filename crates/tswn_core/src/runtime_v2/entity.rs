use crate::runtime_v2::extension::{
    DamageSharePolicy, MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, ProcMask,
    RegistrationOrder, SkillId, SkillPriority, StateId,
};
use crate::runtime_v2::{EntitySlotStorage, ExtensionRegistry};
use smallvec::SmallVec;
use std::collections::HashMap;

use crate::player::{MOVE_POINT_THRESHOLD, PlayerStatus, PlrId, skill::SkillBoost};
use crate::rc4::RC4;

mod runtime;
mod state;

pub use runtime::*;
pub use state::*;

const DEFAULT_AT_BOOST_MILLIONTHS: i64 = 1_000_000;
const DEFAULT_AT_BOOST_BITS: u64 = 1.0_f64.to_bits();
const CLONE_ATTR_DECAY: f64 = 0.7799999713897705;

pub fn at_boost_to_millionths(at_boost: f64) -> i64 { (at_boost * DEFAULT_AT_BOOST_MILLIONTHS as f64).round() as i64 }

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
    at_boost_delta_bits: u64,
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
    pub at_boost_bits: u64,
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
    child_name_factor_bits: u64,
    adjustments: CloneStatAdjustments,
}

impl CloneBuildData {
    pub fn from_legacy(attrs: [u32; 8], weapon_attr_bonus: [i32; 8], name_factor: f64, status: &PlayerStatus) -> Self {
        let raw = Self::derive_raw(attrs, name_factor);
        Self {
            attrs,
            weapon_attr_bonus,
            name_factor_bits: name_factor.to_bits(),
            child_name_factor_bits: name_factor.to_bits(),
            adjustments: CloneStatAdjustments {
                max_hp: status.max_hp - raw.max_hp,
                attack: status.attack - raw.attack,
                magic: status.magic - raw.magic,
                wisdom: status.wisdom - raw.wisdom,
                speed: status.speed - raw.speed,
                defense: status.defense - raw.defense,
                resistance: status.resistance - raw.resistance,
                agility: status.agility - raw.agility,
                at_boost_delta_bits: (status.at_boost - f64::from_bits(raw.at_boost_bits)).to_bits(),
                attr_sum: i64::from(status.attr_sum) - i64::from(raw.attr_sum),
                atk_sum: status.atk_sum - raw.atk_sum,
                attract_delta_bits: (status.attract - f64::from_bits(raw.attract_bits)).to_bits(),
            },
        }
    }

    pub(crate) fn with_child_name_factor(mut self, child_name_factor: f64) -> Self {
        self.child_name_factor_bits = child_name_factor.to_bits();
        self
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
        // 特殊编号玩家的本体因子可能被强制为零，但分身仍会按根玩家名字重新计算构造因子。
        child.name_factor_bits = child.child_name_factor_bits;
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

    pub fn refresh_summon_owner_attrs(&mut self, owner: &Self) {
        self.attrs[1] = owner.attrs[1];
        self.attrs[5] = owner.attrs[5];
    }

    pub fn derive_stats(&self) -> CloneDerivedStats {
        let raw = Self::derive_raw(self.attrs, f64::from_bits(self.name_factor_bits));
        let attr_sum = i64::from(raw.attr_sum) + self.adjustments.attr_sum;
        let attract = f64::from_bits(raw.attract_bits) + f64::from_bits(self.adjustments.attract_delta_bits);
        let at_boost = f64::from_bits(raw.at_boost_bits) + f64::from_bits(self.adjustments.at_boost_delta_bits);
        CloneDerivedStats {
            max_hp: raw.max_hp + self.adjustments.max_hp,
            attack: raw.attack + self.adjustments.attack,
            magic: raw.magic + self.adjustments.magic,
            wisdom: raw.wisdom + self.adjustments.wisdom,
            speed: raw.speed + self.adjustments.speed,
            defense: raw.defense + self.adjustments.defense,
            resistance: raw.resistance + self.adjustments.resistance,
            agility: raw.agility + self.adjustments.agility,
            at_boost_bits: at_boost.to_bits(),
            at_boost_millionths: at_boost_to_millionths(at_boost),
            attr_sum: attr_sum.try_into().expect("runtime_v2 clone attr_sum became negative"),
            atk_sum: raw.atk_sum + self.adjustments.atk_sum,
            attract_bits: attract.to_bits(),
        }
    }

    /// 输入名字派生属性时使用的短号系数。
    ///
    /// 这是构造期冷数据，Runtime v2 的 CLI/展示入口需要它来复刻 legacy 的玩家状态摘要，
    /// 热路径不会读取该值。
    pub fn name_factor(&self) -> f64 { f64::from_bits(self.name_factor_bits) }

    /// 复刻 legacy `PlayerStatus::all_sum` 的当前构造属性总和。
    pub fn all_sum(&self) -> u32 { self.attrs[..7].iter().sum::<u32>() * 3 + self.attrs[7] }

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
            at_boost_bits: DEFAULT_AT_BOOST_BITS,
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
    pub reserved_player_ids_before_spawn: u32,
    pub name: String,
    pub id_key_name: String,
    pub clan_name: String,
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
    pub at_boost_bits: u64,
    pub at_boost_millionths: i64,
    pub attr_sum: u32,
    pub atk_sum: i32,
    pub attract_bits: u64,
    pub move_state: MoveState,
    pub policy_overrides: PlayerPolicyOverrides,
    pub clone_build: Option<CloneBuildData>,
    /// 使魔复活重施时是否沿用死亡对象上的当前技能表。
    pub reuse_skills_on_recast: bool,
    /// 使魔复活重施时是否沿用死亡对象上的持久属性。
    pub reuse_stats_on_recast: bool,
    /// 构造使魔属性时是否继承 owner 当前的防御与抗性。
    pub inherit_owner_def_res: bool,
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
            reserved_player_ids_before_spawn: 0,
            display_name: name.clone(),
            id_key_name: name.clone(),
            clan_name: name.clone(),
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
            at_boost_bits: DEFAULT_AT_BOOST_BITS,
            at_boost_millionths: DEFAULT_AT_BOOST_MILLIONTHS,
            attr_sum: 0,
            atk_sum: attack,
            attract_bits: 32768.0_f64.to_bits(),
            move_state: MoveState::default(),
            policy_overrides: PlayerPolicyOverrides::default(),
            clone_build: None,
            reuse_skills_on_recast: false,
            reuse_stats_on_recast: false,
            inherit_owner_def_res: false,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    /// 保存 legacy 输入身份的冷数据，供 CLI/replay/图标层使用。
    pub fn with_identity_names(mut self, id_key_name: impl Into<String>, clan_name: impl Into<String>) -> Self {
        self.id_key_name = id_key_name.into();
        self.clan_name = clan_name.into();
        self
    }

    /// 运行期子实体沿用根 owner 的 clan，并据当前 `name` 重建 legacy `id_key_name`。
    pub fn set_runtime_clan_name(&mut self, clan_name: impl Into<String>) {
        let clan_name = clan_name.into();
        self.id_key_name = if clan_name.is_empty() || clan_name == self.name {
            self.name.clone()
        } else {
            format!("{}@{clan_name}", self.name)
        };
        self.clan_name = clan_name;
    }

    pub fn with_reserved_player_ids_before_spawn(mut self, count: u32) -> Self {
        self.reserved_player_ids_before_spawn = count;
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
        self.at_boost_bits = (at_boost_millionths as f64 / DEFAULT_AT_BOOST_MILLIONTHS as f64).to_bits();
        self
    }

    pub fn with_at_boost(mut self, at_boost: f64) -> Self {
        assert!(
            at_boost.is_finite() && at_boost >= 0.0,
            "runtime_v2 player at_boost must be a non-negative finite number"
        );
        self.at_boost_bits = at_boost.to_bits();
        self.at_boost_millionths = at_boost_to_millionths(at_boost);
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

    pub fn apply_derived_stats(&mut self, stats: CloneDerivedStats) {
        self.max_hp = stats.max_hp.max(1);
        self.attack = stats.attack.max(0);
        self.magic = stats.magic.max(0);
        self.wisdom = stats.wisdom.max(0);
        self.speed = stats.speed.max(0);
        self.defense = stats.defense.max(0);
        self.resistance = stats.resistance.max(0);
        self.agility = stats.agility.max(0);
        let at_boost = f64::from_bits(stats.at_boost_bits);
        let at_boost = if at_boost.is_finite() { at_boost.max(0.0) } else { 0.0 };
        self.at_boost_bits = at_boost.to_bits();
        self.at_boost_millionths = at_boost_to_millionths(at_boost);
        self.attr_sum = stats.attr_sum;
        self.atk_sum = stats.atk_sum;
        self.attract_bits = stats.attract_bits;
    }

    /// 从准备模板恢复一场战斗可能修改的热字段。
    ///
    /// 名字和策略等冷数据在固定 roster 的连续对局间不会变化；技能等级和 clone build
    /// 会被升级、驱散、合体等机制修改，仍必须恢复。避免整份 `PlayerTemplate::clone_from`
    /// 可以省掉每局重复复制身份字符串。
    pub fn reset_battle_fields_from(&mut self, prepared: &Self) {
        debug_assert_eq!(self.id, prepared.id);
        debug_assert_eq!(self.name, prepared.name);
        debug_assert_eq!(self.id_key_name, prepared.id_key_name);
        debug_assert_eq!(self.clan_name, prepared.clan_name);
        self.skills.clone_from(&prepared.skills);
        self.team = prepared.team;
        self.max_hp = prepared.max_hp;
        self.attack = prepared.attack;
        self.magic = prepared.magic;
        self.magic_point = prepared.magic_point;
        self.wisdom = prepared.wisdom;
        self.speed = prepared.speed;
        self.defense = prepared.defense;
        self.resistance = prepared.resistance;
        self.agility = prepared.agility;
        self.at_boost_bits = prepared.at_boost_bits;
        self.at_boost_millionths = prepared.at_boost_millionths;
        self.attr_sum = prepared.attr_sum;
        self.atk_sum = prepared.atk_sum;
        self.attract_bits = prepared.attract_bits;
        self.move_state = prepared.move_state;
        self.clone_build.clone_from(&prepared.clone_build);
    }

    pub fn reuse_summon_stats_from(&mut self, source: &Self) {
        self.max_hp = source.max_hp;
        self.attack = source.attack;
        self.magic = source.magic;
        self.magic_point = (source.wisdom >> 1).max(0);
        self.wisdom = source.wisdom;
        self.speed = source.speed;
        self.defense = source.defense;
        self.resistance = source.resistance;
        self.agility = source.agility;
        self.at_boost_bits = source.at_boost_bits;
        self.at_boost_millionths = source.at_boost_millionths;
        self.attr_sum = source.attr_sum;
        self.atk_sum = source.atk_sum;
        self.attract_bits = source.attract_bits;
        self.clone_build = source.clone_build.clone();
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
    build_levels: SmallVec<[u32; 8]>,
    boosts: SmallVec<[Option<SkillBoost>; 8]>,
    boosted: SmallVec<[bool; 8]>,
    fixed_lane_keys: SmallVec<[usize; 8]>,
    merge_lane_order: SmallVec<[usize; 8]>,
    active_order: SmallVec<[usize; 8]>,
    pre_action_order: SmallVec<[usize; 8]>,
    post_damage_order: SmallVec<[usize; 8]>,
    post_action_after_states: SmallVec<[(u64, usize); 4]>,
}

impl SkillLoadout {
    pub fn from_skills(skills: impl IntoIterator<Item = SkillId>) -> Self {
        let skills = skills.into_iter().collect::<SmallVec<[SkillId; 8]>>();
        let levels = std::iter::repeat_n(1, skills.len()).collect::<SmallVec<[u32; 8]>>();
        let build_levels = levels.clone();
        let boosts = std::iter::repeat_n(None, skills.len()).collect();
        let boosted = std::iter::repeat_n(false, skills.len()).collect();
        let fixed_lane_keys = (0..skills.len()).collect();
        let merge_lane_order = (0..skills.len()).collect();
        let active_order = (0..skills.len()).collect();
        let post_damage_order = (0..skills.len()).collect();
        Self {
            skills,
            levels,
            build_levels,
            boosts,
            boosted,
            fixed_lane_keys,
            merge_lane_order,
            active_order,
            pre_action_order: SmallVec::new(),
            post_damage_order,
            post_action_after_states: SmallVec::new(),
        }
    }

    pub fn from_skill_levels(skills: impl IntoIterator<Item = (SkillId, u32)>) -> Self {
        let (skills, levels): (SmallVec<[SkillId; 8]>, SmallVec<[u32; 8]>) = skills.into_iter().unzip();
        let build_levels = levels.clone();
        let boosts = std::iter::repeat_n(None, skills.len()).collect();
        let boosted = std::iter::repeat_n(false, skills.len()).collect();
        let fixed_lane_keys = (0..skills.len()).collect();
        let merge_lane_order = (0..skills.len()).collect();
        let active_order = (0..skills.len()).collect();
        let post_damage_order = (0..skills.len()).collect();
        Self {
            skills,
            levels,
            build_levels,
            boosts,
            boosted,
            fixed_lane_keys,
            merge_lane_order,
            active_order,
            pre_action_order: SmallVec::new(),
            post_damage_order,
            post_action_after_states: SmallVec::new(),
        }
    }

    pub fn from_skill_levels_and_boosts(skills: impl IntoIterator<Item = (SkillId, u32, Option<SkillBoost>)>) -> Self {
        let mut skill_ids = SmallVec::<[SkillId; 8]>::new();
        let mut levels = SmallVec::<[u32; 8]>::new();
        let mut boosts = SmallVec::<[Option<SkillBoost>; 8]>::new();
        for (skill_id, level, boost) in skills {
            skill_ids.push(skill_id);
            levels.push(level);
            boosts.push(boost);
        }
        let build_levels = levels
            .iter()
            .zip(&boosts)
            .map(|(level, boost)| boost.as_ref().map_or(*level, SkillBoost::base_level))
            .collect::<SmallVec<[u32; 8]>>();
        let boosted = boosts
            .iter()
            .map(|boost| matches!(boost, Some(SkillBoost::LastBoost(_) | SkillBoost::SlotBoost { .. })))
            .collect();
        let fixed_lane_keys = (0..skill_ids.len()).collect();
        let merge_lane_order = (0..skill_ids.len()).collect();
        let active_order = (0..skill_ids.len()).collect();
        let post_damage_order = (0..skill_ids.len()).collect();
        Self {
            skills: skill_ids,
            levels,
            build_levels,
            boosts,
            boosted,
            fixed_lane_keys,
            merge_lane_order,
            active_order,
            pre_action_order: SmallVec::new(),
            post_damage_order,
            post_action_after_states: SmallVec::new(),
        }
    }

    pub fn skills(&self) -> &[SkillId] { &self.skills }

    pub fn levels(&self) -> &[u32] { &self.levels }

    pub fn level_at(&self, fixed_lane: usize) -> Option<u32> { self.levels.get(fixed_lane).copied() }

    pub fn build_level_at(&self, fixed_lane: usize) -> Option<u32> { self.build_levels.get(fixed_lane).copied() }

    pub fn boost_at(&self, fixed_lane: usize) -> Option<&SkillBoost> { self.boosts.get(fixed_lane).and_then(Option::as_ref) }

    pub fn boosted_at(&self, fixed_lane: usize) -> Option<bool> { self.boosted.get(fixed_lane).copied() }

    pub fn fixed_lane_key_at(&self, fixed_lane: usize) -> Option<usize> { self.fixed_lane_keys.get(fixed_lane).copied() }

    pub fn merge_lane_order(&self) -> &[usize] { &self.merge_lane_order }

    pub fn set_level_at(&mut self, fixed_lane: usize, level: u32) -> bool {
        let Some(current) = self.levels.get_mut(fixed_lane) else {
            return false;
        };
        *current = level;
        true
    }

    pub fn active_order(&self) -> &[usize] { &self.active_order }

    pub fn pre_action_order(&self) -> &[usize] { &self.pre_action_order }

    pub fn post_damage_order(&self) -> &[usize] { &self.post_damage_order }

    pub fn post_action_after_states(&self) -> &[(u64, usize)] { &self.post_action_after_states }

    pub fn is_empty(&self) -> bool { self.skills.is_empty() }

    pub fn len(&self) -> usize { self.skills.len() }

    pub fn rebuilt_for_clone(&self) -> Self {
        let mut clone = self.clone();
        for lane in 0..clone.levels.len() {
            let clamped = self.build_levels[lane].min(self.levels[lane]);
            clone.levels[lane] = match &self.boosts[lane] {
                None | Some(SkillBoost::Normal(_)) => clamped,
                Some(SkillBoost::LastBoost(_)) => clamped.saturating_mul(2),
                Some(SkillBoost::SlotBoost { boost, .. }) => clamped.saturating_add((*boost).min(clamped)),
            };
        }
        clone
            .pre_action_order
            .retain(|lane| clone.levels.get(*lane).is_some_and(|level| *level > 0));
        clone
            .post_action_after_states
            .retain(|(_, lane)| clone.levels.get(*lane).is_some_and(|level| *level > 0));
        clone
    }

    pub fn disable_action_lane(&mut self, fixed_lane: usize) { self.active_order.retain(|lane| *lane != fixed_lane); }

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

    pub fn with_post_damage_order(mut self, post_damage_order: impl IntoIterator<Item = usize>) -> Self {
        self.post_damage_order = post_damage_order.into_iter().collect();
        assert!(
            self.post_damage_order.iter().all(|idx| *idx < self.skills.len()),
            "runtime_v2 skill post-damage order must reference existing fixed lanes"
        );
        self
    }

    pub fn with_post_action_after_states(mut self, post_action_after_states: impl IntoIterator<Item = (u64, usize)>) -> Self {
        self.post_action_after_states = post_action_after_states.into_iter().collect();
        assert!(
            self.post_action_after_states.iter().all(|(_, idx)| *idx < self.skills.len()),
            "runtime_v2 deferred post-action order must reference existing fixed lanes"
        );
        self.post_action_after_states.sort_by_key(|(cursor, _)| *cursor);
        self
    }

    pub fn register_post_action_after_states(&mut self, fixed_lane: usize, state_order_cursor: u64) {
        assert!(
            fixed_lane < self.skills.len(),
            "runtime_v2 deferred post-action order must reference an existing fixed lane"
        );
        if self.post_action_after_states.iter().any(|(_, lane)| *lane == fixed_lane) {
            return;
        }
        self.post_action_after_states.push((state_order_cursor, fixed_lane));
        self.post_action_after_states.sort_by_key(|(cursor, _)| *cursor);
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

    pub fn with_boosted_flags(mut self, boosted: impl IntoIterator<Item = bool>) -> Self {
        self.boosted = boosted.into_iter().collect();
        assert_eq!(
            self.boosted.len(),
            self.skills.len(),
            "runtime_v2 boosted flags must match skill loadout length"
        );
        self
    }

    /// 复刻 legacy `boost_last()`：从行动顺序末尾寻找首个未强化的正等级技能。
    pub fn boost_last_active_except_key(&mut self, excluded_fixed_key: usize) -> bool {
        for &lane in self.active_order.iter().rev() {
            if self.fixed_lane_keys[lane] == excluded_fixed_key || self.levels[lane] == 0 || self.boosted[lane] {
                continue;
            }
            let base = self.levels[lane];
            self.levels[lane] = base.saturating_mul(2);
            self.build_levels[lane] = base;
            self.boosts[lane] = Some(SkillBoost::LastBoost(base));
            self.boosted[lane] = true;
            return true;
        }
        false
    }

    pub fn with_merge_lane_order(mut self, merge_lane_order: impl IntoIterator<Item = usize>) -> Self {
        self.merge_lane_order = merge_lane_order.into_iter().collect();
        assert!(
            self.merge_lane_order.iter().all(|idx| *idx < self.skills.len()),
            "runtime_v2 merge lane order must reference existing fixed lanes"
        );
        self
    }

    pub fn merge_fixed_lanes_from(&mut self, source: &Self, policy: MergePolicy) -> bool {
        match policy {
            MergePolicy::None => false,
            MergePolicy::FixedLane => {
                // legacy 的 Merge 只会按双方 `slot_skill` 的位置配对；store 中额外注册的
                // 分摊伤害等技能不属于固定槽位，不能挤进普通技能槽位。
                let lane_count = self.merge_lane_order.len().min(source.merge_lane_order.len());
                (0..lane_count)
                    .map(|position| {
                        let owner_idx = self.merge_lane_order[position];
                        let source_idx = source.merge_lane_order[position];
                        self.merge_level_at(owner_idx, source.levels[source_idx])
                    })
                    .fold(false, |changed, lane_changed| changed || lane_changed)
            }
            MergePolicy::DropUnmappedSkills => {
                let mut changed = false;
                for owner_idx in 0..self.levels.len() {
                    let fixed_lane_key = self.fixed_lane_keys[owner_idx];
                    let Some(source_idx) = source.fixed_lane_keys.iter().position(|source_key| *source_key == fixed_lane_key)
                    else {
                        continue;
                    };
                    changed |= self.merge_level_at(owner_idx, source.levels[source_idx]);
                }
                changed
            }
        }
    }

    fn merge_level_at(&mut self, owner_idx: usize, source_level: u32) -> bool {
        let owner_level = &mut self.levels[owner_idx];
        if source_level <= *owner_level {
            return false;
        }
        let was_zero = *owner_level == 0;
        *owner_level = source_level;
        if was_zero {
            self.active_order.retain(|lane| *lane != owner_idx);
            self.active_order.push(owner_idx);
            // legacy 会在 Merge 把零级槽位抬为正等级时注册新钩子，因此同优先级的
            // post_damage 新钩子应排在原有活跃钩子之后，与固定槽位编号无关。
            self.post_damage_order.retain(|lane| *lane != owner_idx);
            self.post_damage_order.push(owner_idx);
        }
        true
    }
}

#[cfg(test)]
mod derived_stats_tests;
#[cfg(test)]
mod tests;

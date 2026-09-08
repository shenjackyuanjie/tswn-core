use crate::namerena::{PlayerStats, SkillBoost};
use crate::rc4::RC4;
use crate::runtime::extension::{
    DamageSharePolicy, MergePolicy, OwnerResolutionPolicy, PlayerKindFlags, PlayerKindId, PlayerKindPolicies, ProcMask,
    RegistrationOrder, SkillId, SkillPostActionPhase, SkillPriority, StateId, TargetPolicy,
};
use crate::runtime::{BuiltinActiveSkill, EntitySlotStorage, ExtensionRegistry};
use smallvec::SmallVec;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{MOVE_POINT_THRESHOLD, PlrId};

mod runtime;
mod skill_loadout;
mod state;

pub use runtime::*;
pub use skill_loadout::*;
pub use state::*;

const DEFAULT_AT_BOOST_MILLIONTHS: i64 = 1_000_000;
const DEFAULT_AT_BOOST_BITS: u64 = 1.0_f64.to_bits();
const CLONE_ATTR_DECAY: f64 = 0.7799999713897705;

pub fn at_boost_to_millionths(at_boost: f64) -> i64 { (at_boost * DEFAULT_AT_BOOST_MILLIONTHS as f64).round() as i64 }

/// 分身相对本体模板的逐项修正量；字段公开供状态导出读取，不表示可变。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CloneStatAdjustments {
    pub max_hp: i32,
    pub attack: i32,
    pub magic: i32,
    pub wisdom: i32,
    pub speed: i32,
    pub defense: i32,
    pub resistance: i32,
    pub agility: i32,
    pub at_boost_delta_bits: u64,
    pub attr_sum: i64,
    pub atk_sum: i32,
    pub attract_delta_bits: u64,
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

/// 分身评分时的初始强化位与槽位强化计划；字段公开供状态导出读取。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScoreCloneSkillBoostPlan {
    pub initially_boosted_mask: u64,
    pub slot_boosts: [Option<(u8, u8)>; 2],
}

/// 分身的构造参数；字段公开供状态导出读取，语义与生成顺序见 `derive_raw` / `derive_stats`。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CloneBuildData {
    pub attrs: [u32; 8],
    pub weapon_attr_bonus: [i32; 8],
    pub name_factor_bits: u64,
    pub child_name_factor_bits: u64,
    pub adjustments: CloneStatAdjustments,
    pub score_skill_boost_plan: Option<ScoreCloneSkillBoostPlan>,
}

impl CloneBuildData {
    pub fn from_legacy(attrs: [u32; 8], weapon_attr_bonus: [i32; 8], name_factor: f64, status: &PlayerStats) -> Self {
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
            score_skill_boost_plan: None,
        }
    }

    pub(crate) fn with_child_name_factor(mut self, child_name_factor: f64) -> Self {
        self.child_name_factor_bits = child_name_factor.to_bits();
        self
    }

    pub(crate) fn with_score_skill_boost_plan(mut self, score_skill_boost_plan: ScoreCloneSkillBoostPlan) -> Self {
        self.score_skill_boost_plan = Some(score_skill_boost_plan);
        self
    }

    /// 直接构造未叠加武器和名字系数的数字 score profile 分身数据。
    pub(crate) fn from_score_profile(
        attrs: [u32; 8],
        child_name_factor: f64,
        score_skill_boost_plan: ScoreCloneSkillBoostPlan,
    ) -> Self {
        Self {
            attrs,
            weapon_attr_bonus: [0; 8],
            name_factor_bits: 0.0_f64.to_bits(),
            child_name_factor_bits: child_name_factor.to_bits(),
            adjustments: CloneStatAdjustments {
                max_hp: 0,
                attack: 0,
                magic: 0,
                wisdom: 0,
                speed: 0,
                defense: 0,
                resistance: 0,
                agility: 0,
                at_boost_delta_bits: 0.0_f64.to_bits(),
                attr_sum: 0,
                atk_sum: 0,
                attract_delta_bits: 0.0_f64.to_bits(),
            },
            score_skill_boost_plan: Some(score_skill_boost_plan),
        }
    }

    /// 返回子分身与召唤物已经计算好的名字系数，避免技能触发时重复解析名字。
    pub(crate) fn child_name_factor(&self) -> f64 { f64::from_bits(self.child_name_factor_bits) }

    pub(crate) fn score_skill_boost_plan(&self) -> Option<&ScoreCloneSkillBoostPlan> { self.score_skill_boost_plan.as_ref() }

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
            attr_sum: attr_sum.try_into().expect("runtime clone attr_sum became negative"),
            atk_sum: raw.atk_sum + self.adjustments.atk_sum,
            attract_bits: attract.to_bits(),
        }
    }

    /// 输入名字派生属性时使用的短号系数。
    ///
    /// 这是构造期冷数据，Runtime 的 CLI/展示入口需要它来复刻 legacy 的玩家状态摘要，
    /// 热路径不会读取该值。
    pub fn name_factor(&self) -> f64 { f64::from_bits(self.name_factor_bits) }

    /// 复刻 legacy `PlayerStats::all_sum` 的当前构造属性总和。
    pub fn all_sum(&self) -> u32 { self.attrs[..7].iter().sum::<u32>() * 3 + self.attrs[7] }

    /// 返回构造召唤物蓝图所需的 owner 原始八围。
    pub(crate) fn attrs(&self) -> [u32; 8] { self.attrs }

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
        assert!(max_hp > 0, "runtime player max_hp must be positive");
        assert!(attack >= 0, "runtime player attack must be non-negative");
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

    /// 消费已经回收的身份字符串，直接构造数字 score profile 模板。
    pub(crate) fn from_score_profile(
        id: PlrId,
        name: String,
        id_key_name: String,
        clan_name: String,
        display_name: String,
        team: usize,
        status: &PlayerStats,
        skills: SkillLoadout,
        clone_build: CloneBuildData,
    ) -> Self {
        debug_assert!(status.max_hp > 0);
        debug_assert!(status.attack >= 0);
        Self {
            id,
            reserved_player_ids_before_spawn: 0,
            name,
            id_key_name,
            clan_name,
            display_name,
            kind: Self::DEFAULT_KIND,
            skills,
            team,
            max_hp: status.max_hp,
            attack: status.attack,
            magic: status.magic,
            magic_point: status.magic_point,
            wisdom: status.wisdom,
            speed: status.speed,
            defense: status.defense,
            resistance: status.resistance,
            agility: status.agility,
            at_boost_bits: status.at_boost.to_bits(),
            at_boost_millionths: at_boost_to_millionths(status.at_boost),
            attr_sum: status.attr_sum,
            atk_sum: status.atk_sum,
            attract_bits: status.attract.to_bits(),
            move_state: MoveState::default(),
            policy_overrides: PlayerPolicyOverrides::default(),
            clone_build: Some(clone_build),
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
        assert!(magic >= 0, "runtime player magic must be non-negative");
        self.magic = magic;
        self
    }

    pub fn with_magic_point(mut self, magic_point: i32) -> Self {
        self.magic_point = magic_point;
        self
    }

    pub fn with_wisdom(mut self, wisdom: i32) -> Self {
        assert!(wisdom >= 0, "runtime player wisdom must be non-negative");
        self.wisdom = wisdom;
        self
    }

    pub fn with_speed(mut self, speed: i32) -> Self {
        assert!(speed >= 0, "runtime player speed must be non-negative");
        self.speed = speed;
        self
    }

    pub fn with_at_boost_millionths(mut self, at_boost_millionths: i64) -> Self {
        assert!(
            at_boost_millionths >= 0,
            "runtime player at_boost_millionths must be non-negative"
        );
        self.at_boost_millionths = at_boost_millionths;
        self.at_boost_bits = (at_boost_millionths as f64 / DEFAULT_AT_BOOST_MILLIONTHS as f64).to_bits();
        self
    }

    pub fn with_at_boost(mut self, at_boost: f64) -> Self {
        assert!(
            at_boost.is_finite() && at_boost >= 0.0,
            "runtime player at_boost must be a non-negative finite number"
        );
        self.at_boost_bits = at_boost.to_bits();
        self.at_boost_millionths = at_boost_to_millionths(at_boost);
        self
    }

    pub fn with_target_score_stats(mut self, attr_sum: u32, atk_sum: i32, attract: f64) -> Self {
        assert!(attract.is_finite(), "runtime player attract must be finite");
        self.attr_sum = attr_sum;
        self.atk_sum = atk_sum;
        self.attract_bits = attract.to_bits();
        self
    }

    pub fn with_def_res(mut self, defense: i32, resistance: i32) -> Self {
        assert!(defense >= 0, "runtime player defense must be non-negative");
        assert!(resistance >= 0, "runtime player resistance must be non-negative");
        self.defense = defense;
        self.resistance = resistance;
        self
    }

    pub fn with_agility(mut self, agility: i32) -> Self {
        assert!(agility >= 0, "runtime player agility must be non-negative");
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
        self.skills.reset_battle_fields_from(&prepared.skills);
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

#[cfg(test)]
mod derived_stats_tests;
#[cfg(test)]
mod tests;

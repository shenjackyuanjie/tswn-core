//! 默认游戏规则的配置入口：导出名称、主动技能分类、注册、导入与 handler 绑定。
//!
//! [`default_custom_runtime_import_config`] 组装普通名字、Boss、Boost、召唤物与
//! bed2 的运行配置。`registry` 描述技能/状态/种类/槽，`import` 将构建资料转成模板，
//! `hooks` 提供阶段实现；这三部分共同将游戏机制接入 Runtime。
//! export_name 用于查找与导入，legacy key 用于兼容原游戏的技能/状态类别。
//! 新机制可以沿注册、导入、函数绑定和数据投影四个环节接入。

use super::*;
use crate::runtime::entity::ScoreSkillHookPlanEntry;

mod hooks;
mod import;
mod registry;

pub use hooks::*;
pub use registry::*;

/// bed2 阵容成员使用的默认体力。
pub const DEFAULT_BED2_HP: i32 = 3000;
/// bed2 阵容成员使用的默认防御。
pub const DEFAULT_BED2_DEFENSE: i32 = 99;
/// bed2 阵容成员使用的默认抗性。
pub const DEFAULT_BED2_RESISTANCE: i32 = 99;

/// 防御技能的默认导出名。
pub const DEFAULT_CORE_DEFEND_SKILL_EXPORT: &str = "core.skill.defend";
/// 伤害反弹技能的默认导出名。
pub const DEFAULT_CORE_REFLECT_SKILL_EXPORT: &str = "core.skill.reflect";
/// 守护技能的默认导出名。
pub const DEFAULT_CORE_PROTECT_SKILL_EXPORT: &str = "core.skill.protect";
/// 护盾技能的默认导出名。
pub const DEFAULT_CORE_SHIELD_SKILL_EXPORT: &str = "core.skill.shield";
/// 垂死抗争技能的默认导出名。
pub const DEFAULT_CORE_UPGRADE_SKILL_EXPORT: &str = "core.skill.upgrade";
/// 隐匿技能的默认导出名。
pub const DEFAULT_CORE_HIDE_SKILL_EXPORT: &str = "core.skill.hide";
/// 反击技能的默认导出名。
pub const DEFAULT_CORE_COUNTER_SKILL_EXPORT: &str = "core.skill.counter";
/// 吞噬技能的默认导出名。
pub const DEFAULT_CORE_MERGE_SKILL_EXPORT: &str = "core.skill.merge";
/// 召唤亡灵技能的默认导出名。
pub const DEFAULT_CORE_ZOMBIE_SKILL_EXPORT: &str = "core.skill.zombie";
/// 护身符技能的默认导出名。
pub const DEFAULT_CORE_RERAISE_SKILL_EXPORT: &str = "core.skill.reraise";
/// 魅惑状态的默认导出名。
pub const DEFAULT_CORE_CHARM_STATE_EXPORT: &str = "core.state.charm";
/// 诅咒状态的默认导出名。
pub const DEFAULT_CORE_CURSE_STATE_EXPORT: &str = "core.state.curse";
/// 中毒状态的默认导出名。
pub const DEFAULT_CORE_POISON_STATE_EXPORT: &str = "core.state.poison";
/// 疾走状态的默认导出名。
pub const DEFAULT_CORE_HASTE_STATE_EXPORT: &str = "core.state.haste";
/// 迟缓状态的默认导出名。
pub const DEFAULT_CORE_SLOW_STATE_EXPORT: &str = "core.state.slow";
/// 铁壁状态的默认导出名。
pub const DEFAULT_CORE_IRON_STATE_EXPORT: &str = "core.state.iron";
/// 新冠传染状态的默认导出名。
pub const DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT: &str = "core.state.covid-infection";
/// 懒癌传染状态的默认导出名。
pub const DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT: &str = "core.state.lazy-infection";
/// 一拳超人 Boss 状态的默认导出名。
pub const DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT: &str = "core.state.saitama-boss";
/// 幻影玩家类型的默认导出名。
pub const DEFAULT_CORE_SHADOW_KIND_EXPORT: &str = "core.kind.shadow";
/// 召唤物玩家类型的默认导出名。
pub const DEFAULT_CORE_SUMMON_KIND_EXPORT: &str = "core.kind.summon";
/// 亡灵玩家类型的默认导出名。
pub const DEFAULT_CORE_ZOMBIE_KIND_EXPORT: &str = "core.kind.zombie";
/// 分身玩家类型的默认导出名。
pub const DEFAULT_CORE_CLONE_KIND_EXPORT: &str = "core.kind.clone";
/// Boss 玩家类型的默认导出名。
pub const DEFAULT_CORE_BOSS_KIND_EXPORT: &str = "core.kind.boss";
/// 加成玩家类型的默认导出名。
pub const DEFAULT_CORE_BOOST_KIND_EXPORT: &str = "core.kind.boost";
/// 幻影蓝图实体槽的默认导出名。
pub const DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.shadow_blueprint";
/// 召唤物蓝图实体槽的默认导出名。
pub const DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.summon_blueprint";
/// 亡灵蓝图实体槽的默认导出名。
pub const DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.zombie_blueprint";
/// 懒癌蓝图参数实体槽的默认导出名。
pub const DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT: &str = "core.entity.lazy_blueprint_rq";
/// 已召唤实体记录槽的默认导出名。
pub const DEFAULT_CORE_SUMMON_ENTITY_EXPORT: &str = "core.entity.summoned_entity";
/// 使魔命名计数槽的默认导出名。
pub const DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT: &str = "core.entity.minion_counter";
/// 召唤物自爆技能的默认导出名。
pub const DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT: &str = "core.skill.summon-explode";
/// 召唤物分摊伤害技能的默认导出名。
pub const DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT: &str = "core.skill.summon-share-damage";
/// 剁手刀武器技能的默认导出名。
pub const DEFAULT_CORE_WEAPON_S11_SKILL_EXPORT: &str = "core.weapon.s11";
/// 死亡笔记武器技能的默认导出名。
pub const DEFAULT_CORE_WEAPON_DEATH_NOTE_SKILL_EXPORT: &str = "core.weapon.death-note";
/// 玄月反击武器技能的默认导出名。
pub const DEFAULT_CORE_WEAPON_XUAN_YUE_COUNTER_SKILL_EXPORT: &str = "core.weapon.xuan-yue-counter";
/// Rinick 冰河时代武器技能的默认导出名。
pub const DEFAULT_CORE_WEAPON_RINICK_ICE_AGE_SKILL_EXPORT: &str = "core.weapon.rinick-ice-age";
/// Rinick 尤里控制武器技能的默认导出名。
pub const DEFAULT_CORE_WEAPON_RINICK_YURI_CONTROL_SKILL_EXPORT: &str = "core.weapon.rinick-yuri-control";
/// bed2 召唤技能的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT: &str = "custom.summon";
/// bed2 召唤物火球技能的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT: &str = "custom.summon.fire";
/// bed2 召唤物自爆技能的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT: &str = "custom.summon.explode";
/// bed2 已召唤实体记录槽的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SUMMON_ENTITY_EXPORT: &str = "custom.bed2.summoned_entity";
/// bed2 召唤物模板槽的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT: &str = "custom.bed2.summon_template";
/// bed2 幻影模板槽的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT: &str = "custom.bed2.shadow_template";
/// bed2 亡灵模板槽的默认导出名。
pub const DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT: &str = "custom.bed2.zombie_template";
/// bed2 召唤物玩家类型的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT: &str = "custom.bed2.summon";
/// bed2 幻影玩家类型的默认导出名。
pub const DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT: &str = "custom.bed2.shadow";
/// bed2 亡灵玩家类型的默认导出名。
pub const DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT: &str = "custom.bed2.zombie";
/// 使魔附体技能的默认导出名。
pub const DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT: &str = "custom.minion.possess";
/// 使魔技能导出名的公共前缀。
pub const DEFAULT_CUSTOM_MINION_SKILL_EXPORT_PREFIX: &str = "custom.minion";

/// 内置主动技能枚举，覆盖普通技能、召唤物技能与武器/修改器技能。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinActiveSkill {
    Fire,
    Ice,
    Thunder,
    Quake,
    Absorb,
    Poison,
    Rapid,
    Critical,
    Half,
    Exchange,
    Berserk,
    Charm,
    Haste,
    Slow,
    Curse,
    Heal,
    Revive,
    Disperse,
    Iron,
    Charge,
    Accumulate,
    Assassinate,
    Summon,
    SummonExplode,
    Clone,
    Shadow,
    Possess,
    WeaponS11,
    WeaponDeathNote,
    WeaponXuanYue,
    RinickIceAge,
    RinickYuriControl,
}

/// 火焰精通状态的 legacy 兼容键。
pub const PLAIN_FIRE_STATE_KEY: u32 = 0;
/// 冰冻状态的 legacy 兼容键。
pub const PLAIN_ICE_STATE_KEY: u32 = 1;
/// 狂暴状态的 legacy 兼容键。
pub const PLAIN_BERSERK_STATE_KEY: u32 = 10;
/// 诅咒状态的 legacy 兼容键。
pub const PLAIN_CURSE_STATE_KEY: u32 = 73;
/// 护盾状态的 legacy 兼容键。
pub const PLAIN_SHIELD_STATE_KEY: u32 = 74;
/// 中毒状态的 legacy 兼容键。
pub const PLAIN_POISON_STATE_KEY: u32 = 75;
/// 魅惑状态的 legacy 兼容键。
pub const PLAIN_CHARM_STATE_KEY: u32 = 76;
/// 疾走状态的 legacy 兼容键。
pub const PLAIN_HASTE_STATE_KEY: u32 = 77;
/// 迟缓状态的 legacy 兼容键。
pub const PLAIN_SLOW_STATE_KEY: u32 = 78;
/// 铁壁状态的 legacy 兼容键。
pub const PLAIN_IRON_STATE_KEY: u32 = 79;
/// 新冠 Boss 变异状态的 legacy 兼容键。
pub const PLAIN_COVID_BOSS_STATE_KEY: u32 = 90;
/// 新冠传染状态的 legacy 兼容键。
pub const PLAIN_COVID_INFECTION_STATE_KEY: u32 = 91;
/// 懒癌 Boss 加成状态的 legacy 兼容键。
pub const PLAIN_LAZY_BOSS_STATE_KEY: u32 = 92;
/// 懒癌传染状态的 legacy 兼容键。
pub const PLAIN_LAZY_INFECTION_STATE_KEY: u32 = 93;
/// 一拳超人 Boss状态的 legacy 兼容键。
pub const PLAIN_SAITAMA_BOSS_STATE_KEY: u32 = 94;
/// 马里奥成长状态的 legacy 兼容键。
pub const PLAIN_MARIO_BOSS_STATE_KEY: u32 = 95;

/// 默认规则会产生的 legacy state key，按原始值升序冻结给 encoder 分类词表。
pub const PLAIN_STATE_LEGACY_KEYS: [u32; 16] = [
    PLAIN_FIRE_STATE_KEY,
    PLAIN_ICE_STATE_KEY,
    PLAIN_BERSERK_STATE_KEY,
    PLAIN_CURSE_STATE_KEY,
    PLAIN_SHIELD_STATE_KEY,
    PLAIN_POISON_STATE_KEY,
    PLAIN_CHARM_STATE_KEY,
    PLAIN_HASTE_STATE_KEY,
    PLAIN_SLOW_STATE_KEY,
    PLAIN_IRON_STATE_KEY,
    PLAIN_COVID_BOSS_STATE_KEY,
    PLAIN_COVID_INFECTION_STATE_KEY,
    PLAIN_LAZY_BOSS_STATE_KEY,
    PLAIN_LAZY_INFECTION_STATE_KEY,
    PLAIN_SAITAMA_BOSS_STATE_KEY,
    PLAIN_MARIO_BOSS_STATE_KEY,
];

impl BuiltinActiveSkill {
    /// 核心主动技能集合；集合顺序即模型技能编号。
    pub const CORE: [Self; 25] = [
        Self::Fire,
        Self::Ice,
        Self::Thunder,
        Self::Quake,
        Self::Absorb,
        Self::Poison,
        Self::Rapid,
        Self::Critical,
        Self::Half,
        Self::Exchange,
        Self::Berserk,
        Self::Charm,
        Self::Haste,
        Self::Slow,
        Self::Curse,
        Self::Heal,
        Self::Revive,
        Self::Disperse,
        Self::Iron,
        Self::Charge,
        Self::Accumulate,
        Self::Assassinate,
        Self::Summon,
        Self::Clone,
        Self::Shadow,
    ];

    /// 全部内置主动技能，含武器与属性修改器技能。
    pub const ALL: [Self; 31] = [
        Self::Fire,
        Self::Ice,
        Self::Thunder,
        Self::Quake,
        Self::Absorb,
        Self::Poison,
        Self::Rapid,
        Self::Critical,
        Self::Half,
        Self::Exchange,
        Self::Berserk,
        Self::Charm,
        Self::Haste,
        Self::Slow,
        Self::Curse,
        Self::Heal,
        Self::Revive,
        Self::Disperse,
        Self::Iron,
        Self::Charge,
        Self::Accumulate,
        Self::Assassinate,
        Self::Summon,
        Self::Clone,
        Self::Shadow,
        Self::Possess,
        Self::WeaponS11,
        Self::WeaponDeathNote,
        Self::WeaponXuanYue,
        Self::RinickIceAge,
        Self::RinickYuriControl,
    ];

    /// 原游戏中的技能兼容键，用于 score 与 legacy 输入。
    pub const fn legacy_key(self) -> usize {
        match self {
            Self::Fire => 0,
            Self::Ice => 1,
            Self::Thunder => 2,
            Self::Quake => 3,
            Self::Absorb => 4,
            Self::Poison => 5,
            Self::Rapid => 6,
            Self::Critical => 7,
            Self::Half => 8,
            Self::Exchange => 9,
            Self::Berserk => 10,
            Self::Charm => 11,
            Self::Haste => 12,
            Self::Slow => 13,
            Self::Curse => 14,
            Self::Heal => 15,
            Self::Revive => 16,
            Self::Disperse => 17,
            Self::Iron => 18,
            Self::Charge => 19,
            Self::Accumulate => 20,
            Self::Assassinate => 21,
            Self::Summon => 22,
            Self::SummonExplode => 255,
            Self::Clone => 23,
            Self::Shadow => 24,
            Self::Possess => 43,
            Self::WeaponS11 => 44,
            Self::WeaponDeathNote => 45,
            Self::WeaponXuanYue => 46,
            Self::RinickIceAge => 47,
            Self::RinickYuriControl => 48,
        }
    }

    /// 注册表内的本地名。
    pub const fn local_name(self) -> &'static str {
        match self {
            Self::Fire => "fire",
            Self::Ice => "ice",
            Self::Thunder => "thunder",
            Self::Quake => "quake",
            Self::Absorb => "absorb",
            Self::Poison => "poison",
            Self::Rapid => "rapid",
            Self::Critical => "critical",
            Self::Half => "half",
            Self::Exchange => "exchange",
            Self::Berserk => "berserk",
            Self::Charm => "charm",
            Self::Haste => "haste",
            Self::Slow => "slow",
            Self::Curse => "curse",
            Self::Heal => "heal",
            Self::Revive => "revive",
            Self::Disperse => "disperse",
            Self::Iron => "iron",
            Self::Charge => "charge",
            Self::Accumulate => "accumulate",
            Self::Assassinate => "assassinate",
            Self::Summon => "summon",
            Self::SummonExplode => "summon-explode",
            Self::Clone => "clone",
            Self::Shadow => "shadow",
            Self::Possess => "minion-possess",
            Self::WeaponS11 => "weapon-s11",
            Self::WeaponDeathNote => "weapon-death-note",
            Self::WeaponXuanYue => "weapon-xuan-yue-counter",
            Self::RinickIceAge => "weapon-rinick-ice-age",
            Self::RinickYuriControl => "weapon-rinick-yuri-control",
        }
    }

    /// 导出名，用于导入与跨版本查找。
    pub const fn export_name(self) -> &'static str {
        match self {
            Self::Fire => "core.skill.fire",
            Self::Ice => "core.skill.ice",
            Self::Thunder => "core.skill.thunder",
            Self::Quake => "core.skill.quake",
            Self::Absorb => "core.skill.absorb",
            Self::Poison => "core.skill.poison",
            Self::Rapid => "core.skill.rapid",
            Self::Critical => "core.skill.critical",
            Self::Half => "core.skill.half",
            Self::Exchange => "core.skill.exchange",
            Self::Berserk => "core.skill.berserk",
            Self::Charm => "core.skill.charm",
            Self::Haste => "core.skill.haste",
            Self::Slow => "core.skill.slow",
            Self::Curse => "core.skill.curse",
            Self::Heal => "core.skill.heal",
            Self::Revive => "core.skill.revive",
            Self::Disperse => "core.skill.disperse",
            Self::Iron => "core.skill.iron",
            Self::Charge => "core.skill.charge",
            Self::Accumulate => "core.skill.accumulate",
            Self::Assassinate => "core.skill.assassinate",
            Self::Summon => "core.skill.summon",
            Self::SummonExplode => DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT,
            Self::Clone => "core.skill.clone",
            Self::Shadow => "core.skill.shadow",
            Self::Possess => DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT,
            Self::WeaponS11 => DEFAULT_CORE_WEAPON_S11_SKILL_EXPORT,
            Self::WeaponDeathNote => DEFAULT_CORE_WEAPON_DEATH_NOTE_SKILL_EXPORT,
            Self::WeaponXuanYue => DEFAULT_CORE_WEAPON_XUAN_YUE_COUNTER_SKILL_EXPORT,
            Self::RinickIceAge => DEFAULT_CORE_WEAPON_RINICK_ICE_AGE_SKILL_EXPORT,
            Self::RinickYuriControl => DEFAULT_CORE_WEAPON_RINICK_YURI_CONTROL_SKILL_EXPORT,
        }
    }

    /// 该技能的默认目标策略。
    pub const fn target_policy(self) -> TargetPolicy {
        match self {
            Self::Iron
            | Self::Charge
            | Self::Accumulate
            | Self::Summon
            | Self::Clone
            | Self::Shadow
            | Self::WeaponS11
            | Self::WeaponDeathNote
            | Self::WeaponXuanYue => TargetPolicy::None,
            Self::Haste | Self::Heal | Self::Revive => TargetPolicy::Ally,
            _ => TargetPolicy::Enemy,
        }
    }

    /// 按兼容键反查内置技能。
    pub fn from_legacy_key(key: usize) -> Option<Self> { Self::ALL.into_iter().find(|skill| skill.legacy_key() == key) }

    /// 按导出名反查内置技能。
    pub fn from_export_name(export_name: &str) -> Option<Self> {
        if export_name == DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT {
            return Some(Self::SummonExplode);
        }
        Self::ALL.into_iter().find(|skill| skill.export_name() == export_name)
    }
}

/// 内置技能在注册表中的 ID 映射，以及 score 用的 hook 计划缓存。
#[derive(Debug, Clone)]
pub struct BuiltinSkillImportMap {
    plain_by_legacy_key: [Option<SkillId>; 35],
    score_hook_plan: Vec<ScoreSkillHookPlanEntry>,
    score_hook_plan_needs_active_sort: bool,
    special_skill_ids: [Option<SkillId>; 9],
}

impl BuiltinSkillImportMap {
    /// 从注册表构建内置技能映射；导出名缺失会 panic。
    pub fn new(registry: &ExtensionRegistry) -> Self {
        let mut active_by_legacy_key = [None; 256];
        for skill in BuiltinActiveSkill::ALL {
            active_by_legacy_key[skill.legacy_key()] = registry.skill_id_by_export_name(skill.export_name());
        }
        let mut plain_by_legacy_key = [None; 35];
        plain_by_legacy_key[..25].copy_from_slice(&active_by_legacy_key[..25]);
        for (key, export_name) in [
            DEFAULT_CORE_DEFEND_SKILL_EXPORT,
            DEFAULT_CORE_PROTECT_SKILL_EXPORT,
            DEFAULT_CORE_REFLECT_SKILL_EXPORT,
            DEFAULT_CORE_RERAISE_SKILL_EXPORT,
            DEFAULT_CORE_SHIELD_SKILL_EXPORT,
            DEFAULT_CORE_COUNTER_SKILL_EXPORT,
            DEFAULT_CORE_MERGE_SKILL_EXPORT,
            DEFAULT_CORE_ZOMBIE_SKILL_EXPORT,
            DEFAULT_CORE_UPGRADE_SKILL_EXPORT,
            DEFAULT_CORE_HIDE_SKILL_EXPORT,
        ]
        .into_iter()
        .enumerate()
        {
            plain_by_legacy_key[25 + key] = registry.skill_id_by_export_name(export_name);
        }
        let mut score_hook_plan = Vec::new();
        for (legacy_key, skill_id) in plain_by_legacy_key.iter().copied().enumerate() {
            let Some(skill_id) = skill_id else {
                continue;
            };
            let spec = registry
                .skill(skill_id)
                .unwrap_or_else(|| panic!("默认 score 技能映射引用了未知技能 {}", skill_id.0));
            let mut hooks = spec.hook_mask.0 & 0xff;
            while hooks != 0 {
                let hook_index = hooks.trailing_zeros() as u8;
                hooks &= hooks - 1;
                score_hook_plan.push(ScoreSkillHookPlanEntry {
                    legacy_key,
                    hook_index,
                    skill_id: spec.id,
                    target_policy: spec.target_policy,
                    priority: spec.priority,
                    post_action_phase: spec.post_action_phase,
                    registration_order: spec.registration_order,
                });
            }
        }
        score_hook_plan.sort_by_key(|entry| (entry.hook_index, entry.priority, entry.registration_order));
        let score_hook_plan_needs_active_sort = score_hook_plan
            .windows(2)
            .any(|pair| pair[0].hook_index == pair[1].hook_index && pair[0].priority == pair[1].priority);
        Self {
            plain_by_legacy_key,
            score_hook_plan,
            score_hook_plan_needs_active_sort,
            special_skill_ids: [
                registry.skill_id_by_export_name(BuiltinActiveSkill::Fire.export_name()),
                registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_WEAPON_S11_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_WEAPON_DEATH_NOTE_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_WEAPON_XUAN_YUE_COUNTER_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_WEAPON_RINICK_ICE_AGE_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_WEAPON_RINICK_YURI_CONTROL_SKILL_EXPORT),
            ],
        }
    }

    /// 只解析数字 score 召唤物需要的四个技能，避免首次触发时扫描整张 legacy 映射表。
    pub(crate) fn new_score_minions(registry: &ExtensionRegistry) -> Self {
        Self {
            plain_by_legacy_key: [None; 35],
            score_hook_plan: Vec::new(),
            score_hook_plan_needs_active_sort: false,
            special_skill_ids: [
                registry.skill_id_by_export_name(BuiltinActiveSkill::Fire.export_name()),
                registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT),
                registry.skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT),
                None,
                None,
                None,
                None,
                None,
            ],
        }
    }

    /// 按索引取召唤物专用技能 ID；默认 profile 未注册该索引时 panic。
    fn special_skill_id(&self, index: usize) -> SkillId {
        self.special_skill_ids[index].unwrap_or_else(|| panic!("runtime 默认 profile 未注册召唤物技能索引 {index}"))
    }

    /// 把导入的技能条目与各顺序键落成 [`SkillLoadout`]：固定键映射到 lane，未列入行动序的 lane 追加到末尾。
    fn finish_import(
        &self,
        imported: Vec<(usize, SkillId, u32, bool, Option<crate::namerena::SkillBoost>)>,
        merge_lane_order_keys: &[usize],
        active_order_keys: &[usize],
        pre_action_order_keys: &[usize],
        post_damage_order_keys: &[usize],
        post_action_after_states_keys: &[(u64, usize)],
    ) -> SkillLoadout {
        let mut lane_by_key = vec![usize::MAX; imported.iter().map(|(key, _, _, _, _)| *key).max().unwrap_or(0) + 1];
        for (lane, (key, _, _, _, _)) in imported.iter().enumerate() {
            lane_by_key[*key] = lane;
        }
        let lane_for_key = |key: usize| lane_by_key.get(key).copied().filter(|lane| *lane != usize::MAX);
        let merge_lane_order = merge_lane_order_keys.iter().filter_map(|key| lane_for_key(*key)).collect::<Vec<_>>();
        let mut active_order = active_order_keys.iter().filter_map(|key| lane_for_key(*key)).collect::<Vec<_>>();
        let mut active_lanes = vec![false; imported.len()];
        for &lane in &active_order {
            active_lanes[lane] = true;
        }
        for lane in 0..imported.len() {
            if !active_lanes[lane] {
                active_order.push(lane);
            }
        }
        let pre_action_order = pre_action_order_keys.iter().filter_map(|key| lane_for_key(*key)).collect::<Vec<_>>();
        let post_damage_order = post_damage_order_keys.iter().filter_map(|key| lane_for_key(*key)).collect::<Vec<_>>();
        let post_action_after_states = post_action_after_states_keys
            .iter()
            .filter_map(|(cursor, key)| lane_for_key(*key).map(|lane| (*cursor, lane)))
            .collect::<Vec<_>>();

        let fixed_lane_keys = imported.iter().map(|(key, _, _, _, _)| *key).collect::<Vec<_>>();
        let boosted = imported.iter().map(|(_, _, _, boosted, _)| *boosted).collect::<Vec<_>>();
        SkillLoadout::from_skill_levels_and_boosts(
            imported.into_iter().map(|(_, skill_id, level, _, boost)| (skill_id, level, boost)),
        )
        .with_fixed_lane_keys(fixed_lane_keys)
        .with_boosted_flags(boosted)
        .with_merge_lane_order(merge_lane_order)
        .with_active_order(active_order)
        .with_pre_action_order(pre_action_order)
        .with_post_damage_order(post_damage_order)
        .with_post_action_after_states(post_action_after_states)
    }

    /// 将 namerena 纯数据技能表解析为 Runtime loadout。
    pub fn import_namerena(&self, spec: &crate::namerena::SkillLoadoutSpec) -> SkillLoadout {
        use crate::namerena::BuiltinSkillRef;

        let imported = spec
            .entries
            .iter()
            .filter_map(|entry| {
                let skill_id = match entry.skill {
                    BuiltinSkillRef::Normal(kind_key) => self.plain_by_legacy_key.get(kind_key).copied().flatten(),
                    BuiltinSkillRef::SummonFire => self.special_skill_ids[0],
                    BuiltinSkillRef::SummonExplode => self.special_skill_ids[1],
                    BuiltinSkillRef::SummonShareDamage => self.special_skill_ids[2],
                    BuiltinSkillRef::Possess => self.special_skill_ids[3],
                    BuiltinSkillRef::WeaponS11 => self.special_skill_ids[4],
                    BuiltinSkillRef::WeaponDeathNote => self.special_skill_ids[5],
                    BuiltinSkillRef::WeaponXuanYue => self.special_skill_ids[6],
                    BuiltinSkillRef::RinickIceAge => self.special_skill_ids[7],
                    BuiltinSkillRef::RinickYuriControl => self.special_skill_ids[8],
                }?;
                Some((entry.key, skill_id, entry.level, entry.boosted, entry.boost.clone()))
            })
            .collect();
        self.finish_import(
            imported,
            &spec.fixed_lanes,
            &spec.active_order,
            &spec.pre_action_order,
            &spec.post_damage_order,
            &spec.post_action_after_states,
        )
        .with_effective_skill_weights(
            spec.effective_skill_weights.as_deref(),
            spec.effective_skill_redistribution,
            spec.effective_skill_adjustment.as_deref(),
        )
    }

    /// 原地重填标准 score profile 技能表，复用上一轮的所有 SmallVec 容量。
    pub(crate) fn reset_score_profile(
        &self,
        loadout: &mut SkillLoadout,
        levels: &[u32; 35],
        boosted: &[bool; 35],
        boosts: &[Option<crate::namerena::SkillBoost>; 35],
        action_order: &[u32; 40],
    ) {
        loadout.reset_score_profile(
            &self.plain_by_legacy_key,
            &self.score_hook_plan,
            self.score_hook_plan_needs_active_sort,
            levels,
            boosted,
            boosts,
            action_order,
        );
    }

    /// 直接构造无 overlay 幻影的唯一技能，跳过 legacy SkillStorage。
    pub(crate) fn import_score_shadow_minion(&self, base_level: u32) -> SkillLoadout {
        let possess = self.special_skill_id(3);
        let (level, boosted, boost) = if base_level > 0 {
            (
                base_level.saturating_mul(2),
                true,
                Some(crate::namerena::SkillBoost::LastBoost(base_level)),
            )
        } else {
            (0, false, None)
        };
        SkillLoadout::from_skill_levels_and_boosts([(possess, level, boost)])
            .with_boosted_flags([boosted])
            .with_fixed_lane_keys([0])
            .with_merge_lane_order([0])
            .with_active_order([0])
            .with_post_damage_order([])
    }

    /// 直接构造无 overlay 使魔的固定四技能布局，保持 legacy 洗牌与末位加成顺序。
    pub(crate) fn import_score_summon_minion(&self, slot_levels: [u32; 3], action_order: [usize; 3]) -> SkillLoadout {
        let mut levels = [0u32; 3];
        for (slot, &key) in action_order.iter().enumerate() {
            levels[key] = slot_levels[slot];
        }
        let mut boosted = [false; 3];
        let mut boosts: [Option<crate::namerena::SkillBoost>; 3] = std::array::from_fn(|_| None);
        for &key in action_order.iter().rev() {
            let base = levels[key];
            if base == 0 {
                continue;
            }
            levels[key] = base.saturating_mul(2);
            boosted[key] = true;
            boosts[key] = Some(crate::namerena::SkillBoost::LastBoost(base));
            break;
        }

        let fire = self.special_skill_id(0);
        let explode = self.special_skill_id(1);
        let share = self.special_skill_id(2);
        let imported = [
            (fire, levels[0], boosts[0].clone()),
            (fire, levels[1], boosts[1].clone()),
            (explode, levels[2], boosts[2].clone()),
            (share, 1, None),
        ];
        SkillLoadout::from_skill_levels_and_boosts(imported)
            .with_boosted_flags([boosted[0], boosted[1], boosted[2], false])
            .with_fixed_lane_keys([0, 1, 2, crate::namerena::SUMMON_SHARE_DAMAGE_SKILL_KEY])
            .with_merge_lane_order([0, 1, 2])
            .with_active_order(action_order.into_iter().chain([3]))
            .with_post_damage_order([3])
    }
}

/// 一条 bed2 行的导入结果：名字、可选队伍与体力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2Import {
    pub name: String,
    pub team: Option<String>,
    pub hp: i32,
}

/// bed2 roster 导入失败：给出队伍、行内位置与原始文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2RosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
}

/// bed2 召唤物模板 overlay 配置：模板槽、召唤物类型与火球/自爆导出名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2SummonTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub summon_kind: PlayerKindId,
    pub fire_skill_export_name: &'a str,
    pub explode_skill_export_name: &'a str,
}

/// bed2 幻影模板 overlay 配置：模板槽、幻影类型与附体技能导出名。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2ShadowTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub shadow_kind: PlayerKindId,
    pub possess_skill_export_name: &'a str,
}

/// bed2 亡灵模板 overlay 配置：模板槽、亡灵类型与技能导出名前缀。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2ZombieTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub zombie_kind: PlayerKindId,
    pub skill_export_name_prefix: &'a str,
}

/// 三类使魔模板 overlay 的组合配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2MinionOverlayConfig<'a> {
    pub summon: CustomBed2SummonTemplateConfig<'a>,
    pub shadow: CustomBed2ShadowTemplateConfig<'a>,
    pub zombie: CustomBed2ZombieTemplateConfig<'a>,
}

/// 自定义 Runtime 导入配置：注册表、bed2 参数、overlay 与 handler 绑定。
#[derive(Clone)]
pub struct CustomRuntimeImportConfig<'a> {
    pub registry: ExtensionRegistry,
    pub bed2_kind: PlayerKindId,
    pub bed2_summon_skill: SkillId,
    pub bed2_minion_overlays: Option<CustomBed2MinionOverlayConfig<'a>>,
    pub skill_handlers: Vec<RuntimeSkillHandlerBinding>,
    pub state_handlers: Vec<RuntimeStateHandlerBinding>,
}

/// 一条 skill handler 绑定：技能 ID、函数与上下文能力。
#[derive(Clone)]
pub struct RuntimeSkillHandlerBinding {
    pub skill_id: SkillId,
    pub handler: SkillHandlerFn,
    pub capabilities: Vec<ExtensionCapability>,
}

/// 一条 state handler 绑定：状态 ID、函数与上下文能力。
#[derive(Clone)]
pub struct RuntimeStateHandlerBinding {
    pub state_id: StateId,
    pub handler: StateHandlerFn,
    pub capabilities: Vec<ExtensionCapability>,
}

impl<'a> CustomRuntimeImportConfig<'a> {
    /// 构造只含注册表与 bed2 参数的导入配置。
    pub fn new(registry: ExtensionRegistry, bed2_kind: PlayerKindId, bed2_summon_skill: SkillId) -> Self {
        Self {
            registry,
            bed2_kind,
            bed2_summon_skill,
            bed2_minion_overlays: None,
            skill_handlers: Vec::new(),
            state_handlers: Vec::new(),
        }
    }

    /// 附加使魔模板 overlay 配置。
    pub fn with_bed2_minion_overlays(mut self, config: CustomBed2MinionOverlayConfig<'a>) -> Self {
        self.bed2_minion_overlays = Some(config);
        self
    }

    /// 追加一条不声明额外能力的 skill handler 绑定。
    pub fn with_skill_handler(mut self, skill_id: SkillId, handler: SkillHandlerFn) -> Self {
        self.skill_handlers.push(RuntimeSkillHandlerBinding {
            skill_id,
            handler,
            capabilities: Vec::new(),
        });
        self
    }

    /// 追加一条带上下文能力的 skill handler 绑定。
    pub fn with_skill_handler_with_capabilities(
        mut self,
        skill_id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) -> Self {
        self.skill_handlers.push(RuntimeSkillHandlerBinding {
            skill_id,
            handler,
            capabilities: capabilities.to_vec(),
        });
        self
    }

    /// 追加一条不声明额外能力的 state handler 绑定。
    pub fn with_state_handler(mut self, state_id: StateId, handler: StateHandlerFn) -> Self {
        self.state_handlers.push(RuntimeStateHandlerBinding {
            state_id,
            handler,
            capabilities: Vec::new(),
        });
        self
    }

    /// 追加一条带上下文能力的 state handler 绑定。
    pub fn with_state_handler_with_capabilities(
        mut self,
        state_id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) -> Self {
        self.state_handlers.push(RuntimeStateHandlerBinding {
            state_id,
            handler,
            capabilities: capabilities.to_vec(),
        });
        self
    }
}

/// 默认 profile 组装失败：注册表构建错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultCustomRuntimeProfileError {
    Registry(ExtensionError),
}

impl From<ExtensionError> for DefaultCustomRuntimeProfileError {
    /// 把注册表构建错误转换为 [`DefaultCustomRuntimeProfileError::Registry`] 变体。
    fn from(error: ExtensionError) -> Self { Self::Registry(error) }
}

/// 召唤物模板 overlay 导入失败：roster、导出名缺失或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2SummonTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2SummonTemplateImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2SummonTemplateImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2SummonTemplateImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2SummonTemplateImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2SummonTemplateImportError {
    /// 把槽错误转换为 [`CustomBed2SummonTemplateImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 幻影模板 overlay 导入失败：roster、导出名缺失或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ShadowTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ShadowTemplateImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2ShadowTemplateImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ShadowTemplateImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2ShadowTemplateImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ShadowTemplateImportError {
    /// 把槽错误转换为 [`CustomBed2ShadowTemplateImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 亡灵模板 overlay 导入失败：roster、导出名缺失或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ZombieTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ZombieTemplateImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2ZombieTemplateImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ZombieTemplateImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2ZombieTemplateImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ZombieTemplateImportError {
    /// 把槽错误转换为 [`CustomBed2ZombieTemplateImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 使魔模板 overlay 组合导入失败：任一步骤错误或槽错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2MinionOverlayImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Summon(CustomBed2SummonTemplateImportError),
    Shadow(CustomBed2ShadowTemplateImportError),
    Zombie(CustomBed2ZombieTemplateImportError),
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2MinionOverlayImportError {
    /// 把 bed2 roster 错误转换为 [`CustomBed2MinionOverlayImportError::Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2MinionOverlayImportError {
    /// 把混合 roster 错误转换为 [`CustomBed2MinionOverlayImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2SummonTemplateImportError> for CustomBed2MinionOverlayImportError {
    /// 把召唤模板导入错误转换为 [`CustomBed2MinionOverlayImportError::Summon`] 变体。
    fn from(error: CustomBed2SummonTemplateImportError) -> Self { Self::Summon(error) }
}

impl From<CustomBed2ShadowTemplateImportError> for CustomBed2MinionOverlayImportError {
    /// 把影袭模板导入错误转换为 [`CustomBed2MinionOverlayImportError::Shadow`] 变体。
    fn from(error: CustomBed2ShadowTemplateImportError) -> Self { Self::Shadow(error) }
}

impl From<CustomBed2ZombieTemplateImportError> for CustomBed2MinionOverlayImportError {
    /// 把亡灵模板导入错误转换为 [`CustomBed2MinionOverlayImportError::Zombie`] 变体。
    fn from(error: CustomBed2ZombieTemplateImportError) -> Self { Self::Zombie(error) }
}

impl From<SlotError> for CustomBed2MinionOverlayImportError {
    /// 把槽错误转换为 [`CustomBed2MinionOverlayImportError::Slot`] 变体。
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

/// 自定义 Runtime 导入的顶层错误，汇总 roster、overlay、初始化与就绪校验失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomRuntimeImportError {
    Bed2Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Bed2MinionOverlay(CustomBed2MinionOverlayImportError),
    BattleInit(RuntimeBattleInitError),
    NotReady(RuntimeReadyError),
}

impl From<CustomBed2RosterImportError> for CustomRuntimeImportError {
    /// 把 bed2 roster 错误转换为 [`CustomRuntimeImportError::Bed2Roster`] 变体。
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Bed2Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomRuntimeImportError {
    /// 把混合 roster 错误转换为 [`CustomRuntimeImportError::MixedRoster`] 变体。
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2MinionOverlayImportError> for CustomRuntimeImportError {
    /// 把使魔 overlay 导入错误转换为 [`CustomRuntimeImportError::Bed2MinionOverlay`] 变体。
    fn from(error: CustomBed2MinionOverlayImportError) -> Self { Self::Bed2MinionOverlay(error) }
}

impl From<RuntimeBattleInitError> for CustomRuntimeImportError {
    /// 把战斗初始化错误转换为 [`CustomRuntimeImportError::BattleInit`] 变体。
    fn from(error: RuntimeBattleInitError) -> Self { Self::BattleInit(error) }
}

impl From<RuntimeReadyError> for CustomRuntimeImportError {
    /// 把就绪校验错误转换为 [`CustomRuntimeImportError::NotReady`] 变体。
    fn from(error: RuntimeReadyError) -> Self { Self::NotReady(error) }
}

/// 混合 roster 导入失败：给出队伍、行内位置、原文与原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMixedRosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
    pub message: String,
}

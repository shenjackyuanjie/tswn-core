use super::*;

mod hooks;
mod import;
mod registry;

pub use hooks::*;
pub use registry::*;

pub const DEFAULT_BED2_HP: i32 = 3000;
pub const DEFAULT_BED2_DEFENSE: i32 = 99;
pub const DEFAULT_BED2_RESISTANCE: i32 = 99;

pub const DEFAULT_CORE_DEFEND_SKILL_EXPORT: &str = "core.skill.defend";
pub const DEFAULT_CORE_REFLECT_SKILL_EXPORT: &str = "core.skill.reflect";
pub const DEFAULT_CORE_PROTECT_SKILL_EXPORT: &str = "core.skill.protect";
pub const DEFAULT_CORE_SHIELD_SKILL_EXPORT: &str = "core.skill.shield";
pub const DEFAULT_CORE_UPGRADE_SKILL_EXPORT: &str = "core.skill.upgrade";
pub const DEFAULT_CORE_HIDE_SKILL_EXPORT: &str = "core.skill.hide";
pub const DEFAULT_CORE_COUNTER_SKILL_EXPORT: &str = "core.skill.counter";
pub const DEFAULT_CORE_MERGE_SKILL_EXPORT: &str = "core.skill.merge";
pub const DEFAULT_CORE_ZOMBIE_SKILL_EXPORT: &str = "core.skill.zombie";
pub const DEFAULT_CORE_RERAISE_SKILL_EXPORT: &str = "core.skill.reraise";
pub const DEFAULT_CORE_CHARM_STATE_EXPORT: &str = "core.state.charm";
pub const DEFAULT_CORE_CURSE_STATE_EXPORT: &str = "core.state.curse";
pub const DEFAULT_CORE_POISON_STATE_EXPORT: &str = "core.state.poison";
pub const DEFAULT_CORE_HASTE_STATE_EXPORT: &str = "core.state.haste";
pub const DEFAULT_CORE_SLOW_STATE_EXPORT: &str = "core.state.slow";
pub const DEFAULT_CORE_IRON_STATE_EXPORT: &str = "core.state.iron";
pub const DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT: &str = "core.state.covid-infection";
pub const DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT: &str = "core.state.lazy-infection";
pub const DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT: &str = "core.state.saitama-boss";
pub const DEFAULT_CORE_SHADOW_KIND_EXPORT: &str = "core.kind.shadow";
pub const DEFAULT_CORE_SUMMON_KIND_EXPORT: &str = "core.kind.summon";
pub const DEFAULT_CORE_ZOMBIE_KIND_EXPORT: &str = "core.kind.zombie";
pub const DEFAULT_CORE_CLONE_KIND_EXPORT: &str = "core.kind.clone";
pub const DEFAULT_CORE_BOSS_KIND_EXPORT: &str = "core.kind.boss";
pub const DEFAULT_CORE_BOOST_KIND_EXPORT: &str = "core.kind.boost";
pub const DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.shadow_blueprint";
pub const DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.summon_blueprint";
pub const DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.zombie_blueprint";
pub const DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT: &str = "core.entity.lazy_blueprint_rq";
pub const DEFAULT_CORE_SUMMON_ENTITY_EXPORT: &str = "core.entity.summoned_entity";
pub const DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT: &str = "core.entity.minion_counter";
pub const DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT: &str = "core.skill.summon-explode";
pub const DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT: &str = "core.skill.summon-share-damage";
pub const DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT: &str = "custom.summon";
pub const DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT: &str = "custom.summon.fire";
pub const DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT: &str = "custom.summon.explode";
pub const DEFAULT_CUSTOM_BED2_SUMMON_ENTITY_EXPORT: &str = "custom.bed2.summoned_entity";
pub const DEFAULT_CUSTOM_BED2_SUMMON_TEMPLATE_EXPORT: &str = "custom.bed2.summon_template";
pub const DEFAULT_CUSTOM_BED2_SHADOW_TEMPLATE_EXPORT: &str = "custom.bed2.shadow_template";
pub const DEFAULT_CUSTOM_BED2_ZOMBIE_TEMPLATE_EXPORT: &str = "custom.bed2.zombie_template";
pub const DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT: &str = "custom.bed2.summon";
pub const DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT: &str = "custom.bed2.shadow";
pub const DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT: &str = "custom.bed2.zombie";
pub const DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT: &str = "custom.minion.possess";
pub const DEFAULT_CUSTOM_MINION_SKILL_EXPORT_PREFIX: &str = "custom.minion";

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
}

pub const PLAIN_FIRE_STATE_KEY: u32 = 0;
pub const PLAIN_ICE_STATE_KEY: u32 = 1;
pub const PLAIN_BERSERK_STATE_KEY: u32 = 10;
pub const PLAIN_CURSE_STATE_KEY: u32 = 73;
pub const PLAIN_POISON_STATE_KEY: u32 = 75;
pub const PLAIN_HASTE_STATE_KEY: u32 = 77;
pub const PLAIN_IRON_STATE_KEY: u32 = 79;
pub const PLAIN_COVID_BOSS_STATE_KEY: u32 = 90;
pub const PLAIN_COVID_INFECTION_STATE_KEY: u32 = 91;
pub const PLAIN_LAZY_BOSS_STATE_KEY: u32 = 92;
pub const PLAIN_LAZY_INFECTION_STATE_KEY: u32 = 93;
pub const PLAIN_SAITAMA_BOSS_STATE_KEY: u32 = 94;

impl BuiltinActiveSkill {
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

    pub const ALL: [Self; 26] = [
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
    ];

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
        }
    }

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
        }
    }

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
        }
    }

    pub const fn target_policy(self) -> TargetPolicy {
        match self {
            Self::Iron | Self::Charge | Self::Accumulate | Self::Summon | Self::Clone | Self::Shadow => TargetPolicy::None,
            Self::Haste | Self::Heal | Self::Revive => TargetPolicy::Ally,
            _ => TargetPolicy::Enemy,
        }
    }

    pub fn from_legacy_key(key: usize) -> Option<Self> { Self::ALL.into_iter().find(|skill| skill.legacy_key() == key) }

    pub fn from_export_name(export_name: &str) -> Option<Self> {
        if export_name == DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT {
            return Some(Self::SummonExplode);
        }
        Self::ALL.into_iter().find(|skill| skill.export_name() == export_name)
    }
}

#[derive(Debug, Clone)]
pub struct PlainLegacySkillImportMap {
    active_by_legacy_key: [Option<SkillId>; 256],
    plain_by_legacy_key: [Option<SkillId>; 35],
    special_by_runtime_kind: [(&'static str, Option<SkillId>); 4],
    passive_by_runtime_kind: [(&'static str, Option<SkillId>); 10],
}

impl PlainLegacySkillImportMap {
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
        Self {
            active_by_legacy_key,
            plain_by_legacy_key,
            special_by_runtime_kind: [
                (
                    std::any::type_name::<crate::player::skill::act::fire::FireSkill>(),
                    registry.skill_id_by_export_name(BuiltinActiveSkill::Fire.export_name()),
                ),
                (
                    std::any::type_name::<crate::player::skill::act::summon::SummonExplodeSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::act::summon::SummonShareDamageSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::act::possess::PossessSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT),
                ),
            ],
            passive_by_runtime_kind: [
                (
                    std::any::type_name::<crate::player::skill::defend::DefendSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_DEFEND_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::reflect::ReflectSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_REFLECT_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::protect::ProtectSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::shield::ShieldSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_SHIELD_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::upgrade::UpgradeSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_UPGRADE_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::hide::HideSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::counter::CounterSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_COUNTER_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::merge::MergeSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_MERGE_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::zombie::ZombieSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_ZOMBIE_SKILL_EXPORT),
                ),
                (
                    std::any::type_name::<crate::player::skill::reraise::ReraiseSkill>(),
                    registry.skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT),
                ),
            ],
        }
    }

    fn resolve(
        &self,
        key: usize,
        runtime_kind: &'static str,
        level: u32,
        boosted: bool,
        boost: Option<crate::player::skill::SkillBoost>,
    ) -> Option<(usize, SkillId, u32, bool, Option<crate::player::skill::SkillBoost>)> {
        let active_skill = if let Some((_, skill_id)) = self
            .special_by_runtime_kind
            .iter()
            .find(|(registered_kind, _)| *registered_kind == runtime_kind)
        {
            *skill_id
        } else if key < self.active_by_legacy_key.len() {
            self.active_by_legacy_key[key]
        } else {
            None
        };
        let skill_id = if let Some(skill_id) = active_skill {
            Some(skill_id)
        } else if let Some((_, skill_id)) = self
            .passive_by_runtime_kind
            .iter()
            .find(|(registered_kind, _)| *registered_kind == runtime_kind)
        {
            *skill_id
        } else {
            None
        };
        skill_id.map(|skill_id| (key, skill_id, level, boosted, boost))
    }

    fn finish_import(
        &self,
        imported: Vec<(usize, SkillId, u32, bool, Option<crate::player::skill::SkillBoost>)>,
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

    pub fn import(&self, snapshot: &crate::player::skill::store::SkillLoadoutSnapshot) -> SkillLoadout {
        let resolve = |entry: &crate::player::skill::store::SkillSnapshot| {
            self.resolve(entry.key, entry.runtime_kind, entry.level, entry.boosted, entry.boost.clone())
        };

        let mut imported = Vec::new();
        for key in &snapshot.fixed_lanes {
            let Some(entry) = snapshot.entries.iter().find(|entry| entry.key == *key) else {
                continue;
            };
            if let Some(mapped) = resolve(entry) {
                imported.push(mapped);
            }
        }
        for entry in &snapshot.entries {
            if imported.iter().any(|(key, _, _, _, _)| *key == entry.key) {
                continue;
            }
            if let Some(mapped) = resolve(entry) {
                imported.push(mapped);
            }
        }

        self.finish_import(
            imported,
            &snapshot.fixed_lanes,
            &snapshot.active_order,
            &snapshot.pre_action_order,
            &snapshot.post_damage_order,
            &snapshot.post_action_after_states,
        )
    }

    pub fn import_storage(&self, storage: &crate::player::skill::store::SkillStorage) -> SkillLoadout {
        let resolve = |key: usize| {
            let skill = storage.store.get(&key)?;
            self.resolve(
                key,
                skill.debug_skill_type_name(),
                skill.level(),
                skill.boosted,
                skill.diy_boost.clone(),
            )
        };

        let mut imported = Vec::new();
        let mut imported_keys = Vec::<bool>::new();
        for &key in &storage.slot_skill {
            if let Some(mapped) = resolve(key) {
                imported.push(mapped);
                if key >= imported_keys.len() {
                    imported_keys.resize(key + 1, false);
                }
                imported_keys[key] = true;
            }
        }
        for key in storage.store.keys() {
            if imported_keys.get(key).copied().unwrap_or(false) {
                continue;
            }
            if let Some(mapped) = resolve(key) {
                imported.push(mapped);
            }
        }

        self.finish_import(
            imported,
            &storage.slot_skill,
            &storage.skill,
            &storage.pre_action,
            &storage.post_damage,
            &storage.post_action_after_states,
        )
    }

    /// 直接导入无 overlay 的普通 score profile，跳过 legacy 技能对象与 proc 缓存。
    pub(crate) fn import_score_profile(
        &self,
        levels: [u32; 35],
        boosted: [bool; 35],
        boosts: [Option<crate::player::skill::SkillBoost>; 35],
        action_order: &[u32; 40],
    ) -> SkillLoadout {
        let mut lane_by_key = [usize::MAX; 35];
        let mut fixed_lane_keys = Vec::with_capacity(35);
        let mut imported = Vec::with_capacity(35);
        let mut imported_boosted = Vec::with_capacity(35);
        for key in 0..35 {
            let Some(skill_id) = self.plain_by_legacy_key[key] else {
                continue;
            };
            lane_by_key[key] = imported.len();
            fixed_lane_keys.push(key);
            imported.push((skill_id, levels[key], boosts[key].clone()));
            imported_boosted.push(boosted[key]);
        }
        let lane = |key: usize| lane_by_key.get(key).copied().filter(|lane| *lane != usize::MAX);
        let active_order = action_order.iter().filter_map(|key| lane(*key as usize)).collect::<Vec<_>>();
        let pre_action_order = [29usize, 34]
            .into_iter()
            .filter(|key| levels[*key] > 0)
            .filter_map(lane)
            .collect::<Vec<_>>();
        let post_damage_order = [30usize, 33, 34, 21]
            .into_iter()
            .filter(|key| levels[*key] > 0)
            .filter_map(lane)
            .collect::<Vec<_>>();
        let merge_lane_order = (0..35).filter_map(lane).collect::<Vec<_>>();

        SkillLoadout::from_skill_levels_and_boosts(imported)
            .with_fixed_lane_keys(fixed_lane_keys)
            .with_boosted_flags(imported_boosted)
            .with_merge_lane_order(merge_lane_order)
            .with_active_order(active_order)
            .with_pre_action_order(pre_action_order)
            .with_post_damage_order(post_damage_order)
    }
}

pub fn import_plain_legacy_skill_loadout(
    registry: &ExtensionRegistry,
    snapshot: &crate::player::skill::store::SkillLoadoutSnapshot,
) -> SkillLoadout {
    PlainLegacySkillImportMap::new(registry).import(snapshot)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2Import {
    pub name: String,
    pub team: Option<String>,
    pub hp: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomBed2RosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2SummonTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub summon_kind: PlayerKindId,
    pub fire_skill_export_name: &'a str,
    pub explode_skill_export_name: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2ShadowTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub shadow_kind: PlayerKindId,
    pub possess_skill_export_name: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2ZombieTemplateConfig<'a> {
    pub template_slot: TemplateSlotId,
    pub zombie_kind: PlayerKindId,
    pub skill_export_name_prefix: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomBed2MinionOverlayConfig<'a> {
    pub summon: CustomBed2SummonTemplateConfig<'a>,
    pub shadow: CustomBed2ShadowTemplateConfig<'a>,
    pub zombie: CustomBed2ZombieTemplateConfig<'a>,
}

#[derive(Clone)]
pub struct CustomRuntimeV2ImportConfig<'a> {
    pub registry: ExtensionRegistry,
    pub bed2_kind: PlayerKindId,
    pub bed2_summon_skill: SkillId,
    pub bed2_minion_overlays: Option<CustomBed2MinionOverlayConfig<'a>>,
    pub skill_handlers: Vec<RuntimeV2SkillHandlerBinding>,
    pub state_handlers: Vec<RuntimeV2StateHandlerBinding>,
}

#[derive(Clone)]
pub struct RuntimeV2SkillHandlerBinding {
    pub skill_id: SkillId,
    pub handler: SkillHandlerFn,
    pub capabilities: Vec<ExtensionCapability>,
}

#[derive(Clone)]
pub struct RuntimeV2StateHandlerBinding {
    pub state_id: StateId,
    pub handler: StateHandlerFn,
    pub capabilities: Vec<ExtensionCapability>,
}

impl<'a> CustomRuntimeV2ImportConfig<'a> {
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

    pub fn with_bed2_minion_overlays(mut self, config: CustomBed2MinionOverlayConfig<'a>) -> Self {
        self.bed2_minion_overlays = Some(config);
        self
    }

    pub fn with_skill_handler(mut self, skill_id: SkillId, handler: SkillHandlerFn) -> Self {
        self.skill_handlers.push(RuntimeV2SkillHandlerBinding {
            skill_id,
            handler,
            capabilities: Vec::new(),
        });
        self
    }

    pub fn with_skill_handler_with_capabilities(
        mut self,
        skill_id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) -> Self {
        self.skill_handlers.push(RuntimeV2SkillHandlerBinding {
            skill_id,
            handler,
            capabilities: capabilities.to_vec(),
        });
        self
    }

    pub fn with_state_handler(mut self, state_id: StateId, handler: StateHandlerFn) -> Self {
        self.state_handlers.push(RuntimeV2StateHandlerBinding {
            state_id,
            handler,
            capabilities: Vec::new(),
        });
        self
    }

    pub fn with_state_handler_with_capabilities(
        mut self,
        state_id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) -> Self {
        self.state_handlers.push(RuntimeV2StateHandlerBinding {
            state_id,
            handler,
            capabilities: capabilities.to_vec(),
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefaultCustomRuntimeV2ProfileError {
    Registry(ExtensionError),
}

impl From<ExtensionError> for DefaultCustomRuntimeV2ProfileError {
    fn from(error: ExtensionError) -> Self { Self::Registry(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2SummonTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2SummonTemplateImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2SummonTemplateImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2SummonTemplateImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ShadowTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ShadowTemplateImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ShadowTemplateImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ShadowTemplateImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomBed2ZombieTemplateImportError {
    Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    MissingSkillExportName { export_name: String },
    Slot(SlotError),
}

impl From<CustomBed2RosterImportError> for CustomBed2ZombieTemplateImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2ZombieTemplateImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<SlotError> for CustomBed2ZombieTemplateImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

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
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2SummonTemplateImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2SummonTemplateImportError) -> Self { Self::Summon(error) }
}

impl From<CustomBed2ShadowTemplateImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2ShadowTemplateImportError) -> Self { Self::Shadow(error) }
}

impl From<CustomBed2ZombieTemplateImportError> for CustomBed2MinionOverlayImportError {
    fn from(error: CustomBed2ZombieTemplateImportError) -> Self { Self::Zombie(error) }
}

impl From<SlotError> for CustomBed2MinionOverlayImportError {
    fn from(error: SlotError) -> Self { Self::Slot(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomRuntimeV2ImportError {
    Bed2Roster(CustomBed2RosterImportError),
    MixedRoster(CustomMixedRosterImportError),
    Bed2MinionOverlay(CustomBed2MinionOverlayImportError),
    BattleInit(RuntimeV2BattleInitError),
    NotReady(RuntimeV2ReadyError),
}

impl From<CustomBed2RosterImportError> for CustomRuntimeV2ImportError {
    fn from(error: CustomBed2RosterImportError) -> Self { Self::Bed2Roster(error) }
}

impl From<CustomMixedRosterImportError> for CustomRuntimeV2ImportError {
    fn from(error: CustomMixedRosterImportError) -> Self { Self::MixedRoster(error) }
}

impl From<CustomBed2MinionOverlayImportError> for CustomRuntimeV2ImportError {
    fn from(error: CustomBed2MinionOverlayImportError) -> Self { Self::Bed2MinionOverlay(error) }
}

impl From<RuntimeV2BattleInitError> for CustomRuntimeV2ImportError {
    fn from(error: RuntimeV2BattleInitError) -> Self { Self::BattleInit(error) }
}

impl From<RuntimeV2ReadyError> for CustomRuntimeV2ImportError {
    fn from(error: RuntimeV2ReadyError) -> Self { Self::NotReady(error) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomMixedRosterImportError {
    pub team_index: usize,
    pub player_index: usize,
    pub raw: String,
    pub message: String,
}

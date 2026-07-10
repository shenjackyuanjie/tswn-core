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
pub const DEFAULT_CORE_BOSS_KIND_EXPORT: &str = "core.kind.boss";
pub const DEFAULT_CORE_BOOST_KIND_EXPORT: &str = "core.kind.boost";
pub const DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.shadow_blueprint";
pub const DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT: &str = "core.entity.summon_blueprint";
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

pub fn import_plain_legacy_skill_loadout(
    registry: &ExtensionRegistry,
    snapshot: &crate::player::skill::store::SkillLoadoutSnapshot,
) -> SkillLoadout {
    let defend_kind = std::any::type_name::<crate::player::skill::defend::DefendSkill>();
    let defend_skill = registry.skill_id_by_export_name(DEFAULT_CORE_DEFEND_SKILL_EXPORT);
    let reflect_kind = std::any::type_name::<crate::player::skill::reflect::ReflectSkill>();
    let reflect_skill = registry.skill_id_by_export_name(DEFAULT_CORE_REFLECT_SKILL_EXPORT);
    let protect_kind = std::any::type_name::<crate::player::skill::protect::ProtectSkill>();
    let protect_skill = registry.skill_id_by_export_name(DEFAULT_CORE_PROTECT_SKILL_EXPORT);
    let shield_kind = std::any::type_name::<crate::player::skill::shield::ShieldSkill>();
    let shield_skill = registry.skill_id_by_export_name(DEFAULT_CORE_SHIELD_SKILL_EXPORT);
    let upgrade_kind = std::any::type_name::<crate::player::skill::upgrade::UpgradeSkill>();
    let upgrade_skill = registry.skill_id_by_export_name(DEFAULT_CORE_UPGRADE_SKILL_EXPORT);
    let hide_kind = std::any::type_name::<crate::player::skill::hide::HideSkill>();
    let hide_skill = registry.skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT);
    let counter_kind = std::any::type_name::<crate::player::skill::counter::CounterSkill>();
    let counter_skill = registry.skill_id_by_export_name(DEFAULT_CORE_COUNTER_SKILL_EXPORT);
    let merge_kind = std::any::type_name::<crate::player::skill::merge::MergeSkill>();
    let merge_skill = registry.skill_id_by_export_name(DEFAULT_CORE_MERGE_SKILL_EXPORT);
    let reraise_kind = std::any::type_name::<crate::player::skill::reraise::ReraiseSkill>();
    let reraise_skill = registry.skill_id_by_export_name(DEFAULT_CORE_RERAISE_SKILL_EXPORT);
    let fire_kind = std::any::type_name::<crate::player::skill::act::fire::FireSkill>();
    let fire_skill = registry.skill_id_by_export_name(BuiltinActiveSkill::Fire.export_name());
    let summon_explode_kind = std::any::type_name::<crate::player::skill::act::summon::SummonExplodeSkill>();
    let summon_explode_skill = registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT);
    let summon_share_kind = std::any::type_name::<crate::player::skill::act::summon::SummonShareDamageSkill>();
    let summon_share_skill = registry.skill_id_by_export_name(DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT);
    let possess_kind = std::any::type_name::<crate::player::skill::act::possess::PossessSkill>();
    let possess_skill = registry.skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT);
    let resolve = |entry: &crate::player::skill::store::SkillSnapshot| {
        let active_skill = if entry.runtime_kind == fire_kind {
            fire_skill
        } else if entry.runtime_kind == summon_explode_kind {
            summon_explode_skill
        } else if entry.runtime_kind == summon_share_kind {
            summon_share_skill
        } else if entry.runtime_kind == possess_kind {
            possess_skill
        } else {
            BuiltinActiveSkill::from_legacy_key(entry.key).and_then(|skill| registry.skill_id_by_export_name(skill.export_name()))
        };
        let skill_id = if let Some(skill_id) = active_skill {
            Some(skill_id)
        } else if entry.runtime_kind == defend_kind {
            defend_skill
        } else if entry.runtime_kind == reflect_kind {
            reflect_skill
        } else if entry.runtime_kind == protect_kind {
            protect_skill
        } else if entry.runtime_kind == shield_kind {
            shield_skill
        } else if entry.runtime_kind == upgrade_kind {
            upgrade_skill
        } else if entry.runtime_kind == hide_kind {
            hide_skill
        } else if entry.runtime_kind == counter_kind {
            counter_skill
        } else if entry.runtime_kind == merge_kind {
            merge_skill
        } else if entry.runtime_kind == reraise_kind {
            reraise_skill
        } else {
            None
        };
        skill_id.map(|skill_id| (entry.key, skill_id, entry.level, entry.boost.clone()))
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
        if imported.iter().any(|(key, _, _, _)| *key == entry.key) {
            continue;
        }
        if let Some(mapped) = resolve(entry) {
            imported.push(mapped);
        }
    }

    let mut active_order = snapshot
        .active_order
        .iter()
        .filter_map(|key| imported.iter().position(|(imported_key, _, _, _)| imported_key == key))
        .collect::<Vec<_>>();
    for lane in 0..imported.len() {
        if !active_order.contains(&lane) {
            active_order.push(lane);
        }
    }
    let pre_action_order = snapshot
        .pre_action_order
        .iter()
        .filter_map(|key| imported.iter().position(|(imported_key, _, _, _)| imported_key == key))
        .collect::<Vec<_>>();

    let fixed_lane_keys = imported.iter().map(|(key, _, _, _)| *key).collect::<Vec<_>>();
    SkillLoadout::from_skill_levels_and_boosts(imported.into_iter().map(|(_, skill_id, level, boost)| (skill_id, level, boost)))
        .with_fixed_lane_keys(fixed_lane_keys)
        .with_active_order(active_order)
        .with_pre_action_order(pre_action_order)
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

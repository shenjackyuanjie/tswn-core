//! 面向学习数据的机制状态；不包含名字文本、战斗随机数、未来标签或展示缓存。
use super::entity::{AccumulateRuntime, ChargeRuntime};
use super::*;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

impl From<&SkillLoadout> for ModelSkills {
    fn from(value: &SkillLoadout) -> Self {
        use crate::namerena::SkillBoost;
        Self {
            lanes: value
                .skills()
                .iter()
                .enumerate()
                .map(|(lane, skill)| ModelSkill {
                    skill_id: skill.0 + 1,
                    level: value.level_at(lane).unwrap(),
                    build_level: value.build_level_at(lane).unwrap(),
                    boost: value.boost_at(lane).map(|boost| match boost {
                        SkillBoost::Normal(base) => ModelSkillBoost {
                            kind: "normal".into(),
                            base: *base,
                            extra: 0,
                        },
                        SkillBoost::LastBoost(base) => ModelSkillBoost {
                            kind: "last_boost".into(),
                            base: *base,
                            extra: 0,
                        },
                        SkillBoost::SlotBoost { base, boost } => ModelSkillBoost {
                            kind: "slot_boost".into(),
                            base: *base,
                            extra: *boost,
                        },
                    }),
                    boosted: value.boosted_at(lane).unwrap(),
                    fixed_lane_key: value.fixed_lane_key_at(lane).unwrap(),
                })
                .collect(),
            merge_lane_order: value.merge_lane_order().to_vec(),
            active_order: value.active_order().to_vec(),
            pre_action_order: value.pre_action_order().to_vec(),
            post_damage_order: value.post_damage_order().to_vec(),
            post_action_after_states: value
                .post_action_after_states()
                .iter()
                .map(|(state_cursor, fixed_lane)| ModelDeferredSkill {
                    state_cursor: *state_cursor,
                    fixed_lane: *fixed_lane,
                })
                .collect(),
        }
    }
}

impl ModelSlot {
    fn project(slot_id: u32, value: &SlotValue, clans: &mut Vec<String>) -> Result<Self, ModelStateError> {
        let mut slot = Self {
            slot_id,
            bool_value: None,
            i64_value: None,
            u64_value: None,
            template: None,
        };
        match value {
            SlotValue::Bool(value) => slot.bool_value = Some(*value),
            SlotValue::I64(value) => slot.i64_value = Some(*value),
            SlotValue::U64(value) => slot.u64_value = Some(*value),
            SlotValue::PlayerTemplate(value) => slot.template = Some(ModelTemplate::project(value, clans)),
            SlotValue::Text(_) => return Err(ModelStateError(format!("未审计的文本槽 {slot_id}"))),
        }
        Ok(slot)
    }
}

impl RuntimeRunner {
    /// 导出默认规则的完整当前机制状态，不包含战斗 RNG。
    pub fn model_state(&self) -> Result<BattleModelState, ModelStateError> {
        static DEFAULT_REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();
        let registry =
            DEFAULT_REGISTRY.get_or_init(|| default_custom_runtime_import_config().expect("默认注册表必须有效").registry);
        let rt = &self.runtime;
        if &rt.registry != registry {
            return Err(ModelStateError("状态导出只支持已审计的默认规则注册表".into()));
        }
        self.validate_ready().map_err(|error| ModelStateError(error.to_string()))?;
        if !rt.effects.is_empty() {
            return Err(ModelStateError("只能在效果队列已排空的回合边界导出".into()));
        }
        if rt.template_slots.len() > registry.template_slots().len() || rt.slots.len() > registry.battle_slots().len() {
            return Err(ModelStateError("存在未注册的全局槽".into()));
        }
        let mut clans = Vec::new();
        let mut entities = Vec::new();
        for (id, entity) in rt.entities.iter() {
            if entity.slots.len() > registry.entity_slots().len()
                || entity.states.entries().len() != entity.states.runtime_registration_orders().len()
            {
                return Err(ModelStateError(format!("实体 {} 的槽或状态注册表不完整", id.0)));
            }
            if let Some(slot) = registry.entity_slot_id_by_export_name(DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT)
                && let Some(value) = entity.slots.get(slot)
                && (!matches!(value, SlotValue::U64(_)) || entity.template.clone_build.is_none())
            {
                return Err(ModelStateError(format!("实体 {} 的延迟蓝图参数无效", id.0)));
            }
            let input_team_index = self
                .input_groups
                .iter()
                .position(|team| team.contains(&entity.runtime.root_owner))
                .ok_or_else(|| ModelStateError(format!("实体 {} 无法映射到输入队伍", id.0)))?;
            let mut slots = Vec::new();
            for index in 0..registry.entity_slots().len() {
                let slot_id = EntitySlotId(index as u32);
                let export = &registry.entity_slot(slot_id).unwrap().export_name;
                let kind = match export.as_str() {
                    DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT => Some(crate::namerena::MinionKind::Shadow),
                    DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT => Some(crate::namerena::MinionKind::Summon),
                    DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT => Some(crate::namerena::MinionKind::Zombie),
                    _ => None,
                };
                if kind.is_some()
                    && entity
                        .slots
                        .get(slot_id)
                        .is_some_and(|value| !matches!(value, SlotValue::PlayerTemplate(_)))
                {
                    return Err(ModelStateError(format!("实体 {} 的蓝图槽 {} 类型无效", id.0, slot_id.0)));
                }
                // 缓存未生成时计算同一蓝图，但绝不写回 Runtime。蓝图自身的名字随机化与战斗 RNG 无关。
                let preview = kind
                    .filter(|_| entity.template.clone_build.is_some())
                    .and_then(|kind| rt.preview_plain_minion_blueprint(id, kind));
                if let Some(template) = preview {
                    slots.push(ModelSlot::project(
                        slot_id.0,
                        &SlotValue::PlayerTemplate(Box::new(template)),
                        &mut clans,
                    )?);
                } else if let Some(value) = entity.slots.get(slot_id) {
                    slots.push(ModelSlot::project(slot_id.0, value, &mut clans)?);
                }
            }
            entities.push(ModelEntity {
                id,
                input_team_index,
                template: ModelTemplate::project(&entity.template, &mut clans),
                runtime: ModelPlayerRuntime::from(&entity.runtime),
                states: entity
                    .states
                    .entries()
                    .iter()
                    .zip(entity.states.runtime_registration_orders())
                    .map(|(entry, order)| ModelStateEntry {
                        legacy_order_key: entry.legacy_order_key,
                        extension_state_id: entry.extension_state_id.map(|id| id.0),
                        hook_mask: entry.hook_mask.0,
                        priority: entry.priority.0,
                        registration_order: entry.registration_order.0,
                        runtime_registration_order: *order,
                        payload: ModelPayload::from(&entry.payload),
                    })
                    .collect(),
                state_registration_cursor: entity.states.post_action_registration_cursor(),
                compressed_state_flags: entity.states.model_compressed_flags(),
                slots,
            });
        }
        let template_slots = rt
            .template_slots
            .iter()
            .map(|(id, value)| ModelSlot::project(id.0, value, &mut clans))
            .collect::<Result<_, _>>()?;
        let battle_slots = (0..registry.battle_slots().len())
            .filter_map(|index| rt.slots.get(BattleSlotId(index as u32)).map(|value| (index, value)))
            .map(|(index, value)| ModelSlot::project(index as u32, value, &mut clans))
            .collect::<Result<_, _>>()?;
        let state = BattleModelState {
            schema_version: MODEL_STATE_SCHEMA_VERSION,
            round: rt.round,
            entity_slot_count: rt.entities.len(),
            input_teams: self.input_groups.clone(),
            world: rt.world.model_state(),
            entities,
            template_slots,
            battle_slots,
            legacy_step_scheduler: rt.scheduler.uses_legacy_step_scheduler(),
            ice_release_events: rt.scheduler.model_ice_events(),
        };
        state.validate()?;
        Ok(state)
    }
}

impl BattleModelState {
    /// 检查实体、队伍与顺序引用；可在数据集回读时再次执行。
    pub fn validate(&self) -> Result<(), ModelStateError> {
        use std::collections::BTreeSet;
        let ids: BTreeSet<_> = self.entities.iter().map(|entity| entity.id).collect();
        let check = |id: EntityIdx| -> Result<(), ModelStateError> {
            if ids.contains(&id) {
                Ok(())
            } else {
                Err(ModelStateError(format!("无效实体引用 {}", id.0)))
            }
        };
        if self.schema_version != MODEL_STATE_SCHEMA_VERSION
            || ids.len() != self.entities.len()
            || self.entities.iter().any(|e| e.id.0 as usize >= self.entity_slot_count)
        {
            return Err(ModelStateError("schema 版本或实体编号无效".into()));
        }
        for id in self
            .input_teams
            .iter()
            .flatten()
            .chain(self.world.round_order.iter())
            .chain(self.world.flat_alive.iter())
            .chain(self.world.team_roster.iter().flatten())
            .chain(self.world.team_alive.iter().flatten())
            .chain(self.ice_release_events.iter())
        {
            check(*id)?;
        }
        for entity in &self.entities {
            let runtime = &entity.runtime;
            check(runtime.owner)?;
            check(runtime.root_owner)?;
            if !self
                .input_teams
                .get(entity.input_team_index)
                .is_some_and(|team| team.contains(&runtime.root_owner))
            {
                return Err(ModelStateError("输入队伍归属无效".into()));
            }
            if let Some(id) = runtime.protect_to {
                check(id)?;
            }
            for link in &runtime.protect_from {
                check(link.owner)?;
            }
            if let Some(assassinate) = runtime.assassinate {
                check(assassinate.target)?;
            }
            if let Some(id) = runtime.counter.last_target {
                check(id)?;
            }
            for state in &entity.states {
                let payload = &state.payload;
                if let Some(poison) = &payload.poison {
                    for id in [poison.caster, poison.target].into_iter().flatten() {
                        check(EntityIdx(id))?;
                    }
                }
                if let Some(charm) = &payload.charm
                    && let Some(id) = charm.target
                {
                    check(EntityIdx(id))?;
                }
                if let Some(infection) = &payload.covid_infection {
                    for entry in &infection.entries {
                        check(entry.boss)?;
                    }
                }
                if let Some(boss) = &payload.saitama_boss {
                    for id in boss.hitters.iter().chain(&boss.minions) {
                        check(*id)?;
                    }
                }
                if let Some(infection) = &payload.lazy_infection {
                    check(infection.boss)?;
                }
            }
        }
        Ok(())
    }
}

pub const MODEL_STATE_SCHEMA_VERSION: u32 = 1;

/// 编号空间固定为 1–50；当前 42 个处理器沿用注册顺序，43–50 预留。
pub const MODEL_SKILL_ID_LIMIT: u32 = 50;
pub const MODEL_SKILL_EXPORTS: [&str; 42] = [
    "custom.summon",
    "custom.summon.fire",
    "custom.summon.explode",
    "custom.minion.possess",
    "custom.minion.heal",
    "core.skill.fire",
    "core.skill.ice",
    "core.skill.thunder",
    "core.skill.quake",
    "core.skill.absorb",
    "core.skill.poison",
    "core.skill.rapid",
    "core.skill.critical",
    "core.skill.half",
    "core.skill.exchange",
    "core.skill.berserk",
    "core.skill.charm",
    "core.skill.haste",
    "core.skill.slow",
    "core.skill.curse",
    "core.skill.heal",
    "core.skill.revive",
    "core.skill.disperse",
    "core.skill.iron",
    "core.skill.charge",
    "core.skill.accumulate",
    "core.skill.assassinate",
    "core.skill.summon",
    "core.skill.clone",
    "core.skill.shadow",
    "core.skill.summon-explode",
    "core.skill.summon-share-damage",
    "core.skill.shield",
    "core.skill.protect",
    "core.skill.defend",
    "core.skill.reflect",
    "core.skill.upgrade",
    "core.skill.hide",
    "core.skill.counter",
    "core.skill.merge",
    "core.skill.zombie",
    "core.skill.reraise",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStateError(pub String);
impl std::fmt::Display for ModelStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(&self.0) }
}
impl std::error::Error for ModelStateError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleModelState {
    pub schema_version: u32,
    pub round: u64,
    /// 包括为使魔生成而保留的空 ID 槽；与实体列表长度不同。
    pub entity_slot_count: usize,
    pub input_teams: Vec<Vec<EntityIdx>>,
    pub world: ModelWorld,
    pub entities: Vec<ModelEntity>,
    pub template_slots: Vec<ModelSlot>,
    pub battle_slots: Vec<ModelSlot>,
    pub legacy_step_scheduler: bool,
    pub ice_release_events: Vec<EntityIdx>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEntity {
    pub id: EntityIdx,
    pub input_team_index: usize,
    pub template: ModelTemplate,
    pub runtime: ModelPlayerRuntime,
    pub states: Vec<ModelStateEntry>,
    pub state_registration_cursor: u64,
    /// 位次依次是 Shield、Protect、Upgrade、Corpse、Minion。
    pub compressed_state_flags: u8,
    pub slots: Vec<ModelSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelWorld {
    pub round_order: Vec<EntityIdx>,
    pub team_roster: Vec<Vec<EntityIdx>>,
    pub team_alive: Vec<Vec<EntityIdx>>,
    pub flat_alive: Vec<EntityIdx>,
    pub alive_group_count: usize,
    pub round_pos: i32,
    pub winner_team: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelIdentity {
    /// 仅表达阵营相等关系，不保存阵营文本或哈希。
    pub clan_group: usize,
    pub boss_kind: Option<usize>,
    pub boss_action_prob_count: usize,
    pub boost_immune_threshold: u32,
    pub immunity: Vec<ModelImmunity>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelImmunity {
    pub status: String,
    pub threshold: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSkill {
    /// 默认注册表的稳定 1-based 技能 ID；不是 fixed_lane 或 legacy_key。
    pub skill_id: u32,
    pub level: u32,
    pub build_level: u32,
    pub boost: Option<ModelSkillBoost>,
    pub boosted: bool,
    pub fixed_lane_key: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSkillBoost {
    pub kind: String,
    pub base: u32,
    pub extra: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDeferredSkill {
    pub state_cursor: u64,
    pub fixed_lane: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSkills {
    pub lanes: Vec<ModelSkill>,
    pub merge_lane_order: Vec<usize>,
    pub active_order: Vec<usize>,
    pub pre_action_order: Vec<usize>,
    pub post_damage_order: Vec<usize>,
    pub post_action_after_states: Vec<ModelDeferredSkill>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSlot {
    pub slot_id: u32,
    pub bool_value: Option<bool>,
    pub i64_value: Option<i64>,
    pub u64_value: Option<u64>,
    pub template: Option<ModelTemplate>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelStateEntry {
    pub legacy_order_key: u32,
    pub extension_state_id: Option<u32>,
    pub hook_mask: u64,
    pub priority: i32,
    pub registration_order: u32,
    pub runtime_registration_order: u64,
    pub payload: ModelPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTemplate {
    pub identity: ModelIdentity,
    pub id: PlrId,
    pub reserved_player_ids_before_spawn: u32,
    pub kind: PlayerKindId,
    pub skills: ModelSkills,
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
    pub reuse_skills_on_recast: bool,
    pub reuse_stats_on_recast: bool,
    pub inherit_owner_def_res: bool,
}

impl ModelTemplate {
    fn project(value: &PlayerTemplate, clans: &mut Vec<String>) -> Self {
        let clan_group = clans.iter().position(|name| name == &value.clan_name).unwrap_or_else(|| {
            clans.push(value.clan_name.clone());
            clans.len() - 1
        });
        Self {
            identity: ModelIdentity {
                clan_group,
                boss_kind: crate::namerena::BOSS_NAMES.iter().position(|name| *name == value.name),
                boss_action_prob_count: crate::namerena::boss_action_prob_count(&value.name),
                boost_immune_threshold: crate::namerena::boost_value(&value.name),
                immunity: [
                    "assassinate",
                    "charm",
                    "berserk",
                    "half",
                    "curse",
                    "exchange",
                    "slow",
                    "ice",
                    "fire",
                ]
                .into_iter()
                .map(|status| ModelImmunity {
                    status: status.into(),
                    threshold: crate::namerena::boss_immune_threshold(&value.name, status),
                })
                .collect(),
            },
            id: value.id,
            reserved_player_ids_before_spawn: value.reserved_player_ids_before_spawn,
            kind: value.kind,
            skills: ModelSkills::from(&value.skills),
            team: value.team,
            max_hp: value.max_hp,
            attack: value.attack,
            magic: value.magic,
            magic_point: value.magic_point,
            wisdom: value.wisdom,
            speed: value.speed,
            defense: value.defense,
            resistance: value.resistance,
            agility: value.agility,
            at_boost_bits: value.at_boost_bits,
            at_boost_millionths: value.at_boost_millionths,
            attr_sum: value.attr_sum,
            atk_sum: value.atk_sum,
            attract_bits: value.attract_bits,
            move_state: value.move_state,
            policy_overrides: value.policy_overrides,
            clone_build: value.clone_build.clone(),
            reuse_skills_on_recast: value.reuse_skills_on_recast,
            reuse_stats_on_recast: value.reuse_stats_on_recast,
            inherit_owner_def_res: value.inherit_owner_def_res,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPayload {
    pub kind: String,
    pub fire_mag_half_steps: Option<i32>,
    pub ice: Option<IcePayload>,
    pub shield_value: Option<i32>,
    pub curse: Option<CursePayload>,
    pub poison: Option<PoisonPayload>,
    pub haste: Option<HastePayload>,
    pub berserk: Option<BerserkPayload>,
    pub charm: Option<CharmPayload>,
    pub slow: Option<SlowPayload>,
    pub iron: Option<IronPayload>,
    pub covid_boss: Option<CovidBossPayload>,
    pub covid_infection: Option<CovidInfectionPayload>,
    pub saitama_boss: Option<SaitamaBossPayload>,
    pub lazy_boss: Option<LazyBossPayload>,
    pub lazy_infection: Option<LazyInfectionPayload>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IcePayload {
    pub frozen_step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursePayload {
    pub prob: i32,
    pub multiply: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PoisonPayload {
    pub caster: Option<u32>,
    pub target: Option<u32>,
    pub atp_bits: u64,
    pub count: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HastePayload {
    pub faster: i32,
    pub effective_faster: i32,
    pub step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BerserkPayload {
    pub step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharmPayload {
    pub group_id: usize,
    pub effective_team_idx: Option<usize>,
    pub source_team_idx: Option<usize>,
    pub target: Option<u32>,
    pub step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlowPayload {
    pub step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IronPayload {
    pub protect: i32,
    pub step: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovidBossPayload {
    pub mutation: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CovidInfectionPayload {
    pub entries: Vec<CovidInfectionEntry>,
    pub mutation_set: Vec<i32>,
    pub recovered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaitamaBossPayload {
    pub turns: i32,
    pub damages: i32,
    pub hitters: Vec<EntityIdx>,
    pub minions: Vec<EntityIdx>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LazyBossPayload {
    pub at_boost_bits: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LazyInfectionPayload {
    pub boss: EntityIdx,
}

impl From<&StatePayload> for ModelPayload {
    fn from(value: &StatePayload) -> Self {
        let mut result = Self::default();
        match value {
            StatePayload::None => {
                result.kind = "none".into();
            }
            StatePayload::FireMagHalfSteps(value) => {
                result.kind = "fire_mag_half_steps".into();
                result.fire_mag_half_steps = Some(*value);
            }
            StatePayload::Ice { frozen_step } => {
                result.kind = "ice".into();
                result.ice = Some(IcePayload {
                    frozen_step: *frozen_step,
                });
            }
            StatePayload::ShieldValue(value) => {
                result.kind = "shield_value".into();
                result.shield_value = Some(*value);
            }
            StatePayload::Curse { prob, multiply } => {
                result.kind = "curse".into();
                result.curse = Some(CursePayload {
                    prob: *prob,
                    multiply: *multiply,
                });
            }
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                count,
            } => {
                result.kind = "poison".into();
                result.poison = Some(PoisonPayload {
                    caster: *caster,
                    target: *target,
                    atp_bits: *atp_bits,
                    count: *count,
                });
            }
            StatePayload::Haste {
                faster,
                effective_faster,
                step,
            } => {
                result.kind = "haste".into();
                result.haste = Some(HastePayload {
                    faster: *faster,
                    effective_faster: *effective_faster,
                    step: *step,
                });
            }
            StatePayload::Berserk { step } => {
                result.kind = "berserk".into();
                result.berserk = Some(BerserkPayload { step: *step });
            }
            StatePayload::Charm {
                group_id,
                effective_team_idx,
                source_team_idx,
                target,
                step,
            } => {
                result.kind = "charm".into();
                result.charm = Some(CharmPayload {
                    group_id: *group_id,
                    effective_team_idx: *effective_team_idx,
                    source_team_idx: *source_team_idx,
                    target: *target,
                    step: *step,
                });
            }
            StatePayload::Slow { step } => {
                result.kind = "slow".into();
                result.slow = Some(SlowPayload { step: *step });
            }
            StatePayload::Iron { protect, step } => {
                result.kind = "iron".into();
                result.iron = Some(IronPayload {
                    protect: *protect,
                    step: *step,
                });
            }
            StatePayload::CovidBoss { mutation } => {
                result.kind = "covid_boss".into();
                result.covid_boss = Some(CovidBossPayload { mutation: *mutation });
            }
            StatePayload::CovidInfection {
                entries,
                mutation_set,
                recovered,
            } => {
                result.kind = "covid_infection".into();
                result.covid_infection = Some(CovidInfectionPayload {
                    entries: entries.to_vec(),
                    mutation_set: mutation_set.to_vec(),
                    recovered: *recovered,
                });
            }
            StatePayload::SaitamaBoss {
                turns,
                damages,
                hitters,
                minions,
            } => {
                result.kind = "saitama_boss".into();
                result.saitama_boss = Some(SaitamaBossPayload {
                    turns: *turns,
                    damages: *damages,
                    hitters: hitters.to_vec(),
                    minions: minions.to_vec(),
                });
            }
            StatePayload::LazyBoss { at_boost_bits } => {
                result.kind = "lazy_boss".into();
                result.lazy_boss = Some(LazyBossPayload {
                    at_boost_bits: *at_boost_bits,
                });
            }
            StatePayload::LazyInfection { boss } => {
                result.kind = "lazy_infection".into();
                result.lazy_infection = Some(LazyInfectionPayload { boss: *boss });
            }
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCounter {
    pub pending: bool,
    pub last_target: Option<EntityIdx>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPlayerRuntime {
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
    pub at_boost_bits: u64,
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
    pub protect_pre_defend_skill_count: Option<usize>,
    pub upgrade_active: bool,
    pub hide: Option<HideRuntime>,
    pub assassinate: Option<AssassinateRuntime>,
    pub counter: ModelCounter,
    pub corpse: RuntimeCorpseKind,
}

impl From<&PlayerRuntime> for ModelPlayerRuntime {
    fn from(value: &PlayerRuntime) -> Self {
        Self {
            hp: value.hp,
            alive: value.alive,
            attack: value.attack,
            magic: value.magic,
            magic_point: value.magic_point,
            wisdom: value.wisdom,
            speed: value.speed,
            defense: value.defense,
            resistance: value.resistance,
            agility: value.agility,
            at_boost_bits: value.at_boost_bits,
            at_boost_millionths: value.at_boost_millionths,
            attr_sum: value.attr_sum,
            atk_sum: value.atk_sum,
            attract_bits: value.attract_bits,
            kind: value.kind,
            owner: value.owner,
            root_owner: value.root_owner,
            team: value.team,
            flags: value.flags,
            policies: value.policies,
            move_state: value.move_state,
            charge: value.charge,
            accumulate: value.accumulate,
            shield: value.shield,
            protect_to: value.protect_to,
            protect_from: value.protect_from.clone(),
            protect_pre_defend_skill_count: value.protect_pre_defend_skill_count,
            upgrade_active: value.upgrade_active,
            hide: value.hide,
            assassinate: value.assassinate,
            counter: ModelCounter {
                pending: value.counter.pending,
                last_target: value.counter.last_target,
            },
            corpse: value.corpse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runner() -> RuntimeRunner {
        RuntimeRunner::new_from_namerena_raw("private-left@red\n\nprivate-right@blue\nseed:private-seed".into()).unwrap()
    }

    #[test]
    fn snapshot_is_read_only_and_excludes_identity_and_allocator_counters() {
        let mut runner = runner();
        runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.counter.last_updates_id = Some(100);
        let before = runner.runtime.clone();
        let state = runner.model_state().unwrap();
        assert_eq!(before.entities, runner.runtime.entities);
        assert_eq!(before.world, runner.runtime.world);
        assert_eq!(before.rng.main_val, runner.runtime.rng.main_val);
        assert_eq!((before.rng.i, before.rng.j), (runner.runtime.rng.i, runner.runtime.rng.j));
        runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.counter.last_updates_id = Some(999_999);
        assert_eq!(state, runner.model_state().unwrap());
        let json = serde_json::to_string(&state).unwrap();
        for forbidden in [
            "private-left",
            "private-right",
            "private-seed",
            "last_updates_id",
            "baseline_id",
            "clan_name",
            "main_val",
        ] {
            assert!(!json.contains(forbidden), "特征不应包含 {forbidden}");
        }
        assert_eq!(serde_json::from_str::<BattleModelState>(&json).unwrap(), state);
    }

    #[test]
    fn skills_use_stable_one_based_registry_ids() {
        let registry = default_custom_runtime_import_config().unwrap().registry;
        assert_eq!(registry.skills().len(), 42);
        assert_eq!(
            registry.skills().iter().map(|skill| skill.export_name.as_str()).collect::<Vec<_>>(),
            MODEL_SKILL_EXPORTS
        );
        assert!(registry.skills().len() <= MODEL_SKILL_ID_LIMIT as usize);
        let loadout = SkillLoadout::from_skills(registry.skills().iter().map(|skill| skill.id));
        let skills = ModelSkills::from(&loadout);
        assert_eq!(
            skills.lanes.iter().map(|skill| skill.skill_id).collect::<Vec<_>>(),
            (1..=42).collect::<Vec<_>>()
        );
        assert_eq!(registry.skills()[0].export_name, DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT);
    }

    #[test]
    fn preserves_hidden_payloads_registration_order_and_compressed_states() {
        let mut runner = runner();
        let states = &mut runner.runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
        states.register_compressed_legacy_state(CompressedLegacyState::Protect);
        let payloads = [
            StatePayload::Poison {
                caster: Some(1),
                target: Some(0),
                atp_bits: 123.5f64.to_bits(),
                count: 3,
            },
            StatePayload::Charm {
                group_id: 1,
                effective_team_idx: Some(1),
                source_team_idx: Some(0),
                target: Some(1),
                step: 2,
            },
            StatePayload::CovidInfection {
                entries: vec![CovidInfectionEntry {
                    boss: EntityIdx(1),
                    mutation: 7,
                    days: 2,
                }]
                .into(),
                mutation_set: vec![7].into(),
                recovered: false,
            },
            StatePayload::SaitamaBoss {
                turns: 8,
                damages: 99,
                hitters: vec![EntityIdx(1)].into(),
                minions: vec![EntityIdx(0)].into(),
            },
            StatePayload::LazyInfection { boss: EntityIdx(1) },
            StatePayload::Iron { protect: 15, step: 2 },
        ];
        for (index, payload) in payloads.iter().enumerate() {
            let mut entry = StateEntry::legacy(100 + index as u32);
            entry.payload = payload.clone();
            states.add_entry(entry);
        }
        let exported = runner.model_state().unwrap();
        let entity = &exported.entities[0];
        assert_ne!(entity.compressed_state_flags & (1 << 1), 0);
        assert_eq!(entity.states.len(), payloads.len());
        assert_eq!(entity.state_registration_cursor, 7);
        for (index, state) in entity.states.iter().enumerate() {
            assert_eq!(state.runtime_registration_order, index as u64 + 1);
            assert_eq!(state.payload, ModelPayload::from(&payloads[index]));
        }
        assert_eq!(entity.states[0].payload.poison.as_ref().unwrap().atp_bits, 123.5f64.to_bits());
    }

    #[test]
    fn entity_list_order_does_not_replace_action_order_or_team_identity() {
        let mut state = runner().model_state().unwrap();
        let order = state.world.round_order.clone();
        state.entities.reverse();
        state.validate().unwrap();
        assert_eq!(state.world.round_order, order);
        for entity in &state.entities {
            assert!(state.input_teams[entity.input_team_index].contains(&entity.runtime.root_owner));
        }
        state.entities[0].runtime.root_owner = EntityIdx(u32::MAX);
        assert!(state.validate().is_err());
    }

    #[test]
    fn builtin_bosses_export_name_derived_mechanics() {
        for name in crate::namerena::BOSS_NAMES {
            let mut runner = RuntimeRunner::new_from_namerena_raw(format!("{name}@!\n\nleft\nright")).unwrap();
            for _ in 0..20 {
                let state = runner.model_state().unwrap();
                let boss = &state.entities[0].template.identity;
                assert_eq!(
                    boss.boss_kind,
                    crate::namerena::BOSS_NAMES.iter().position(|candidate| *candidate == name)
                );
                assert_eq!(
                    boss.immunity.iter().find(|entry| entry.status == "charm").unwrap().threshold,
                    crate::namerena::boss_immune_threshold(name, "charm")
                );
                if runner.have_winner() {
                    break;
                }
                runner.main_round();
            }
        }
    }

    #[test]
    fn lazy_blueprint_projection_does_not_fill_cache_and_matches_materialized_state() {
        let mut runner = runner();
        let registry = &runner.runtime.registry;
        let blueprint = registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
            .unwrap();
        let lazy = registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_LAZY_BLUEPRINT_RQ_ENTITY_EXPORT)
            .unwrap();
        let entity = runner.runtime.entities.get_mut(EntityIdx(0)).unwrap();
        entity.slots.remove(blueprint);
        entity
            .slots
            .set(lazy, SlotValue::U64(crate::namerena::eval_name::DEFAULT_EVAL_RQ.to_bits()))
            .unwrap();
        let rng = runner.runtime.rng.clone();
        let before = runner.model_state().unwrap();
        assert!(runner.runtime.entities.get(EntityIdx(0)).unwrap().slots.get(blueprint).is_none());
        assert!(
            runner
                .runtime
                .ensure_plain_minion_blueprint(EntityIdx(0), crate::namerena::MinionKind::Summon)
        );
        assert_eq!(before, runner.model_state().unwrap());
        assert_eq!(rng.main_val, runner.runtime.rng.main_val);
        assert_eq!((rng.i, rng.j), (runner.runtime.rng.i, runner.runtime.rng.j));
    }

    #[test]
    fn unsupported_registry_and_invalid_blueprint_are_errors() {
        let mut runner = runner();
        let blueprint = runner
            .runtime
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
            .unwrap();
        runner
            .runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(blueprint, SlotValue::Text("unsupported".into()))
            .unwrap();
        assert!(runner.model_state().unwrap_err().to_string().contains("类型无效"));
        runner.runtime.registry = ExtensionRegistry::default();
        assert!(runner.model_state().unwrap_err().to_string().contains("默认规则注册表"));
    }
}

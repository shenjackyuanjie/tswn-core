//! 编码主路径：状态校验 → 局内重映射 → global / entity / template 三族写入。
//!
//! 本文件实现规格第 14 节的拟议接口，并按外部评审 3.1 增加可复用缓冲的批路径：
//!
//! ```text
//! FeatureEncoder::new(manifest)                          // 初始化期校验并冻结运行配置
//! FeatureEncoder::encode(state)                          // B=1 便捷入口
//! FeatureEncoder::encode_into(state, batch_index, batch)  // 批槽位写入
//! ```
//!
//! 约束（外部评审 3.1）：manifest 在初始化阶段校验；目标槽位先恢复 padding，随后检查
//! schema、引用、可空分支与容量，再写入字段。校验或写入返回错误时再次清理目标槽位，
//! 不暴露部分结果，也不影响相邻批槽位；同一 state 放在不同 batch 位置不改变其有效内容。
//!
//! lane / state / slot / list / extra 四族的张量写入按 handoff 分块计划在后续提交追加；
//! 本文件的校验部分已覆盖全部引用域（含 charm `group_id` 与槽内 U64 实体引用）。

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::encoder::batch::{CapacityDims, EncodedBatch};
use crate::encoder::capacity::{CAPACITY_DIMS, CapacityMeasure, runtime_team_ids};
use crate::encoder::error::EncodeError;
use crate::encoder::manifest::{EncoderManifest, FIXED_COUNT_SLOTS, ProfileSpec, SupportDomain, UNSUPPORTED_PAYLOAD_KINDS};
use crate::encoder::numeric::{TransformKind, normalize_fitted, normalize_fixed_count, to_f32_checked};
use crate::encoder::slots::{SlotScope, SlotSemantic, resolve_slot};
use crate::encoder::vocab::{Vocabulary, boss_kind_vocabulary, player_kind_vocabulary};
use crate::runtime::entity::RuntimeCorpseKind;
use crate::runtime::extension::{DamageSharePolicy, MergePolicy, OwnerResolutionPolicy};
use crate::runtime::model_state::{
    BattleModelState, MODEL_STATE_SCHEMA_VERSION, ModelPayload, ModelPlayerRuntime, ModelTemplate,
};
use crate::runtime::{EntityIdx, ExtensionRegistry, PlrId, default_custom_runtime_import_config};

/// `entity_num` 的 24 个槽位：`(槽位, 校准字段路径)`；顺序即规格第 3 节的字段顺序。
pub const ENTITY_NUM_SLOTS: [(usize, &str); 24] = [
    (0, "entity.runtime.hp"),
    (1, "entity.runtime.attack"),
    (2, "entity.runtime.magic"),
    (3, "entity.runtime.magic_point"),
    (4, "entity.runtime.wisdom"),
    (5, "entity.runtime.speed"),
    (6, "entity.runtime.defense"),
    (7, "entity.runtime.resistance"),
    (8, "entity.runtime.agility"),
    (9, "entity.runtime.at_boost"),
    (10, "entity.runtime.attr_sum"),
    (11, "entity.runtime.atk_sum"),
    (12, "entity.runtime.attract"),
    (13, "entity.runtime.shield"),
    (14, "entity.runtime.protect_pre_defend_skill_count"),
    (15, "entity.runtime.move_state.speed_points"),
    (16, "entity.runtime.charge.step"),
    (17, "entity.runtime.accumulate.acc"),
    (18, "entity.runtime.accumulate.charge_bonus"),
    (19, "entity.runtime.hide.level"),
    (20, "entity.runtime.hide.attract"),
    (21, "entity.runtime.hide.agility"),
    (22, "entity.runtime.hide.defense"),
    (23, "entity.runtime.hide.resistance"),
];

/// `template_num` 的 31 个槽位；`[17..30]` 由 `clone_build` 整体 presence 控制。
pub const TEMPLATE_NUM_SLOTS: [(usize, &str); 31] = [
    (0, "template.max_hp"),
    (1, "template.attack"),
    (2, "template.magic"),
    (3, "template.magic_point"),
    (4, "template.wisdom"),
    (5, "template.speed"),
    (6, "template.defense"),
    (7, "template.resistance"),
    (8, "template.agility"),
    (9, "template.at_boost"),
    (10, "template.attr_sum"),
    (11, "template.atk_sum"),
    (12, "template.attract"),
    (13, "template.reserved_player_ids_before_spawn"),
    (14, "template.identity.boss_action_prob_count"),
    (15, "template.identity.boost_immune_threshold"),
    (16, "template.move_state.speed_points"),
    (17, "template.clone_build.name_factor"),
    (18, "template.clone_build.child_name_factor"),
    (19, "template.clone_build.adjustments.max_hp"),
    (20, "template.clone_build.adjustments.attack"),
    (21, "template.clone_build.adjustments.magic"),
    (22, "template.clone_build.adjustments.wisdom"),
    (23, "template.clone_build.adjustments.speed"),
    (24, "template.clone_build.adjustments.defense"),
    (25, "template.clone_build.adjustments.resistance"),
    (26, "template.clone_build.adjustments.agility"),
    (27, "template.clone_build.adjustments.at_boost_delta"),
    (28, "template.clone_build.adjustments.attr_sum"),
    (29, "template.clone_build.adjustments.atk_sum"),
    (30, "template.clone_build.adjustments.attract_delta"),
];

/// `immunity_num` 的九个固定 status 轴（与 model_state 投影顺序一致）。
pub const IMMUNITY_AXIS: [&str; 9] = [
    "assassinate",
    "charm",
    "berserk",
    "half",
    "curse",
    "exchange",
    "slow",
    "ice",
    "fire",
];

/// 编码器消费的全部数值槽：`(校准路径, 变换类型)`。manifest 必须逐项声明且类型一致，
/// 缺一即 [`EncodeError::MissingCalibration`]，不允许用 0/1 顶替（规格第 5 节）。
pub fn required_normalization() -> Vec<(&'static str, TransformKind)> {
    let mut required: Vec<(&'static str, TransformKind)> = FIXED_COUNT_SLOTS
        .iter()
        .map(|(path, divisor)| (*path, TransformKind::FixedCount { divisor: *divisor }))
        .collect();
    required.push(("global.round", TransformKind::Fitted));
    required.push(("global.world.alive_group_count", TransformKind::Fitted));
    required.push(("global.world.round_pos", TransformKind::Fitted));
    required.extend(ENTITY_NUM_SLOTS.iter().map(|(_, path)| (*path, TransformKind::Fitted)));
    required.extend(TEMPLATE_NUM_SLOTS.iter().map(|(_, path)| (*path, TransformKind::Fitted)));
    required.push(("template.identity.immunity.threshold", TransformKind::Fitted));
    required.extend([
        ("lane.level", TransformKind::Fitted),
        ("lane.build_level", TransformKind::Fitted),
        ("lane.boost.base", TransformKind::Fitted),
        ("lane.boost.extra", TransformKind::Fitted),
    ]);
    required
}

/// 容量维度名 → 错误路径名（供 `CapacityExceeded` 定位）。
fn dim_path(dim: &str) -> String {
    let name = match dim {
        "e" => "entities",
        "t" => "input_teams",
        "r" => "runtime_team",
        "h" => "templates",
        "l" => "lanes",
        "s" => "states",
        "q" => "slots",
        "v" => "list",
        "x" => "extra",
        other => other,
    };
    name.to_owned()
}

fn profile_limit(profile: &ProfileSpec, dim: &str) -> usize {
    let index = CAPACITY_DIMS.iter().position(|candidate| *candidate == dim).expect("capacity dim");
    profile.dims()[index]
}

/// 默认规则注册表；词表派生只做一次。
fn default_registry() -> &'static ExtensionRegistry {
    static REGISTRY: OnceLock<ExtensionRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| default_custom_runtime_import_config().expect("默认规则注册表必须有效").registry)
}

/// 局内重映射结果。
///
/// - `entity_rows`：`EntityIdx` → 实体行（死亡实体仍在表内，mask=1）；
/// - `template_rows`：模板表行 → 模板；前 `e` 行是实体模板，其后是槽内蓝图与全局槽模板，
///   与 [`CapacityMeasure`](crate::encoder::capacity::CapacityMeasure) 的 `h` 计费口径一致；
/// - `runtime_teams`：runtime team 原值 → 关系行（原值升序，跨 entities/templates/world 统一）；
/// - `player_keys`：`PlrId` → 局内相等关系键（模板表行序首次出现）。
struct SampleIndex<'a> {
    entity_rows: BTreeMap<u32, usize>,
    template_rows: Vec<&'a ModelTemplate>,
    template_row_of_entity: Vec<usize>,
    runtime_teams: BTreeMap<usize, usize>,
    player_keys: BTreeMap<PlrId, usize>,
}

#[derive(Debug, Clone, Copy)]
struct ListRecord {
    owner_scope: i32,
    owner: i32,
    field_class: i32,
    ordinal: i32,
    target: i32,
    order_key: Option<u64>,
    order_domain: Option<usize>,
}

impl ListRecord {
    fn with_owner(mut self, owner: i32) -> Self {
        self.owner = owner;
        self
    }
}

fn entity_list_record(
    index: &SampleIndex<'_>,
    id: EntityIdx,
    field_class: i32,
    ordinal: usize,
    owner_scope: i32,
) -> Result<ListRecord, EncodeError> {
    Ok(ListRecord {
        owner_scope,
        owner: 0,
        field_class,
        ordinal: ordinal as i32,
        target: index.entity_row(id, "list.target")? as i32,
        order_key: None,
        order_domain: None,
    })
}

impl SampleIndex<'_> {
    fn entity_row(&self, id: EntityIdx, path: &str) -> Result<usize, EncodeError> {
        self.entity_rows.get(&id.0).copied().ok_or_else(|| EncodeError::InvalidReference {
            path: path.to_owned(),
            raw: id.0.to_string(),
        })
    }
}

/// FeatureEncoder：字段映射、局内引用、presence 与容量预检的唯一实现。
///
/// 不依赖 Python、展示 DTO、文件格式或模型参数，可在 `wasm32-unknown-unknown` 上编译。
#[derive(Debug, Clone)]
pub struct FeatureEncoder {
    manifest: EncoderManifest,
    profile: ProfileSpec,
    vocabularies: BTreeMap<&'static str, Vocabulary>,
}

impl FeatureEncoder {
    /// 构造编码器：manifest 自洽性 + 与当前实现的交叉核对。
    ///
    /// 任何一端过期（版本身份、容量不变量、词表与默认注册表不一致、数值槽不齐全、
    /// 变换类型不符、支持域不符）都拒绝，不自动采用“最新版本”。
    pub fn new(manifest: EncoderManifest) -> Result<Self, EncodeError> {
        manifest.validate()?;
        let registry = default_registry();
        let derived: [(&'static str, Vocabulary); 3] = [
            ("runtime.kind", player_kind_vocabulary(registry)),
            ("template.kind", player_kind_vocabulary(registry)),
            ("template.identity.boss_kind", boss_kind_vocabulary()),
        ];
        let mut vocabularies = BTreeMap::new();
        for (name, vocabulary) in derived {
            let declared = manifest.vocabularies.get(name).ok_or_else(|| EncodeError::MissingVocabulary {
                path: format!("vocabularies.{name}"),
            })?;
            if declared != &vocabulary {
                return Err(EncodeError::ManifestMismatch {
                    path: format!("vocabularies.{name}"),
                    detail: "词表与默认注册表派生结果不一致".to_owned(),
                });
            }
            vocabularies.insert(name, vocabulary);
        }
        for (path, kind) in required_normalization() {
            let field = manifest
                .normalization
                .get(path)
                .ok_or_else(|| EncodeError::MissingCalibration { path: path.to_owned() })?;
            if field.transform != kind {
                return Err(EncodeError::ManifestMismatch {
                    path: path.to_owned(),
                    detail: format!("manifest 声明的变换 {:?} 与实现登记的 {:?} 不符", field.transform, kind),
                });
            }
        }
        if manifest.support != SupportDomain::current() {
            return Err(EncodeError::ManifestMismatch {
                path: "support".to_owned(),
                detail: "支持域与当前实现不符".to_owned(),
            });
        }
        Ok(Self {
            profile: manifest.profile.clone(),
            manifest,
            vocabularies,
        })
    }

    pub fn manifest(&self) -> &EncoderManifest { &self.manifest }

    pub fn profile(&self) -> &ProfileSpec { &self.profile }

    /// B=1 便捷入口。
    pub fn encode(&self, state: &BattleModelState) -> Result<EncodedBatch, EncodeError> {
        let mut batch = EncodedBatch::new(&self.profile, 1);
        self.encode_into(state, 0, &mut batch)?;
        Ok(batch)
    }

    /// 批槽位写入；状态校验或字段写入失败时，目标槽位整体恢复 padding。
    /// profile 不匹配或 batch_index 越界属于调用错误，不修改任何槽位。
    pub fn encode_into(&self, state: &BattleModelState, batch_index: usize, out: &mut EncodedBatch) -> Result<(), EncodeError> {
        if out.dims() != &self.profile.dims() {
            return Err(EncodeError::ManifestMismatch {
                path: "profile".to_owned(),
                detail: "批缓冲容量与 manifest 声明不一致".to_owned(),
            });
        }
        out.clear_slot(batch_index)?;
        let result = (|| -> Result<(), EncodeError> {
            self.validate_state(state)?;
            let index = self.build_index(state)?;
            self.write_global(state, &index, batch_index, out)?;
            self.write_entity_family(state, &index, batch_index, out)?;
            self.write_template_family(&index, batch_index, out)?;
            self.write_lane_family(&index, batch_index, out)?;
            self.write_list_family(state, &index, batch_index, out)?;
            Ok(())
        })();
        if result.is_err() {
            // 词表、免疫分类与数值校验也可能在写入过程中失败，不能留下已置 1 的 mask。
            out.clear_slot(batch_index)?;
        }
        result
    }

    // ---------------------------------------------------------------- 校验

    /// 状态校验：schema、终局门禁、输入队伍、实体唯一性、全部引用域、压缩标志保留位、
    /// 载荷 kind 与分支匹配、槽语义白名单、容量预检。
    ///
    /// 注意 [`BattleModelState::validate`] 不是替代品：它未覆盖 charm `group_id` 实体引用、
    /// 槽内 U64 实体引用与载荷分支一致性（规格第 16 节第 7 条）。
    fn validate_state(&self, state: &BattleModelState) -> Result<(), EncodeError> {
        if state.schema_version != MODEL_STATE_SCHEMA_VERSION {
            return Err(EncodeError::SchemaMismatch {
                path: "schema_version".to_owned(),
                expected: MODEL_STATE_SCHEMA_VERSION.to_string(),
                actual: state.schema_version.to_string(),
            });
        }
        if state.world.winner_team.is_some() {
            return Err(EncodeError::AlreadyDecided {
                path: "world.winner_team".to_owned(),
            });
        }
        // team_mask 只按输入标签位置生成：至少两队、每队非空（规格第 10 节）。
        if state.input_teams.len() < 2 {
            return Err(EncodeError::InvalidState {
                path: "input_teams".to_owned(),
            });
        }
        let mut ids: BTreeSet<u32> = BTreeSet::new();
        for (row, entity) in state.entities.iter().enumerate() {
            // 稀疏 ID 可以大于实体行数，但必须落在包含预留空洞的槽范围内。
            if entity.id.0 as usize >= state.entity_slot_count || !ids.insert(entity.id.0) {
                return Err(EncodeError::InvalidState {
                    path: format!("entities[{row}].id"),
                });
            }
        }
        let check = |id: EntityIdx, path: String| -> Result<(), EncodeError> {
            if ids.contains(&id.0) {
                Ok(())
            } else {
                Err(EncodeError::InvalidReference {
                    path,
                    raw: id.0.to_string(),
                })
            }
        };
        for (team, members) in state.input_teams.iter().enumerate() {
            if members.is_empty() {
                return Err(EncodeError::InvalidState {
                    path: format!("input_teams[{team}]"),
                });
            }
            for (index, id) in members.iter().enumerate() {
                check(*id, format!("input_teams[{team}][{index}]"))?;
            }
        }
        for (index, id) in state.world.round_order.iter().enumerate() {
            check(*id, format!("world.round_order[{index}]"))?;
        }
        for (index, id) in state.world.flat_alive.iter().enumerate() {
            check(*id, format!("world.flat_alive[{index}]"))?;
        }
        for (team, roster) in state.world.team_roster.iter().enumerate() {
            for (index, id) in roster.iter().enumerate() {
                check(*id, format!("world.team_roster[{team}][{index}]"))?;
            }
        }
        for (team, alive) in state.world.team_alive.iter().enumerate() {
            for (index, id) in alive.iter().enumerate() {
                check(*id, format!("world.team_alive[{team}][{index}]"))?;
            }
        }
        for (index, id) in state.ice_release_events.iter().enumerate() {
            check(*id, format!("ice_release_events[{index}]"))?;
        }
        for (row, entity) in state.entities.iter().enumerate() {
            let prefix = format!("entities[{row}]");
            let runtime = &entity.runtime;
            check(runtime.owner, format!("{prefix}.runtime.owner"))?;
            check(runtime.root_owner, format!("{prefix}.runtime.root_owner"))?;
            if !state
                .input_teams
                .get(entity.input_team_index)
                .is_some_and(|team| team.contains(&runtime.root_owner))
            {
                return Err(EncodeError::InvalidState {
                    path: format!("{prefix}.input_team_index"),
                });
            }
            if let Some(id) = runtime.protect_to {
                check(id, format!("{prefix}.runtime.protect_to"))?;
            }
            for (index, link) in runtime.protect_from.iter().enumerate() {
                check(link.owner, format!("{prefix}.runtime.protect_from[{index}].owner"))?;
            }
            if let Some(assassinate) = runtime.assassinate {
                check(assassinate.target, format!("{prefix}.runtime.assassinate.target"))?;
            }
            if let Some(id) = runtime.counter.last_target {
                check(id, format!("{prefix}.runtime.counter.last_target"))?;
            }
            if entity.compressed_state_flags & 0b1110_0000 != 0 {
                return Err(EncodeError::ReservedFlagBitSet {
                    path: format!("{prefix}.compressed_state_flags"),
                });
            }
            for (index, entry) in entity.states.iter().enumerate() {
                let state_prefix = format!("{prefix}.states[{index}]");
                validate_payload(&state_prefix, &entry.payload)?;
                if let Some(poison) = &entry.payload.poison {
                    for (field, raw) in [("caster", poison.caster), ("target", poison.target)] {
                        if let Some(raw) = raw {
                            check(EntityIdx(raw), format!("{state_prefix}.payload.poison.{field}"))?;
                        }
                    }
                }
                if let Some(charm) = &entry.payload.charm {
                    if let Some(raw) = charm.target {
                        check(EntityIdx(raw), format!("{state_prefix}.payload.charm.target"))?;
                    }
                    // `validate` 未覆盖的引用域：charm.group_id 是施法者 EntityIdx。
                    let group = u32::try_from(charm.group_id).map_err(|_| EncodeError::InvalidReference {
                        path: format!("{state_prefix}.payload.charm.group_id"),
                        raw: charm.group_id.to_string(),
                    })?;
                    check(EntityIdx(group), format!("{state_prefix}.payload.charm.group_id"))?;
                }
            }
            for (index, slot) in entity.slots.iter().enumerate() {
                let slot_prefix = format!("{prefix}.slots[{index}]");
                let resolved = resolve_slot(SlotScope::Entity, slot, &slot_prefix)?;
                if resolved.semantic == SlotSemantic::EntityRef {
                    let raw = slot.u64_value.ok_or_else(|| EncodeError::InvalidSlotValue {
                        path: format!("{slot_prefix}.u64_value"),
                    })?;
                    let id = u32::try_from(raw).map_err(|_| EncodeError::InvalidReference {
                        path: format!("{slot_prefix}.u64_value"),
                        raw: raw.to_string(),
                    })?;
                    check(EntityIdx(id), format!("{slot_prefix}.u64_value"))?;
                }
            }
        }
        for (index, slot) in state.template_slots.iter().enumerate() {
            resolve_slot(SlotScope::Template, slot, &format!("template_slots[{index}]"))?;
        }
        for (index, slot) in state.battle_slots.iter().enumerate() {
            resolve_slot(SlotScope::Battle, slot, &format!("battle_slots[{index}]"))?;
        }
        let measure = CapacityMeasure::measure(state);
        for dim in CAPACITY_DIMS {
            let actual = measure.value(dim);
            let limit = profile_limit(&self.profile, dim);
            if actual > limit {
                return Err(EncodeError::CapacityExceeded {
                    path: dim_path(dim),
                    actual,
                    limit,
                });
            }
        }
        if measure.l > self.profile.l_max {
            return Err(EncodeError::CapacityExceeded {
                path: dim_path("l"),
                actual: measure.l,
                limit: self.profile.l_max,
            });
        }
        Ok(())
    }

    // ---------------------------------------------------------------- 重映射

    fn build_index<'a>(&self, state: &'a BattleModelState) -> Result<SampleIndex<'a>, EncodeError> {
        let mut entity_rows: BTreeMap<u32, usize> = BTreeMap::new();
        for (row, entity) in state.entities.iter().enumerate() {
            entity_rows.insert(entity.id.0, row);
        }
        let mut template_rows: Vec<&ModelTemplate> = state.entities.iter().map(|entity| &entity.template).collect();
        for entity in &state.entities {
            for slot in &entity.slots {
                if let Some(template) = &slot.template {
                    template_rows.push(template);
                }
            }
        }
        for slot in state.template_slots.iter().chain(state.battle_slots.iter()) {
            if let Some(template) = &slot.template {
                template_rows.push(template);
            }
        }
        // runtime team 关系域与容量计费共用同一个集合，包含蓝图和空 world 团队行。
        let teams = runtime_team_ids(state);
        let runtime_teams: BTreeMap<usize, usize> = teams.iter().copied().enumerate().map(|(row, raw)| (raw, row)).collect();
        // PlrId 相等关系键：模板表行序首次出现分配。
        let mut player_keys: BTreeMap<PlrId, usize> = BTreeMap::new();
        for template in &template_rows {
            let next = player_keys.len();
            player_keys.entry(template.id).or_insert(next);
        }
        if runtime_teams.len() > self.profile.r_max {
            return Err(EncodeError::CapacityExceeded {
                path: dim_path("r"),
                actual: runtime_teams.len(),
                limit: self.profile.r_max,
            });
        }
        Ok(SampleIndex {
            entity_rows,
            template_rows,
            template_row_of_entity: (0..state.entities.len()).collect(),
            runtime_teams,
            player_keys,
        })
    }

    // ---------------------------------------------------------------- global

    fn write_global(
        &self,
        state: &BattleModelState,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        let row = out.f32_row_mut("global_num", batch_index)?;
        row[0] = self.fitted_value(state.round as f64, "global.round", "")?;
        row[1] = self.fixed_count_value(state.entity_slot_count as f64, "global.entity_slot_count", "")?;
        row[2] = self.fitted_value(state.world.alive_group_count as f64, "global.world.alive_group_count", "")?;
        row[3] = self.fitted_value(i64::from(state.world.round_pos) as f64, "global.world.round_pos", "")?;
        row[4] = if state.legacy_step_scheduler { 1.0 } else { 0.0 };
        row[5] = self.fixed_count_value(state.entities.len() as f64, "global.entity_count", "")?;
        // team_mask 只表达输入标签位置存在性：零存活队伍仍是 1（规格第 10 节）。
        let team_mask = out.u8_row_mut("team_mask", batch_index)?;
        for (team, present) in team_mask.iter_mut().enumerate() {
            *present = u8::from(team < state.input_teams.len());
        }
        let runtime_team_mask = out.u8_row_mut("runtime_team_mask", batch_index)?;
        for (team, present) in runtime_team_mask.iter_mut().enumerate() {
            *present = u8::from(team < index.runtime_teams.len());
        }
        Ok(())
    }

    // ---------------------------------------------------------------- entity

    fn write_entity_family(
        &self,
        state: &BattleModelState,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        for (row, entity) in state.entities.iter().enumerate() {
            let prefix = format!("entities[{row}]");
            let runtime = &entity.runtime;
            let owner_row = index.entity_row(runtime.owner, &format!("{prefix}.runtime.owner"))?;
            let root_row = index.entity_row(runtime.root_owner, &format!("{prefix}.runtime.root_owner"))?;
            out.u8_row_mut("entity_mask", batch_index)?[row] = 1;
            out.i32_row_mut("entity_template", batch_index)?[row] = index.template_row_of_entity[row] as i32;
            {
                let team = out.i32_row_mut("entity_team", batch_index)?;
                team[2 * row] = entity.input_team_index as i32;
                team[2 * row + 1] = index.runtime_teams[&runtime.team] as i32;
            }
            {
                let values = entity_num_values(runtime);
                let present = entity_num_present(runtime);
                let target = out.f32_row_mut("entity_num", batch_index)?;
                for (slot, path) in ENTITY_NUM_SLOTS {
                    target[row * 24 + slot] = if present[slot] {
                        self.fitted_value(values[slot], path, &prefix)?
                    } else {
                        0.0
                    };
                }
                let presence = out.u8_row_mut("entity_num_present", batch_index)?;
                for (slot, _) in ENTITY_NUM_SLOTS {
                    presence[row * 24 + slot] = u8::from(present[slot]);
                }
            }
            {
                let bools = out.u8_row_mut("entity_bool", batch_index)?;
                bools[row * 10] = u8::from(runtime.alive);
                bools[row * 10 + 1] = u8::from(runtime.upgrade_active);
                bools[row * 10 + 2] = u8::from(runtime.counter.pending);
                bools[row * 10 + 3] = u8::from(runtime.charge.active);
                bools[row * 10 + 4] = u8::from(runtime.charge.post_action_active);
                bools[row * 10 + 5] = u8::from(runtime.accumulate.active);
                bools[row * 10 + 6] = u8::from(runtime.policies.inherit_owner_def_res);
                bools[row * 10 + 7] = u8::from(runtime.hide.is_some());
                bools[row * 10 + 8] = u8::from(runtime.assassinate.is_some());
                bools[row * 10 + 9] = u8::from(runtime.assassinate.is_some_and(|a| a.break_on_damage));
            }
            {
                let flags = out.u8_row_mut("entity_flags", batch_index)?;
                for (bit, slot) in flags[row * 8..row * 8 + 8].iter_mut().enumerate() {
                    *slot = (entity.compressed_state_flags >> bit) & 1;
                }
            }
            {
                let kind_flags = out.u8_row_mut("entity_kind_flags", batch_index)?;
                for (bit, slot) in kind_flags[row * 6..row * 6 + 6].iter_mut().enumerate() {
                    *slot = u8::from(((runtime.flags.0 >> bit) & 1) != 0);
                }
            }
            {
                let cats = out.i32_row_mut("entity_cat", batch_index)?;
                cats[row * 5] = self.dense_id("runtime.kind", runtime.kind.0, &format!("{prefix}.runtime.kind"))?;
                cats[row * 5 + 1] = corpse_kind_id(runtime.corpse);
                cats[row * 5 + 2] = owner_resolution_id(runtime.policies.owner_resolution);
                cats[row * 5 + 3] = damage_share_id(runtime.policies.damage_share);
                cats[row * 5 + 4] = merge_id(runtime.policies.merge);
            }
            {
                let refs = out.i32_row_mut("entity_ref", batch_index)?;
                refs[row * 5] = owner_row as i32;
                refs[row * 5 + 1] = root_row as i32;
                if let Some(id) = runtime.protect_to {
                    refs[row * 5 + 2] = index.entity_row(id, &format!("{prefix}.runtime.protect_to"))? as i32;
                }
                if let Some(id) = runtime.counter.last_target {
                    refs[row * 5 + 3] = index.entity_row(id, &format!("{prefix}.runtime.counter.last_target"))? as i32;
                }
                if let Some(assassinate) = runtime.assassinate {
                    refs[row * 5 + 4] =
                        index.entity_row(assassinate.target, &format!("{prefix}.runtime.assassinate.target"))? as i32;
                }
                let presence = out.u8_row_mut("entity_ref_present", batch_index)?;
                // `[0,1]` 跟随实体行；`[2,3]` 与 `[4]` 跟随各自 Option。
                presence[row * 5] = 1;
                presence[row * 5 + 1] = 1;
                presence[row * 5 + 2] = u8::from(runtime.protect_to.is_some());
                presence[row * 5 + 3] = u8::from(runtime.counter.last_target.is_some());
                presence[row * 5 + 4] = u8::from(runtime.assassinate.is_some());
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- template

    fn write_template_family(
        &self,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        let h_max = self.profile.h_max;
        let mut clan_groups: Vec<usize> = Vec::with_capacity(index.template_rows.len());
        for (row, template) in index.template_rows.iter().enumerate() {
            let prefix = format!("templates[{row}]");
            let identity = &template.identity;
            out.u8_row_mut("template_mask", batch_index)?[row] = 1;
            {
                let values = template_num_values(template);
                let present = template_num_present(template);
                let target = out.f32_row_mut("template_num", batch_index)?;
                for (slot, path) in TEMPLATE_NUM_SLOTS {
                    target[row * 31 + slot] = if present[slot] {
                        self.fitted_value(values[slot], path, &prefix)?
                    } else {
                        0.0
                    };
                }
                let presence = out.u8_row_mut("template_num_present", batch_index)?;
                for (slot, _) in TEMPLATE_NUM_SLOTS {
                    presence[row * 31 + slot] = u8::from(present[slot]);
                }
            }
            {
                let overrides = &template.policy_overrides;
                let bools = out.u8_row_mut("template_bool", batch_index)?;
                bools[row * 5] = u8::from(template.reuse_skills_on_recast);
                bools[row * 5 + 1] = u8::from(template.reuse_stats_on_recast);
                bools[row * 5 + 2] = u8::from(template.inherit_owner_def_res);
                bools[row * 5 + 3] = u8::from(overrides.inherit_owner_def_res.unwrap_or(false));
                bools[row * 5 + 4] = u8::from(template.clone_build.is_some());
            }
            {
                let overrides = &template.policy_overrides;
                let cats = out.i32_row_mut("template_cat", batch_index)?;
                cats[row * 5] = self.dense_id("template.kind", template.kind.0, &format!("{prefix}.kind"))?;
                if let Some(boss_kind) = identity.boss_kind {
                    let raw = u32::try_from(boss_kind).map_err(|_| EncodeError::UnknownCategory {
                        path: format!("{prefix}.identity.boss_kind"),
                        raw: boss_kind.to_string(),
                    })?;
                    cats[row * 5 + 1] =
                        self.dense_id("template.identity.boss_kind", raw, &format!("{prefix}.identity.boss_kind"))?;
                }
                for (slot, value) in [
                    overrides.owner_resolution.map(owner_resolution_id),
                    overrides.damage_share.map(damage_share_id),
                    overrides.merge.map(merge_id),
                ]
                .into_iter()
                .enumerate()
                {
                    if let Some(value) = value {
                        cats[row * 5 + 2 + slot] = value;
                    }
                }
                let presence = out.u8_row_mut("template_cat_present", batch_index)?;
                // `[0]` 跟随模板行；`[1]` 是 boss_kind 的 Option；`[2..4]` 跟随三路 override。
                presence[row * 5] = 1;
                presence[row * 5 + 1] = u8::from(identity.boss_kind.is_some());
                presence[row * 5 + 2] = u8::from(overrides.owner_resolution.is_some());
                presence[row * 5 + 3] = u8::from(overrides.damage_share.is_some());
                presence[row * 5 + 4] = u8::from(overrides.merge.is_some());
            }
            out.i32_row_mut("template_team", batch_index)?[row] = index.runtime_teams[&template.team] as i32;
            out.i32_row_mut("template_player_ref", batch_index)?[row] = index.player_keys[&template.id] as i32;
            {
                let overrides = &template.policy_overrides;
                let present = out.u8_row_mut("template_override_present", batch_index)?;
                present[row * 4] = u8::from(overrides.owner_resolution.is_some());
                present[row * 4 + 1] = u8::from(overrides.damage_share.is_some());
                present[row * 4 + 2] = u8::from(overrides.merge.is_some());
                present[row * 4 + 3] = u8::from(overrides.inherit_owner_def_res.is_some());
            }
            {
                let attrs = out.u32_row_mut("template_clone_attr", batch_index)?;
                if let Some(clone_build) = &template.clone_build {
                    for (slot, value) in clone_build.attrs.iter().enumerate() {
                        attrs[row * 8 + slot] = *value;
                    }
                }
            }
            {
                let bonus = out.i32_row_mut("template_clone_weapon_bonus", batch_index)?;
                if let Some(clone_build) = &template.clone_build {
                    for (slot, value) in clone_build.weapon_attr_bonus.iter().enumerate() {
                        bonus[row * 8 + slot] = *value;
                    }
                }
            }
            {
                let mut axis: [Option<i32>; 9] = [None; 9];
                for entry in &identity.immunity {
                    let slot = IMMUNITY_AXIS.iter().position(|status| *status == entry.status).ok_or_else(|| {
                        EncodeError::UnknownCategory {
                            path: format!("{prefix}.identity.immunity[].status"),
                            raw: entry.status.clone(),
                        }
                    })?;
                    if axis[slot].is_some() {
                        return Err(EncodeError::DuplicateCategory {
                            path: format!("{prefix}.identity.immunity[{slot}].status"),
                        });
                    }
                    axis[slot] = Some(entry.threshold);
                }
                let numbers = out.f32_row_mut("immunity_num", batch_index)?;
                for (slot, threshold) in axis.iter().enumerate() {
                    if let Some(threshold) = threshold {
                        numbers[row * 9 + slot] =
                            self.fitted_value(f64::from(*threshold), "template.identity.immunity.threshold", &prefix)?;
                    }
                }
                let presence = out.u8_row_mut("immunity_present", batch_index)?;
                for (slot, threshold) in axis.iter().enumerate() {
                    presence[row * 9 + slot] = u8::from(threshold.is_some());
                }
            }
            clan_groups.push(identity.clan_group);
        }
        // clan_equal：阵营相等关系矩阵（u8 0/1），包含自比。
        {
            let matrix = out.u8_row_mut("clan_equal", batch_index)?;
            for (left, left_group) in clan_groups.iter().enumerate() {
                for (right, right_group) in clan_groups.iter().enumerate() {
                    matrix[left * h_max + right] = u8::from(left_group == right_group);
                }
            }
        }
        Ok(())
    }

    fn write_lane_family(&self, index: &SampleIndex<'_>, batch_index: usize, out: &mut EncodedBatch) -> Result<(), EncodeError> {
        let mut lane_row = 0usize;
        let mut lane_starts = Vec::with_capacity(index.template_rows.len());
        let mut next_lane = 0usize;
        for template in &index.template_rows {
            lane_starts.push(next_lane);
            next_lane += template.skills.lanes.len();
        }
        for (template_row, template) in index.template_rows.iter().enumerate() {
            for (ordinal, lane) in template.skills.lanes.iter().enumerate() {
                let prefix = format!("templates[{template_row}].skills.lanes[{ordinal}]");
                out.u8_row_mut("lane_mask", batch_index)?[lane_row] = 1;
                out.i32_row_mut("lane_skill_id", batch_index)?[lane_row] =
                    i32::try_from(lane.skill_id).map_err(|_| EncodeError::UnknownCategory {
                        path: format!("{prefix}.skill_id"),
                        raw: lane.skill_id.to_string(),
                    })?;
                let boost_kind = match lane.boost.as_ref().map(|boost| boost.kind.as_str()) {
                    None => 1,
                    Some("normal") => 2,
                    Some("last_boost") => 3,
                    Some("slot_boost") => 4,
                    Some(raw) => {
                        return Err(EncodeError::UnknownCategory {
                            path: format!("{prefix}.boost.kind"),
                            raw: raw.to_owned(),
                        });
                    }
                };
                out.i32_row_mut("lane_boost_kind", batch_index)?[lane_row] = boost_kind;
                out.i32_row_mut("lane_template", batch_index)?[lane_row] = template_row as i32;
                out.i32_row_mut("lane_key", batch_index)?[lane_row] =
                    i32::try_from(lane.fixed_lane_key).map_err(|_| EncodeError::InvalidSlotValue {
                        path: format!("{prefix}.fixed_lane_key"),
                    })?;
                let values = [
                    f64::from(lane.level),
                    f64::from(lane.build_level),
                    f64::from(lane.boost.as_ref().map_or(0, |boost| boost.base)),
                    f64::from(lane.boost.as_ref().map_or(0, |boost| boost.extra)),
                ];
                let present = [true, true, lane.boost.is_some(), lane.boost.is_some()];
                {
                    let numbers = out.f32_row_mut("lane_num", batch_index)?;
                    for (slot, value) in values.into_iter().enumerate() {
                        numbers[lane_row * 4 + slot] = if present[slot] {
                            self.fitted_value(
                                value,
                                ["lane.level", "lane.build_level", "lane.boost.base", "lane.boost.extra"][slot],
                                &prefix,
                            )?
                        } else {
                            0.0
                        };
                    }
                }
                {
                    let presence = out.u8_row_mut("lane_num_present", batch_index)?;
                    for (slot, value) in present.into_iter().enumerate() {
                        presence[lane_row * 4 + slot] = u8::from(value);
                    }
                }
                out.u8_row_mut("lane_bool", batch_index)?[lane_row] = u8::from(lane.boosted);
                lane_row += 1;
            }
        }
        Ok(())
    }

    fn write_list_family(
        &self,
        state: &BattleModelState,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        let mut records = Vec::new();
        let mut lane_starts = Vec::with_capacity(index.template_rows.len());
        let mut next_lane = 0usize;
        for template in &index.template_rows {
            lane_starts.push(next_lane);
            next_lane += template.skills.lanes.len();
        }
        for (template_row, template) in index.template_rows.iter().enumerate() {
            let lists = [
                (256, (0..template.skills.lanes.len()).collect::<Vec<_>>()),
                (257, template.skills.merge_lane_order.clone()),
                (258, template.skills.active_order.clone()),
                (259, template.skills.pre_action_order.clone()),
                (260, template.skills.post_damage_order.clone()),
            ];
            for (field_class, values) in lists {
                let length = values.len();
                for (ordinal, target_lane) in values.into_iter().enumerate() {
                    let target = if field_class == 256 {
                        lane_starts[template_row] + ordinal
                    } else {
                        lane_starts[template_row] + target_lane
                    };
                    records.push(ListRecord {
                        owner_scope: 9,
                        owner: template_row as i32,
                        field_class,
                        ordinal: ordinal as i32,
                        target: target as i32,
                        order_key: None,
                        order_domain: Some(length),
                    });
                }
            }
            for (ordinal, deferred) in template.skills.post_action_after_states.iter().enumerate() {
                let target = lane_starts[template_row] + deferred.fixed_lane;
                records.push(ListRecord {
                    owner_scope: 9,
                    owner: template_row as i32,
                    field_class: 261,
                    ordinal: ordinal as i32,
                    target: target as i32,
                    order_key: Some(deferred.state_cursor),
                    order_domain: Some(template.skills.post_action_after_states.len()),
                });
            }
        }
        for (ordinal, id) in state.world.round_order.iter().enumerate() {
            records.push(entity_list_record(index, *id, 262, ordinal, 1)?);
        }
        for (team, roster) in state.world.team_roster.iter().enumerate() {
            for (ordinal, id) in roster.iter().enumerate() {
                records.push(entity_list_record(index, *id, 263, ordinal, 8)?.with_owner(team as i32));
            }
        }
        for (team, alive) in state.world.team_alive.iter().enumerate() {
            for (ordinal, id) in alive.iter().enumerate() {
                records.push(entity_list_record(index, *id, 264, ordinal, 8)?.with_owner(team as i32));
            }
        }
        for (ordinal, id) in state.world.flat_alive.iter().enumerate() {
            records.push(entity_list_record(index, *id, 265, ordinal, 1)?);
        }
        for (team, members) in state.input_teams.iter().enumerate() {
            for (ordinal, id) in members.iter().enumerate() {
                records.push(entity_list_record(index, *id, 268, ordinal, 7)?.with_owner(team as i32));
            }
        }
        for (entity_row, entity) in state.entities.iter().enumerate() {
            for (ordinal, _) in entity.states.iter().enumerate() {
                records.push(ListRecord {
                    owner_scope: 2,
                    owner: entity_row as i32,
                    field_class: 266,
                    ordinal: ordinal as i32,
                    target: (state.entities[..entity_row].iter().map(|e| e.states.len()).sum::<usize>() + ordinal) as i32,
                    order_key: None,
                    order_domain: Some(entity.states.len()),
                });
            }
            for (ordinal, link) in entity.runtime.protect_from.iter().enumerate() {
                records.push(entity_list_record(index, link.owner, 267, ordinal, 2)?.with_owner(entity_row as i32));
            }
        }
        for (ordinal, id) in state.ice_release_events.iter().enumerate() {
            records.push(entity_list_record(index, *id, 269, ordinal, 1)?);
        }
        if records.len() > self.profile.v_max {
            return Err(EncodeError::CapacityExceeded {
                path: dim_path("v"),
                actual: records.len(),
                limit: self.profile.v_max,
            });
        }
        {
            let mask = out.u8_row_mut("list_mask", batch_index)?;
            for row in 0..records.len() {
                mask[row] = 1;
            }
        }
        {
            let indices = out.i32_row_mut("list_index", batch_index)?;
            for (row, record) in records.iter().enumerate() {
                indices[row * 5..row * 5 + 5].copy_from_slice(&[
                    record.owner_scope,
                    record.owner,
                    record.field_class,
                    record.ordinal,
                    record.target,
                ]);
            }
        }
        {
            let positions = out.f32_row_mut("list_position", batch_index)?;
            for (row, record) in records.iter().enumerate() {
                positions[row] = normalize_position(record.ordinal as usize, record.order_domain.unwrap_or(1));
            }
        }
        let order_values: Vec<Option<u64>> = records.iter().map(|record| record.order_key).collect();
        {
            let keys = out.u32_row_mut("order_key", batch_index)?;
            for (row, key) in order_values.iter().enumerate().filter_map(|(row, key)| key.map(|key| (row, key))) {
                keys[row * 2] = key as u32;
                keys[row * 2 + 1] = (key >> 32) as u32;
            }
        }
        {
            let present = out.u8_row_mut("order_key_present", batch_index)?;
            for (row, key) in order_values.iter().enumerate() {
                present[row] = u8::from(key.is_some());
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- 数值助手

    /// 逐字段 `N_f`；manifest 缺该字段或声明不是 fitted 时报错，不套用默认常数。
    fn fitted_value(&self, value: f64, path: &str, prefix: &str) -> Result<f32, EncodeError> {
        // 必须检查变换前的原值；否则 ±Inf 会被 clamp 伪装成有限的 ±4。
        if !value.is_finite() {
            return Err(EncodeError::NonFiniteValue {
                path: format!("{prefix}{path}"),
            });
        }
        let field = self.manifest.normalization.get(path).ok_or_else(|| EncodeError::MissingCalibration {
            path: format!("{prefix}{path}"),
        })?;
        let fitted = field.fitted.as_ref().ok_or_else(|| EncodeError::ManifestMismatch {
            path: format!("{prefix}{path}"),
            detail: "声明为 fitted 但缺少常数".to_owned(),
        })?;
        let transformed = normalize_fitted(value, fitted.s_f, fitted.c_f);
        to_f32_checked(transformed, &format!("{prefix}{path}"))
    }

    /// 固定计数尺度；分母来自 `FIXED_COUNT_SLOTS`，不随容量放大。
    fn fixed_count_value(&self, value: f64, path: &str, prefix: &str) -> Result<f32, EncodeError> {
        let divisor = FIXED_COUNT_SLOTS
            .iter()
            .find(|(candidate, _)| *candidate == path)
            .map(|(_, divisor)| *divisor)
            .ok_or_else(|| EncodeError::ManifestMismatch {
                path: format!("{prefix}{path}"),
                detail: "未登记的固定计数槽".to_owned(),
            })?;
        to_f32_checked(normalize_fixed_count(value, divisor), &format!("{prefix}{path}"))
    }

    fn dense_id(&self, domain: &'static str, raw: u32, path: &str) -> Result<i32, EncodeError> {
        self.vocabularies[domain].dense_id(raw, path)
    }
}

/// 载荷 kind 与分支匹配校验；Boss 专属 kind 直接拒绝（规格第 8、9 节）。
fn validate_payload(prefix: &str, payload: &ModelPayload) -> Result<(), EncodeError> {
    let kind = payload.kind.as_str();
    let branches: [(&str, bool); 15] = [
        ("fire_mag_half_steps", payload.fire_mag_half_steps.is_some()),
        ("ice", payload.ice.is_some()),
        ("shield_value", payload.shield_value.is_some()),
        ("curse", payload.curse.is_some()),
        ("poison", payload.poison.is_some()),
        ("haste", payload.haste.is_some()),
        ("berserk", payload.berserk.is_some()),
        ("charm", payload.charm.is_some()),
        ("slow", payload.slow.is_some()),
        ("iron", payload.iron.is_some()),
        ("covid_boss", payload.covid_boss.is_some()),
        ("covid_infection", payload.covid_infection.is_some()),
        ("saitama_boss", payload.saitama_boss.is_some()),
        ("lazy_boss", payload.lazy_boss.is_some()),
        ("lazy_infection", payload.lazy_infection.is_some()),
    ];
    if kind == "none" {
        if branches.iter().any(|(_, present)| *present) {
            return Err(EncodeError::InvalidState {
                path: format!("{prefix}.payload.kind"),
            });
        }
        return Ok(());
    }
    if UNSUPPORTED_PAYLOAD_KINDS.contains(&kind) || !branches.iter().any(|(name, _)| *name == kind) {
        return Err(EncodeError::UnsupportedPayloadKind {
            path: format!("{prefix}.payload.kind"),
            kind: kind.to_owned(),
        });
    }
    for (name, present) in branches {
        if present != (name == kind) {
            return Err(EncodeError::InvalidState {
                path: format!("{prefix}.payload.kind"),
            });
        }
    }
    Ok(())
}

/// `entity_num` 原始值；absent Option 的槽位填 0（presence 另行记录）。
fn entity_num_values(runtime: &ModelPlayerRuntime) -> [f64; 24] {
    let mut values = [0.0f64; 24];
    values[0] = f64::from(runtime.hp);
    values[1] = f64::from(runtime.attack);
    values[2] = f64::from(runtime.magic);
    values[3] = f64::from(runtime.magic_point);
    values[4] = f64::from(runtime.wisdom);
    values[5] = f64::from(runtime.speed);
    values[6] = f64::from(runtime.defense);
    values[7] = f64::from(runtime.resistance);
    values[8] = f64::from(runtime.agility);
    values[9] = f64::from_bits(runtime.at_boost_bits);
    values[10] = f64::from(runtime.attr_sum);
    values[11] = f64::from(runtime.atk_sum);
    values[12] = f64::from_bits(runtime.attract_bits);
    values[13] = f64::from(runtime.shield);
    values[14] = runtime.protect_pre_defend_skill_count.map_or(0.0, |count| count as f64);
    values[15] = f64::from(runtime.move_state.speed_points);
    values[16] = f64::from(runtime.charge.step);
    values[17] = runtime.accumulate.acc();
    values[18] = runtime.accumulate.charge_bonus();
    if let Some(hide) = runtime.hide {
        values[19] = f64::from(hide.level);
        values[20] = f64::from_bits(hide.attract_bits);
        values[21] = f64::from(hide.agility);
        values[22] = f64::from(hide.defense);
        values[23] = f64::from(hide.resistance);
    }
    values
}

/// `entity_num_present`：只有 `[14]` 与 hide 的 `[19..23]` 跟随 Option，其余跟随实体行。
fn entity_num_present(runtime: &ModelPlayerRuntime) -> [bool; 24] {
    let mut present = [true; 24];
    present[14] = runtime.protect_pre_defend_skill_count.is_some();
    for slot in present.iter_mut().skip(19).take(5) {
        *slot = runtime.hide.is_some();
    }
    present
}

/// `template_num` 原始值；`[17..30]` 在 `clone_build` 缺失时保持 0。
fn template_num_values(template: &ModelTemplate) -> [f64; 31] {
    let mut values = [0.0f64; 31];
    values[0] = f64::from(template.max_hp);
    values[1] = f64::from(template.attack);
    values[2] = f64::from(template.magic);
    values[3] = f64::from(template.magic_point);
    values[4] = f64::from(template.wisdom);
    values[5] = f64::from(template.speed);
    values[6] = f64::from(template.defense);
    values[7] = f64::from(template.resistance);
    values[8] = f64::from(template.agility);
    values[9] = f64::from_bits(template.at_boost_bits);
    values[10] = f64::from(template.attr_sum);
    values[11] = f64::from(template.atk_sum);
    values[12] = f64::from_bits(template.attract_bits);
    values[13] = f64::from(template.reserved_player_ids_before_spawn);
    values[14] = template.identity.boss_action_prob_count as f64;
    values[15] = f64::from(template.identity.boost_immune_threshold);
    values[16] = f64::from(template.move_state.speed_points);
    if let Some(clone_build) = &template.clone_build {
        values[17] = f64::from_bits(clone_build.name_factor_bits);
        values[18] = f64::from_bits(clone_build.child_name_factor_bits);
        let adjustments = &clone_build.adjustments;
        values[19] = f64::from(adjustments.max_hp);
        values[20] = f64::from(adjustments.attack);
        values[21] = f64::from(adjustments.magic);
        values[22] = f64::from(adjustments.wisdom);
        values[23] = f64::from(adjustments.speed);
        values[24] = f64::from(adjustments.defense);
        values[25] = f64::from(adjustments.resistance);
        values[26] = f64::from(adjustments.agility);
        values[27] = f64::from_bits(adjustments.at_boost_delta_bits);
        values[28] = adjustments.attr_sum as f64;
        values[29] = f64::from(adjustments.atk_sum);
        values[30] = f64::from_bits(adjustments.attract_delta_bits);
    }
    values
}

/// `template_num_present`：`[0..16]`（含 `move_state` 槽 16）跟随模板行，
/// `[17..30]` 跟随 `clone_build` 整体存在性。
fn template_num_present(template: &ModelTemplate) -> [bool; 31] {
    let mut present = [false; 31];
    for slot in present.iter_mut().take(17) {
        *slot = true;
    }
    if template.clone_build.is_some() {
        for slot in present.iter_mut().skip(17) {
            *slot = true;
        }
    }
    present
}

fn normalize_position(ordinal: usize, length: usize) -> f32 { ordinal as f32 / length.saturating_sub(1).max(1) as f32 }

/// `RuntimeCorpseKind` 的固定词表：枚举 `None` 是真实类 1，不是缺失。
fn corpse_kind_id(corpse: RuntimeCorpseKind) -> i32 {
    match corpse {
        RuntimeCorpseKind::None => 1,
        RuntimeCorpseKind::Merge => 2,
        RuntimeCorpseKind::Zombie => 3,
    }
}

/// `owner_resolution` 的固定词表（1 起，与规格第 4 节分类落域表一致）。
fn owner_resolution_id(policy: OwnerResolutionPolicy) -> i32 {
    match policy {
        OwnerResolutionPolicy::SelfEntity => 1,
        OwnerResolutionPolicy::RootOwner => 2,
    }
}

/// `damage_share` 的固定词表。
fn damage_share_id(policy: DamageSharePolicy) -> i32 {
    match policy {
        DamageSharePolicy::None => 1,
        DamageSharePolicy::ShareToOwner => 2,
        DamageSharePolicy::ShareToSummons => 3,
    }
}

/// `merge` 的固定词表。
fn merge_id(policy: MergePolicy) -> i32 {
    match policy {
        MergePolicy::None => 1,
        MergePolicy::FixedLane => 2,
        MergePolicy::DropUnmappedSkills => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::batch::{Dtype, TENSOR_SPECS};
    use crate::encoder::capacity::BASELINE_64;
    use crate::encoder::manifest::{CALIBRATION_SCHEMA, CALIBRATION_SCHEMA_VERSION, CalibrationFieldReport, CalibrationReport};
    use crate::runtime::RuntimeRunner;
    use serde_json::Value;

    fn runner() -> RuntimeRunner {
        RuntimeRunner::new_from_namerena_raw("private-left@red\n\nprivate-right@blue\nseed:private-seed".into()).unwrap()
    }

    /// 跑若干回合后的状态；召唤/分身类 build 会带来更多实体与蓝图。
    fn battle_state(rounds: usize) -> BattleModelState {
        let mut runner = runner();
        for _ in 0..rounds {
            if runner.have_winner() {
                break;
            }
            runner.main_round();
        }
        runner.model_state().unwrap()
    }

    fn calibration_field() -> CalibrationFieldReport {
        CalibrationFieldReport {
            count: 100,
            min: 0.0,
            max: 100.0,
            p50: 10.0,
            p99: 64.0,
            abs_p50: 10.0,
            abs_p99: 64.0,
            s_f: 4.0,
            c_f: 64.0,
        }
    }

    /// 合成覆盖全部数值槽的报告，包含真实采集器也会输出的固定计数统计。
    fn manifest() -> EncoderManifest {
        let mut fields = BTreeMap::new();
        for (path, _) in required_normalization() {
            fields.insert(path.to_owned(), calibration_field());
        }
        let report = CalibrationReport {
            schema: CALIBRATION_SCHEMA.to_owned(),
            schema_version: CALIBRATION_SCHEMA_VERSION,
            state_schema_version: MODEL_STATE_SCHEMA_VERSION,
            split: "train".to_owned(),
            require_label: true,
            samples_seen: 100,
            samples_selected: 100,
            input_sha256: "aa".to_owned(),
            executable_sha256: "bb".to_owned(),
            selected_rows_digest: "cc".to_owned(),
            calibrator: "test-calibrator".to_owned(),
            fields,
        };
        EncoderManifest::from_calibration(&report, &BASELINE_64).unwrap()
    }

    fn encoder() -> FeatureEncoder { FeatureEncoder::new(manifest()).expect("测试 manifest 必须有效") }

    /// `core.entity.minion_counter` 槽（U64 计数语义）。
    fn counter_slot(value: u64) -> crate::runtime::model_state::ModelSlot {
        crate::runtime::model_state::ModelSlot {
            slot_id: 5,
            bool_value: None,
            i64_value: None,
            u64_value: Some(value),
            template: None,
        }
    }

    /// 复制实体并分配新 id；引用仍指向原实体，因此状态保持自洽。
    fn replicate_entities(state: &BattleModelState, extra: usize) -> BattleModelState {
        let mut cloned = state.clone();
        let base = cloned.entities.len();
        for index in 0..extra {
            let mut entity = cloned.entities[index % base].clone();
            entity.id = EntityIdx(10_000 + index as u32);
            cloned.entity_slot_count = cloned.entity_slot_count.max(entity.id.0 as usize + 1);
            cloned.entities.push(entity);
        }
        cloned
    }

    /// 重排实体存储顺序（世界列表与引用不变，由 encoder 按 EntityIdx 解引用）。
    fn permuted_entities(state: &BattleModelState) -> BattleModelState {
        let mut cloned = state.clone();
        cloned.entities.reverse();
        cloned
    }

    /// 按 dtype 取出整段字节，用于逐字节比较。
    fn tensor_bytes(batch: &EncodedBatch, name: &str) -> Vec<u8> {
        let spec = EncodedBatch::spec(name).unwrap();
        match spec.dtype {
            Dtype::F32 => slice_as_bytes(batch.f32_all(name).unwrap()).to_vec(),
            Dtype::I32 => slice_as_bytes(batch.i32_all(name).unwrap()).to_vec(),
            Dtype::U8 => batch.u8_all(name).unwrap().to_vec(),
            Dtype::U32 => slice_as_bytes(batch.u32_all(name).unwrap()).to_vec(),
        }
    }

    fn slice_as_bytes<T>(values: &[T]) -> &[u8] {
        unsafe { std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values)) }
    }

    // ------------------------------------------------------------ 基本路径

    #[test]
    fn encodes_global_entity_and_template_families() {
        let state = battle_state(12);
        let batch = encoder().encode(&state).unwrap();
        let mask = batch.u8_all("entity_mask").unwrap();
        assert_eq!(mask[..state.entities.len()], vec![1; state.entities.len()][..]);
        assert!(
            mask[state.entities.len()..].iter().all(|value| *value == 0),
            "padding 行必须为 0"
        );
        let template_mask = batch.u8_all("template_mask").unwrap();
        let measure = CapacityMeasure::measure(&state);
        assert_eq!(template_mask[..measure.h], vec![1; measure.h][..]);
        assert!(template_mask[measure.h..].iter().all(|value| *value == 0));
        // global_num[4] 是 legacy_step_scheduler 布尔；[5] 是固定计数尺度。
        let global = batch.f32_all("global_num").unwrap();
        assert_eq!(global[4], f32::from(state.legacy_step_scheduler));
        assert!((global[5] - state.entities.len() as f32 / 15.0).abs() < 1e-6);
        // team_mask 覆盖全部输入队伍。
        let team_mask = batch.u8_all("team_mask").unwrap();
        assert_eq!(team_mask[..state.input_teams.len()], vec![1; state.input_teams.len()][..]);
        assert!(team_mask[state.input_teams.len()..].iter().all(|value| *value == 0));
    }

    #[test]
    fn encodes_lane_rows_and_boost_presence() {
        let state = battle_state(2);
        let batch = encoder().encode(&state).unwrap();
        let measure = CapacityMeasure::measure(&state);
        assert_eq!(batch.u8_all("lane_mask").unwrap()[..measure.l], vec![1; measure.l][..]);
        assert!(batch.u8_all("lane_mask").unwrap()[measure.l..].iter().all(|value| *value == 0));
        let templates = batch.i32_all("lane_template").unwrap();
        assert!(templates[..measure.l].iter().all(|row| *row >= 0));
        let presence = batch.u8_all("lane_num_present").unwrap();
        for row in 0..measure.l {
            assert_eq!(&presence[row * 4..row * 4 + 2], &[1, 1]);
        }
        let list = batch.i32_all("list_index").unwrap();
        let list_mask = batch.u8_all("list_mask").unwrap();
        let list_count = list_mask.iter().position(|value| *value == 0).unwrap_or(list_mask.len());
        assert!(list_mask[..list_count].iter().all(|value| *value == 1));
        assert!(list_mask[list_count..].iter().all(|value| *value == 0));
        for row in 0..list_count {
            assert!(list[row * 5 + 4] >= 0);
            if (256..=261).contains(&list[row * 5 + 2]) {
                assert_eq!(list[row * 5], 9);
            }
        }
    }

    #[test]
    fn encoding_is_deterministic_within_one_build() {
        let state = battle_state(8);
        let encoder = encoder();
        let first = encoder.encode(&state).unwrap();
        let second = encoder.encode(&state).unwrap();
        for spec in TENSOR_SPECS {
            assert_eq!(
                tensor_bytes(&first, spec.name),
                tensor_bytes(&second, spec.name),
                "{}",
                spec.name
            );
        }
    }

    // ------------------------------------------------------------ 容量边界

    #[test]
    fn entity_capacity_boundary_is_enforced() {
        let state = battle_state(2);
        let encoder = encoder();
        let base = state.entities.len();
        assert!(
            encoder.encode(&replicate_entities(&state, 64 - base)).is_ok(),
            "E=64 必须可编码"
        );
        assert!(
            matches!(
                encoder.encode(&replicate_entities(&state, 65 - base)).unwrap_err(),
                EncodeError::CapacityExceeded { ref path, actual: 65, limit: 64 } if path == "entities"
            ),
            "E=65 必须报 CapacityExceeded"
        );
    }

    #[test]
    fn input_team_capacity_boundary_is_enforced() {
        let state = battle_state(2);
        let encoder = encoder();
        let mut at_limit = state.clone();
        let member = at_limit.entities[0].runtime.root_owner;
        while at_limit.input_teams.len() < 32 {
            at_limit.input_teams.push(vec![member]);
        }
        assert!(encoder.encode(&at_limit).is_ok(), "T=32 必须可编码");
        at_limit.input_teams.push(vec![member]);
        assert!(matches!(
            encoder.encode(&at_limit).unwrap_err(),
            EncodeError::CapacityExceeded { ref path, actual: 33, limit: 32 } if path == "input_teams"
        ));
    }

    #[test]
    fn runtime_team_capacity_boundary_is_enforced() {
        let state = battle_state(2);
        let encoder = encoder();
        let base = state.entities.len();
        // 30 个实体各占一个 runtime team 原值，加上 world 两行 = 32，恰好达标。
        let mut at_limit = replicate_entities(&state, 30 - base);
        for (index, entity) in at_limit.entities.iter_mut().enumerate() {
            entity.runtime.team = 100 + index;
        }
        assert_eq!(CapacityMeasure::measure(&at_limit).r, 32);
        assert!(encoder.encode(&at_limit).is_ok(), "R=32 必须可编码");
        let mut over = replicate_entities(&state, 31 - base);
        for (index, entity) in over.entities.iter_mut().enumerate() {
            entity.runtime.team = 100 + index;
        }
        assert_eq!(CapacityMeasure::measure(&over).r, 33);
        assert!(matches!(
            encoder.encode(&over).unwrap_err(),
            EncodeError::CapacityExceeded { ref path, limit: 32, .. } if path == "runtime_team"
        ));
    }

    #[test]
    fn runtime_team_measure_matches_written_union_with_blueprints_and_empty_rows() {
        let mut state = battle_state(0);
        state.world.team_roster.resize(3, Vec::new());
        state.world.team_alive.resize(4, Vec::new());
        for entity in &mut state.entities {
            entity.runtime.team = 2;
            entity.template.team = 6;
            entity.slots.clear();
        }
        state.template_slots.clear();
        state.battle_slots.clear();
        let mut blueprint = state.entities[0].template.clone();
        blueprint.team = 1000;
        state.entities[0].slots.push(crate::runtime::model_state::ModelSlot {
            slot_id: 0,
            bool_value: None,
            i64_value: None,
            u64_value: None,
            template: Some(blueprint.clone()),
        });
        blueprint.team = 2000;
        state.template_slots.push(crate::runtime::model_state::ModelSlot {
            slot_id: 0,
            bool_value: None,
            i64_value: None,
            u64_value: None,
            template: Some(blueprint),
        });
        // 集合为 {0,1,2,3,6,1000,2000}；计量的是关系行数，不是最大原编号加一。
        let measure = CapacityMeasure::measure(&state);
        assert_eq!(measure.r, 7);
        let batch = encoder().encode(&state).unwrap();
        let mask = batch.u8_all("runtime_team_mask").unwrap();
        assert_eq!(&mask[..7], &[1; 7]);
        assert!(mask[7..].iter().all(|value| *value == 0));
        for row in 0..measure.e {
            assert_eq!(batch.i32_all("entity_team").unwrap()[2 * row + 1], 2);
            assert_eq!(batch.i32_all("template_team").unwrap()[row], 4);
        }
        assert_eq!(batch.i32_all("template_team").unwrap()[measure.e], 5);
        assert_eq!(batch.i32_all("template_team").unwrap()[measure.e + 1], 6);
        assert_eq!(measure.h, measure.e + 2);
        assert_eq!(
            batch.u8_all("template_mask").unwrap().iter().filter(|value| **value == 1).count(),
            measure.h
        );
    }

    #[test]
    fn slot_capacity_boundary_is_enforced() {
        let state = battle_state(2);
        let encoder = encoder();
        let base = CapacityMeasure::measure(&state).q;
        let mut at_limit = state.clone();
        for index in 0..(512 - base) {
            at_limit.entities[0].slots.push(counter_slot(index as u64));
        }
        assert_eq!(CapacityMeasure::measure(&at_limit).q, 512);
        assert!(encoder.encode(&at_limit).is_ok(), "Q=512 必须可编码");
        let mut over = at_limit.clone();
        over.entities[0].slots.push(counter_slot(1));
        assert!(matches!(
            encoder.encode(&over).unwrap_err(),
            EncodeError::CapacityExceeded { ref path, actual: 513, limit: 512 } if path == "slots"
        ));
    }

    // ------------------------------------------------------------ 门禁与拒绝路径

    #[test]
    fn already_decided_state_is_rejected() {
        let mut state = battle_state(2);
        state.world.winner_team = Some(0);
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::AlreadyDecided { .. }
        ));
    }

    #[test]
    fn schema_mismatch_is_rejected() {
        let mut state = battle_state(2);
        state.schema_version = MODEL_STATE_SCHEMA_VERSION + 1;
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::SchemaMismatch { .. }
        ));
    }

    #[test]
    fn entity_ids_must_fit_entity_slot_count() {
        let mut state = battle_state(0);
        let max_id = state.entities.iter().map(|entity| entity.id.0 as usize).max().unwrap();
        state.entity_slot_count = max_id + 1;
        assert!(encoder().encode(&state).is_ok());
        for invalid_count in [max_id, 0] {
            state.entity_slot_count = invalid_count;
            assert!(matches!(
                encoder().encode(&state).unwrap_err(),
                EncodeError::InvalidState { ref path } if path.ends_with(".id")
            ));
        }
    }

    #[test]
    fn reserved_flag_bits_are_rejected_and_named_bits_are_written() {
        let mut state = battle_state(2);
        state.entities[0].compressed_state_flags = 0b0001_0101;
        let batch = encoder().encode(&state).unwrap();
        let flags = batch.u8_all("entity_flags").unwrap();
        assert_eq!(&flags[..8], &[1, 0, 1, 0, 1, 0, 0, 0]);
        state.entities[0].compressed_state_flags = 0b0010_0000;
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::ReservedFlagBitSet { .. }
        ));
    }

    #[test]
    fn non_finite_value_is_rejected() {
        let encoder = encoder();
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut state = battle_state(2);
            state.entities[0].runtime.at_boost_bits = value.to_bits();
            assert!(matches!(
                encoder.encode(&state).unwrap_err(),
                EncodeError::NonFiniteValue { .. }
            ));
            let mut state = battle_state(2);
            state.entities[0].template.at_boost_bits = value.to_bits();
            assert!(matches!(
                encoder.encode(&state).unwrap_err(),
                EncodeError::NonFiniteValue { .. }
            ));
        }
    }

    #[test]
    fn unknown_category_value_is_rejected() {
        let mut state = battle_state(2);
        state.entities[0].template.kind = crate::runtime::extension::PlayerKindId(9999);
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::UnknownCategory { .. }
        ));
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn oversized_boss_kind_does_not_alias_a_valid_category() {
        let mut state = battle_state(0);
        state.entities[0].template.identity.boss_kind = Some((1u64 << 32) as usize);
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::UnknownCategory { ref path, ref raw }
                if path == "templates[0].identity.boss_kind" && raw == "4294967296"
        ));
    }

    #[test]
    fn payload_kind_and_branch_must_match() {
        let mut state = battle_state(2);
        state.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
            legacy_order_key: 1,
            extension_state_id: None,
            hook_mask: 0,
            priority: 0,
            registration_order: 1,
            runtime_registration_order: 1,
            payload: ModelPayload {
                kind: "none".to_owned(),
                ..ModelPayload::default()
            },
        });
        assert!(encoder().encode(&state).is_ok(), "kind=none 且无分支必须可编码");
        let mut boss = state.clone();
        boss.entities[0].states[0].payload.kind = "covid_boss".to_owned();
        boss.entities[0].states[0].payload.covid_boss = Some(crate::runtime::model_state::CovidBossPayload { mutation: 1 });
        assert!(matches!(
            encoder().encode(&boss).unwrap_err(),
            EncodeError::UnsupportedPayloadKind { .. }
        ));
        let mut mismatched = state.clone();
        mismatched.entities[0].states[0].payload.kind = "poison".to_owned();
        assert!(matches!(
            encoder().encode(&mismatched).unwrap_err(),
            EncodeError::InvalidState { .. }
        ));
        let mut extra = state.clone();
        extra.entities[0].states[0].payload.shield_value = Some(3);
        assert!(matches!(
            encoder().encode(&extra).unwrap_err(),
            EncodeError::InvalidState { .. }
        ));
    }

    #[test]
    fn slot_whitelist_is_enforced() {
        let mut state = battle_state(2);
        state.entities[0].slots.push(crate::runtime::model_state::ModelSlot {
            slot_id: 9,
            bool_value: None,
            i64_value: None,
            u64_value: Some(1),
            template: None,
        });
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::UnknownSlotSemantics { .. }
        ));
        let mut dangling = battle_state(2);
        dangling.entities[0].slots.push(crate::runtime::model_state::ModelSlot {
            slot_id: 4,
            bool_value: None,
            i64_value: None,
            u64_value: Some(4242),
            template: None,
        });
        assert!(matches!(
            encoder().encode(&dangling).unwrap_err(),
            EncodeError::InvalidReference { .. }
        ));
    }

    #[test]
    fn charm_group_reference_is_validated() {
        let mut state = battle_state(2);
        state.entities[0].states.push(crate::runtime::model_state::ModelStateEntry {
            legacy_order_key: 1,
            extension_state_id: None,
            hook_mask: 0,
            priority: 0,
            registration_order: 1,
            runtime_registration_order: 1,
            payload: ModelPayload {
                kind: "charm".to_owned(),
                charm: Some(crate::runtime::model_state::CharmPayload {
                    group_id: usize::MAX,
                    effective_team_idx: None,
                    source_team_idx: None,
                    target: None,
                    step: 1,
                }),
                ..ModelPayload::default()
            },
        });
        assert!(matches!(
            encoder().encode(&state).unwrap_err(),
            EncodeError::InvalidReference { .. }
        ));
    }

    // ------------------------------------------------------------ presence / mask 语义

    #[test]
    fn option_absence_is_distinct_from_zero() {
        let mut state = battle_state(2);
        state.entities[0].runtime.protect_pre_defend_skill_count = Some(0);
        let batch = encoder().encode(&state).unwrap();
        assert_eq!(batch.u8_all("entity_num_present").unwrap()[14], 1, "Some(0) 仍是存在");
        assert_eq!(batch.f32_all("entity_num").unwrap()[14], 0.0);

        state.entities[0].runtime.protect_pre_defend_skill_count = None;
        let batch = encoder().encode(&state).unwrap();
        assert_eq!(batch.u8_all("entity_num_present").unwrap()[14], 0, "None 必须 presence=0");
        assert_eq!(batch.f32_all("entity_num").unwrap()[14], 0.0);
    }

    #[test]
    fn hide_presence_drives_numeric_slots() {
        let mut state = battle_state(2);
        state.entities[0].runtime.hide = Some(crate::runtime::entity::HideRuntime {
            level: 3,
            attract_bits: 2.5f64.to_bits(),
            agility: 7,
            defense: 8,
            resistance: 9,
        });
        let batch = encoder().encode(&state).unwrap();
        assert_eq!(batch.u8_all("entity_bool").unwrap()[7], 1, "hide 整体 presence");
        assert_eq!(&batch.u8_all("entity_num_present").unwrap()[19..24], &[1, 1, 1, 1, 1]);
        let numbers = batch.f32_all("entity_num").unwrap();
        assert_eq!(numbers[19], normalize_fitted(3.0, 4.0, 64.0) as f32);
        assert_eq!(numbers[21], normalize_fitted(7.0, 4.0, 64.0) as f32);
        assert_eq!(numbers[22], normalize_fitted(8.0, 4.0, 64.0) as f32);
        assert_eq!(numbers[23], normalize_fitted(9.0, 4.0, 64.0) as f32);
        // attract 走 f64 还原后归一化（s_f=4、c_f=64）。
        let expected = normalize_fitted(2.5, 4.0, 64.0) as f32;
        assert!((numbers[20] - expected).abs() < 1e-6);

        state.entities[0].runtime.hide = None;
        let batch = encoder().encode(&state).unwrap();
        assert_eq!(batch.u8_all("entity_bool").unwrap()[7], 0);
        assert_eq!(&batch.u8_all("entity_num_present").unwrap()[19..24], &[0, 0, 0, 0, 0]);
        assert_eq!(&batch.f32_all("entity_num").unwrap()[19..24], &[0.0; 5]);
    }

    #[test]
    fn optional_references_use_minus_one_without_presence() {
        let mut state = battle_state(2);
        state.entities[0].runtime.protect_to = None;
        state.entities[0].runtime.counter.last_target = None;
        state.entities[0].runtime.assassinate = None;
        let batch = encoder().encode(&state).unwrap();
        let refs = batch.i32_all("entity_ref").unwrap();
        let presence = batch.u8_all("entity_ref_present").unwrap();
        assert_eq!(refs[2], -1, "缺失引用必须是 -1");
        assert_eq!(&presence[2..5], &[0, 0, 0]);
        assert!(refs[0] >= 0 && refs[1] >= 0, "owner/root_owner 必填");
        assert_eq!(&presence[0..2], &[1, 1]);

        let target = state.entities[1].id;
        state.entities[0].runtime.protect_to = Some(target);
        state.entities[0].runtime.assassinate = Some(crate::runtime::entity::AssassinateRuntime {
            fixed_lane: 0,
            target,
            break_on_damage: true,
        });
        let batch = encoder().encode(&state).unwrap();
        let refs = batch.i32_all("entity_ref").unwrap();
        let presence = batch.u8_all("entity_ref_present").unwrap();
        assert_eq!(refs[2], 1, "protect_to 指向实体 1 的稠密行");
        assert_eq!(presence[2], 1);
        assert_eq!(refs[4], 1);
        assert_eq!(presence[4], 1);
    }

    #[test]
    fn team_mask_ignores_alive_and_alive_group_count() {
        let mut state = battle_state(2);
        for entity in &mut state.entities {
            if entity.input_team_index == 0 {
                entity.runtime.alive = false;
            }
        }
        state.world.flat_alive.clear();
        state.world.team_alive = vec![Vec::new(); state.world.team_alive.len()];
        state.world.alive_group_count = 0;
        let batch = encoder().encode(&state).unwrap();
        let team_mask = batch.u8_all("team_mask").unwrap();
        assert_eq!(team_mask[0], 1, "零存活队伍仍是输入标签位置");
        assert_eq!(team_mask[1], 1);
        // alive_group_count 只作机制特征进入 global_num[2]，不作 mask。
        assert_eq!(batch.f32_all("global_num").unwrap()[2], 0.0);
    }

    #[test]
    fn clone_build_presence_controls_template_slots() {
        let mut state = battle_state(4);
        // 先清零前两个实体模板，保证与具体名字无关；再只给第一个挂合成 clone_build。
        for entity in state.entities.iter_mut().take(2) {
            entity.template.clone_build = None;
        }
        let batch = encoder().encode(&state).unwrap();
        assert_eq!(batch.u8_all("template_bool").unwrap()[4], 0);
        // `[0..16]` 跟随模板行（含 move_state 槽 16），`[17..30]` 跟随 clone_build。
        let presence = batch.u8_all("template_num_present").unwrap();
        assert_eq!(&presence[..17], &[1; 17][..]);
        assert_eq!(&presence[17..31], &[0; 14]);

        state.entities[0].template.clone_build = Some(crate::runtime::entity::CloneBuildData {
            attrs: [1; 8],
            weapon_attr_bonus: [0; 8],
            name_factor_bits: 0.5f64.to_bits(),
            child_name_factor_bits: 0.25f64.to_bits(),
            adjustments: crate::runtime::entity::CloneStatAdjustments {
                max_hp: 0,
                attack: 0,
                magic: 0,
                wisdom: 0,
                speed: 0,
                defense: 0,
                resistance: 0,
                agility: 0,
                at_boost_delta_bits: 0.0f64.to_bits(),
                attr_sum: 0,
                atk_sum: 0,
                attract_delta_bits: 0.0f64.to_bits(),
            },
            score_skill_boost_plan: None,
        });
        let batch = encoder().encode(&state).unwrap();
        let bools = batch.u8_all("template_bool").unwrap();
        assert_eq!(bools[4], 1, "clone 整体 presence");
        let presence = batch.u8_all("template_num_present").unwrap();
        assert_eq!(&presence[..17], &[1; 17][..]);
        assert_eq!(&presence[17..31], &[1; 14][..], "`[17..30]` 跟随 clone_build");
        let numbers = batch.f32_all("template_num").unwrap();
        assert_eq!(numbers[17], normalize_fitted(0.5, 4.0, 64.0) as f32);
        assert_eq!(numbers[18], normalize_fitted(0.25, 4.0, 64.0) as f32);
        // 第二个模板仍无 clone_build（template_bool 每行 5 个元素）。
        assert_eq!(bools[5 + 4], 0);
        assert_eq!(&presence[31 + 17..31 + 31], &[0; 14][..]);
        assert_eq!(&presence[31..31 + 17], &[1; 17][..]);
        // clone 数组在整体存在时写入，缺失时为 0。
        let attrs = batch.u32_all("template_clone_attr").unwrap();
        assert_eq!(&attrs[..8], &[1; 8][..]);
        assert_eq!(&attrs[8..16], &[0; 8][..]);
    }

    // ------------------------------------------------------------ 引用与置换

    #[test]
    fn references_are_remapped_to_dense_rows_without_leaking_entity_idx() {
        let mut state = battle_state(4);
        // 稀疏 EntityIdx（带空洞），原编号远大于 E_max。
        let mut renames: BTreeMap<u32, u32> = BTreeMap::new();
        for (index, entity) in state.entities.iter().enumerate() {
            renames.insert(entity.id.0, 500 + index as u32 * 7);
        }
        state.entity_slot_count = state.entity_slot_count.max(*renames.values().max().unwrap() as usize + 1);
        for entity in &mut state.entities {
            entity.id = EntityIdx(renames[&entity.id.0]);
            entity.runtime.owner = EntityIdx(renames[&entity.runtime.owner.0]);
            entity.runtime.root_owner = EntityIdx(renames[&entity.runtime.root_owner.0]);
            if let Some(id) = entity.runtime.protect_to {
                entity.runtime.protect_to = Some(EntityIdx(renames[&id.0]));
            }
            if let Some(id) = entity.runtime.counter.last_target {
                entity.runtime.counter.last_target = Some(EntityIdx(renames[&id.0]));
            }
        }
        for team in &mut state.input_teams {
            for id in team.iter_mut() {
                *id = EntityIdx(renames[&id.0]);
            }
        }
        for list in [&mut state.world.round_order, &mut state.world.flat_alive] {
            for id in list.iter_mut() {
                *id = EntityIdx(renames[&id.0]);
            }
        }
        for roster in state.world.team_roster.iter_mut().chain(state.world.team_alive.iter_mut()) {
            for id in roster.iter_mut() {
                *id = EntityIdx(renames[&id.0]);
            }
        }
        let batch = encoder().encode(&state).unwrap();
        let rows = state.entities.len();
        for value in &batch.i32_all("entity_ref").unwrap()[..5 * rows] {
            if *value >= 0 {
                assert!(*value < rows as i32, "引用必须是稠密行");
            }
        }
        // 原始 EntityIdx 不得出现在任何 i32 张量里。
        for spec in TENSOR_SPECS {
            if spec.dtype != Dtype::I32 {
                continue;
            }
            for value in batch.i32_all(spec.name).unwrap() {
                assert!(*value < 500 || *value == -1, "{} 泄漏了原始编号 {value}", spec.name);
            }
        }
    }

    #[test]
    fn entity_storage_permutation_is_equivariant() {
        let state = battle_state(6);
        let encoder = encoder();
        let baseline = encoder.encode(&state).unwrap();
        let moved = encoder.encode(&permuted_entities(&state)).unwrap();
        let rows = state.entities.len();
        for row in 0..rows {
            let source = rows - 1 - row;
            assert_eq!(
                baseline.u8_all("entity_mask").unwrap()[source],
                moved.u8_all("entity_mask").unwrap()[row]
            );
            let base_ref = &baseline.i32_all("entity_ref").unwrap()[source * 5..source * 5 + 5];
            let moved_ref = &moved.i32_all("entity_ref").unwrap()[row * 5..row * 5 + 5];
            for (left, right) in base_ref.iter().zip(moved_ref) {
                let expected = if *left < 0 { -1 } else { rows as i32 - 1 - *left };
                assert_eq!(*right, expected, "引用必须随置换同步");
            }
            assert_eq!(
                &baseline.f32_all("entity_num").unwrap()[source * 24..source * 24 + 24],
                &moved.f32_all("entity_num").unwrap()[row * 24..row * 24 + 24],
                "数值槽必须随置换同步"
            );
        }
    }

    // ------------------------------------------------------------ 批布局与缓冲复用

    #[test]
    fn batch_slot_matches_standalone_encoding() {
        let state = battle_state(6);
        let encoder = encoder();
        let standalone = encoder.encode(&state).unwrap();
        let mut batch = EncodedBatch::new(encoder.profile(), 3);
        encoder.encode_into(&state, 2, &mut batch).unwrap();
        for spec in TENSOR_SPECS {
            let shape = crate::encoder::batch::tensor_shape(&encoder.profile().dims(), spec.name).unwrap();
            let per_sample: usize = shape.iter().product();
            let element = spec.dtype.size();
            assert_eq!(
                tensor_bytes(&standalone, spec.name),
                tensor_bytes(&batch, spec.name)[2 * per_sample * element..3 * per_sample * element],
                "{} 的批槽位必须与 B=1 逐字节一致",
                spec.name
            );
        }
    }

    #[test]
    fn failed_encode_clears_only_target_slot_and_allows_reuse() {
        let valid = battle_state(0);
        let encoder = encoder();
        let standalone = encoder.encode(&valid).unwrap();
        let padding = EncodedBatch::new(encoder.profile(), 1);
        for case in 0..4 {
            let mut invalid = valid.clone();
            match case {
                0 => invalid.schema_version += 1,
                1 => invalid.entities[0].runtime.at_boost_bits = f64::NAN.to_bits(),
                2 => invalid.entities[0].template.kind = crate::runtime::extension::PlayerKindId(9999),
                _ => {
                    let entry = crate::runtime::model_state::ModelImmunity {
                        status: "fire".to_owned(),
                        threshold: 0,
                    };
                    invalid.entities[0].template.identity.immunity = vec![entry.clone(), entry];
                }
            }
            let mut batch = EncodedBatch::new(encoder.profile(), 3);
            // 所有 dtype、所有张量都写入哨兵，确保清理错误不会被本来就是 padding 的邻居掩盖。
            for spec in TENSOR_SPECS {
                for slot in 0..3 {
                    match spec.dtype {
                        Dtype::F32 => batch.f32_row_mut(spec.name, slot).unwrap().fill((slot + 1) as f32),
                        Dtype::I32 => batch.i32_row_mut(spec.name, slot).unwrap().fill((slot + 1) as i32),
                        Dtype::U8 => batch.u8_row_mut(spec.name, slot).unwrap().fill((slot + 1) as u8),
                        Dtype::U32 => batch.u32_row_mut(spec.name, slot).unwrap().fill((slot + 1) as u32),
                    }
                }
            }
            encoder.encode_into(&valid, 1, &mut batch).unwrap();
            let before: Vec<_> = TENSOR_SPECS.iter().map(|spec| tensor_bytes(&batch, spec.name)).collect();
            let error = encoder.encode_into(&invalid, 1, &mut batch).unwrap_err();
            match case {
                0 => assert!(matches!(error, EncodeError::SchemaMismatch { .. })),
                1 => assert!(matches!(error, EncodeError::NonFiniteValue { .. })),
                2 => assert!(matches!(error, EncodeError::UnknownCategory { .. })),
                _ => assert!(matches!(error, EncodeError::DuplicateCategory { .. })),
            }
            for (spec, previous) in TENSOR_SPECS.iter().zip(&before) {
                let empty = tensor_bytes(&padding, spec.name);
                let stride = empty.len();
                let after = tensor_bytes(&batch, spec.name);
                assert_eq!(&after[..stride], &previous[..stride], "{} 左邻居被修改", spec.name);
                assert_eq!(&after[stride..2 * stride], empty.as_slice(), "{} 失败后不是 padding", spec.name);
                assert_eq!(&after[2 * stride..], &previous[2 * stride..], "{} 右邻居被修改", spec.name);
            }
            encoder.encode_into(&valid, 1, &mut batch).unwrap();
            for (spec, previous) in TENSOR_SPECS.iter().zip(&before) {
                let expected = tensor_bytes(&standalone, spec.name);
                let stride = expected.len();
                let after = tensor_bytes(&batch, spec.name);
                assert_eq!(
                    &after[stride..2 * stride],
                    expected.as_slice(),
                    "{} 失败后复用不一致",
                    spec.name
                );
                assert_eq!(&after[..stride], &previous[..stride]);
                assert_eq!(&after[2 * stride..], &previous[2 * stride..]);
            }
        }
    }

    #[test]
    fn out_of_range_encode_does_not_modify_batch() {
        let state = battle_state(0);
        let encoder = encoder();
        let mut batch = encoder.encode(&state).unwrap();
        let before: Vec<_> = TENSOR_SPECS.iter().map(|spec| tensor_bytes(&batch, spec.name)).collect();
        assert!(matches!(
            encoder.encode_into(&state, 1, &mut batch).unwrap_err(),
            EncodeError::BatchSlotOutOfRange { batch: 1, limit: 1 }
        ));
        for (spec, expected) in TENSOR_SPECS.iter().zip(before) {
            assert_eq!(tensor_bytes(&batch, spec.name), expected, "{} 被越界调用修改", spec.name);
        }
    }

    #[test]
    fn reused_slot_does_not_keep_stale_values() {
        let mut rich = battle_state(8);
        rich.entities[0].runtime.protect_to = Some(rich.entities[1].id);
        rich.entities[0].runtime.hide = Some(crate::runtime::entity::HideRuntime {
            level: 2,
            attract_bits: 1.5f64.to_bits(),
            agility: 1,
            defense: 2,
            resistance: 3,
        });
        let encoder = encoder();
        let mut batch = EncodedBatch::new(encoder.profile(), 1);
        encoder.encode_into(&rich, 0, &mut batch).unwrap();
        let poor = battle_state(2);
        assert!(poor.entities.len() < rich.entities.len(), "测试需要实体数更少的样本");
        encoder.encode_into(&poor, 0, &mut batch).unwrap();
        assert!(
            batch.u8_all("entity_mask").unwrap()[poor.entities.len()..]
                .iter()
                .all(|value| *value == 0),
            "padding 行必须被清掉"
        );
        assert!(
            batch.i32_all("entity_ref").unwrap()[poor.entities.len() * 5..]
                .iter()
                .all(|value| *value == -1),
            "引用 padding 必须为 -1"
        );
        assert_eq!(
            batch.u8_all("entity_bool").unwrap()[7],
            0,
            "上一个样本的 hide presence 不得残留"
        );
    }

    #[test]
    fn batch_and_encoder_profiles_must_match() {
        let state = battle_state(2);
        let encoder = encoder();
        let mut smaller = BASELINE_64;
        smaller.e_max = 32;
        smaller.h_max = 256;
        let mut batch = EncodedBatch::new(&smaller, 1);
        batch.u8_row_mut("entity_mask", 0).unwrap()[0] = 1;
        assert!(matches!(
            encoder.encode_into(&state, 0, &mut batch).unwrap_err(),
            EncodeError::ManifestMismatch { .. }
        ));
        assert_eq!(batch.u8_all("entity_mask").unwrap()[0], 1, "profile 错配不能修改缓冲");
    }

    // ------------------------------------------------------------ manifest 门禁

    #[test]
    fn missing_calibration_is_reported_at_construction() {
        let mut incomplete = manifest();
        incomplete.normalization.remove("entity.runtime.hide.level");
        assert!(matches!(
            FeatureEncoder::new(incomplete).unwrap_err(),
            EncodeError::MissingCalibration { ref path } if path == "entity.runtime.hide.level"
        ));
    }

    #[test]
    fn vocabulary_must_match_default_registry() {
        let mut tampered = manifest();
        tampered.vocabularies.get_mut("runtime.kind").unwrap().entries.insert(4321, 99);
        assert!(matches!(
            FeatureEncoder::new(tampered).unwrap_err(),
            EncodeError::ManifestMismatch { .. }
        ));
    }

    #[test]
    fn wrong_transform_kind_is_rejected() {
        let mut tampered = manifest();
        tampered.normalization.get_mut("global.entity_slot_count").unwrap().transform = TransformKind::Fitted;
        assert!(matches!(
            FeatureEncoder::new(tampered).unwrap_err(),
            EncodeError::ManifestMismatch { .. }
        ));
    }

    #[test]
    fn skill_bearing_battles_encode_end_to_end() {
        // 覆盖火焰/治疗/幻影/毒/冰/急速/魅惑/诅咒/保护/强化/反射/铁壁/分身/召唤/复活等技能，
        // 让状态载荷、幻影与蓝图在真实对局里被编码。
        let pools = [
            "mario@red+fire\nluigi@red+heal\n\npeach@blue+shadow\nbowser@blue+poison\nseed:s1",
            "a@red+ice\nb@red+haste\n\nc@blue+charm\nd@blue+curse\nseed:s2",
            "e@red+protect\nf@red+upgrade\n\ng@blue+reflect\nh@blue+iron\nseed:s3",
            "i@red+clone\nj@red+shadow\n\nk@blue+summon\nl@blue+revive\nseed:s4",
            "m@red+charge\nn@red+accumulate\n\no@blue+hide\np@blue+assassinate\nseed:s5",
        ];
        let encoder = encoder();
        let mut frames = 0;
        for pool in pools {
            let mut runner = RuntimeRunner::new_from_namerena_raw(pool.to_owned()).unwrap();
            for _ in 0..60 {
                if runner.have_winner() {
                    break;
                }
                let state = runner.model_state().unwrap();
                let batch = encoder.encode(&state).unwrap_or_else(|error| panic!("{pool} 编码失败：{error}"));
                // 每个被编码的样本，实体行与模板行都必须与容量计费口径一致。
                let measure = CapacityMeasure::measure(&state);
                let mask = batch.u8_all("entity_mask").unwrap();
                assert_eq!(&mask[..state.entities.len()], &vec![1; state.entities.len()][..]);
                assert_eq!(&batch.u8_all("template_mask").unwrap()[..measure.h], &vec![1; measure.h][..]);
                frames += 1;
                runner.main_round();
            }
            // 终局帧带 winner_team，必须被 AlreadyDecided 拒绝（结果泄漏门禁）。
            let terminal = runner.model_state().unwrap();
            assert!(terminal.world.winner_team.is_some(), "{pool} 应已产生胜者");
            assert!(matches!(
                encoder.encode(&terminal).unwrap_err(),
                EncodeError::AlreadyDecided { .. }
            ));
        }
        assert!(frames > 100, "样本过少（{frames}），冒烟覆盖不足");
    }

    // ------------------------------------------------------------ 字段覆盖

    /// 第 3 节字段覆盖测试：state JSON 的每个叶子都必须有归属。
    ///
    /// - `consumed`：本块写入或校验的字段；
    /// - `planned`：已列入 handoff 分块计划、由后续块写入的族；
    /// - `excluded`：有明确理由不进入模型的字段（重复近似量、bit 通道）。
    ///
    /// 新字段若三类都不匹配，测试失败——不允许 wildcard 吞掉未登记字段。
    #[test]
    fn every_state_field_is_consumed_planned_or_excluded() {
        let state = battle_state(6);
        let value: Value = serde_json::to_value(&state).unwrap();
        let mut leaves = BTreeSet::new();
        flatten(&value, String::new(), &mut leaves);
        assert!(leaves.len() > 20, "展开结果过少，测试可能失效");
        let consumed = [
            "schema_version",
            "round",
            "entity_slot_count",
            "legacy_step_scheduler",
            "world.alive_group_count",
            "world.round_pos",
            "world.winner_team",
            "entities[].id",
            "entities[].input_team_index",
            "entities[].compressed_state_flags",
            "entities[].template.",
            "entities[].runtime.",
        ];
        let planned = [
            "input_teams",
            "world.round_order",
            "world.team_roster",
            "world.team_alive",
            "world.flat_alive",
            "ice_release_events",
            "entities[].states",
            "entities[].state_registration_cursor",
            "entities[].slots",
            "entities[].template.skills",
            "entities[].template.clone_build.score_skill_boost_plan",
            "template_slots",
            "battle_slots",
        ];
        let excluded = [
            "entities[].runtime.at_boost_millionths",
            "entities[].template.at_boost_millionths",
            "entities[].template.clone_build.score_skill_boost_plan.initially_boosted_mask",
        ];
        let mut unknown = Vec::new();
        for leaf in &leaves {
            let hit = |prefixes: &[&str]| prefixes.iter().any(|prefix| leaf.starts_with(prefix));
            if !hit(&excluded) && !hit(&consumed) && !hit(&planned) {
                unknown.push(leaf.clone());
            }
        }
        assert!(unknown.is_empty(), "未登记的 state 字段：{unknown:?}");
        // 抽核几个必须逐字段消费的路径，防止前缀被过度放宽。
        for path in ["entities[].runtime.hp", "template.clone_build", "world.round_pos"] {
            assert!(leaves.iter().any(|leaf| leaf.contains(path)), "state 应包含 {path}");
        }
    }

    fn flatten(value: &Value, prefix: String, out: &mut BTreeSet<String>) {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    flatten(child, format!("{prefix}.{key}"), out);
                }
            }
            Value::Array(items) => {
                for item in items {
                    flatten(item, format!("{prefix}[]"), out);
                }
            }
            _ => {
                out.insert(prefix.trim_start_matches('.').to_owned());
            }
        }
    }
}

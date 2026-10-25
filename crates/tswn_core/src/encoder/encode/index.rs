//! 样本内实体、模板、队伍与 lane 引用的稠密重映射。
//!
//! 索引借用输入模板，不克隆完整模板。模板行先按实体存储顺序排列，再接实体槽蓝图与
//! 全局槽模板；队伍原值升序压缩，玩家键按模板中首次出现分配。索引仅在本次编码内有效，
//! 稀疏实体 ID 与模板内 lane 下标必须经本模块解析，不能直接当张量行号。

use super::*;

/// 局内重映射结果。
///
/// - `entity_rows`：`EntityIdx` → 实体行（死亡实体仍在表内，mask=1）；
/// - `template_rows`：模板表行 → 模板；前 `e` 行是实体模板，其后是槽内蓝图与全局槽模板，
///   与 [`CapacityMeasure`] 的 `h` 计费口径一致；
/// - `runtime_teams`：runtime team 原值 → 关系行（原值升序，跨 entities/templates/world 统一）；
/// - `player_keys`：`PlrId` → 局内相等关系键（模板表行序首次出现）。
pub(super) struct SampleIndex<'a> {
    pub(super) entity_rows: BTreeMap<u32, usize>,
    pub(super) template_rows: Vec<&'a ModelTemplate>,
    pub(super) template_row_of_entity: Vec<usize>,
    pub(super) runtime_teams: BTreeMap<usize, usize>,
    pub(super) player_keys: BTreeMap<PlrId, usize>,
}

impl SampleIndex<'_> {
    /// Runtime 的 fixed_lane 是所属模板内的数组下标，不是 fixed_lane_key。
    pub(super) fn lane_row(&self, template_row: usize, fixed_lane: usize, path: &str) -> Result<usize, EncodeError> {
        let offset: usize = self
            .template_rows
            .iter()
            .take(template_row)
            .map(|template| template.skills.lanes.len())
            .sum();
        self.template_rows[template_row]
            .skills
            .lanes
            .get(fixed_lane)
            .map(|_| offset + fixed_lane)
            .ok_or_else(|| EncodeError::InvalidReference {
                path: path.to_owned(),
                raw: fixed_lane.to_string(),
            })
    }
}

impl SampleIndex<'_> {
    pub(super) fn entity_row(&self, id: EntityIdx, path: &str) -> Result<usize, EncodeError> {
        self.entity_rows.get(&id.0).copied().ok_or_else(|| EncodeError::InvalidReference {
            path: path.to_owned(),
            raw: id.0.to_string(),
        })
    }
}

impl FeatureEncoder {
    // ---------------------------------------------------------------- 重映射

    pub(super) fn build_index<'a>(&self, state: &'a BattleModelState) -> Result<SampleIndex<'a>, EncodeError> {
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
}

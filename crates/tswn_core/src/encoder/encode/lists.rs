//! 变长关系列表、源顺序与注册序旁路写入。
//!
//! 将技能 lane 顺序、世界名单、实体状态和保护关系等展开为带 owner 与 ordinal 的列表行。
//! 列表位置保持源顺序，注册序与运行序在各自比较域内生成排名，原始 u64 顺序键另存精确旁路。
//! 写入依赖已完成的容量预检和局内索引；悬空 lane 或实体引用仍返回错误，由外层恢复目标批槽位。

use super::*;

#[derive(Debug, Clone, Copy)]
struct ListRecord {
    owner_scope: i32,
    owner: i32,
    field_class: i32,
    ordinal: i32,
    target: i32,
    order_key: Option<u64>,
    rank_domain: Option<OrderRankDomain>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum OrderRankDomain {
    Registration(usize),
    Runtime(usize),
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
        rank_domain: None,
    })
}

impl FeatureEncoder {
    pub(super) fn write_list_family(
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
                for (ordinal, target_lane) in values.into_iter().enumerate() {
                    if target_lane >= template.skills.lanes.len() {
                        return Err(EncodeError::InvalidReference {
                            path: format!("templates[{template_row}].skills.list[{field_class}][{ordinal}]"),
                            raw: target_lane.to_string(),
                        });
                    }
                    let target = lane_starts[template_row] + target_lane;
                    records.push(ListRecord {
                        owner_scope: 3,
                        owner: template_row as i32,
                        field_class,
                        ordinal: ordinal as i32,
                        target: target as i32,
                        order_key: None,
                        rank_domain: None,
                    });
                }
            }
            for (ordinal, deferred) in template.skills.post_action_after_states.iter().enumerate() {
                if deferred.fixed_lane >= template.skills.lanes.len() {
                    return Err(EncodeError::InvalidReference {
                        path: format!("templates[{template_row}].skills.post_action_after_states[{ordinal}].fixed_lane"),
                        raw: deferred.fixed_lane.to_string(),
                    });
                }
                let target = lane_starts[template_row] + deferred.fixed_lane;
                records.push(ListRecord {
                    owner_scope: 3,
                    owner: template_row as i32,
                    field_class: 261,
                    ordinal: ordinal as i32,
                    target: target as i32,
                    order_key: Some(deferred.state_cursor),
                    // 前 e 个模板分别属于实体；其余蓝图没有当前实体注册序比较域。
                    rank_domain: (template_row < state.entities.len()).then_some(OrderRankDomain::Runtime(template_row)),
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
                    rank_domain: None,
                });
            }
            for (ordinal, entry) in entity.states.iter().enumerate() {
                let target = state.entities[..entity_row].iter().map(|e| e.states.len()).sum::<usize>() + ordinal;
                records.push(ListRecord {
                    owner_scope: 2,
                    owner: entity_row as i32,
                    field_class: 270,
                    ordinal: ordinal as i32,
                    target: target as i32,
                    order_key: Some(u64::from(entry.registration_order)),
                    rank_domain: Some(OrderRankDomain::Registration(entity_row)),
                });
                records.push(ListRecord {
                    owner_scope: 2,
                    owner: entity_row as i32,
                    field_class: 271,
                    ordinal: ordinal as i32,
                    target: target as i32,
                    order_key: Some(entry.runtime_registration_order),
                    rank_domain: Some(OrderRankDomain::Runtime(entity_row)),
                });
            }
            records.push(ListRecord {
                owner_scope: 2,
                owner: entity_row as i32,
                field_class: 272,
                ordinal: 0,
                target: entity_row as i32,
                order_key: Some(entity.state_registration_cursor),
                rank_domain: Some(OrderRankDomain::Runtime(entity_row)),
            });
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
            let mut lengths = BTreeMap::<(i32, i32, i32), usize>::new();
            for record in &records {
                *lengths.entry((record.owner_scope, record.owner, record.field_class)).or_default() += 1;
            }
            let positions = out.f32_row_mut("list_position", batch_index)?;
            for (row, record) in records.iter().enumerate() {
                let length = lengths[&(record.owner_scope, record.owner, record.field_class)];
                positions[row] = normalize_position(record.ordinal as usize, length);
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
        {
            let mut distinct = BTreeMap::<OrderRankDomain, BTreeSet<u64>>::new();
            for record in &records {
                if let (Some(domain), Some(key)) = (record.rank_domain, record.order_key) {
                    distinct.entry(domain).or_default().insert(key);
                }
            }
            let mut ranks = Vec::new();
            for (row, record) in records.iter().enumerate() {
                let Some(domain) = record.rank_domain else { continue };
                let Some(key) = record.order_key else { continue };
                let keys = &distinct[&domain];
                let rank = keys.range(..key).count();
                ranks.push((row, rank as f32 / keys.len().saturating_sub(1).max(1) as f32));
            }
            {
                let values = out.f32_row_mut("order_rank", batch_index)?;
                for (row, rank) in &ranks {
                    values[*row] = *rank;
                }
            }
            {
                let present = out.u8_row_mut("order_rank_present", batch_index)?;
                for (row, _) in ranks {
                    present[row] = 1;
                }
            }
        }
        Ok(())
    }
}

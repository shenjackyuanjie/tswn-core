//! global、实体、模板与 lane 表写入。
//!
//! 将已校验状态写入一个批槽位：数值按 manifest 归一化，类别与局内引用分别重映射，
//! 可空字段配套写入 presence。实体表包含死亡实体；输入队伍和 runtime 队伍各用独立 mask，
//! 不以存活数量替代队伍存在性。模板表也包含槽内蓝图，lane 行依次归属于对应模板。

use super::*;

impl FeatureEncoder {
    // ---------------------------------------------------------------- global

    pub(super) fn write_global(
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

    pub(super) fn write_entity_family(
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

    pub(super) fn write_template_family(
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

    pub(super) fn write_lane_family(
        &self,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
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
                // 0 是 PAD，43–50 尚未启用；能转换成 i32 不等于合法技能类别。
                if lane.skill_id == 0 || lane.skill_id as usize > crate::runtime::model_state::MODEL_SKILL_EXPORTS.len() {
                    return Err(EncodeError::UnknownCategory {
                        path: format!("{prefix}.skill_id"),
                        raw: lane.skill_id.to_string(),
                    });
                }
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
}

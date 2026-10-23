//! 槽值、复合字段与原始 bit 精确旁路写入。
//!
//! 槽表按白名单语义分流：蓝图指向模板表，计数进入拟合数值通道，实体引用放入精确关系旁路。
//! 复合字段和原始 bit 以 scope、owner、field、ordinal 标识；u64 拆为低 32 位与高 32 位，
//! 避免通过 f32 往返丢失身份。旁路逐行检查容量，任何中途错误交由编码主路径清空目标槽位。

use super::*;

struct ExtraWriter<'a> {
    encoder: &'a FeatureEncoder,
    batch_index: usize,
    out: &'a mut EncodedBatch,
    row: usize,
}

impl ExtraWriter<'_> {
    fn begin(&mut self, scope: i32, owner: i32, field: i32, ordinal: i32) -> Result<(), EncodeError> {
        if self.row >= self.encoder.profile.x_max {
            return Err(EncodeError::CapacityExceeded {
                path: "extra".to_owned(),
                actual: self.row + 1,
                limit: self.encoder.profile.x_max,
            });
        }
        self.out.u8_row_mut("extra_mask", self.batch_index)?[self.row] = 1;
        self.out.i32_row_mut("extra_index", self.batch_index)?[self.row * 4..self.row * 4 + 4]
            .copy_from_slice(&[scope, owner, field, ordinal]);
        Ok(())
    }
    fn number(&mut self, scope: i32, field: i32, owner: i32, ordinal: i32, value: f32) -> Result<(), EncodeError> {
        self.begin(scope, owner, field, ordinal)?;
        self.out.f32_row_mut("extra_num", self.batch_index)?[self.row] = value;
        self.row += 1;
        Ok(())
    }
    fn reference(&mut self, scope: i32, field: i32, owner: i32, ordinal: i32, value: i32) -> Result<(), EncodeError> {
        self.begin(scope, owner, field, ordinal)?;
        self.out.i32_row_mut("extra_ref", self.batch_index)?[self.row] = value;
        self.row += 1;
        Ok(())
    }
    fn bits(&mut self, scope: i32, field: i32, owner: i32, ordinal: i32, value: u64) -> Result<(), EncodeError> {
        self.begin(scope, owner, field, ordinal)?;
        let bits = self.out.u32_row_mut("extra_bits", self.batch_index)?;
        bits[self.row * 2] = value as u32;
        bits[self.row * 2 + 1] = (value >> 32) as u32;
        self.row += 1;
        Ok(())
    }

    fn raw_u64(&mut self, owner_scope: i32, owner: i32, field: i32, value: u64) -> Result<(), EncodeError> {
        self.bits(owner_scope, field, owner, 0, value)
    }

    fn raw_u32(&mut self, owner_scope: i32, owner: i32, field: i32, value: u32) -> Result<(), EncodeError> {
        self.raw_u64(owner_scope, owner, field, u64::from(value))
    }

    fn raw_entity(&mut self, owner: i32, entity: &crate::runtime::model_state::ModelEntity) -> Result<(), EncodeError> {
        self.raw_u32(2, owner, 4096, entity.runtime.kind.0)?;
        self.raw_u64(2, owner, 4104, entity.runtime.at_boost_bits)?;
        self.raw_u64(2, owner, 4105, entity.runtime.at_boost_millionths as u64)?;
        self.raw_u64(2, owner, 4106, entity.runtime.attract_bits)?;
        self.raw_u64(2, owner, 4107, entity.runtime.flags.0)?;
        self.raw_u64(2, owner, 4108, entity.runtime.accumulate.acc_bits)?;
        self.raw_u64(2, owner, 4109, entity.runtime.accumulate.charge_bonus_bits)?;
        if let Some(hide) = entity.runtime.hide {
            self.raw_u64(2, owner, 4110, hide.attract_bits)?;
        }
        Ok(())
    }

    fn raw_template(&mut self, owner: i32, template: &crate::runtime::model_state::ModelTemplate) -> Result<(), EncodeError> {
        self.raw_u32(3, owner, 4097, template.kind.0)?;
        if let Some(kind) = template.identity.boss_kind {
            self.raw_u32(3, owner, 4098, kind as u32)?;
        }
        self.raw_u64(3, owner, 4111, u64::from(template.reserved_player_ids_before_spawn))?;
        self.raw_u64(3, owner, 4112, template.at_boost_bits)?;
        self.raw_u64(3, owner, 4113, template.at_boost_millionths as u64)?;
        self.raw_u64(3, owner, 4114, template.attract_bits)?;
        if let Some(build) = &template.clone_build {
            self.raw_u64(3, owner, 4115, build.name_factor_bits)?;
            self.raw_u64(3, owner, 4116, build.child_name_factor_bits)?;
            self.raw_u64(3, owner, 4117, build.adjustments.at_boost_delta_bits)?;
            self.raw_u64(3, owner, 4118, build.adjustments.attract_delta_bits)?;
        }
        Ok(())
    }

    fn raw_state(&mut self, owner: usize, entry: &crate::runtime::model_state::ModelStateEntry) -> Result<(), EncodeError> {
        self.raw_u32(4, owner as i32, 4099, entry.legacy_order_key)?;
        if let Some(value) = entry.extension_state_id {
            self.raw_u32(4, owner as i32, 4100, value)?;
        }
        if let Some(poison) = &entry.payload.poison {
            self.raw_u64(4, owner as i32, 4119, poison.atp_bits)?;
        }
        Ok(())
    }

    fn raw_world(&mut self, state: &BattleModelState) -> Result<(), EncodeError> {
        self.raw_u64(1, 0, 4120, state.world.alive_group_count as u64)?;
        self.raw_u64(1, 0, 4121, state.world.round_pos as i64 as u64)
    }
}

impl FeatureEncoder {
    pub(super) fn write_slot_family(
        &self,
        state: &BattleModelState,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        let mut row = 0usize;
        let mut write_slot = |scope: SlotScope,
                              owner: i32,
                              slot: &crate::runtime::model_state::ModelSlot,
                              path: String|
         -> Result<(), EncodeError> {
            let resolved = resolve_slot(scope, slot, &path)?;
            let value_type = match resolved.storage {
                SlotStorage::Bool => 1,
                SlotStorage::I64 => 2,
                SlotStorage::U64 => 3,
                SlotStorage::Template => 4,
            };
            let slot_id = scope.whitelist_id(slot.slot_id).ok_or_else(|| EncodeError::UnknownSlotSemantics {
                path: format!("{path}.slot_id"),
            })?;
            out.u8_row_mut("slot_mask", batch_index)?[row] = 1;
            let index_row = out.i32_row_mut("slot_index", batch_index)?;
            index_row[row * 4..row * 4 + 4].copy_from_slice(&[scope as i32 + 1, owner, slot_id as i32, value_type]);
            out.u8_row_mut("slot_field_present", batch_index)?[row * 4 + value_type as usize - 1] = 1;
            match resolved.semantic {
                SlotSemantic::BlueprintTemplate => {
                    let template = slot
                        .template
                        .as_ref()
                        .ok_or_else(|| EncodeError::InvalidSlotValue { path: path.clone() })?;
                    let template_row = index
                        .template_rows
                        .iter()
                        .position(|candidate| std::ptr::eq(*candidate, template))
                        .ok_or_else(|| EncodeError::InvalidReference {
                            path: format!("{path}.template"),
                            raw: "unknown".to_owned(),
                        })?;
                    out.i32_row_mut("slot_template", batch_index)?[row] = template_row as i32;
                    out.u8_row_mut("slot_template_present", batch_index)?[row] = 1;
                }
                SlotSemantic::Count => {
                    let value = slot.u64_value.ok_or_else(|| EncodeError::InvalidSlotValue { path: path.clone() })?;
                    out.f32_row_mut("slot_value", batch_index)?[row] =
                        self.fitted_value(value as f64, "slot.minion_counter", &path)?;
                    out.u8_row_mut("slot_value_present", batch_index)?[row] = 1;
                }
                SlotSemantic::EntityRef | SlotSemantic::Excluded => {}
            }
            row += 1;
            Ok(())
        };
        for (entity_row, entity) in state.entities.iter().enumerate() {
            for (ordinal, slot) in entity.slots.iter().enumerate() {
                write_slot(
                    SlotScope::Entity,
                    entity_row as i32,
                    slot,
                    format!("entities[{entity_row}].slots[{ordinal}]"),
                )?;
            }
        }
        for (ordinal, slot) in state.template_slots.iter().enumerate() {
            write_slot(SlotScope::Template, 0, slot, format!("template_slots[{ordinal}]"))?;
        }
        for (ordinal, slot) in state.battle_slots.iter().enumerate() {
            write_slot(SlotScope::Battle, 0, slot, format!("battle_slots[{ordinal}]"))?;
        }
        Ok(())
    }

    pub(super) fn write_extra_family(
        &self,
        state: &BattleModelState,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        let mut writer = ExtraWriter {
            encoder: self,
            batch_index,
            out,
            row: 0,
        };
        // extra 的输出次序不决定 owner；owner 必须指向 write_slot_family 的 Q 轴行号。
        let global_slot_start: usize = state.entities.iter().map(|entity| entity.slots.len()).sum();
        for (ordinal, slot) in state.template_slots.iter().chain(state.battle_slots.iter()).enumerate() {
            let slot_row = (global_slot_start + ordinal) as i32;
            writer.raw_u32(5, slot_row, 4101, slot.slot_id)?;
            if let Some(value) = slot.u64_value {
                let scope = if ordinal < state.template_slots.len() {
                    SlotScope::Template
                } else {
                    SlotScope::Battle
                };
                let resolved = resolve_slot(scope, slot, "global.slots")?;
                if !matches!(resolved.semantic, SlotSemantic::EntityRef | SlotSemantic::Excluded) {
                    writer.raw_u64(5, slot_row, 4103, value)?;
                }
            }
        }
        let mut slot_row = 0usize;
        for (entity_row, entity) in state.entities.iter().enumerate() {
            let prefix = format!("entities[{entity_row}]");
            if let Some(assassinate) = entity.runtime.assassinate {
                let lane = index.lane_row(
                    entity_row,
                    assassinate.fixed_lane,
                    &format!("{prefix}.runtime.assassinate.fixed_lane"),
                )?;
                writer.reference(2, 1, entity_row as i32, 0, lane as i32)?;
            }
            for (ordinal, link) in entity.runtime.protect_from.iter().enumerate() {
                writer.number(
                    2,
                    2,
                    entity_row as i32,
                    ordinal as i32,
                    self.fitted_value(
                        link.level as f64,
                        "entity.runtime.protect_from.level",
                        &format!("{prefix}.runtime.protect_from[{ordinal}]."),
                    )?,
                )?;
            }
            for (ordinal, slot) in entity.slots.iter().enumerate() {
                let resolved = resolve_slot(SlotScope::Entity, slot, &format!("{prefix}.slots[{ordinal}]"))?;
                writer.raw_u32(5, slot_row as i32, 4101, slot.slot_id)?;
                if let Some(value) = slot.u64_value {
                    if !matches!(resolved.semantic, SlotSemantic::EntityRef | SlotSemantic::Excluded) {
                        writer.raw_u64(5, slot_row as i32, 4103, value)?;
                    }
                }
                if let Some(raw) = slot.u64_value.filter(|_| resolved.semantic == SlotSemantic::EntityRef) {
                    writer.reference(
                        5,
                        8,
                        slot_row as i32,
                        0,
                        index.entity_row(
                            EntityIdx(u32::try_from(raw).map_err(|_| EncodeError::InvalidReference {
                                path: format!("{prefix}.slots[{ordinal}].u64_value"),
                                raw: raw.to_string(),
                            })?),
                            &format!("{prefix}.slots[{ordinal}].u64_value"),
                        )? as i32,
                    )?;
                }
                slot_row += 1;
            }
        }
        for (template_row, template) in index.template_rows.iter().enumerate() {
            if let Some(build) = &template.clone_build {
                if let Some(plan) = &build.score_skill_boost_plan {
                    writer.bits(3, 3, template_row as i32, 0, plan.initially_boosted_mask)?;
                    for (slot, boost) in plan.slot_boosts.iter().enumerate() {
                        if let Some((a, b)) = boost {
                            writer.number(
                                3,
                                4 + (slot * 2) as i32,
                                template_row as i32,
                                0,
                                self.fitted_value(
                                    f64::from(*a),
                                    [
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.0.0",
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.0.1",
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.1.0",
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.1.1",
                                    ][slot * 2],
                                    "",
                                )?,
                            )?;
                            writer.number(
                                3,
                                5 + (slot * 2) as i32,
                                template_row as i32,
                                0,
                                self.fitted_value(
                                    f64::from(*b),
                                    [
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.0.0",
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.0.1",
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.1.0",
                                        "template.clone_build.score_skill_boost_plan.slot_boosts.1.1",
                                    ][slot * 2 + 1],
                                    "",
                                )?,
                            )?;
                        }
                    }
                }
            }
            writer.raw_template(template_row as i32, template)?;
        }
        let mut state_row = 0usize;
        for (entity_row, entity) in state.entities.iter().enumerate() {
            writer.raw_entity(entity_row as i32, entity)?;
            for entry in &entity.states {
                writer.raw_state(state_row, entry)?;
                state_row += 1;
            }
        }
        writer.raw_world(state)?;
        Ok(())
    }
}

//! 状态表、presence 与载荷的数值／引用映射。
//!
//! 按实体及其状态的源顺序生成行；状态类别使用冻结词表，hook mask 展开为 64 个 bit。
//! 载荷 kind 决定数值、实体引用与队伍引用的固定槽序，各分支独立写 presence，
//! 缺失引用保留 -1。该层依赖先前的 kind/分支校验，未知类别、非有限值或悬空引用仍会拒绝写入。

use super::*;

impl FeatureEncoder {
    pub(super) fn write_state_family(
        &self,
        state: &BattleModelState,
        index: &SampleIndex<'_>,
        batch_index: usize,
        out: &mut EncodedBatch,
    ) -> Result<(), EncodeError> {
        let mut row = 0usize;
        for (entity_row, entity) in state.entities.iter().enumerate() {
            for (ordinal, entry) in entity.states.iter().enumerate() {
                let prefix = format!("entities[{entity_row}].states[{ordinal}]");
                let kind = state_kind_id(entry.payload.kind.as_str()).ok_or_else(|| EncodeError::UnsupportedPayloadKind {
                    path: format!("{prefix}.payload.kind"),
                    kind: entry.payload.kind.clone(),
                })?;
                out.u8_row_mut("state_mask", batch_index)?[row] = 1;
                out.i32_row_mut("state_entity", batch_index)?[row] = entity_row as i32;
                out.i32_row_mut("state_kind", batch_index)?[row] = kind;
                {
                    let cats = out.i32_row_mut("state_cat", batch_index)?;
                    cats[row * 2] = self.dense_id(
                        "state.legacy_order_key",
                        entry.legacy_order_key,
                        &format!("{prefix}.legacy_order_key"),
                    )?;
                    if let Some(extension) = entry.extension_state_id {
                        cats[row * 2 + 1] =
                            self.dense_id("state.extension_state_id", extension, &format!("{prefix}.extension_state_id"))?;
                    }
                }
                {
                    let presence = out.u8_row_mut("state_cat_present", batch_index)?;
                    presence[row * 2] = 1;
                    presence[row * 2 + 1] = u8::from(entry.extension_state_id.is_some());
                }
                {
                    let hooks = out.u8_row_mut("state_hook", batch_index)?;
                    for bit in 0..64 {
                        hooks[row * 64 + bit] = ((entry.hook_mask >> bit) & 1) as u8;
                    }
                }
                out.f32_row_mut("state_num", batch_index)?[row * 9] =
                    self.fitted_value(entry.priority as f64, "state.priority", &format!("{prefix}."))?;
                out.u8_row_mut("state_num_present", batch_index)?[row * 9] = 1;
                let (values, value_presence, refs, groups) = state_payload_values(&entry.payload, &prefix)?;
                let payload_paths = state_payload_paths(entry.payload.kind.as_str());
                let mut payload_nums = [0.0f32; 8];
                for slot in 0..8 {
                    if value_presence[slot] {
                        payload_nums[slot] = self.fitted_value(values[slot], payload_paths[slot], &format!("{prefix}."))?;
                    }
                }
                out.f32_row_mut("state_num", batch_index)?[row * 9 + 1..row * 9 + 9].copy_from_slice(&payload_nums);
                out.u8_row_mut("state_num_present", batch_index)?[row * 9 + 1..row * 9 + 9]
                    .copy_from_slice(&value_presence.map(u8::from));
                let mut payload_refs = [-1i32; 4];
                let mut refs_present = [0u8; 4];
                for slot in 0..4 {
                    if let Some(id) = refs[slot] {
                        payload_refs[slot] = index.entity_row(id, &format!("{prefix}.payload.reference"))? as i32;
                        refs_present[slot] = 1;
                    }
                }
                out.i32_row_mut("state_ref", batch_index)?[row * 4..row * 4 + 4].copy_from_slice(&payload_refs);
                out.u8_row_mut("state_ref_present", batch_index)?[row * 4..row * 4 + 4].copy_from_slice(&refs_present);
                let mut payload_groups = [-1i32; 3];
                let mut groups_present = [0u8; 3];
                for slot in 0..3 {
                    if let Some(team) = groups[slot] {
                        let dense = index.runtime_teams.get(&team).copied().ok_or_else(|| EncodeError::InvalidReference {
                            path: format!("{prefix}.payload.team"),
                            raw: team.to_string(),
                        })?;
                        payload_groups[slot] = dense as i32;
                        groups_present[slot] = 1;
                    }
                }
                out.i32_row_mut("state_group", batch_index)?[row * 3..row * 3 + 3].copy_from_slice(&payload_groups);
                out.u8_row_mut("state_group_present", batch_index)?[row * 3..row * 3 + 3].copy_from_slice(&groups_present);
                row += 1;
            }
        }
        Ok(())
    }
}

fn state_kind_id(kind: &str) -> Option<i32> {
    Some(match kind {
        "none" => 1,
        "fire_mag_half_steps" => 2,
        "ice" => 3,
        "shield_value" => 4,
        "curse" => 5,
        "poison" => 6,
        "haste" => 7,
        "berserk" => 8,
        "charm" => 9,
        "slow" => 10,
        "iron" => 11,
        _ => return None,
    })
}

fn state_payload_paths(kind: &str) -> [&'static str; 8] {
    let mut paths = ["state.payload.unused"; 8];
    match kind {
        "fire_mag_half_steps" => paths[0] = "state.payload.fire_mag_half_steps",
        "ice" => paths[0] = "state.payload.ice.frozen_step",
        "shield_value" => paths[0] = "state.payload.shield_value",
        "curse" => {
            paths[0] = "state.payload.curse.prob";
            paths[1] = "state.payload.curse.multiply";
        }
        "poison" => {
            paths[0] = "state.payload.poison.atp";
            paths[1] = "state.payload.poison.count";
        }
        "haste" => {
            paths[0] = "state.payload.haste.faster";
            paths[1] = "state.payload.haste.effective_faster";
            paths[2] = "state.payload.haste.step";
        }
        "berserk" => paths[0] = "state.payload.berserk.step",
        "charm" => paths[0] = "state.payload.charm.step",
        "slow" => paths[0] = "state.payload.slow.step",
        "iron" => {
            paths[0] = "state.payload.iron.protect";
            paths[1] = "state.payload.iron.step";
        }
        "none" => {}
        _ => {}
    }
    paths
}

type PayloadValues = ([f64; 8], [bool; 8], [Option<EntityIdx>; 4], [Option<usize>; 3]);

fn state_payload_values(payload: &ModelPayload, prefix: &str) -> Result<PayloadValues, EncodeError> {
    let mut values = [0.0; 8];
    let mut presence = [false; 8];
    let mut refs = [None; 4];
    let mut groups = [None; 3];
    let mut set = |slot: usize, value: f64| {
        values[slot] = value;
        presence[slot] = true;
    };
    match payload.kind.as_str() {
        "none" => {}
        "fire_mag_half_steps" => set(0, f64::from(payload.fire_mag_half_steps.unwrap())),
        "ice" => set(0, f64::from(payload.ice.as_ref().unwrap().frozen_step)),
        "shield_value" => set(0, f64::from(payload.shield_value.unwrap())),
        "curse" => {
            let p = payload.curse.as_ref().unwrap();
            set(0, f64::from(p.prob));
            set(1, f64::from(p.multiply));
        }
        "poison" => {
            let p = payload.poison.as_ref().unwrap();
            refs[0] = p.caster.map(EntityIdx);
            refs[1] = p.target.map(EntityIdx);
            set(0, f64::from_bits(p.atp_bits));
            set(1, f64::from(p.count));
        }
        "haste" => {
            let p = payload.haste.as_ref().unwrap();
            set(0, f64::from(p.faster));
            set(1, f64::from(p.effective_faster));
            set(2, f64::from(p.step));
        }
        "berserk" => set(0, f64::from(payload.berserk.as_ref().unwrap().step)),
        "charm" => {
            let p = payload.charm.as_ref().unwrap();
            refs[0] = p.target.map(EntityIdx);
            let group = u32::try_from(p.group_id).map_err(|_| EncodeError::InvalidReference {
                path: format!("{prefix}.payload.charm.group_id"),
                raw: p.group_id.to_string(),
            })?;
            refs[1] = Some(EntityIdx(group));
            groups[0] = p.effective_team_idx;
            groups[1] = p.source_team_idx;
            set(0, f64::from(p.step));
        }
        "slow" => set(0, f64::from(payload.slow.as_ref().unwrap().step)),
        "iron" => {
            let p = payload.iron.as_ref().unwrap();
            set(0, f64::from(p.protect));
            set(1, f64::from(p.step));
        }
        _ => {
            return Err(EncodeError::UnsupportedPayloadKind {
                path: format!("{prefix}.payload.kind"),
                kind: payload.kind.clone(),
            });
        }
    }
    Ok((values, presence, refs, groups))
}

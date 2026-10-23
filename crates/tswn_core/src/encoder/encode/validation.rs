//! 状态支持域、引用、载荷与容量校验。
//!
//! 在任何张量字段写入前检查状态 schema、未决条件、实体唯一性、引用归属与槽白名单，
//! 并返回同一次九维容量计量供后续写入核对。这里不替代各张量写入层对类别与数值的检查；
//! 校验失败同样由编码主入口处理目标槽位恢复，不改变输入状态。

use super::*;

/// 容量维度名 → 错误路径名（供 `CapacityExceeded` 定位）。
pub(super) fn dim_path(dim: &str) -> String {
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

impl FeatureEncoder {
    // ---------------------------------------------------------------- 校验

    /// 状态校验：schema、终局门禁、输入队伍、实体唯一性、全部引用域、压缩标志保留位、
    /// 载荷 kind 与分支匹配、槽语义白名单、容量预检。
    ///
    /// 注意 [`BattleModelState::validate`] 不是替代品：它未覆盖 charm `group_id` 实体引用、
    /// 槽内 U64 实体引用与载荷分支一致性（规格第 16 节第 7 条）。
    /// 成功返回本次容量计量，供写入结束后核对精确旁路行数，避免再次遍历输入。
    pub(super) fn validate_state(&self, state: &BattleModelState) -> Result<CapacityMeasure, EncodeError> {
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
        Ok(measure)
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

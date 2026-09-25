//! 机制槽语义白名单与槽校验（规格第 3.2 节）。
//!
//! 默认注册表只登记 7 个实体槽、3 个全局模板槽和 0 个 battle 槽，而同一个 `U64` 存储槽
//! 同时表示**实体引用、计数和浮点 bit** 三种语义，因此不能按 `SlotValue` 的存储类型推断用途。
//! 本模块把白名单冻结成数据表：encoder 只接受该表内的槽，未登记、未知 `export_name` 或
//! 存储类型不符一律返回 [`EncodeError::UnknownSlotSemantics`]，不猜测、不按存储类型冒充。
//!
//! 张量写入（`slot_index`/`slot_value`/`slot_template` 与 X 记录）属于 handoff 分块计划的
//! 后续块；本模块先落地**校验部分**（槽 ID、值分支、实体引用），闭合规格第 16 节第 7 条
//! 指出的“`validate` 未覆盖槽内 U64 实体引用”缺口。

use crate::encoder::error::EncodeError;
use crate::runtime::model_state::ModelSlot;

/// 槽值的存储类型；与 `ModelSlot::project` 的四个分支一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotStorage {
    Bool,
    I64,
    U64,
    Template,
}

/// 槽的机制语义；决定该槽的值进入哪条通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotSemantic {
    /// 蓝图模板缓存；值进入同一模板表。
    BlueprintTemplate,
    /// 被排除的配置量：整份数据同一常量，且存在性已被 `template_bool[4]` 覆盖。
    Excluded,
    /// “记住的召唤物”实体引用；禁止当数值。
    EntityRef,
    /// 召唤／分身命名计数器，单调递增；走 `N_f`。
    Count,
}

/// 白名单中的一项槽登记。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotWhitelistEntry {
    pub slot_id: u32,
    pub export_name: &'static str,
    pub storage: SlotStorage,
    pub semantic: SlotSemantic,
}

/// entity 作用域的 7 个槽（`core.entity.*` 与 `custom.bed2.*`）。
pub const ENTITY_SLOT_WHITELIST: &[SlotWhitelistEntry] = &[
    SlotWhitelistEntry {
        slot_id: 0,
        export_name: "core.entity.shadow_blueprint",
        storage: SlotStorage::Template,
        semantic: SlotSemantic::BlueprintTemplate,
    },
    SlotWhitelistEntry {
        slot_id: 1,
        export_name: "core.entity.summon_blueprint",
        storage: SlotStorage::Template,
        semantic: SlotSemantic::BlueprintTemplate,
    },
    SlotWhitelistEntry {
        slot_id: 2,
        export_name: "core.entity.zombie_blueprint",
        storage: SlotStorage::Template,
        semantic: SlotSemantic::BlueprintTemplate,
    },
    SlotWhitelistEntry {
        slot_id: 3,
        export_name: "core.entity.lazy_blueprint_rq",
        storage: SlotStorage::U64,
        semantic: SlotSemantic::Excluded,
    },
    SlotWhitelistEntry {
        slot_id: 4,
        export_name: "core.entity.summoned_entity",
        storage: SlotStorage::U64,
        semantic: SlotSemantic::EntityRef,
    },
    SlotWhitelistEntry {
        slot_id: 5,
        export_name: "core.entity.minion_counter",
        storage: SlotStorage::U64,
        semantic: SlotSemantic::Count,
    },
    SlotWhitelistEntry {
        slot_id: 6,
        export_name: "custom.bed2.summoned_entity",
        storage: SlotStorage::U64,
        semantic: SlotSemantic::EntityRef,
    },
];

/// template 作用域的 3 个全局模板槽。
pub const TEMPLATE_SLOT_WHITELIST: &[SlotWhitelistEntry] = &[
    SlotWhitelistEntry {
        slot_id: 0,
        export_name: "custom.bed2.summon_template",
        storage: SlotStorage::Template,
        semantic: SlotSemantic::BlueprintTemplate,
    },
    SlotWhitelistEntry {
        slot_id: 1,
        export_name: "custom.bed2.shadow_template",
        storage: SlotStorage::Template,
        semantic: SlotSemantic::BlueprintTemplate,
    },
    SlotWhitelistEntry {
        slot_id: 2,
        export_name: "custom.bed2.zombie_template",
        storage: SlotStorage::Template,
        semantic: SlotSemantic::BlueprintTemplate,
    },
];

/// battle 作用域没有登记任何槽；出现即拒绝。
pub const BATTLE_SLOT_WHITELIST: &[SlotWhitelistEntry] = &[];

/// 槽的作用域；与规格第 14 节 `slot_index` 的 `scope` 取值一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotScope {
    Entity,
    Template,
    Battle,
}

impl SlotScope {
    pub fn name(self) -> &'static str {
        match self {
            Self::Entity => "entity",
            Self::Template => "template",
            Self::Battle => "battle",
        }
    }

    fn whitelist(self) -> &'static [SlotWhitelistEntry] {
        match self {
            Self::Entity => ENTITY_SLOT_WHITELIST,
            Self::Template => TEMPLATE_SLOT_WHITELIST,
            Self::Battle => BATTLE_SLOT_WHITELIST,
        }
    }
}

/// 一个真实槽行的语义判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedSlot {
    pub storage: SlotStorage,
    pub semantic: SlotSemantic,
}

/// 解析一个槽：查白名单、核对值分支唯一性、核对存储类型。
///
/// `prefix` 是错误路径前缀（如 `entities[3].slots[5]`）。
pub fn resolve_slot(scope: SlotScope, slot: &ModelSlot, prefix: &str) -> Result<ResolvedSlot, EncodeError> {
    let path = format!("{prefix}.slot_id");
    let entry = scope
        .whitelist()
        .iter()
        .find(|candidate| candidate.slot_id == slot.slot_id)
        .ok_or_else(|| EncodeError::UnknownSlotSemantics {
            path: format!("{path}（{} 作用域未登记）", scope.name()),
        })?;
    let branches = [
        (SlotStorage::Bool, slot.bool_value.is_some()),
        (SlotStorage::I64, slot.i64_value.is_some()),
        (SlotStorage::U64, slot.u64_value.is_some()),
        (SlotStorage::Template, slot.template.is_some()),
    ];
    let present: Vec<_> = branches.iter().filter(|(_, some)| *some).collect();
    match present.as_slice() {
        [] => {
            return Err(EncodeError::InvalidSlotValue {
                path: format!("{prefix}（四个值分支全为 None）"),
            });
        }
        [single] => {
            if single.0 != entry.storage {
                return Err(EncodeError::UnknownSlotSemantics {
                    path: format!("{path}（{} 的存储类型与白名单不符）", entry.export_name),
                });
            }
        }
        _ => {
            return Err(EncodeError::InvalidSlotValue {
                path: format!("{prefix}（多个值分支同时为 Some）"),
            });
        }
    }
    Ok(ResolvedSlot {
        storage: entry.storage,
        semantic: entry.semantic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(slot_id: u32, u64_value: Option<u64>) -> ModelSlot {
        ModelSlot {
            slot_id,
            bool_value: None,
            i64_value: None,
            u64_value,
            template: None,
        }
    }

    #[test]
    fn whitelist_covers_default_registry_counts() {
        // 默认注册表：7 个实体槽、3 个全局模板槽、0 个 battle 槽（J:265–277）。
        assert_eq!(ENTITY_SLOT_WHITELIST.len(), 7);
        assert_eq!(TEMPLATE_SLOT_WHITELIST.len(), 3);
        assert!(BATTLE_SLOT_WHITELIST.is_empty());
    }

    #[test]
    fn unregistered_or_mistyped_slot_is_rejected() {
        assert!(matches!(
            resolve_slot(SlotScope::Entity, &slot(7, Some(1)), "entities[0].slots[0]").unwrap_err(),
            EncodeError::UnknownSlotSemantics { .. }
        ));
        // minion_counter 必须是 U64；换成 bool_value 即类型不符。
        let mut mistyped = slot(5, None);
        mistyped.bool_value = Some(true);
        assert!(matches!(
            resolve_slot(SlotScope::Entity, &mistyped, "entities[0].slots[0]").unwrap_err(),
            EncodeError::UnknownSlotSemantics { .. }
        ));
        // battle 作用域没有任何登记槽。
        assert!(matches!(
            resolve_slot(SlotScope::Battle, &slot(0, Some(1)), "battle_slots[0]").unwrap_err(),
            EncodeError::UnknownSlotSemantics { .. }
        ));
    }

    #[test]
    fn value_branch_must_be_exactly_one() {
        assert!(matches!(
            resolve_slot(SlotScope::Entity, &slot(5, None), "entities[0].slots[0]").unwrap_err(),
            EncodeError::InvalidSlotValue { .. }
        ));
        let mut both = slot(5, Some(3));
        both.i64_value = Some(3);
        assert!(matches!(
            resolve_slot(SlotScope::Entity, &both, "entities[0].slots[0]").unwrap_err(),
            EncodeError::InvalidSlotValue { .. }
        ));
    }

    #[test]
    fn whitelist_semantics_are_frozen_per_slot() {
        let semantic = |slot_id: u32| {
            ENTITY_SLOT_WHITELIST
                .iter()
                .find(|entry| entry.slot_id == slot_id)
                .map(|entry| entry.semantic)
                .unwrap()
        };
        // 三个蓝图槽、一个排除项、两个实体引用（bed2 恒缺失）、一个计数。
        assert_eq!(semantic(0), SlotSemantic::BlueprintTemplate);
        assert_eq!(semantic(3), SlotSemantic::Excluded);
        assert_eq!(semantic(4), SlotSemantic::EntityRef);
        assert_eq!(semantic(6), SlotSemantic::EntityRef);
        assert_eq!(semantic(5), SlotSemantic::Count);
    }
}

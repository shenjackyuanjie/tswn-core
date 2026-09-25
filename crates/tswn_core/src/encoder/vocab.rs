//! 分类词表：原始 u32 → 局内稠密 ID。
//!
//! 规格第 4 节：分类通道只存**按冻结词表重映射后的稠密 ID**（0=PAD，有效类从 1 起），
//! 原始值只进精确旁路。同一份 manifest 在所有样本、所有消费端之间冻结同一张表，
//! 因此 manifest 必须显式登记词表，实现侧再从默认注册表/固定序列派生并交叉核对，
//! 不允许按“该局出现了什么”临时编号，也不允许把未知原值折叠为 0。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::encoder::error::EncodeError;
use crate::namerena::BOSS_NAMES;
use crate::runtime::extension::ExtensionRegistry;

/// 冻结的分类词表；`entries` 的稠密 ID 必须恰好覆盖 `1..=K`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vocabulary {
    pub entries: BTreeMap<u32, i32>,
}

impl Vocabulary {
    /// 构造并自校验：ID 唯一、从 1 起且无空洞。
    pub fn new(entries: BTreeMap<u32, i32>) -> Result<Self, EncodeError> {
        let vocabulary = Self { entries };
        vocabulary.validate()?;
        Ok(vocabulary)
    }

    /// 自校验；`path` 是 manifest 中该词表的键名（如 `runtime.kind`）。
    pub fn validate(&self) -> Result<(), EncodeError> {
        let count = self.entries.len();
        let mut seen = vec![false; count];
        for id in self.entries.values() {
            if *id < 1 || *id as usize > count {
                return Err(EncodeError::ManifestMismatch {
                    path: String::new(),
                    detail: format!("稠密 ID {id} 超出 1..={count}"),
                });
            }
            let slot = &mut seen[*id as usize - 1];
            if *slot {
                return Err(EncodeError::ManifestMismatch {
                    path: String::new(),
                    detail: format!("稠密 ID {id} 重复"),
                });
            }
            *slot = true;
        }
        if seen.iter().any(|used| !used) {
            return Err(EncodeError::ManifestMismatch {
                path: String::new(),
                detail: "稠密 ID 存在空洞".to_owned(),
            });
        }
        Ok(())
    }

    pub fn len(&self) -> usize { self.entries.len() }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    /// 查表；未知原值报 [`EncodeError::UnknownCategory`]，不折叠为 PAD。
    pub fn dense_id(&self, raw: u32, path: &str) -> Result<i32, EncodeError> {
        self.entries
            .get(&raw)
            .copied()
            .ok_or_else(|| EncodeError::UnknownCategory { path: path.to_owned(), raw: raw.to_string() })
    }
}

/// `runtime.kind` / `template.kind` 词表：默认规则的 player-kind 注册表加实测有效哨兵
/// `u32::MAX`（规格第 4 节“原始 0 若为注册类就映射到正 ID，`u32::MAX` 必须映射到正 ID”）。
pub fn player_kind_vocabulary(registry: &ExtensionRegistry) -> Vocabulary {
    let mut raws: Vec<u32> = registry.player_kinds().iter().map(|spec| spec.id.0).collect();
    raws.push(u32::MAX);
    raws.sort_unstable();
    raws.dedup();
    let entries = raws.into_iter().enumerate().map(|(index, raw)| (raw, index as i32 + 1)).collect();
    Vocabulary::new(entries).expect("注册表派生的 player-kind 词表必须自洽")
}

/// `identity.boss_kind` 词表：按 `BOSS_NAMES` 的位置投影，原始位置 `k` → 类 `k+1`。
pub fn boss_kind_vocabulary() -> Vocabulary {
    let entries = (0..BOSS_NAMES.len() as u32).map(|index| (index, index as i32 + 1)).collect();
    Vocabulary::new(entries).expect("BOSS_NAMES 位置词表必须自洽")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary(pairs: &[(u32, i32)]) -> Vocabulary {
        Vocabulary::new(pairs.iter().copied().collect()).unwrap()
    }

    #[test]
    fn dense_ids_must_start_at_one_without_holes() {
        assert!(Vocabulary::new([(0, 0)].into_iter().collect()).is_err());
        assert!(Vocabulary::new([(0, 1), (5, 1)].into_iter().collect()).is_err());
        assert!(Vocabulary::new([(0, 1), (5, 3)].into_iter().collect()).is_err());
        assert_eq!(vocabulary(&[(0, 1), (5, 2)]).len(), 2);
    }

    #[test]
    fn unknown_raw_is_rejected_rather_than_padded() {
        let vocabulary = vocabulary(&[(0, 1), (u32::MAX, 2)]);
        assert_eq!(vocabulary.dense_id(u32::MAX, "runtime.kind").unwrap(), 2);
        let error = vocabulary.dense_id(7, "runtime.kind").unwrap_err();
        assert!(matches!(error, EncodeError::UnknownCategory { .. }), "{error}");
    }

    #[test]
    fn player_kind_vocabulary_covers_registry_and_sentinel() {
        let config = crate::runtime::default_custom_runtime_import_config().unwrap();
        let vocabulary = player_kind_vocabulary(&config.registry);
        let kinds = config.registry.player_kinds().len() as i32;
        assert_eq!(vocabulary.len() as i32, kinds + 1);
        // 每个注册类都映射到正 ID，哨兵映射到最后一类。
        for (index, spec) in config.registry.player_kinds().iter().enumerate() {
            assert_eq!(vocabulary.dense_id(spec.id.0, "runtime.kind").unwrap(), index as i32 + 1);
        }
        assert_eq!(vocabulary.dense_id(u32::MAX, "runtime.kind").unwrap(), kinds + 1);
    }

    #[test]
    fn boss_kind_vocabulary_maps_positions_from_one() {
        let vocabulary = boss_kind_vocabulary();
        assert_eq!(vocabulary.len(), BOSS_NAMES.len());
        assert_eq!(vocabulary.dense_id(0, "template.identity.boss_kind").unwrap(), 1);
        assert!(vocabulary.dense_id(BOSS_NAMES.len() as u32, "x").is_err());
    }
}

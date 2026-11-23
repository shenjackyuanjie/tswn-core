//! 科研 Boss 的可克隆运行态；计时全部采用本体实际行动次数。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SquallRuntime {
    pub samples: u8,
    /// 上一次采样台词，用于避免连续重复，不参与战斗数值。
    pub last_sample_quote: Option<u8>,
    pub turn: u32,
    pub observed: bool,
    pub marked: Option<EntityIdx>,
    pub damage: Vec<(EntityIdx, i64)>,
    pub next_skill: [u32; 6],
    pub active_count: u8,
    pub summoned: bool,
    pub heal_count: u8,
    pub iron_until: u32,
    pub amplify: u16,
    pub exposed: bool,
    pub burned: bool,
    /// 是否已进入红温；整场持续，不占普通状态槽，净化无法解除。
    #[serde(default)]
    pub red: bool,
    pub pruning: Vec<EntityIdx>,
    pub prune_at: u32,
    pub taunt: Option<(EntityIdx, EntityIdx, u32)>,
}

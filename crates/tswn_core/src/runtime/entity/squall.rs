//! 科研 Boss 可克隆状态；出勤和呼号器按本体真正行动计时。
use super::EntityIdx;
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SquallRuntime {
    pub samples: u8,
    pub last_sample_quote: Option<u8>,
    pub turn: u32,
    pub marked: Option<EntityIdx>,
    pub damage: Vec<(EntityIdx, i64)>,
    pub summoned: bool,
    pub red: bool,
    pub attendance: bool,
    pub force_red_at: Option<u32>,
}

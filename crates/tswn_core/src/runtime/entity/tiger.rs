//! TigerStar Boss 的运行态：回合、我是M、红温与拜谢名单。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TigerRuntime {
    /// 本体实际行动次数。
    pub turn: u32,
    /// 「我是M」剩余回合。
    pub m_turns: u32,
    /// 是否已进入红温。
    pub red: bool,
    /// 被本体拜谢过的目标。
    pub thanks: Vec<EntityIdx>,
    /// 冗余记录拥有者。
    pub owner: Option<EntityIdx>,
}

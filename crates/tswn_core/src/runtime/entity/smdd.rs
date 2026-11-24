//! smdd Boss 的做局层数与 rp 层数。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SmddRuntime {
    /// 本体实际行动次数。
    pub turn: u32,
    /// 「局」层数：开始做局始终激活，每回合与被命中都会累积。
    pub stacks: u8,
    /// “我 rp 神”的叠加层数，最多两层。
    pub rp_stacks: u8,
    /// 冗余记录拥有者，方便派生逻辑溯源。
    pub owner: Option<EntityIdx>,
}

//! smdd Boss 的做局层数、卖牢次数与技能冷却。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SmddRuntime {
    /// 本体实际行动次数。
    pub turn: u32,
    /// 「局」层数：被命中与主动做局时累积。
    pub stacks: u8,
    /// 卖牢次数，每场最多两次。
    pub sold: u8,
    /// 是否已召唤猴儿。
    pub summoned: bool,
    /// 各技能的冷却终点。
    pub next_skill: [u32; 10],
    /// 嘲讽持续到第几次自身行动。
    pub taunt_until: u32,
    /// 冗余记录拥有者，方便派生逻辑溯源。
    pub owner: Option<EntityIdx>,
}

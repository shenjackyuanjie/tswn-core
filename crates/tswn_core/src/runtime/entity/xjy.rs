//! 新纪元 Boss 的行动计数、铁壁与拜谢状态。
//!
//! `turn` 是本体实际行动次数，铁壁与拜谢都以它为期限基准；整个结构随实体克隆与恢复。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct XjyRuntime {
    /// 本体实际行动次数。
    pub turn: u32,
    /// 铁壁持续到第几次自身行动。
    pub iron_until: u32,
    /// 铁壁发动时的行动计数。
    pub iron_turn: u32,
    /// 拜谢目标、解除回合与原本的攻魔。
    pub thanks: Vec<(EntityIdx, u32, i32, i32)>,
    /// 冗余记录拥有者。
    pub owner: Option<EntityIdx>,
}

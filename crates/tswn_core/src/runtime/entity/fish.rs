//! 沙冰鱼 Boss 的行动计数与魔力压制名单。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FishRuntime {
    /// 本体实际行动次数。
    pub turn: u32,
    /// 被魔力压制目标、恢复回合与原魔力。
    pub magic_zero: Vec<(EntityIdx, u32, i32)>,
    /// 冗余记录拥有者。
    pub owner: Option<EntityIdx>,
}

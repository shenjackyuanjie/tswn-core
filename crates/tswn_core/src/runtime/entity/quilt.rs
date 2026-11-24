//! 被子衍生物的计数器与临时标记；随实体一起克隆与恢复。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuiltMinionState {
    /// 鱼钻被次数；累计 3 次后“用进废退”变成猴区。
    pub dives: u8,
    /// 得意层数。
    pub buffs: u8,
    /// 本回合睡5觉/钻被，受击为 0。
    pub sleeping: bool,
    /// 下次行动从被子里钻出一只猴区（鱼钻被的后续）。
    pub pending_spawn: bool,
    /// 鱼已被飞翔猴区逮捕，猴区因此减伤。
    pub flying: bool,
    /// 上一次行动时是否处于冰冻，用于“猴儿冰冻”结算。
    pub was_frozen: bool,
    /// 分裂出来的子体不再触发 HOUQU_BLAST。
    pub no_split: bool,
    /// 记录所属被子，便于衍生物被转让后仍能溯源。
    pub owner: Option<EntityIdx>,
}

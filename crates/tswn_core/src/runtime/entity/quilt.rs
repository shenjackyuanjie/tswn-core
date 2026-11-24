//! 被子衍生物的计数器与临时标记；随实体一起克隆与恢复。
use super::super::EntityIdx;

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct QuiltMinionState {
    /// 鱼钻被次数；累计 3 次后“用进废退”变成猴区。
    pub dives: u8,
    /// 每个实体独立的行动计数与得意到期回合。
    pub turn: u32,
    pub pride: Vec<u32>,
    /// 本体是否已经完成一次占领。
    pub occupied: bool,
    /// 本回合睡5觉/钻被，受击为 0。
    pub sleeping: bool,
    /// 下次行动从被子里钻出一只猴区（鱼钻被的后续）。
    pub pending_spawn: bool,
    /// 鱼已被飞翔猴区逮捕，猴区因此减伤。
    pub flying: bool,
    /// 解冻的全局调度轮，用来区分解冻当轮与下次行动。
    pub thawed_at: Option<u64>,
    /// 分裂出来的子体不再触发 HOUQU_BLAST。
    pub no_split: bool,
    /// 记录所属被子，便于衍生物被转让后仍能溯源。
    pub owner: Option<EntityIdx>,
}

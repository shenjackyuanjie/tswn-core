//! 一场战斗内的临时行动标记，与实体、技能和持久槽数据分开保存。
//!
//! 当前只记录最近一次选中行动者时的 Runtime 回合游标，不表示可见帧号。
//! prepared runner 复位时调用 [`BattleScratch::clear`]，不把上一局游标带入下一局。

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BattleScratch {
    pub selected_actor_round: u64,
}

impl BattleScratch {
    pub fn clear(&mut self) { self.selected_actor_round = 0; }
}

//! Rosmontic 专属状态随战斗快照复制，新战斗由默认值重新初始化。
use super::EntityIdx;
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RosmonticRuntime {
    pub actions: u32,
    pub rumia_moons: u32,
    /// 保存月影实际增量，区分变身前后获得的增防。
    pub rumia_def_bonus: i32,
    pub rumia_destroy: bool,
    pub charge_target: Option<EntityIdx>,
    pub resolve_used: bool,
    /// 以蓄力开始时的 HP 决定是否挂受伤打断钩子。
    pub break_on_damage: bool,
}

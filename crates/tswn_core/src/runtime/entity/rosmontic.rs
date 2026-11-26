//! Rosmontic 专属状态随战斗快照复制，新战斗由默认值重新初始化。
use super::EntityIdx;
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct RosmonticRuntime {
    pub actions: u32,
    /// 专属技能按参考槽号保存等级，动态降级和一次性技能随快照复制。
    pub levels: Vec<i32>,
    pub smart: bool,
    pub dream: u8,
    pub barrier_turns: u8,
    pub energy: u8,
    pub charged_skill: Option<usize>,
    pub child: Option<EntityIdx>,
    /// 1 为妖精、2 为逆召唤帕秋莉、3 为咲夜残影。
    pub child_kind: u8,
    pub elements: u8,
    pub time_stop: u8,
    pub scarlet: u32,
    pub aura: bool,
    pub aura_bonus: bool,
    pub qi_bits: u64,
    /// 属性变化独立保留，状态刷新时不丢失偷取、五行和妖精的复活增益。
    pub stat_add: [i32; 7],
    pub stat_scale_bits: [u64; 7],
    /// 燃烧存储未结算的威力；普通角色也可以仅携带这一状态而不接管行动。
    pub burn_owner: Option<EntityIdx>,
    pub burn_atp_bits: u64,
    pub burn_turns: u8,
    pub rumia_moons: u32,
    /// 保存月影实际增量，区分变身前后获得的增防。
    pub rumia_def_bonus: i32,
    pub rumia_destroy: bool,
    pub charge_target: Option<EntityIdx>,
    pub resolve_used: bool,
    /// 以蓄力开始时的 HP 决定是否挂受伤打断钩子。
    pub break_on_damage: bool,
}

impl RosmonticRuntime {
    /// 固定槽位与参考实现一致，动态熟练度随快照保存。
    pub fn for_boss(name: &str) -> Self {
        let levels: &[i32] = match name {
            "reimu" => &[0, 20, 45, 65, 100, 100],
            "marisa" => &[1000, 33, 54, 65, 150],
            "daiyousei" => &[100, 80, 70, 60, 80],
            "hong" => &[0, 100, 50, 65, 150, 200],
            "koakuma" => &[0, 200, 50, 56, 100],
            "patchouli" => &[35, 35, 45, 45, 60, 60, 67, 67, 120, 120, 300],
            "sakuya" => &[50, 35, 45, 126, 100],
            "remilia" => &[0, 70, 50, 70, 300],
            _ => &[],
        };
        Self {
            levels: levels.to_vec(),
            ..Self::default()
        }
    }
}

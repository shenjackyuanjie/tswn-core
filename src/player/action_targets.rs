//! # 行动目标集合 (action_targets)
//!
//! 本模块定义 [`ActionTargets`]，记录玩家在执行一次行动时可选目标的分类列表。
//!
//! ## 字段说明
//!
//! | 字段            | 内容                                 |
//! |----------------|--------------------------------------|
//! | `enemy_alive`  | 敌方存活玩家                         |
//! | `ally_alive`   | 同队存活玩家（含自身）                |
//! | `ally_all`     | 同队全部玩家（含已死亡，供复活类技能用）|
//! | `ally_dead`    | 同队已死亡玩家                       |
//! | `all_alive`    | 全场存活玩家（可能跨队伍）              |
//!
//! 如果玩家处于魅惑状态（`CharmState`），`tick::select_targets` 会自动将
//! 它的队伍视角反转（将敌方视为友方、友方视为敌方）。

use crate::player::PlrId;

#[derive(Clone, Debug, Default)]
pub struct ActionTargets {
    pub enemy_alive: Vec<PlrId>,
    pub ally_alive: Vec<PlrId>,
    pub ally_all: Vec<PlrId>,
    pub ally_dead: Vec<PlrId>,
    pub all_alive: Vec<PlrId>,
}

impl ActionTargets {
    pub fn from_enemy_alive(enemy_alive: &[PlrId]) -> Self {
        Self {
            enemy_alive: enemy_alive.to_vec(),
            all_alive: enemy_alive.to_vec(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForcedAttackTargetDomain {
    EnemyAlive,
    AllAlive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForcedAttackScoreMode {
    Default,
    RandomAttract,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForcedAttackConfig {
    pub smart: bool,
    pub target_domain: ForcedAttackTargetDomain,
    pub score_mode: ForcedAttackScoreMode,
    pub use_mag: bool,
    pub attack_scale: f64,
    pub message: &'static str,
}

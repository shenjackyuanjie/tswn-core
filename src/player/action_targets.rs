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

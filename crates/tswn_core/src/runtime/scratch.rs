#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct BattleScratch {
    pub selected_actor_round: u64,
}

impl BattleScratch {
    pub fn clear(&mut self) { self.selected_actor_round = 0; }
}

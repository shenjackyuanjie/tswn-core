use crate::player::{PlrId, StateTrait};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MinionKind {
    #[default]
    Clone,
    Summon,
    Shadow,
    Zombie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MinionRuntimeState {
    pub owner: Option<PlrId>,
    pub kind: MinionKind,
}

impl MinionRuntimeState {
    #[inline]
    pub fn is_combat_minion(&self) -> bool { !matches!(self.kind, MinionKind::Clone) }
}

#[inline]
pub fn is_combat_minion(player: &crate::player::Player) -> bool {
    player
        .get_state::<MinionRuntimeState>()
        .map(MinionRuntimeState::is_combat_minion)
        .unwrap_or(false)
}

impl StateTrait for MinionRuntimeState {
    fn meta_type(&self) -> i32 { 0 }

    fn die_message_priority(&self) -> i32 { 100 }

    fn die_message(&self) -> Option<&'static str> { self.is_combat_minion().then_some("[1]消失了") }

    fn linked_owner(&self) -> Option<PlrId> { self.is_combat_minion().then_some(self.owner).flatten() }

    fn on_linked_owner_die(&mut self, owner: PlrId, self_id: PlrId, updates: &mut crate::engine::update::RunUpdates) -> bool {
        if !self.is_combat_minion() {
            return false;
        }
        updates.emit(crate::engine::update::RunUpdate::new_newline);
        updates.emit(|| crate::engine::update::RunUpdate::new("[1]消失了", owner, self_id, 50));
        true
    }



    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

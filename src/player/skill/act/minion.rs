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

impl StateTrait for MinionRuntimeState {
    fn meta_type(&self) -> i32 { 0 }

    fn die_message_priority(&self) -> i32 { 100 }

    fn die_message(&self) -> Option<&'static str> { Some("[1]消失了") }

    fn linked_owner(&self) -> Option<PlrId> { self.owner }

    fn on_linked_owner_die(&mut self, owner: PlrId, self_id: PlrId, updates: &mut crate::engine::update::RunUpdates) -> bool {
        updates.add(crate::engine::update::RunUpdate::new_newline());
        updates.add(crate::engine::update::RunUpdate::new("[1]消失了", owner, self_id, 30));
        true
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

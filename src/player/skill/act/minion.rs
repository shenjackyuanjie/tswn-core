use crate::player::{PlrId, StateTrait};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinionKind {
    Clone,
    Summon,
    Shadow,
    Zombie,
}

impl Default for MinionKind {
    fn default() -> Self { Self::Clone }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MinionRuntimeState {
    pub owner: Option<PlrId>,
    pub kind: MinionKind,
}

impl StateTrait for MinionRuntimeState {
    fn meta_type(&self) -> i32 { 0 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

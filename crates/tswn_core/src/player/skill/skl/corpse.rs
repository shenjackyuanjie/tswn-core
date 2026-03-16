use crate::player::StateTrait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorpseKind {
    Merge,
    Zombie,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorpseState {
    pub kind: CorpseKind,
}

impl CorpseState {
    pub const fn merge() -> Self { Self { kind: CorpseKind::Merge } }

    pub const fn zombie() -> Self {
        Self {
            kind: CorpseKind::Zombie,
        }
    }
}

impl StateTrait for CorpseState {
    fn meta_type(&self) -> i32 { 0 }



    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

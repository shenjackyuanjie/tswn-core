use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct MergeSkill;

impl MergeSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for MergeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for MergeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MergeState {
    pub target: Option<PlrId>,
}


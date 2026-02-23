use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct CharmSkill;

impl CharmSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CharmSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CharmSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CharmState {
    pub group_id: usize,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub step: i32,
}


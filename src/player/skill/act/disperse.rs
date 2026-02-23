use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct DisperseSkill;

impl DisperseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for DisperseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for DisperseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

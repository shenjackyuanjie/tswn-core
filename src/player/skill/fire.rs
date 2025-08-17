use crate::player::{
    PlrPtr,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct FireSkill {}

impl FireSkill {
    pub fn new() -> Self { Self {} }
}

impl SkillExt for FireSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(FireSkill::new()) }
}

impl SkillTrait for FireSkill {
    fn destroy(&self, plr: PlrPtr, args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

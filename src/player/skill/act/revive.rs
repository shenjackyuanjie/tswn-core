use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ReviveSkill {
    pub allow_sneak: bool,
}

impl Default for ReviveSkill {
    fn default() -> Self { Self { allow_sneak: false } }
}

impl ReviveSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ReviveSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ReviveSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

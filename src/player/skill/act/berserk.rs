use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct BerserkSkill;

impl BerserkSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for BerserkSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for BerserkSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BerserkState {
    pub step: i32,
}

impl Default for BerserkState {
    fn default() -> Self { Self { step: 1 } }
}


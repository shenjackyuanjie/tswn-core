use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ShadowSkill;

impl ShadowSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ShadowSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ShadowSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

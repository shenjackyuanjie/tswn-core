use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct HealSkill {
    pub allow_sneak: bool,
}

impl Default for HealSkill {
    fn default() -> Self {
        Self {
            allow_sneak: false,
        }
    }
}

impl HealSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for HealSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HealSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}


use crate::player::{
    PlrPtr,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

/// 真的就是啥都没有啊喂
#[derive(Debug, Clone)]
pub struct NoneSkill {}

impl NoneSkill {
    pub fn new() -> Self { Self {} }
}

impl SkillExt for NoneSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(NoneSkill::new()) }
}

impl SkillTrait for NoneSkill {
    fn destroy(&self, plr: PlrPtr, args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

use crate::engine::storage::PlrId;
use crate::player::skill::{SkillArgs, SkillExt, SkillTrait};

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
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

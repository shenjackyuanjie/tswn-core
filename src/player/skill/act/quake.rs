use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct QuakeSkill {
    pub sel_count: usize,
    pub sel_count_smart: usize,
}

impl Default for QuakeSkill {
    fn default() -> Self {
        Self {
            sel_count: 5,
            sel_count_smart: 6,
        }
    }
}

impl QuakeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for QuakeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for QuakeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

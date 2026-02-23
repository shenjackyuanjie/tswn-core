use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct RapidSkill {
    pub sel_count: usize,
    pub sel_count_smart: usize,
}

impl Default for RapidSkill {
    fn default() -> Self {
        Self {
            sel_count: 3,
            sel_count_smart: 5,
        }
    }
}

impl RapidSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for RapidSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for RapidSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

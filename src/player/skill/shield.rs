use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ShieldSkill;

impl ShieldSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ShieldSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ShieldSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldState {
    pub sort_id: f64,
    pub target: Option<PlrId>,
    pub shield: i32,
}

impl Default for ShieldState {
    fn default() -> Self {
        Self {
            sort_id: 6000.0,
            target: None,
            shield: 0,
        }
    }
}


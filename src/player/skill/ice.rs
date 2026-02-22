use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct IceSkill;

impl IceSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for IceSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for IceSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IceState {
    pub target: Option<PlrId>,
    pub pre_step_impl: Option<()>,
    pub frozen_step: i32,
}

impl Default for IceState {
    fn default() -> Self {
        Self {
            target: None,
            pre_step_impl: None,
            frozen_step: 1024,
        }
    }
}


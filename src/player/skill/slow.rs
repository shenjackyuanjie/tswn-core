use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct SlowSkill;

impl SlowSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for SlowSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for SlowSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlowState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub step: i32,
}

impl Default for SlowState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_post_action: None,
            step: 2,
        }
    }
}


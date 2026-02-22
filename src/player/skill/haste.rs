use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct HasteSkill;

impl HasteSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for HasteSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HasteSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HasteState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub faster: i32,
    pub step: i32,
}

impl Default for HasteState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_post_action: None,
            faster: 2,
            step: 3,
        }
    }
}


use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct CurseSkill;

impl CurseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CurseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CurseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurseState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_update_state: Option<()>,
    pub prob: i32,
    pub multiply: i32,
}

impl Default for CurseState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_update_state: None,
            prob: 42,
            multiply: 2,
        }
    }
}


use crate::engine::update::RunUpdates;
use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct CounterSkill {
    pub pending: bool,
    pub last_updates: Option<RunUpdates>,
    pub last_target: Option<PlrId>,
}

impl Default for CounterSkill {
    fn default() -> Self {
        Self {
            pending: false,
            last_updates: None,
            last_target: None,
        }
    }
}

impl CounterSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for CounterSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CounterSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}


use crate::engine::storage::PlrId;
use crate::engine::event::Event;
use crate::player::skill::{SkillArgs, SkillExt, SkillTrait};


#[derive(Debug, Clone)]
pub struct IceSkill {
    // 我知道你在找什么，但他确实啥属性都没有  
}

#[derive(Debug, Clone)]
pub struct IceState {
    pub target: PlrId,
    pub fronzen_step: u32,
}

impl SkillExt for IceSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(IceSkill {}) }
}

impl SkillTrait for IceSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {
        
    }
    fn clone_box(&self) -> Box<dyn SkillTrait> {
        Box::new(self.clone())
    }
    fn meta(&self) -> i32 {
        -1
    }   
}

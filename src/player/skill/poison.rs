use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct PoisonSkill;

impl PoisonSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for PoisonSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for PoisonSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoisonState {
    pub caster: Option<PlrId>,
    pub target: Option<PlrId>,
    pub atp: f64,
    pub count: i32,
}

impl Default for PoisonState {
    fn default() -> Self {
        Self {
            caster: None,
            target: None,
            atp: 0.0,
            count: 4,
        }
    }
}


use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ReraiseSkill {
    pub sort_id: f64,
}

impl Default for ReraiseSkill {
    fn default() -> Self {
        Self {
            sort_id: 10.0,
        }
    }
}

impl ReraiseSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ReraiseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ReraiseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDeath] }
}


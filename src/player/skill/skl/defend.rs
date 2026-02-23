use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct DefendSkill {
    pub sort_id: f64,
}

impl Default for DefendSkill {
    fn default() -> Self { Self { sort_id: 2000.0 } }
}

impl DefendSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for DefendSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for DefendSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDefend] }
}

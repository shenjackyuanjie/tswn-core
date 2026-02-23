use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct AccumulateSkill {
    pub on_update_state: Option<()>,
    pub acc: f64,
}

impl Default for AccumulateSkill {
    fn default() -> Self {
        Self {
            on_update_state: None,
            acc: 1.7,
        }
    }
}

impl AccumulateSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for AccumulateSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for AccumulateSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::UpdateState] }
}

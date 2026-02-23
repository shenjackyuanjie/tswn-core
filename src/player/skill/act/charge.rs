use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ChargeSkill {
    pub on_update_state: Option<()>,
    pub on_post_action: Option<()>,
    pub step: i32,
}

impl Default for ChargeSkill {
    fn default() -> Self {
        Self {
            on_update_state: None,
            on_post_action: None,
            step: 0,
        }
    }
}

impl ChargeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ChargeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ChargeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction, ProcKind::UpdateState] }
}

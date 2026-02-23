use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct IronSkill {
    pub sort_id: f64,
    pub on_post_defend: Option<()>,
    pub on_post_action: Option<()>,
    pub on_update_state: Option<()>,
    pub protect: i32,
    pub step: i32,
}

impl Default for IronSkill {
    fn default() -> Self {
        Self {
            sort_id: 4000.0,
            on_post_defend: None,
            on_post_action: None,
            on_update_state: None,
            protect: 0,
            step: 0,
        }
    }
}

impl IronSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for IronSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for IronSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDefend, ProcKind::PostAction, ProcKind::UpdateState] }
}


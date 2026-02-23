use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct AssassinateSkill {
    pub on_pre_action: Option<()>,
    pub on_post_damage: Option<()>,
    pub target: Option<PlrId>,
}

impl Default for AssassinateSkill {
    fn default() -> Self {
        Self {
            on_pre_action: None,
            on_post_damage: None,
            target: None,
        }
    }
}

impl AssassinateSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for AssassinateSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for AssassinateSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PreAction, ProcKind::PostDamage] }
}

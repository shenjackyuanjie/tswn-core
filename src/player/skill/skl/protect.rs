use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ProtectSkill {
    pub allow_sneak: bool,
    pub protect_to: Option<PlrId>,
}

impl Default for ProtectSkill {
    fn default() -> Self {
        Self {
            allow_sneak: false,
            protect_to: None,
        }
    }
}

impl ProtectSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ProtectSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ProtectSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction] }
}


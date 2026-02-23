use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ZombieSkill;

impl ZombieSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ZombieSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ZombieSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZombieState {
    pub target: Option<PlrId>,
}


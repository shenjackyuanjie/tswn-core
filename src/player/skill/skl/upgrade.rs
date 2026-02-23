use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct UpgradeSkill {
    pub on_update_state: Option<()>,
}

impl Default for UpgradeSkill {
    fn default() -> Self {
        Self {
            on_update_state: None,
        }
    }
}

impl UpgradeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for UpgradeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for UpgradeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDamage] }
}


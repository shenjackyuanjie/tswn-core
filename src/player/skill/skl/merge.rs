use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, StateTrait,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct MergeSkill;

impl MergeSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for MergeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for MergeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool {
        if args.1.r63() >= 32 {
            return false;
        }
        args.3
            .just_get_player_mut(target)
            .expect("cannot get merge target from storage")
            .set_state(MergeState { target: Some(target) });
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][吞噬]了[1]", args.0, target, 60));
        true
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MergeState {
    pub target: Option<PlrId>,
}

impl StateTrait for MergeState {
    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

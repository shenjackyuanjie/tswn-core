use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, StateTrait,
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

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool {
        if args.1.r63() >= 24 {
            return false;
        }
        args.3
            .just_get_player_mut(target)
            .expect("cannot get zombie target from storage")
            .set_state(ZombieState { target: Some(target) });
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][召唤亡灵]", args.0, target, 60));
        true
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZombieState {
    pub target: Option<PlrId>,
}

impl StateTrait for ZombieState {
    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

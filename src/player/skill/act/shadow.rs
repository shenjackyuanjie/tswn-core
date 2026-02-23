use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ShadowSkill {
    pub step: i32,
}

impl ShadowSkill {
    pub fn new() -> Self { Self { step: 0 } }
}

impl SkillExt for ShadowSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ShadowSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if self.step > 0 {
            return false;
        }
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get shadow owner from storage");
            if owner.get_status().hp < 80 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        self.step = 2;
        args.2.add(RunUpdate::new("[0]使用[幻术]", args.0, args.0, 60));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get shadow owner from storage");
        owner.set_move_point(owner.move_point() + 512);
        args.2.add(RunUpdate::new("召唤出[1]", args.0, args.0, 20));
    }

    fn post_action(&mut self, _args: SkillArgs) {
        if self.step > 0 {
            self.step -= 1;
        }
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.step > 0 {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get shadow owner from storage");
            owner.mul_at_boost(1.2);
            owner.mul_attract(0.8);
        }
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction, ProcKind::UpdateState] }
}

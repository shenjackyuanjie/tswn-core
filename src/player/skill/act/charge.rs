use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ChargeSkill {
    pub on_update_state: Option<()>,
    pub on_post_action: Option<()>,
    pub step: i32,
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

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if self.step > 0 {
            return false;
        }
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get charge owner from storage");
            if owner.get_status().hp < 100 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        self.step += 2;
        self.on_post_action = Some(());
        self.on_update_state = Some(());
        args.2.add(RunUpdate::new("[0]开始[蓄力]", args.0, args.0, 20));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get charge owner from storage");
        owner.update_states();
        owner.set_mp(owner.mp() + 32);
    }

    fn post_action(&mut self, args: SkillArgs) {
        if self.step <= 0 {
            return;
        }
        self.step -= 1;
        if self.step <= 0 {
            self.on_post_action = None;
            self.on_update_state = None;
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get charge owner from storage")
                .update_states();
        }
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.step > 0 {
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get charge owner from storage")
                .mul_at_boost(3.0);
        }
    }

    fn update_state_inline(&mut self, _level: u32, status: &mut crate::player::PlayerStatus) {
        if self.step > 0 {
            status.at_boost *= 3.0;
        }
    }

    fn clear_positive_runtime(&mut self, args: SkillArgs) -> Option<&'static str> {
        if self.on_update_state.is_none() {
            return None;
        }
        self.step = 0;
        self.on_update_state = None;
        self.on_post_action = None;
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get charge owner from storage")
            .update_states();
        Some("[1]的[蓄力]被中止了")
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction, ProcKind::UpdateState] }
}

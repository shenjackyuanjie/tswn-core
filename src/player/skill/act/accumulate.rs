use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct AccumulateSkill {
    pub on_update_state: Option<()>,
    pub acc: f64,
}

impl Default for AccumulateSkill {
    fn default() -> Self {
        Self {
            on_update_state: None,
            acc: 1.7000000476837158,
        }
    }
}

impl AccumulateSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for AccumulateSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for AccumulateSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if self.on_update_state.is_some() {
            return false;
        }
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get accumulate owner from storage");
            if owner.get_status().hp < 120 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if self.on_update_state.is_some() {
            return;
        }
        self.on_update_state = Some(());
        args.2.add(RunUpdate::new("[0]开始[聚气]", args.0, args.0, 1));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get accumulate owner from storage");
        owner.set_move_point(owner.move_point() + 400);
        owner.mul_at_boost(self.acc);
        args.2.add(RunUpdate::new("[0]攻击力上升", args.0, args.0, 0));
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.on_update_state.is_some() {
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get accumulate owner from storage")
                .mul_at_boost(self.acc);
        }
    }

    fn update_state_inline(&mut self, _level: u32, status: &mut crate::player::PlayerStatus) {
        if self.on_update_state.is_some() {
            status.at_boost *= self.acc;
        }
    }

    fn clear_positive_runtime(&mut self, args: SkillArgs) -> Option<&'static str> {
        if self.on_update_state.is_none() {
            return None;
        }
        self.on_update_state = None;
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get accumulate owner from storage")
            .update_states();
        Some("[1]的[聚气]被打消了")
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::UpdateState] }
}

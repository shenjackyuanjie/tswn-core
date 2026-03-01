use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::act::minion::is_combat_minion,
    skill::merge::MergeState,
    skill::zombie::ZombieState,
    skill::{SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ReviveSkill {
    pub allow_sneak: bool,
}

impl ReviveSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ReviveSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ReviveSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_act_level(&self, level: u32) -> u32 { (level + 1) >> 1 }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain_with_level(&self, _level: u32) -> SkillTargetDomain { SkillTargetDomain::AllyAny }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, _smart: bool, args: SkillArgs) -> bool {
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        !target_plr.alive()
            && !is_combat_minion(target_plr)
            && !target_plr.has_state::<MergeState>()
            && !target_plr.has_state::<ZombieState>()
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            target_plr.attr_sum() as f64
        } else {
            args.1.rFFFF() as f64
        }
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get revive owner from storage")
            .get_at(true, args.1);
        let mut heal = (atp / 75.0).ceil() as i32;
        let max_hp = args
            .3
            .get_player(&target_id)
            .expect("cannot get revive target from storage")
            .get_status()
            .max_hp;
        heal = heal.clamp(1, max_hp.max(1));
        args.2.add(RunUpdate::new("[0]使用[苏生术]", args.0, target_id, 40));
        let target = args.3.just_get_player_mut(target_id).expect("cannot get revive target from storage");
        if target.alive() {
            return;
        }
        target.revive_with_hp(heal);
        args.2.add(RunUpdate::new("[1][复活]了", args.0, target_id, (heal + 60) as u32));
        args.2.add(RunUpdate::new("[1]回复体力[2]点", args.0, target_id, heal as u32));
    }
}

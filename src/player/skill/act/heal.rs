use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct HealSkill {
    pub allow_sneak: bool,
}

impl HealSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for HealSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HealSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn post_act_level(&self, level: u32) -> u32 { if level > 8 { level - 1 } else { level } }

    fn target_domain_with_level(&self, _level: u32) -> SkillTargetDomain { SkillTargetDomain::AllyAlive }

    fn select_target_count_with_level(&self, _level: u32, _smart: bool) -> usize { 1 }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        let status = target_plr.get_status();
        if smart {
            status.hp + 80 < status.max_hp
        } else {
            status.hp < status.max_hp
        }
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            let status = target_plr.get_status();
            let mut damaged = (status.max_hp - status.hp).max(0);
            damaged += (target_plr.negative_state_count() as i32) * 64;
            (damaged as f64) * (target_plr.attr_sum().max(1) as f64)
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
            .expect("cannot get heal owner from storage")
            .get_at(true, args.1);
        let missing_hp = {
            let target = args.3.get_player(&target_id).expect("cannot get heal target from storage");
            (target.get_status().max_hp - target.get_status().hp).max(0)
        };
        if missing_hp <= 0 {
            return;
        }
        let heal = ((atp / 60.0).ceil() as i32).clamp(1, missing_hp);
        args.2.add(RunUpdate::new("[0]使用[治愈魔法]", args.0, target_id, 20));
        let target = args.3.just_get_player_mut(target_id).expect("cannot get heal target from storage");
        target.damage(-heal, args.0, on_heal, args.1, args.2, args.3);
        target.clear_negative_states();
    }
}

fn on_heal(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut crate::rc4::RC4,
    _updates: &mut crate::engine::update::RunUpdates,
) {
}

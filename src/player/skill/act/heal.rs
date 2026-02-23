use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct HealSkill {
    pub allow_sneak: bool,
}

impl Default for HealSkill {
    fn default() -> Self { Self { allow_sneak: false } }
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

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        let owner = args.3.get_player(&args.0).expect("cannot get heal owner from storage");
        let lost_hp = owner.get_status().max_hp - owner.get_status().hp;
        if smart && lost_hp < 24 {
            return false;
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get heal owner from storage")
            .get_at(true, args.1);
        let mut heal = (atp / 60.0).ceil() as i32 + (level as i32 / 12);
        if heal <= 0 {
            heal = 1;
        }
        args.2.add(RunUpdate::new("[0]使用[治愈魔法]", args.0, target_id, 20));
        let target = args.3.just_get_player_mut(target_id).expect("cannot get heal target from storage");
        target.damage(-heal, args.0, on_heal, args.1, args.2, args.3);
        target.clear_negative_states();
    }
}

fn on_heal(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut crate::rc4::RC4, _updates: &mut crate::engine::update::RunUpdates) {
}

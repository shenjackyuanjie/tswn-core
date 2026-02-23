use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ReviveSkill {
    pub allow_sneak: bool,
}

impl Default for ReviveSkill {
    fn default() -> Self { Self { allow_sneak: false } }
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

    fn has_action_impl(&self) -> bool { true }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        let owner = args.3.get_player(&args.0).expect("cannot get revive owner from storage");
        if smart && owner.get_status().hp > owner.get_status().max_hp / 2 {
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
            .expect("cannot get revive owner from storage")
            .get_at(true, args.1);
        let mut heal = (atp / 75.0).ceil() as i32 + (level as i32 / 16);
        if heal <= 0 {
            heal = 1;
        }
        let max_hp = args
            .3
            .get_player(&target_id)
            .expect("cannot get revive target from storage")
            .get_status()
            .max_hp;
        if heal > max_hp {
            heal = max_hp;
        }
        args.2.add(RunUpdate::new("[0]使用[苏生术]", args.0, target_id, 40));
        let target = args.3.just_get_player_mut(target_id).expect("cannot get revive target from storage");
        if target.alive() {
            target.damage(-heal, args.0, on_revive, args.1, args.2, args.3);
        } else {
            target.revive_with_hp(heal);
            args.2.add(RunUpdate::new("[1][复活]了", args.0, target_id, (heal + 60) as u32));
        }
    }
}

fn on_revive(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut crate::rc4::RC4,
    _updates: &mut crate::engine::update::RunUpdates,
) {
}

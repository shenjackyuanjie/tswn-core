use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct CloneSkill;

impl CloneSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CloneSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CloneSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
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
        args.2.add(RunUpdate::new("[0]使用[分身]", args.0, args.0, 60));
        let hp = args
            .3
            .get_player(&args.0)
            .expect("cannot get clone owner from storage")
            .get_status()
            .hp;
        let self_damage = (hp / 2).max(1);
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get clone owner from storage");
        owner.damage(self_damage, args.0, on_clone as OnDamageFunc, args.1, args.2, args.3);
        owner.set_move_point(owner.move_point() + args.1.r255() as i32 * 4 + 1024);
        args.2.add(RunUpdate::new("出现一个新的[1]", args.0, args.0, 20));
    }
}

fn on_clone(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

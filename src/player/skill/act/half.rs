use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, Player, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct HalfSkill;

impl HalfSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for HalfSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HalfSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        if !smart {
            return true;
        }
        args.3.get_player(&target).map(|x| x.get_status().hp > 100).unwrap_or(false)
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        let base = if smart {
            target_plr.get_status().attract
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        base * target_plr.get_status().hp as f64
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[生命之轮]", args.0, target_id, 20));
        let owner_magic = args.3.get_player(&args.0).expect("cannot get half owner from storage").get_status().magic;
        let target_hp = args
            .3
            .get_player(&target_id)
            .expect("cannot get half target from storage")
            .get_status()
            .hp;
        let mut chance = (400 - target_hp) / 3;
        if chance < 0 {
            chance = 0;
        }
        let target_dodge = args
            .3
            .get_player(&target_id)
            .expect("cannot get half target from storage")
            .get_status()
            .resistance
            + args
                .3
                .get_player(&target_id)
                .expect("cannot get half target from storage")
                .get_status()
                .agility;
        let target = args.3.just_get_player_mut(target_id).expect("cannot get half target from storage");
        if target.active() && Player::dodge(chance, target_dodge, args.1) {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }
        let mut x = (owner_magic - target.get_status().resistance / 2) / 2 + 47;
        x = x.clamp(1, 99);
        let old_hp = target.get_status().hp;
        let new_hp = ((old_hp as f64) * (100 - x) as f64 / 100.0).ceil() as i32;
        let dmg = (old_hp - new_hp).max(0);
        if dmg > 0 {
            target.damage(dmg, args.0, on_half as OnDamageFunc, args.1, args.2, args.3);
        }
    }
}

fn on_half(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

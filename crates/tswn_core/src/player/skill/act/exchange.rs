use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ExchangeSkill;

impl ExchangeSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ExchangeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ExchangeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn post_act_level(&self, level: u32) -> u32 { (level + 1) >> 1 }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        let Some(owner) = args.3.get_player(&args.0) else {
            return false;
        };
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        if smart {
            target_plr.get_status().hp - owner.get_status().hp > 32
        } else {
            target_plr.get_status().hp > owner.get_status().hp
        }
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let score = if smart {
            let alive_group_count = args.3.alive_group_count();
            let target_alive_group_len = args.3.alive_group_len_containing(target);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                rate_hi_hp(status.hp) * status.attr_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if smart {
            score * target_plr.get_status().hp as f64
        } else {
            score
        }
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[生命之轮]", args.0, target_id, 1));

        let (owner_magic, charge_active, owner_hp, owner_max_hp) = args
            .3
            .get_player(&args.0)
            .map(|owner| {
                (
                    owner.get_status().magic,
                    owner.get_status().at_boost >= 3.0,
                    owner.get_status().hp,
                    owner.get_status().max_hp,
                )
            })
            .expect("cannot get exchange owner from storage");
        let (target_res, target_def, target_agl, target_hp, target_immune, target_active) = args
            .3
            .get_player(&target_id)
            .map(|target| {
                (
                    target.get_status().resistance,
                    target.get_status().defense,
                    target.get_status().agility,
                    target.get_status().hp,
                    target.check_immune("exchange", args.1),
                    target.active(),
                )
            })
            .expect("cannot get exchange target from storage");
        if target_immune
            || (target_active && !charge_active && Player::dodge(owner_magic, target_res + target_def + target_agl, args.1))
        {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        if charge_active {
            let target_move_point = args
                .3
                .get_player(&target_id)
                .expect("cannot get exchange target from storage")
                .move_point();
            {
                let owner = args.3.just_get_player_mut(args.0).expect("cannot get exchange owner from storage");
                owner.set_move_point(owner.move_point() + target_move_point);
            }
            {
                let target = args.3.just_get_player_mut(target_id).expect("cannot get exchange target from storage");
                target.set_move_point(0);
            }
        }

        let owner_new_hp = target_hp.min(owner_max_hp);
        let target_new_hp = owner_hp;
        {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get exchange owner from storage");
            owner.set_hp_raw(owner_new_hp);
        }
        {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get exchange target from storage");
            target.set_hp_raw(target_new_hp);
        }

        args.2.add(RunUpdate::new(
            "[1]的体力值与[0]互换",
            args.0,
            target_id,
            ((target_hp - owner_hp) * 2).max(0) as u32,
        ));

        if target_hp > target_new_hp {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get exchange target from storage");
            target.on_damaged(target_hp - target_new_hp, target_hp, args.0, args.1, args.2, args.3);
        }
    }
}

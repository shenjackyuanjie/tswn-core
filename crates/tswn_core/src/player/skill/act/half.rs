use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlrId,
    skill::{InlineCtx, SkillArgs, SkillExt, SkillTrait},
};

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

    fn has_inline_act(&self) -> bool { true }

    fn has_action_impl(&self) -> bool { true }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        if !smart {
            return true;
        }
        args.3
            .get_player(&target)
            .map(|x| x.get_status().hp > 160 && x.get_status().hp < 400)
            .unwrap_or(false)
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
        let base = if smart {
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
        base * target_plr.get_status().hp as f64
    }

    fn act_inline(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, ctx: &mut InlineCtx) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        ctx.updates.add(RunUpdate::new("[0]使用[瘟疫]", ctx.ptr, target_id, 1));

        let owner_wisdom = ctx.owner.get_status().wisdom;
        let owner_magic = ctx.owner.get_status().magic;
        let charge_active = ctx.owner.get_status().at_boost >= 3.0;
        let (target_hp, target_resistance, target_agility, target_immune, target_active) = ctx
            .storage
            .get_player(&target_id)
            .map(|target| {
                (
                    target.get_status().hp,
                    target.get_status().resistance,
                    target.get_status().agility,
                    target.check_immune("half", ctx.randomer),
                    target.active(),
                )
            })
            .expect("cannot get half target from storage");
        let mut chance = owner_wisdom + ((360 - target_hp) / 3);
        if chance < 0 {
            chance = 0;
        }
        if target_immune
            || (target_active && !charge_active && Player::dodge(chance, target_resistance + target_agility, ctx.randomer))
        {
            ctx.updates.add(RunUpdate::new("[0][回避]了攻击", target_id, ctx.ptr, 20));
            return;
        }

        let mut percent = ((owner_magic - (target_resistance / 2)) / 2) + 47;
        if charge_active {
            percent = owner_magic + 50;
        }
        if percent > 99 {
            percent = 99;
        }
        let old_hp = target_hp;
        let new_hp = ((old_hp as f64) * (100 - percent) as f64 / 100.0).ceil() as i32;
        let dmg = (old_hp - new_hp).max(0);

        let mut update = RunUpdate::new("[1]体力减少[2]%", ctx.ptr, target_id, dmg as u32);
        update.param = Some(percent.max(0) as u32);
        ctx.updates.add(update);
        if dmg <= 0 {
            return;
        }
        let target = ctx.storage.just_get_player_mut(target_id).expect("cannot get half target from storage");
        target.set_hp_raw(new_hp);
        target.on_damaged(dmg, old_hp, ctx.ptr, ctx.randomer, ctx.updates, ctx.storage);
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[瘟疫]", args.0, target_id, 1));

        let (owner_wisdom, owner_magic, charge_active) = args
            .3
            .get_player(&args.0)
            .map(|owner| {
                (
                    owner.get_status().wisdom,
                    owner.get_status().magic,
                    owner.get_status().at_boost >= 3.0,
                )
            })
            .expect("cannot get half owner from storage");
        let (target_hp, target_resistance, target_agility, target_immune, target_active) = args
            .3
            .get_player(&target_id)
            .map(|target| {
                (
                    target.get_status().hp,
                    target.get_status().resistance,
                    target.get_status().agility,
                    target.check_immune("half", args.1),
                    target.active(),
                )
            })
            .expect("cannot get half target from storage");
        let mut chance = owner_wisdom + ((360 - target_hp) / 3);
        if chance < 0 {
            chance = 0;
        }
        if target_immune || (target_active && !charge_active && Player::dodge(chance, target_resistance + target_agility, args.1))
        {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        let mut percent = ((owner_magic - (target_resistance / 2)) / 2) + 47;
        if charge_active {
            percent = owner_magic + 50;
        }
        if percent > 99 {
            percent = 99;
        }
        let old_hp = target_hp;
        let new_hp = ((old_hp as f64) * (100 - percent) as f64 / 100.0).ceil() as i32;
        let dmg = (old_hp - new_hp).max(0);

        let mut update = RunUpdate::new("[1]体力减少[2]%", args.0, target_id, dmg as u32);
        update.param = Some(percent.max(0) as u32);
        args.2.add(update);
        if dmg <= 0 {
            return;
        }
        let target = args.3.just_get_player_mut(target_id).expect("cannot get half target from storage");
        target.set_hp_raw(new_hp);
        target.on_damaged(dmg, old_hp, args.0, args.1, args.2, args.3);
    }
}

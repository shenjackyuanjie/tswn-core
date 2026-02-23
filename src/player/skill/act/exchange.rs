use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, Player, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

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

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

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
        let mut score = if smart {
            target_plr.get_status().attract
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if smart {
            score *= target_plr.get_status().hp as f64;
        }
        score
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[交换]", args.0, target_id, 20));

        let owner_magic = args
            .3
            .get_player(&args.0)
            .expect("cannot get exchange owner from storage")
            .get_status()
            .magic;
        let target_agl = args
            .3
            .get_player(&target_id)
            .expect("cannot get exchange target from storage")
            .get_status()
            .agility;
        if args.3.get_player(&target_id).map(|x| x.active()).unwrap_or(false) && Player::dodge(owner_magic, target_agl, args.1) {
            args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 20));
            return;
        }

        let owner_hp = args
            .3
            .get_player(&args.0)
            .expect("cannot get exchange owner from storage")
            .get_status()
            .hp;
        let target_hp = args
            .3
            .get_player(&target_id)
            .expect("cannot get exchange target from storage")
            .get_status()
            .hp;

        let owner_delta = owner_hp - target_hp;
        if owner_delta != 0 {
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get exchange owner from storage")
                .damage(owner_delta, args.0, on_exchange as OnDamageFunc, args.1, args.2, args.3);
        }
        let target_delta = target_hp - owner_hp;
        if target_delta != 0 {
            args.3
                .just_get_player_mut(target_id)
                .expect("cannot get exchange target from storage")
                .damage(target_delta, args.0, on_exchange as OnDamageFunc, args.1, args.2, args.3);
        }
    }
}

fn on_exchange(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

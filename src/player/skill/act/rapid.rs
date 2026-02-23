use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone)]
pub struct RapidSkill {
    pub sel_count: usize,
    pub sel_count_smart: usize,
}

impl Default for RapidSkill {
    fn default() -> Self {
        Self {
            sel_count: 3,
            sel_count_smart: 5,
        }
    }
}

impl RapidSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for RapidSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for RapidSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn select_target_count(&self, smart: bool) -> usize {
        if smart {
            self.sel_count_smart
        } else {
            self.sel_count
        }
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            target_plr.get_status().max_hp as f64 - target_plr.get_status().hp as f64
        } else {
            args.1.rFFFF() as f64
        }
    }

    fn act_with_level(&mut self, level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let round = if args.1.c50() { 3 } else { 2 };
        let cycle_len = targets.len().min(3).max(1);
        let mut pos = 0usize;
        for hit in 0..round {
            let target_id = targets[pos];
            let owner = args.3.get_player(&args.0).expect("cannot get rapid owner from storage");
            let atp = owner.get_at(false, args.1) * ((0.75 - hit as f64 * 0.15).max(0.35) + level as f64 / 1024.0);
            if hit == 0 {
                args.2.add(RunUpdate::new("[0]发起攻击", args.0, target_id, 8));
            } else {
                args.2.add(RunUpdate::new("[0][连击]", args.0, target_id, 8));
            }
            let dmg = args
                .3
                .just_get_player_mut(target_id)
                .expect("cannot get rapid target from storage")
                .attacked(atp, false, args.0, on_rapid as OnDamageFunc, args.1, args.2, args.3);
            if dmg <= 0 {
                break;
            }
            args.2.add(RunUpdate::new_newline());
            pos = (pos + args.1.r3() as usize) % cycle_len;
        }
    }
}

fn on_rapid(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

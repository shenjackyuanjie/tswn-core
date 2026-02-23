use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone)]
pub struct QuakeSkill {
    pub sel_count: usize,
    pub sel_count_smart: usize,
}

impl Default for QuakeSkill {
    fn default() -> Self {
        Self {
            sel_count: 5,
            sel_count_smart: 6,
        }
    }
}

impl QuakeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for QuakeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for QuakeSkill {
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
            target_plr.get_status().hp as f64
        } else {
            args.1.rFFFF() as f64
        }
    }

    fn act_with_level(&mut self, level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let round = if args.1.c50() { 5 } else { 4 };
        let mut picked = targets;
        picked.truncate(round.min(picked.len()));
        if picked.is_empty() {
            return;
        }
        args.2.add(RunUpdate::new("[0]发动[地裂术]", args.0, picked[0], 10));
        let divisor = picked.len() as f64 + 0.6;
        for target_id in picked {
            let owner = args.3.get_player(&args.0).expect("cannot get quake owner from storage");
            let atp = owner.get_at(true, args.1) * (2.44 + level as f64 / 512.0) / divisor;
            args.2.add(RunUpdate::new_newline());
            args.3
                .just_get_player_mut(target_id)
                .expect("cannot get quake target from storage")
                .attacked(atp, true, args.0, on_quake as OnDamageFunc, args.1, args.2, args.3);
        }
    }
}

fn on_quake(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

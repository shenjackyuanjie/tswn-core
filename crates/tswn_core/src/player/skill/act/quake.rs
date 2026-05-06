use std::sync::Arc;

use crate::engine::storage::Storage;
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

    fn select_target_count(&self, smart: bool) -> usize { if smart { self.sel_count_smart } else { self.sel_count } }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let round = if args.1.c50() { 5 } else { 4 };
        let mut picked = targets;
        picked.truncate(round.min(picked.len()));
        if picked.is_empty() {
            return;
        }
        args.2.add(RunUpdate::new("[0]使用[地裂术]", args.0, picked[0], 1));
        let divisor = picked.len() as f64 + 0.6000000238418579;
        for target_id in picked {
            // JS: getAt is called BEFORE the hp > 0 check, so RC4 is consumed even for dead targets
            let owner = args.3.get_player(&args.0).expect("cannot get quake owner from storage");
            // Keep the decompiled JS float literal exactly. 2.44 shifts some cases across ceil() boundaries.
            let atp = owner.get_at(true, args.1) * 2.440000057220459 / divisor;
            let target_alive = args.3.get_player(&target_id).map(|p| p.get_status().hp > 0).unwrap_or(false);
            if !target_alive {
                continue;
            }
            args.2.add(RunUpdate::new_newline());
            let core = {
                let target = args
                    .3
                    .just_get_player_mut(target_id)
                    .expect("cannot get quake target from storage");
                target.attacked_core(atp, true, args.0, on_quake as OnDamageFunc, args.1, args.2, args.3)
            };
            if core.hit {
                on_quake(args.0, core.target, core.dmg, args.1, args.2, args.3);
                let target = args
                    .3
                    .just_get_player_mut(core.target)
                    .expect("cannot get quake target from storage");
                target.finish_damage(core.dmg, core.old_hp, args.0, args.1, args.2, args.3);
            }
        }
    }
}

fn on_quake(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

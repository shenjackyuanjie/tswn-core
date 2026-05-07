use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{InlineCtx, SkillArgs, SkillExt, SkillTrait},
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

    fn has_inline_act(&self) -> bool { true }

    fn has_action_impl(&self) -> bool { true }

    fn select_target_count(&self, smart: bool) -> usize { if smart { self.sel_count_smart } else { self.sel_count } }

    fn act_inline(&mut self, level: u32, targets: Vec<PlrId>, _smart: bool, ctx: &mut InlineCtx) {
        if targets.is_empty() {
            return;
        }
        let _ = level;
        let round = if ctx.randomer.c50() { 3.0 } else { 2.0 };
        let mut targets = targets;
        if targets.len() > 3 {
            targets.truncate(3);
        }
        let mut hit_scores = vec![0.0f64; targets.len()];
        let mut pos = 0usize;
        let mut i = 0.0f64;
        while i < round {
            if !ctx.owner.active() {
                return;
            }
            let target_id = targets[pos];
            let target_dead = ctx.storage.get_player(&target_id).map(|x| !x.alive()).unwrap_or(true);
            if target_dead {
                i -= 0.5;
            } else {
                let atp = ctx.owner.get_at(false, ctx.randomer) * (0.75 - hit_scores[pos] * 0.15000000596046448);
                hit_scores[pos] += 1.0;
                if i == 0.0 {
                    ctx.updates.add(RunUpdate::new("[0]发起攻击", ctx.ptr, target_id, 0));
                } else {
                    ctx.updates.add(RunUpdate::new("[0][连击]", ctx.ptr, target_id, 1));
                }
                let dmg = {
                    let core = {
                        let target = ctx.storage.just_get_player_mut(target_id).expect("cannot get rapid target from storage");
                        target.attacked_core(atp, false, ctx.ptr, on_rapid as OnDamageFunc, ctx.randomer, ctx.updates, ctx.storage)
                    };
                    if !core.hit {
                        0
                    } else {
                        on_rapid(ctx.ptr, core.target, core.dmg, ctx.randomer, ctx.updates, ctx.storage);
                        let target = ctx.storage.just_get_player_mut(core.target).expect("cannot get rapid target from storage");
                        target.finish_damage(core.dmg, core.old_hp, ctx.ptr, ctx.randomer, ctx.updates, ctx.storage)
                    }
                };
                if dmg <= 0 {
                    return;
                }
                ctx.updates.add(RunUpdate::new_newline());
            }
            pos = (pos + ctx.randomer.r3() as usize) % targets.len();
            i += 1.0;
        }
    }

    fn act_with_level(&mut self, level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let _ = level;
        let round = if args.1.c50() { 3.0 } else { 2.0 };
        let mut targets = targets;
        if targets.len() > 3 {
            targets.truncate(3);
        }
        let mut hit_scores = vec![0.0f64; targets.len()];
        let mut pos = 0usize;
        let mut i = 0.0f64;
        while i < round {
            let owner_active = args.3.get_player(&args.0).map(|x| x.active()).unwrap_or(false);
            if !owner_active {
                return;
            }
            let target_id = targets[pos];
            let target_dead = args.3.get_player(&target_id).map(|x| !x.alive()).unwrap_or(true);
            if target_dead {
                i -= 0.5;
            } else {
                let atp = {
                    let owner = args.3.get_player(&args.0).expect("cannot get rapid owner from storage");
                    owner.get_at(false, args.1) * (0.75 - hit_scores[pos] * 0.15000000596046448)
                };
                hit_scores[pos] += 1.0;
                if i == 0.0 {
                    args.2.add(RunUpdate::new("[0]发起攻击", args.0, target_id, 0));
                } else {
                    args.2.add(RunUpdate::new("[0][连击]", args.0, target_id, 1));
                }
                let dmg = {
                    let core = {
                        let target = args
                            .3
                            .just_get_player_mut(target_id)
                            .expect("cannot get rapid target from storage");
                        target.attacked_core(atp, false, args.0, on_rapid as OnDamageFunc, args.1, args.2, args.3)
                    };
                    if !core.hit {
                        0
                    } else {
                        on_rapid(args.0, core.target, core.dmg, args.1, args.2, args.3);
                        let target = args
                            .3
                            .just_get_player_mut(core.target)
                            .expect("cannot get rapid target from storage");
                        target.finish_damage(core.dmg, core.old_hp, args.0, args.1, args.2, args.3)
                    }
                };
                if dmg <= 0 {
                    return;
                }
                args.2.add(RunUpdate::new_newline());
            }
            pos = (pos + args.1.r3() as usize) % targets.len();
            i += 1.0;
        }
    }
}

fn on_rapid(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

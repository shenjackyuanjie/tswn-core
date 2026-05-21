use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    MOVE_POINT_THRESHOLD, OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct IceSkill;

impl IceSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for IceSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for IceSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn score_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };

        let mut score = if smart {
            let rate_hi_hp = |hp: i32| -> f64 {
                if hp < 20 {
                    30.0
                } else if hp > 300 {
                    300.0
                } else {
                    hp as f64
                }
            };
            let rate_low_hp = |hp: i32| -> f64 { 1.0 / rate_hi_hp(hp) };
            let alive_group_count = args.3.alive_group_count();
            let target_alive_group_len = args
                .3
                .alive_group_at_team_of(target)
                .map(|group| group.len())
                .unwrap_or(0);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                rate_low_hp(status.hp) * status.atk_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };

        if target_plr.has_state::<IceState>() {
            score /= 2.0;
        }

        score
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get ice caster from storage")
            .get_at(true, args.1)
            * 0.7;
        args.2.add(RunUpdate::new("[0]使用[冰冻术]", args.0, target_id, 1));
        let core = {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get ice target from storage");
            target.attacked_core(atp, true, args.0, on_ice as OnDamageFunc, args.1, args.2, args.3)
        };
        if core.hit {
            on_ice(args.0, core.target, core.dmg, args.1, args.2, args.3);
            let target = args.3.just_get_player_mut(core.target).expect("cannot get ice target from storage");
            target.finish_damage(core.dmg, core.old_hp, args.0, args.1, args.2, args.3);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IceState {
    pub target: Option<PlrId>,
    pub pre_step_impl: Option<()>,
    pub frozen_step: i32,
}

impl Default for IceState {
    fn default() -> Self {
        Self {
            target: None,
            pre_step_impl: None,
            frozen_step: 1024,
        }
    }
}

impl StateTrait for IceState {
    fn meta_type(&self) -> i32 { -1 }

    fn update_state_priority(&self) -> i32 { 300 }

    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) { status.set_frozen(true); }

    fn pre_step_priority(&self) -> i32 { 100 }

    fn on_pre_step(
        &mut self,
        owner: PlrId,
        status: &crate::player::PlayerStatus,
        step: &mut i32,
        updates: &mut RunUpdates,
    ) -> bool {
        if *step <= 0 {
            return false;
        }
        if self.frozen_step > 0 {
            self.frozen_step -= *step;
            *step = 0;
            return false;
        }
        if *step + status.move_point >= MOVE_POINT_THRESHOLD {
            *step = 0;
            if status.alive() {
                updates.emit(RunUpdate::new_newline);
                updates.emit(|| RunUpdate::new("[1]从[冰冻]中解除", owner, owner, 0));
            }
            return true;
        }
        false
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_ice(caster: PlrId, target: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }

    let blocked = {
        let Some(target_plr) = storage.just_get_player_mut(target) else {
            return;
        };
        target_plr.get_status().hp <= 0 || !target_plr.alive() || target_plr.check_immune("ice", r)
    };
    if blocked {
        return;
    }

    let charge_active = storage.get_player(&caster).map(|p| p.get_status().at_boost >= 3.0).unwrap_or(false);

    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if let Some(state) = target_plr.get_state_mut::<IceState>() {
        state.frozen_step += 1024;
    } else {
        target_plr.set_state(IceState {
            target: Some(target),
            pre_step_impl: None,
            frozen_step: 1024,
        });
    }
    if charge_active && let Some(state) = target_plr.get_state_mut::<IceState>() {
        state.frozen_step += 2048;
    }
    updates.emit(|| RunUpdate::new("[1]被[冰冻]了", caster, target, 40));
}

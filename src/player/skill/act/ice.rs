use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    MOVE_POINT_THRESHOLD, OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
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
        let target = args.3.just_get_player_mut(target_id).expect("cannot get ice target from storage");
        let _ = target.attacked(atp, true, args.0, on_ice as OnDamageFunc, args.1, args.2, args.3);
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
                updates.add(RunUpdate::new_newline());
                updates.add(RunUpdate::new("[1]从[冰冻]中解除", owner, owner, 0));
            }
            return true;
        }
        false
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

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
    updates.add(RunUpdate::new("[1]被[冰冻]了", caster, target, 40));
}

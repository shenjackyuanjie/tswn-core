use std::any::Any;

use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
};
use crate::rc4::RC4;

/// 火状态（参考 Dart `FireState`）。
#[derive(Clone, Copy, Debug, Default)]
pub struct FireState {
    pub fire_mag: f64,
}

impl StateTrait for FireState {
    fn as_any(&self) -> &dyn Any { self }

    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

#[derive(Debug, Clone, Default)]
pub struct FireSkill;

impl FireSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for FireSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(FireSkill::new()) }
}

impl SkillTrait for FireSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];

        let fire_mag = args
            .3
            .get_player(&target_id)
            .expect("cannot get target from storage")
            .get_state::<FireState>()
            .map(|state| state.fire_mag)
            .unwrap_or(0.0);
        let atp = args.3.get_player(&args.0).expect("cannot get owner from storage").get_at(true, args.1) * (1.5 + fire_mag);

        args.2.add(RunUpdate::new("[0]使用[火球术]", args.0, target_id, 1));

        let target = args.3.just_get_player_mut(target_id).expect("cannot get mutable target in storage");
        let dmg = target.attacked(atp, true, args.0, on_fire as OnDamageFunc, args.1, args.2, args.3);

        // 参考 dart: onFire(dmg > 0 && !target.dead) => fireMag += 0.5
        if dmg > 0 && target.alive() && !target.check_immune(state_tag::<FireState>(), args.1) {
            if let Some(fire) = target.get_state_mut::<FireState>() {
                fire.fire_mag += 0.5;
            } else {
                target.set_state(FireState { fire_mag: 0.5 });
            }
        }
    }
}

fn on_fire(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

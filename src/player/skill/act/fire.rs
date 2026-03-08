use std::any::Any;
use std::sync::Arc;

use crate::engine::storage::Storage;
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

    fn meta_type(&self) -> i32 { -1 }
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
        let get_at_val = args.3.get_player(&args.0).expect("cannot get owner from storage").get_at(true, args.1);
        let atp = get_at_val * (1.5 + fire_mag);
        if std::env::var_os("TSWN_DEBUG_FIRE").is_some() {
            let owner_name = args.3.get_player(&args.0).map(|p| p.id_name()).unwrap_or_default();
            let target_name = args.3.get_player(&target_id).map(|p| p.id_name()).unwrap_or_default();
            eprintln!(
                "[fire] caster={} target={} get_at={} fire_mag={} atp={} rc4=({},{})",
                owner_name, target_name, get_at_val, fire_mag, atp, args.1.i, args.1.j
            );
        }

        args.2.add(RunUpdate::new("[0]使用[火球术]", args.0, target_id, 1));

        let _ = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get mutable target in storage")
            .attacked(atp, true, args.0, on_fire as OnDamageFunc, args.1, args.2, args.3);
    }
}

pub(crate) fn on_fire(_caster: PlrId, target: PlrId, dmg: i32, r: &mut RC4, _updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if target_plr.get_status().hp <= 0 || target_plr.check_immune("fire", r) {
        return;
    }
    if let Some(fire) = target_plr.get_state_mut::<FireState>() {
        fire.fire_mag += 0.5;
    } else {
        target_plr.set_state(FireState { fire_mag: 0.5 });
    }
}

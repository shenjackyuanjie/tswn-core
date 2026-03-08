use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlayerType, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct CurseSkill;

impl CurseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CurseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CurseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        if !smart {
            return true;
        }
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        if target_plr.get_status().hp < 80 {
            return false;
        }
        if let Some(curse) = target_plr.get_state::<CurseState>()
            && curse.prob > 32
        {
            return false;
        }
        true
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
            let target_alive_group_len = args.3.alive_group_containing(target).map(|group| group.len()).unwrap_or(0);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                (1.0 / rate_hi_hp(status.hp)) * status.atk_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if target_plr.get_state::<CurseState>().is_some() {
            base / 2.0
        } else {
            base
        }
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get curse caster from storage")
            .get_at(true, args.1);
        args.2.add(RunUpdate::new("[0]使用[诅咒]", args.0, target_id, 1));
        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get curse target from storage")
            .attacked(atp, true, args.0, on_curse as OnDamageFunc, args.1, args.2, args.3);
        if dmg <= 0 {
            return;
        }
        let target = args.3.just_get_player_mut(target_id).expect("cannot get curse target from storage");
        if !target.alive() || target.check_immune("curse", args.1) {
            return;
        }
        if let Some(state) = target.get_state_mut::<CurseState>() {
            state.prob += 10;
            state.multiply += 1;
            state.owner = Some(args.0);
        } else {
            target.set_state(CurseState {
                owner: Some(args.0),
                target: Some(target_id),
                on_update_state: None,
                prob: 42,
                multiply: 2,
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurseState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_update_state: Option<()>,
    pub prob: i32,
    pub multiply: i32,
}

impl Default for CurseState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_update_state: None,
            prob: 42,
            multiply: 2,
        }
    }
}

impl StateTrait for CurseState {
    fn meta_type(&self) -> i32 { -1 }

    fn update_state_priority(&self) -> i32 { 120 }

    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) { status.atk_sum *= 4; }

    fn post_defend_priority(&self) -> i32 { 110 }

    fn on_post_defend(
        &mut self,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) {
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action.as_deref().map(|name| format!("#{owner}") == name).unwrap_or(false);
        if *dmg <= 0 {
            return;
        }
        if debug_this {
            eprintln!(
                "[curse_post_defend] owner=#{owner} dmg={} prob={} before rc4=({}, {})",
                *dmg, self.prob, randomer.i, randomer.j,
            );
        }
        if randomer.r63() < self.prob as u32 {
            updates.add(RunUpdate::new("[诅咒]使伤害加倍", caster, owner, 0));
            *dmg *= self.multiply;
        }
        if debug_this {
            eprintln!(
                "[curse_post_defend] owner=#{owner} after dmg={} rc4=({}, {})",
                *dmg, randomer.i, randomer.j,
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_curse(caster: PlrId, target: PlrId, dmg: i32, _r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    let Some(target_plr) = storage.get_player(&target) else {
        return;
    };
    if target_plr.get_status().hp <= 0 || matches!(target_plr.player_type, PlayerType::Boss | PlayerType::Boost) {
        return;
    }
    updates.add(RunUpdate::new("[1]被[诅咒]了", caster, target, 60));
}

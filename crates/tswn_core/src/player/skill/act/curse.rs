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
            let target_alive_group_len = args.3.alive_group_len_containing(target);
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

    fn act(&mut self, targets: &[PlrId], _smart: bool, args: SkillArgs) {
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
        let core = {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get curse target from storage");
            target.attacked_core(atp, true, args.0, on_curse as OnDamageFunc, args.1, args.2, args.3)
        };
        if core.hit {
            on_curse(args.0, core.target, core.dmg, args.1, args.2, args.3);
            let target = args.3.just_get_player_mut(core.target).expect("cannot get curse target from storage");
            target.finish_damage(core.dmg, core.old_hp, args.0, args.1, args.2, args.3);
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

    // JS CurseState is inserted directly into y2 (post_defend) with default ga4()=10000.
    // Keep default ordering so Defend/Iron(ga4=4000)/Shield(ga4=6000) run before curse.
    fn post_defend_priority(&self) -> i32 { 10000 }

    fn on_post_defend(
        &mut self,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action.as_deref().map(|name| format!("#{owner}") == name).unwrap_or(false);
        if *dmg <= 0 {
            return false;
        }
        if debug_this {
            eprintln!(
                "[curse_post_defend] owner=#{owner} dmg={} prob={} before rc4=({}, {})",
                *dmg, self.prob, randomer.i, randomer.j,
            );
        }
        if randomer.r63() < self.prob as u32 {
            updates.emit(|| RunUpdate::new("[诅咒]使伤害加倍", caster, owner, 0));
            *dmg *= self.multiply;
        }
        if debug_this {
            eprintln!(
                "[curse_post_defend] owner=#{owner} after dmg={} rc4=({}, {})",
                *dmg, randomer.i, randomer.j,
            );
        }
        false
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_curse(caster: PlrId, target: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    let blocked = {
        let Some(target_plr) = storage.just_get_player_mut(target) else {
            return;
        };
        target_plr.get_status().hp <= 0
            || matches!(target_plr.player_type, PlayerType::Boss | PlayerType::Boost)
            || target_plr.check_immune("curse", r)
    };
    if blocked {
        return;
    }

    let charge_active = storage
        .get_player(&target)
        .map(|target_plr| target_plr.get_status().at_boost >= 3.0)
        .unwrap_or(false);

    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if let Some(state) = target_plr.get_state_mut::<CurseState>() {
        state.prob += 10;
        state.multiply += 1;
        state.owner = Some(caster);
        state.target = Some(target);
        if charge_active {
            state.prob += 10;
            state.multiply += 1;
        }
    } else {
        target_plr.set_state(CurseState {
            owner: Some(caster),
            target: Some(target),
            on_update_state: None,
            prob: 42 + if charge_active { 10 } else { 0 },
            multiply: 2 + if charge_active { 1 } else { 0 },
        });
    }
    updates.emit(|| RunUpdate::new("[1]被[诅咒]了", caster, target, 60));
}

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    ForcedAttackConfig, ForcedAttackScoreMode, ForcedAttackTargetDomain, OnDamageFunc, PlrId, StateTrait,
    skill::act::minion::is_combat_minion,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct BerserkSkill;

impl BerserkSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for BerserkSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for BerserkSkill {
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
        !target_plr.has_state::<BerserkState>() && !is_combat_minion(target_plr)
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
        let mut score = if smart {
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
        if target_plr.has_state::<BerserkState>() || target_plr.has_state::<crate::player::skill::charm::CharmState>() {
            score /= 1.2000000476837158;
        }
        score
    }

    fn act(&mut self, targets: &[PlrId], _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get berserk caster from storage")
            .get_at(true, args.1);
        args.2.add(RunUpdate::new("[0]使用[狂暴术]", args.0, target_id, 1));
        let core = {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get berserk target from storage");
            target.attacked_core(atp, true, args.0, on_berserk as OnDamageFunc, args.1, args.2, args.3)
        };
        if core.hit {
            on_berserk(args.0, core.target, core.dmg, args.1, args.2, args.3);
            let target = args.3.just_get_player_mut(core.target).expect("cannot get berserk target from storage");
            target.finish_damage(core.dmg, core.old_hp, args.0, args.1, args.2, args.3);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BerserkState {
    pub step: i32,
}

impl Default for BerserkState {
    fn default() -> Self { Self { step: 1 } }
}

impl StateTrait for BerserkState {
    fn meta_type(&self) -> i32 { -1 }

    fn action_mode_priority(&self) -> i32 { 100 }

    fn on_action_mode(&self, smart: bool, forced_attack: &mut Option<ForcedAttackConfig>) {
        *forced_attack = Some(ForcedAttackConfig {
            smart,
            target_domain: ForcedAttackTargetDomain::AllAlive,
            score_mode: ForcedAttackScoreMode::RandomAttract,
            use_mag: false,
            attack_scale: 1.2000000476837158,
            message: "[0]发起[狂暴攻击]",
        });
    }

    fn on_forced_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        _randomer: &mut RC4,
        updates: &mut RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        self.step -= 1;
        if self.step > 0 {
            return false;
        }
        if alive {
            updates.emit(RunUpdate::new_newline);
            updates.emit(|| RunUpdate::new("[1]从[狂暴]中解除", owner, owner, 0));
        }
        true
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_berserk(caster: PlrId, target: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    let charge_active = storage
        .get_player(&caster)
        .map(|caster_plr| caster_plr.get_status().at_boost >= 3.0)
        .unwrap_or(false);
    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if target_plr.get_status().hp <= 0 || target_plr.check_immune("berserk", r) {
        return;
    }
    if let Some(state) = target_plr.get_state_mut::<BerserkState>() {
        state.step += 1;
        if charge_active {
            state.step += 1;
        }
    } else {
        target_plr.state.set(BerserkState {
            step: if charge_active { 2 } else { 1 },
        });
        updates.emit(|| RunUpdate::new("[1]进入[狂暴]状态", caster, target, 60));
    }
}

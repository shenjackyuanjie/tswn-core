use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::act::minion::is_combat_minion,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct DisperseSkill;

impl DisperseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for DisperseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for DisperseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

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
        if smart && is_combat_minion(target_plr) && target_plr.get_status().hp > 100 {
            score *= 2.0;
        }
        score
    }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get disperse owner from storage")
            .get_at(true, args.1);
        let target_is_minion = args.3.get_player(&target_id).map(is_combat_minion).unwrap_or(false);
        args.2.add(RunUpdate::new("[0]使用[净化]", args.0, target_id, 20));

        // Note: Dart source has 'Dt.shield'/'Dt.iron' (string literal) instead of Dt.shield/Dt.iron (variable)
        // so these checks NEVER match in Dart/JS. Shield/Iron are NOT pre-cleared before damage.

        args.3
            .just_get_player_mut(target_id)
            .expect("cannot get disperse target from storage")
            .defned(
                if target_is_minion { atp * 2.0 } else { atp },
                true,
                args.0,
                on_disperse as OnDamageFunc,
                args.1,
                args.2,
                args.3,
            );
    }
}

fn on_disperse(caster: PlrId, target_id: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 0 {
        return;
    }
    // Clear positive meta states and collect messages (matching Dart's onDamage callback)
    // Dart iterates sorted meta keys, calling destroy() on each positive meta.
    // In Rust, skill-based clearing (accumulate, charge) and state-based clearing (haste, upgrade)
    // are separate mechanisms. We call both and combine messages.
    let target = storage.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
    let state_messages = target.clear_positive_states_with_messages();
    let skill_messages = target.skills.clear_positive_runtime((target_id, r, updates, storage));
    let mp = target.get_status().mp;
    if mp > 64 {
        target.set_mp(mp - 64);
    } else if mp > 32 {
        target.set_mp(0);
    } else {
        target.set_mp(mp - 32);
    }
    for message in skill_messages.iter().chain(state_messages.iter()) {
        updates.emit(RunUpdate::new_newline);
        updates.emit(|| RunUpdate::new(*message, caster, target_id, 0));
    }
}

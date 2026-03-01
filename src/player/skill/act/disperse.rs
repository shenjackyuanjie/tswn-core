use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::act::iron::IronState,
    skill::act::minion::is_combat_minion,
    skill::shield::ShieldState,
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
            let alive_group_count = {
                let mut group_heads = Vec::new();
                for id in args.3.all_player_ids() {
                    let alive = args.3.get_player(&id).map(|plr| plr.alive()).unwrap_or(false);
                    if !alive {
                        continue;
                    }
                    let Some(group) = args.3.group_containing(id) else {
                        continue;
                    };
                    let Some(head) = group.first() else {
                        continue;
                    };
                    if !group_heads.contains(head) {
                        group_heads.push(*head);
                    }
                }
                group_heads.len()
            };
            let target_alive_group_len = args
                .3
                .group_containing(target)
                .map(|group| {
                    group
                        .iter()
                        .filter(|id| args.3.get_player(id).map(|plr| plr.alive()).unwrap_or(false))
                        .count()
                })
                .unwrap_or(0);
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
        args.2.add(RunUpdate::new("[0]使用[净化]", args.0, target_id, 20));

        {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
            target.clear_state::<ShieldState>();
        }

        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get disperse target from storage")
            .defned(atp, true, args.0, on_disperse as OnDamageFunc, args.1, args.2, args.3);

        if dmg > 0 {
            let had_iron = args
                .3
                .get_player(&target_id)
                .map(|target| target.has_state::<IronState>())
                .unwrap_or(false);
            let target = args.3.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
            target.clear_positive_states();
            target.update_states();
            let mp = target.get_status().mp;
            if mp > 64 {
                target.set_mp(mp - 64);
            } else if mp > 32 {
                target.set_mp(0);
            } else {
                target.set_mp(mp - 32);
            }
            if had_iron {
                args.2.add(RunUpdate::new_newline());
                args.2.add(RunUpdate::new("[1]的[铁壁]被打消了", args.0, target_id, 20));
            }
        }
    }
}

fn on_disperse(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut RC4,
    _updates: &mut RunUpdates,
    _storage: &Arc<Storage>,
) {
}

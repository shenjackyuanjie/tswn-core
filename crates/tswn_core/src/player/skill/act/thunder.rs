use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, Player, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct ThunderSkill;

impl ThunderSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ThunderSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ThunderSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[雷击术]", args.0, target_id, 1));

        let mut agl = 100
            + args
                .3
                .get_player(&args.0)
                .expect("cannot get thunder owner from storage")
                .get_status()
                .agility;
        let mut _hit = false;
        let count = 3 + args.1.r3() as usize;
        for _ in 0..count {
            // JS: if (n.fx > p && !n.A && h.fx > p)
            // 施法者必须处于 active（存活且未冻结），目标必须仍然存活
            let caster_active = args.3.get_player(&args.0).map(|x| x.active()).unwrap_or(false);
            let target_alive = args.3.get_player(&target_id).map(|x| x.alive()).unwrap_or(false);
            if !caster_active || !target_alive {
                continue;
            }
            args.2.add(RunUpdate::new_newline());
            // JS: if (h.fx > 0 && !h.A && T.dodge(...))
            // 只有目标处于 active（存活且未冻结）时，才会触发闪避判定
            let target_active = args.3.get_player(&target_id).map(|x| x.active()).unwrap_or(false);
            if target_active {
                let target_dodge = args
                    .3
                    .get_player(&target_id)
                    .expect("cannot get thunder target from storage")
                    .get_status()
                    .agility
                    + args
                        .3
                        .get_player(&target_id)
                        .expect("cannot get thunder target from storage")
                        .get_status()
                        .resistance;
                if Player::dodge(agl, target_dodge, args.1) {
                    args.2.add(RunUpdate::new("[0][回避]了攻击", target_id, args.0, 0));
                    return;
                }
            }
            agl -= 10;
            let owner = args.3.get_player(&args.0).expect("cannot get thunder owner from storage");
            let atp = owner.get_at(true, args.1) * 0.36000001430511475;
            let update_pos = args.2.updates.len();
            let dmg = args
                .3
                .just_get_player_mut(target_id)
                .expect("cannot get thunder target from storage")
                .defned(atp, true, args.0, on_thunder as OnDamageFunc, args.1, args.2, args.3);
            if dmg > 0 {
                _hit = true;
            }
            if let Some(update) = args.2.updates.get_mut(update_pos) {
                update.delay0 = 300;
            }
        }
    }
}

fn on_thunder(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

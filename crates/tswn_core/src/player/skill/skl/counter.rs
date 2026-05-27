//! 反击被动技能实现。
//!
//! 受到攻击时按概率对攻击者发动反击，造成一定比例的回击伤害。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::{
    OnDamageFunc, PlrId,
    skill::act::charm::CharmState,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct CounterSkill {
    pub pending: bool,
    pub last_target: Option<PlrId>,
    pub last_updates_id: Option<u64>,
}

impl CounterSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for CounterSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CounterSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage_with_level(&mut self, level: u32, dmg: i32, caster: PlrId, args: SkillArgs) {
        let _ = dmg;
        let owner_wisdom = {
            let owner = args.3.get_player(&args.0).expect("cannot get counter owner from storage");
            owner.get_status().wisdom.clamp(0, 127) as u32
        };
        let owner_ally_group = args.3.get_player(&args.0).and_then(|owner| {
            owner
                .get_state::<CharmState>()
                .and_then(|charm| {
                    charm
                        .effective_team_idx
                        .and_then(|team_idx| args.3.get_group(team_idx).cloned())
                        .or_else(|| args.3.group_containing(charm.group_id).cloned())
                })
                .or_else(|| args.3.group_containing(args.0).cloned())
        });
        let caster_group = args.3.group_containing(caster).cloned();
        if owner_ally_group == caster_group && args.1.r63() < owner_wisdom {
            return;
        }
        let updates_id = args.2.id;
        if self.last_updates_id == Some(updates_id) {
            if self.pending && Some(caster) != self.last_target && args.1.r127() < level {
                self.last_target = Some(caster);
            }
            return;
        }
        self.last_updates_id = Some(updates_id);
        if args.1.r255() < level {
            self.last_target = Some(caster);
            self.pending = true;
            args.2.on_update_end.push(args.0);
        } else {
            self.pending = false;
            self.last_target = None;
        }
    }

    fn on_update_end_with_level(&mut self, _level: u32, args: SkillArgs) -> bool {
        if !self.pending || self.last_updates_id != Some(args.2.id) {
            return false;
        }
        self.pending = false;
        self.last_updates_id = None;
        let Some(target) = self.last_target.take() else {
            return false;
        };
        if !args.3.get_player(&target).map(|x| x.alive()).unwrap_or(false) {
            return false;
        }
        let atp = {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get counter owner from storage");
            if !owner.mp_ready(args.1) {
                return false;
            }
            owner.get_at(false, args.1)
        };
        args.2.add(crate::engine::update::RunUpdate::new_newline());
        args.2.add(crate::engine::update::RunUpdate::new(
            "[0]发起[反击][s_counter]",
            args.0,
            target,
            1,
        ));
        args.3
            .just_get_player_mut(target)
            .expect("cannot get counter target from storage")
            .attacked(atp, false, args.0, on_counter as OnDamageFunc, args.1, args.2, args.3);
        true
    }

    fn proc_kinds(&self) -> &'static [ProcKind] { &[ProcKind::PostDamage] }
}

fn on_counter(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

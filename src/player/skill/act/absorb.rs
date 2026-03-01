use std::cell::RefCell;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

thread_local! {
    static ABSORB_STORAGE: RefCell<Option<Arc<Storage>>> = RefCell::new(None);
}

#[derive(Debug, Clone, Default)]
pub struct AbsorbSkill;

impl AbsorbSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for AbsorbSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for AbsorbSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get absorb owner from storage");
            if owner.get_status().max_hp - owner.get_status().hp < 32 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get absorb owner from storage")
            .get_at(true, args.1)
            * 1.3;
        args.2.add(RunUpdate::new("[0]发起[吸血攻击]", args.0, target_id, 1));
        ABSORB_STORAGE.with(|ctx| {
            *ctx.borrow_mut() = Some(args.3.clone());
        });
        args.3
            .just_get_player_mut(target_id)
            .expect("cannot get absorb target from storage")
            .attacked(atp, true, args.0, on_absorb as OnDamageFunc, args.1, args.2, args.3);
        ABSORB_STORAGE.with(|ctx| {
            ctx.borrow_mut().take();
        });
    }
}

fn on_absorb(caster: PlrId, _target: PlrId, dmg: i32, _r: &mut RC4, updates: &mut RunUpdates) {
    if dmg <= 0 {
        return;
    }
    ABSORB_STORAGE.with(|ctx| {
        let Some(storage) = ctx.borrow().as_ref().cloned() else {
            return;
        };
        let Some(owner) = storage.just_get_player_mut(caster) else {
            return;
        };
        if !owner.alive() {
            return;
        }
        let (hp, max_hp) = {
            let status = owner.get_status();
            (status.hp, status.max_hp)
        };
        let healed = ((dmg + 1) / 2).min(max_hp - hp);
        if healed <= 0 {
            return;
        }
        owner.set_hp_raw((hp + healed).min(max_hp));
        updates.add(RunUpdate::new("[1]回复体力[2]点", caster, caster, healed as u32));
    });
}

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct PoisonSkill;

impl PoisonSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for PoisonSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for PoisonSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get poison caster from storage")
            .get_at(true, args.1);
        args.2.add(RunUpdate::new("[0][投毒]", args.0, target_id, 1));
        let _ = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get poison target from storage")
            .attacked(atp, true, args.0, on_poison as OnDamageFunc, args.1, args.2, args.3);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoisonState {
    pub caster: Option<PlrId>,
    pub target: Option<PlrId>,
    pub atp: f64,
    pub count: i32,
}

impl Default for PoisonState {
    fn default() -> Self {
        Self {
            caster: None,
            target: None,
            atp: 0.0,
            count: 4,
        }
    }
}

impl StateTrait for PoisonState {
    fn meta_type(&self) -> i32 { -1 }

    fn post_action_priority(&self) -> i32 { 150 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        if !alive {
            return false;
        }
        let Some(owner_magic) = storage.get_player(&owner).map(|player| player.get_status().magic) else {
            return false;
        };
        let atpp = self.atp * (1.0 + (self.count - 1) as f64 * 0.10000000149011612) / self.count as f64;
        self.atp -= atpp;
        let dmg = (atpp / (owner_magic + 64) as f64).ceil() as i32;
        self.count -= 1;
        updates.add(RunUpdate::new("[1][毒性发作]", self.caster.unwrap_or(owner), owner, 0));
        storage.just_get_player_mut(owner).expect("cannot get poison owner from storage").damage(
            dmg,
            self.caster.unwrap_or(owner),
            on_poison_tick as OnDamageFunc,
            randomer,
            updates,
            storage,
        );

        if self.count > 0 {
            return false;
        }
        if storage.get_player(&owner).map(|player| player.alive()).unwrap_or(false) {
            updates.add(RunUpdate::new_newline());
            updates.add(RunUpdate::new("[1]从[中毒]中解除", owner, owner, 0));
        }
        true
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_poison(caster: PlrId, target: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
    if dmg <= 4 {
        return;
    }

    let blocked = {
        let Some(target_plr) = storage.just_get_player_mut(target) else {
            return;
        };
        target_plr.get_status().hp <= 0 || target_plr.check_immune("poison", r)
    };
    if blocked {
        return;
    }

    let Some(caster_plr) = storage.get_player(&caster) else {
        return;
    };
    let poison_atp = caster_plr.get_at(true, r) * 1.2000000476837158;

    let Some(target_plr) = storage.just_get_player_mut(target) else {
        return;
    };
    if let Some(state) = target_plr.get_state_mut::<PoisonState>() {
        state.atp += poison_atp;
        state.count = 4;
        state.caster = Some(caster);
    } else {
        target_plr.set_state(PoisonState {
            caster: Some(caster),
            target: Some(target),
            atp: poison_atp,
            count: 4,
        });
    }
    updates.add(RunUpdate::new("[1][中毒]", caster, target, 60));
}

fn on_poison_tick(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

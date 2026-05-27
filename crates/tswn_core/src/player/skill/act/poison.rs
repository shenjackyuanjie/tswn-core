//! 中毒主动技能实现。
//!
//! 维护 `PoisonState`，对目标施加中毒效果，使其每回合持续流失生命值，
//! 毒素强度可叠加。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
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

    fn clear_updates_status(&self) -> bool {
        // JS 的 PoisonState.K() 只从 r2/x2 里移除中毒状态，不调用 F()/updateStates。
        // 解除中毒时不能顺手刷新疾走等状态，否则行动顺序会比 md5.js 提前。
        false
    }

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
        updates.emit(|| RunUpdate::new("[1][毒性发作]", self.caster.unwrap_or(owner), owner, 0));
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
            updates.emit(RunUpdate::new_newline);
            updates.emit(|| RunUpdate::new("[1]从[中毒]中解除", owner, owner, 0));
        }
        true
    }

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
        // JS 里 PoisonState 是直接挂到 r2/x2 链上，不会调用 F() 重新计算属性。
        // 中毒本身不会立刻改变 status；如果这里用 set_state()，会顺手刷新已经改过字段但
        // 还没生效的状态（例如蓄力后的 HasteState.faster），导致速度比 JS 提前变化。
        target_plr.set_state_no_update(PoisonState {
            caster: Some(caster),
            target: Some(target),
            atp: poison_atp,
            count: 4,
        });
    }
    updates.emit(|| RunUpdate::new("[1][中毒]", caster, target, 60));
}

fn on_poison_tick(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates, _storage: &Arc<Storage>) {}

#[cfg(test)]
mod tests {
    use crate::player::{
        Player,
        skill::{act::haste::HasteState, poison::PoisonState},
    };

    use super::*;

    #[test]
    fn poison_application_does_not_flush_pending_haste_change() {
        // 这个测试模拟“疾走已经被蓄力改成 faster=4，但 JS 还没调用 F()”的窗口。
        // 首次中毒只应注册 PoisonState，不能让 faster=4 提前写入当前 speed。
        let storage = Storage::new_arc();
        let mut target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        target.attr = [10, 10, 10, 10, 10, 10, 10, 100];
        target.init_values();
        target.update_states();
        let target_id = storage.just_insert_player(target);
        let caster_id = storage.just_insert_player(Player::new_from_namerena_raw("caster".to_string(), storage.clone()).unwrap());

        {
            let target = storage.just_get_player_mut(target_id).unwrap();
            target.set_state(HasteState {
                owner: Some(caster_id),
                target: Some(target_id),
                on_post_action: None,
                faster: 2,
                step: 3,
            });
            target.get_state_mut::<HasteState>().unwrap().faster = 4;
        }
        let speed_before_poison = storage.get_player(&target_id).unwrap().get_status().speed;

        let mut randomer = RC4::default();
        let mut updates = RunUpdates::new();
        on_poison(caster_id, target_id, 5, &mut randomer, &mut updates, &storage);

        let target = storage.get_player(&target_id).unwrap();
        assert!(target.has_state::<PoisonState>());
        assert_eq!(target.get_status().speed, speed_before_poison);
    }

    #[test]
    fn poison_clear_does_not_refresh_runtime_status() {
        let storage = Storage::new_arc();
        let mut target = Player::new_from_namerena_raw("target".to_string(), storage.clone()).unwrap();
        target.attr = [10, 10, 10, 10, 10, 10, 10, 100];
        target.init_values();
        target.update_states();
        let target_id = storage.just_insert_player(target);
        let caster_id = storage.just_insert_player(Player::new_from_namerena_raw("caster".to_string(), storage.clone()).unwrap());

        let target = storage.just_get_player_mut(target_id).unwrap();
        target.set_state(HasteState {
            owner: Some(caster_id),
            target: Some(target_id),
            on_post_action: None,
            faster: 2,
            step: 3,
        });
        target.get_state_mut::<HasteState>().unwrap().faster = 4;
        target.set_state_no_update(PoisonState {
            caster: Some(caster_id),
            target: Some(target_id),
            atp: 10.0,
            count: 1,
        });
        target.status.speed = 1234;

        // JS 的 PoisonState.K() 只注销中毒状态，不调用 F()/updateStates。
        // 如果清理时刷新属性，疾走的 pending faster 会提前生效，行动顺序会偏。
        target.clear_negative_states();

        assert!(!target.has_state::<PoisonState>());
        assert!(target.has_state::<HasteState>());
        assert_eq!(target.get_status().speed, 1234);
    }
}

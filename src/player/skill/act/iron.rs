use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, UpdateType};
use crate::player::{
    PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct IronSkill;

impl IronSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for IronSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for IronSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn prob(&self, level: u32, _smart: bool, args: SkillArgs) -> bool {
        let owner_has_iron = args
            .3
            .get_player(&args.0)
            .and_then(|owner| owner.get_state::<IronState>())
            .map(|state| state.step > 0 && state.protect > 0)
            .unwrap_or(false);
        if owner_has_iron {
            return false;
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        let owner = args.3.get_player(&args.0).expect("cannot get iron owner from storage");
        let owner_magic = owner.get_status().magic;
        let charge_active = owner.get_status().at_boost >= 3.0;

        let mut step = 3;
        let mut protect = 110 + owner_magic;
        if charge_active {
            step += 4;
            protect += 240 + owner_magic * 4;
        }

        args.2.add(RunUpdate::new("[0]发动[铁壁]", args.0, args.0, 60));
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get iron owner from storage");
        owner.set_state(IronState { protect, step });
        owner.set_move_point(owner.move_point() - 256);
        args.2.add(RunUpdate::new("[0]防御力大幅上升", args.0, args.0, 20));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IronState {
    pub protect: i32,
    pub step: i32,
}

impl StateTrait for IronState {
    fn meta_type(&self) -> i32 { self.step.max(0) }

    fn update_state_priority(&self) -> i32 { 4000 }

    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) {
        if self.step > 0 {
            status.attract *= 1.12;
        }
    }

    fn post_action_priority(&self) -> i32 { 205 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        _randomer: &mut RC4,
        updates: &mut crate::engine::update::RunUpdates,
        storage: &Arc<Storage>,
    ) -> bool {
        if self.step <= 0 {
            return true;
        }
        self.step -= 1;
        if self.step > 0 {
            return false;
        }

        if let Some(owner_plr) = storage.just_get_player_mut(owner) {
            owner_plr.set_move_point(owner_plr.move_point() - 128);
        }
        if alive {
            updates.add(RunUpdate::new_newline());
            updates.add(RunUpdate::new("[1]从[铁壁]中解除", owner, owner, 20));
        }
        true
    }

    fn post_defend_priority(&self) -> i32 { 10 }

    fn on_post_defend(
        &mut self,
        owner: PlrId,
        dmg: &mut i32,
        caster: PlrId,
        _randomer: &mut RC4,
        updates: &mut crate::engine::update::RunUpdates,
    ) {
        if self.step <= 0 || self.protect <= 0 {
            return;
        }
        if *dmg <= 0 {
            *dmg = 0;
            return;
        }
        if *dmg <= self.protect {
            let defended = updates
                .updates
                .iter()
                .rev()
                .find(|update| !matches!(update.update_type, UpdateType::NextLine))
                .map(|update| update.message == "[0][防御]" && update.caster == owner && update.target == caster)
                .unwrap_or(false);
            *dmg = if defended { 0 } else { 1 };
            return;
        }

        *dmg -= self.protect;
        self.protect = 0;
        self.step = 0;
        updates.add(RunUpdate::new_newline());
        // 铁壁被击破时应使用“被打消”文案；自然结束才是“从铁壁中解除”。
        updates.add(RunUpdate::new("[1]的[铁壁]被打消了", caster, owner, 20));
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

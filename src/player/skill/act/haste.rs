use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, StateTrait,
    skill::act::minion::is_combat_minion,
    skill::{SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct HasteSkill;

impl HasteSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for HasteSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HasteSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain_with_level(&self, _level: u32) -> SkillTargetDomain { SkillTargetDomain::AllyAlive }

    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        let Some(target_plr) = args.3.get_player(&target) else {
            return false;
        };
        if !target_plr.alive() {
            return false;
        }
        if !smart {
            return true;
        }
        if target_plr.get_status().hp < 60 {
            return false;
        }
        if let Some(haste) = target_plr.get_state::<HasteState>()
            && (haste.step + 1) * 60 > target_plr.get_status().hp
        {
            return false;
        }
        if is_combat_minion(target_plr) {
            return false;
        }
        true
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            let hp = target_plr.get_status().hp;
            let rate_hi_hp = if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            };
            let mut score = rate_hi_hp * target_plr.get_status().attr_sum as f64;
            if target_plr.has_state::<HasteState>() {
                score /= 4.0;
            }
            score
        } else {
            args.1.rFFFF() as f64
        }
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[加速术]", args.0, target_id, 60));
        let charge_active = args
            .3
            .get_player(&args.0)
            .expect("cannot get haste owner from storage")
            .get_status()
            .at_boost
            >= 3.0;

        let owner = args.3.just_get_player_mut(args.0).expect("cannot get haste owner from storage");
        owner.set_move_point(owner.move_point() + owner.get_status().speed);

        let target = args.3.just_get_player_mut(target_id).expect("cannot get haste target from storage");
        if let Some(state) = target.get_state_mut::<HasteState>() {
            state.step += 2;
        } else {
            target.set_state(HasteState {
                owner: Some(args.0),
                target: Some(target_id),
                on_post_action: None,
                faster: 2,
                step: 3,
            });
        }
        if charge_active {
            let state = target.get_state_mut::<HasteState>().expect("haste state should exist after apply");
            state.faster += 2;
            state.step += 2;
        }
        args.2.add(RunUpdate::new("[1]进入[疾走]状态", args.0, target_id, 60));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HasteState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub faster: i32,
    pub step: i32,
}

impl Default for HasteState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_post_action: None,
            faster: 2,
            step: 3,
        }
    }
}

impl StateTrait for HasteState {
    fn meta_type(&self) -> i32 { 1 }

    fn cancel_message(&self, alive: bool) -> Option<&'static str> {
        if alive { Some("[1]从[疾走]中解除") } else { None }
    }

    fn update_state_priority(&self) -> i32 { 100 }

    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) { status.speed *= self.faster; }

    fn post_action_priority(&self) -> i32 { 200 }

    fn on_post_action(
        &mut self,
        owner: PlrId,
        alive: bool,
        _randomer: &mut crate::rc4::RC4,
        updates: &mut crate::engine::update::RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) -> bool {
        self.step -= 1;
        if self.step > 0 {
            return false;
        }
        if alive {
            updates.add(RunUpdate::new_newline());
            updates.add(RunUpdate::new("[1]从[疾走]中解除", owner, owner, 0));
        }
        true
    }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

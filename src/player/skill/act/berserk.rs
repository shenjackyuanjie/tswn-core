use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
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
        !target_plr.has_state::<BerserkState>()
    }

    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        let mut score = if smart {
            target_plr.get_status().attract
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        };
        if target_plr.has_state::<BerserkState>() || target_plr.has_state::<crate::player::skill::charm::CharmState>() {
            score /= 1.2;
        }
        score
    }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
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
        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get berserk target from storage")
            .attacked(atp, true, args.0, on_berserk as OnDamageFunc, args.1, args.2, args.3);
        if dmg <= 0 {
            return;
        }
        let target = args.3.just_get_player_mut(target_id).expect("cannot get berserk target from storage");
        if !target.alive() || target.check_immune(state_tag::<BerserkState>(), args.1) {
            return;
        }
        if let Some(state) = target.get_state_mut::<BerserkState>() {
            state.step += 1;
        } else {
            target.set_state(BerserkState { step: 1 });
            args.2.add(RunUpdate::new("[1]进入[狂暴]状态", args.0, target_id, 60));
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

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_berserk(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

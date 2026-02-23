use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct IceSkill;

impl IceSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for IceSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for IceSkill {
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
            .expect("cannot get ice caster from storage")
            .get_at(true, args.1)
            * 0.7;
        args.2.add(RunUpdate::new("[0]使用[冰冻术]", args.0, target_id, 1));
        let target = args.3.just_get_player_mut(target_id).expect("cannot get ice target from storage");
        let dmg = target.attacked(atp, true, args.0, on_ice as OnDamageFunc, args.1, args.2, args.3);
        if dmg <= 0 || !target.alive() || target.check_immune(state_tag::<IceState>(), args.1) {
            return;
        }
        if let Some(state) = target.get_state_mut::<IceState>() {
            state.frozen_step += 1024;
        } else {
            target.set_state(IceState {
                target: Some(target_id),
                pre_step_impl: None,
                frozen_step: 1024,
            });
        }
        args.2.add(RunUpdate::new("[1]被[冰冻]了", args.0, target_id, 40));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IceState {
    pub target: Option<PlrId>,
    pub pre_step_impl: Option<()>,
    pub frozen_step: i32,
}

impl Default for IceState {
    fn default() -> Self {
        Self {
            target: None,
            pre_step_impl: None,
            frozen_step: 1024,
        }
    }
}

impl StateTrait for IceState {
    fn meta_type(&self) -> i32 { -1 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_ice(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

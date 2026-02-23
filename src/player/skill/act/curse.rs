use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
    state_tag,
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct CurseSkill;

impl CurseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CurseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CurseSkill {
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
            .expect("cannot get curse caster from storage")
            .get_at(true, args.1);
        args.2.add(RunUpdate::new("[0]使用[诅咒]", args.0, target_id, 1));
        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get curse target from storage")
            .attacked(atp, true, args.0, on_curse as OnDamageFunc, args.1, args.2, args.3);
        if dmg <= 0 {
            return;
        }
        let target = args.3.just_get_player_mut(target_id).expect("cannot get curse target from storage");
        if !target.alive() || target.check_immune(state_tag::<CurseState>(), args.1) {
            return;
        }
        if let Some(state) = target.get_state_mut::<CurseState>() {
            state.prob += 10;
            state.multiply += 1;
            state.owner = Some(args.0);
        } else {
            target.set_state(CurseState {
                owner: Some(args.0),
                target: Some(target_id),
                on_update_state: None,
                prob: 42,
                multiply: 2,
            });
        }
        args.2.add(RunUpdate::new("[1]被[诅咒]了", args.0, target_id, 60));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurseState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_update_state: Option<()>,
    pub prob: i32,
    pub multiply: i32,
}

impl Default for CurseState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_update_state: None,
            prob: 42,
            multiply: 2,
        }
    }
}

impl StateTrait for CurseState {
    fn meta_type(&self) -> i32 { -1 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_curse(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

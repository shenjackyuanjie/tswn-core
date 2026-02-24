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
        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get poison target from storage")
            .attacked(atp, true, args.0, on_poison as OnDamageFunc, args.1, args.2, args.3);
        if dmg <= 4 {
            return;
        }
        let poison_atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get poison caster from storage")
            .get_at(true, args.1)
            * 1.2;
        let target = args.3.just_get_player_mut(target_id).expect("cannot get poison target from storage");
        if !target.alive() || target.check_immune(state_tag::<PoisonState>(), args.1) {
            return;
        }
        if let Some(state) = target.get_state_mut::<PoisonState>() {
            state.atp += poison_atp;
            state.count = 4;
            state.caster = Some(args.0);
        } else {
            target.set_state(PoisonState {
                caster: Some(args.0),
                target: Some(target_id),
                atp: poison_atp,
                count: 4,
            });
        }
        args.2.add(RunUpdate::new("[1][中毒]", args.0, target_id, 60));
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

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_poison(caster: PlrId, target: PlrId, dmg: i32, r: &mut RC4, updates: &mut RunUpdates) {
    let _ = (caster, target, dmg, r, updates);
}

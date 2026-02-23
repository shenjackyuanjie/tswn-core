use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::shield::ShieldState,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct DisperseSkill;

impl DisperseSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for DisperseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for DisperseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get disperse owner from storage")
            .get_at(true, args.1);
        args.2.add(RunUpdate::new("[0]使用[净化]", args.0, target_id, 20));

        {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
            target.clear_state::<ShieldState>();
        }

        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get disperse target from storage")
            .attacked(atp, true, args.0, on_disperse as OnDamageFunc, args.1, args.2, args.3);

        if dmg > 0 {
            let target = args.3.just_get_player_mut(target_id).expect("cannot get disperse target from storage");
            target.clear_positive_states();
            let mp = target.get_status().mp;
            if mp > 64 {
                target.set_mp(mp - 64);
            } else if mp > 32 {
                target.set_mp(0);
            } else {
                target.set_mp((mp - 32).max(0));
            }
        }
    }
}

fn on_disperse(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

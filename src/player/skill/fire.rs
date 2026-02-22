use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId, StateValue,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct FireSkill;

impl FireSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for FireSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(FireSkill::new()) }
}

impl SkillTrait for FireSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];

        let fire_mag = args
            .3
            .get_player(&target_id)
            .expect("cannot get target from storage")
            .fire_mag();
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get owner from storage")
            .get_at(true, args.1)
            * (1.5 + fire_mag);

        args.2.add(RunUpdate::new("[0]使用[火球术]", args.0, target_id, 1));

        let target = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get mutable target in storage");
        let dmg = target.attacked(atp, true, args.0, on_fire as OnDamageFunc, args.1, args.2, args.3);

        // 参考 dart: onFire(dmg > 0 && !target.dead) => fireMag += 0.5
        if dmg > 0 && target.alive() && !target.check_immune(StateValue::fire_tag(), args.1) {
            target.add_fire_mag(0.5);
        }
    }
}

fn on_fire(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

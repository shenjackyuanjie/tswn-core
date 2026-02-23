use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlrId,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct AbsorbSkill;

impl AbsorbSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for AbsorbSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for AbsorbSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get absorb owner from storage");
            if owner.get_status().max_hp - owner.get_status().hp < 32 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get absorb owner from storage")
            .get_at(true, args.1)
            * 1.3;
        args.2.add(RunUpdate::new("[0]发起[吸血攻击]", args.0, target_id, 1));
        let dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get absorb target from storage")
            .attacked(atp, true, args.0, on_absorb as OnDamageFunc, args.1, args.2, args.3);
        if dmg > 0 {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get absorb owner from storage");
            let healed = ((dmg + 1) / 2).min(owner.get_status().max_hp - owner.get_status().hp);
            if healed > 0 {
                owner.damage(-healed, args.0, on_absorb as OnDamageFunc, args.1, args.2, args.3);
            }
        }
    }
}

fn on_absorb(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut RunUpdates) {}

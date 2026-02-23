use crate::engine::update::RunUpdate;
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct ZombieSkill {
    pub raised: i32,
}

impl ZombieSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ZombieSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ZombieSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool {
        if args.1.r63() >= 24 {
            return false;
        }
        if args.3.get_player(&args.0).is_none() {
            args.3
                .just_get_player_mut(target)
                .expect("cannot get zombie target from storage")
                .set_state(ZombieState {
                    target: Some(target),
                    count: 1,
                });
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[0][召唤亡灵]", args.0, target, 60));
            return true;
        }
        let target_status = *args
            .3
            .get_player(&target)
            .expect("cannot get zombie target from storage")
            .get_status();
        self.raised += 1;
        {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get zombie owner from storage");
            owner.set_state(ZombieState {
                target: Some(target),
                count: self.raised,
            });
            owner.set_move_point(owner.move_point() + 256);
            owner.set_mp(owner.mp() + (target_status.wisdom / 4).max(8));
            owner.damage(-(target_status.max_hp / 6).max(4), args.0, on_zombie as OnDamageFunc, args.1, args.2, args.3);
        }
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][召唤亡灵]", args.0, target, 60));
        true
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.raised <= 0 {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get zombie owner from storage");
        owner.add_attack(self.raised * 2);
        owner.add_magic(self.raised * 2);
        owner.add_resistance(self.raised * 2);
        owner.add_max_hp(self.raised * 6);
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill, ProcKind::UpdateState] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZombieState {
    pub target: Option<PlrId>,
    pub count: i32,
}

impl StateTrait for ZombieState {
    fn meta_type(&self) -> i32 { 1 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_zombie(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut crate::engine::update::RunUpdates) {}

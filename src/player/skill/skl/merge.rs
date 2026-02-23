use crate::engine::update::RunUpdate;
use crate::player::{
    OnDamageFunc, PlrId, StateTrait,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};
use crate::rc4::RC4;

#[derive(Debug, Clone, Default)]
pub struct MergeSkill {
    pub bonus_attack: i32,
    pub bonus_defense: i32,
    pub bonus_speed: i32,
    pub bonus_agility: i32,
    pub bonus_magic: i32,
    pub bonus_resistance: i32,
    pub bonus_wisdom: i32,
    pub bonus_hp: i32,
}

impl MergeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for MergeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for MergeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool {
        if args.1.r63() >= 32 {
            return false;
        }
        if args.3.get_player(&args.0).is_none() {
            args.3
                .just_get_player_mut(target)
                .expect("cannot get merge target from storage")
                .set_state(MergeState {
                    target: Some(target),
                    stacks: 1,
                });
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[0][吞噬]了[1]", args.0, target, 60));
            return true;
        }
        let target_status = args
            .3
            .get_player(&target)
            .expect("cannot get merge target from storage")
            .get_status()
            .to_owned();
        self.bonus_attack += (target_status.attack / 6).max(1);
        self.bonus_defense += (target_status.defense / 6).max(1);
        self.bonus_speed += (target_status.speed / 8).max(1);
        self.bonus_agility += (target_status.agility / 6).max(1);
        self.bonus_magic += (target_status.magic / 6).max(1);
        self.bonus_resistance += (target_status.resistance / 6).max(1);
        self.bonus_wisdom += (target_status.wisdom / 8).max(1);
        self.bonus_hp += (target_status.max_hp / 4).max(8);

        {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get merge owner from storage");
            owner.set_state(MergeState {
                target: Some(target),
                stacks: owner.get_state::<MergeState>().map(|x| x.stacks + 1).unwrap_or(1),
            });
            owner.set_mp(owner.mp() + (target_status.mp / 2).max(8));
            owner.set_move_point(owner.move_point() + (target_status.move_point / 4).max(128));
            owner.damage(-(target_status.max_hp / 4).max(8), args.0, on_merge as OnDamageFunc, args.1, args.2, args.3);
        }
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][吞噬]了[1]", args.0, target, 60));
        true
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.bonus_hp <= 0 {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get merge owner from storage");
        owner.add_attack(self.bonus_attack);
        owner.add_defense(self.bonus_defense);
        owner.add_speed(self.bonus_speed);
        owner.add_agility(self.bonus_agility);
        owner.add_magic(self.bonus_magic);
        owner.add_resistance(self.bonus_resistance);
        owner.add_wisdom(self.bonus_wisdom);
        owner.add_max_hp(self.bonus_hp);
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill, ProcKind::UpdateState] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MergeState {
    pub target: Option<PlrId>,
    pub stacks: i32,
}

impl StateTrait for MergeState {
    fn meta_type(&self) -> i32 { 1 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

fn on_merge(_caster: PlrId, _target: PlrId, _dmg: i32, _r: &mut RC4, _updates: &mut crate::engine::update::RunUpdates) {}

use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct UpgradeSkill {
    pub on_update_state: Option<()>,
}

impl Default for UpgradeSkill {
    fn default() -> Self { Self { on_update_state: None } }
}

impl UpgradeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for UpgradeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for UpgradeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage_with_level(&mut self, level: u32, _dmg: i32, _caster: PlrId, args: SkillArgs) {
        if level == 0 || self.on_update_state.is_some() {
            return;
        }
        let owner = args.3.get_player(&args.0).expect("cannot get upgrade owner from storage");
        let owner_alive = owner.alive();
        let owner_hp = owner.get_status().hp;
        let move_point = owner.move_point();
        let mut minhp = 16;
        if level > 63 {
            minhp += (level - 63) as i32;
        }
        if owner_alive && owner_hp < minhp + args.1.r63() as i32 && args.1.r63() < level {
            self.on_update_state = Some(());
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[0]做出[垂死]抗争", args.0, args.0, 60));
            args.2.add(RunUpdate::new("[0]所有属性上升", args.0, args.0, 30));
            args.3
                .just_get_player_mut(args.0)
                .expect("cannot get upgrade owner from storage")
                .set_move_point(move_point + 400);
        }
    }

    fn update_state(&mut self, args: SkillArgs) {
        if self.on_update_state.is_none() {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get upgrade owner from storage");
        owner.add_attack(30);
        owner.add_defense(30);
        owner.add_agility(30);
        owner.add_magic(30);
        owner.add_resistance(30);
        owner.add_speed(20);
        owner.add_wisdom(20);
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDamage, ProcKind::UpdateState] }
}

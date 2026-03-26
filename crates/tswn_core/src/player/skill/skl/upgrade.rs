use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, StateTrait,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct UpgradeSkill;

impl UpgradeSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for UpgradeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for UpgradeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage_with_level(&mut self, level: u32, _dmg: i32, _caster: PlrId, args: SkillArgs) {
        let owner = args.3.get_player(&args.0).expect("cannot get upgrade owner from storage");
        let debug_target = std::env::var("TSWN_DEBUG_UPGRADE").ok();
        let debug_this = debug_target.as_deref().map(|name| owner.id_name() == name).unwrap_or(false);
        if level == 0 || owner.has_state::<UpgradeState>() {
            return;
        }
        let owner_alive = owner.alive();
        let owner_hp = owner.get_status().hp;
        let move_point = owner.move_point();
        let mut minhp = 16;
        if level > 63 {
            minhp += (level - 63) as i32;
        }
        if !owner_alive {
            return;
        }
        if owner_hp <= 0 {
            return;
        }
        let roll_hp = args.1.r63() as i32;
        let hp_gate = owner_hp < minhp + roll_hp;
        if debug_this {
            eprintln!(
                "[upgrade_post_damage] owner={} hp={} minhp={} level={} roll_hp={} hp_gate={} rc4=({}, {})",
                owner.id_name(),
                owner_hp,
                minhp,
                level,
                roll_hp,
                hp_gate,
                args.1.i,
                args.1.j
            );
        }
        if !hp_gate {
            return;
        }
        let roll_level = args.1.r63();
        if debug_this {
            eprintln!(
                "[upgrade_post_damage] owner={} roll_level={} rc4=({}, {})",
                owner.id_name(),
                roll_level,
                args.1.i,
                args.1.j
            );
        }
        if roll_level < level {
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[0]做出[垂死]抗争", args.0, args.0, 60));
            args.2.add(RunUpdate::new("[0]所有属性上升", args.0, args.0, 0));
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get upgrade owner from storage");
            owner.set_state(UpgradeState { target: Some(args.0) });
            owner.set_move_point(move_point + 400);
        }
    }

    fn update_state(&mut self, args: SkillArgs) {
        if !args.3.get_player(&args.0).map(|owner| owner.has_state::<UpgradeState>()).unwrap_or(false) {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UpgradeState {
    pub target: Option<PlrId>,
}

impl StateTrait for UpgradeState {
    fn meta_type(&self) -> i32 { 1 }

    fn clear_positive_priority(&self) -> i32 { 500 }

    fn cancel_message(&self, alive: bool) -> Option<&'static str> {
        if alive { Some("[1]的[垂死]属性被打消") } else { None }
    }

    fn update_state_priority(&self) -> i32 { 130 }

    fn apply_update_state(&self, status: &mut crate::player::PlayerStatus) {
        status.attack += 30;
        status.defense += 30;
        status.agility += 30;
        status.magic += 30;
        status.resistance += 30;
        status.speed += 20;
        status.wisdom += 20;
    }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

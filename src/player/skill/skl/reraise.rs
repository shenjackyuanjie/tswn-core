use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct ReraiseSkill {
    pub sort_id: f64,
}

impl Default for ReraiseSkill {
    fn default() -> Self { Self { sort_id: 10.0 } }
}

impl ReraiseSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ReraiseSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ReraiseSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn die_with_level(&mut self, level: &mut u32, _oldhp: i32, _caster: PlrId, args: SkillArgs) -> bool {
        if args.1.r127() >= *level {
            return false;
        }
        // Dart: level = (level+1) ~/ 2
        *level = (*level + 1) / 2;
        let hp = args.1.r16() as i32;
        args.2.add(RunUpdate::new("[0]使用[护身符]抵挡了一次死亡", args.0, args.0, 80));
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get reraise owner from storage")
            .revive_with_hp(hp);
        args.2.add(RunUpdate::new("[1]回复体力[2]点", args.0, args.0, hp as u32));
        true
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDeath] }
}

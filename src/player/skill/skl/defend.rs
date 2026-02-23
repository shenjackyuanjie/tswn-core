use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone)]
pub struct DefendSkill {
    pub sort_id: f64,
}

impl Default for DefendSkill {
    fn default() -> Self { Self { sort_id: 2000.0 } }
}

impl DefendSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for DefendSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for DefendSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_defend_with_level(
        &mut self,
        level: u32,
        dmg: i32,
        caster: PlrId,
        _on_damage: &crate::player::OnDamageFunc,
        args: SkillArgs,
    ) -> i32 {
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get defend owner from storage");
        if args.1.r255() < level && owner.mp_ready(args.1) {
            args.2.add(RunUpdate::new("[0][防御]", args.0, caster, 40));
            return dmg / 2;
        }
        dmg
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDefend] }
}

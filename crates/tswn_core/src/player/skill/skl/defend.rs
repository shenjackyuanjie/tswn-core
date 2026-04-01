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
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get defend owner from storage");
        if debug_this {
            eprintln!(
                "[defend_post_defend] owner={} dmg={} level={} before rc4=({}, {})",
                owner.id_name(),
                dmg,
                level,
                args.1.i,
                args.1.j,
            );
        }
        if args.1.r255() < level && owner.mp_ready(args.1) {
            args.2.add(RunUpdate::new("[0][防御]", args.0, caster, 40));
            if debug_this {
                eprintln!(
                    "[defend_post_defend] owner={} triggered rc4=({}, {})",
                    owner.id_name(),
                    args.1.i,
                    args.1.j,
                );
            }
            return dmg / 2;
        }
        if debug_this {
            eprintln!(
                "[defend_post_defend] owner={} not_triggered rc4=({}, {})",
                owner.id_name(),
                args.1.i,
                args.1.j,
            );
        }
        dmg
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostDefend] }

    fn post_defend_priority(&self) -> i32 { self.sort_id as i32 }
}

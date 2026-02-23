use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
    skill::shield::ShieldState,
};

#[derive(Debug, Clone)]
pub struct ProtectSkill {
    pub allow_sneak: bool,
    pub protect_to: Option<PlrId>,
}

impl Default for ProtectSkill {
    fn default() -> Self {
        Self {
            allow_sneak: false,
            protect_to: None,
        }
    }
}

impl ProtectSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for ProtectSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ProtectSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_action_with_level(&mut self, level: u32, args: SkillArgs) {
        self.protect_to = Some(args.0);
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get protect owner from storage");
        if args.1.r127() >= level || !owner.mp_ready(args.1) {
            return;
        }
        let shield = (level as i32 / 8 + 1).max(1);
        if let Some(state) = owner.get_state_mut::<ShieldState>() {
            state.shield += shield;
        } else {
            owner.set_state(ShieldState {
                sort_id: 6000.0,
                target: Some(args.0),
                shield,
            });
        }
        args.2.add(RunUpdate::new("[0][守护][1]", args.0, args.0, 40));
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostAction] }
}

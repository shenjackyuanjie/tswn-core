use crate::player::{
    PlrId, StateTrait,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct ShieldSkill;

impl ShieldSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ShieldSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ShieldSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn pre_action(&mut self, args: SkillArgs) {
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get shield owner from storage");
        if let Some(state) = owner.get_state_mut::<ShieldState>() {
            state.shield += args.1.r7() as i32 + 1;
        } else {
            owner.set_state(ShieldState {
                sort_id: 6000.0,
                target: Some(args.0),
                shield: args.1.r7() as i32 + 1,
            });
        }
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PreAction] }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldState {
    pub sort_id: f64,
    pub target: Option<PlrId>,
    pub shield: i32,
}

impl Default for ShieldState {
    fn default() -> Self {
        Self {
            sort_id: 6000.0,
            target: None,
            shield: 0,
        }
    }
}

impl StateTrait for ShieldState {
    fn meta_type(&self) -> i32 { if self.shield > 0 { 1 } else { 0 } }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

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

    fn pre_action_with_level(&mut self, level: u32, args: SkillArgs) {
        if level == 0 {
            return;
        }
        let owner = args.3.just_get_player_mut(args.0).expect("cannot get shield owner from storage");
        let state = if let Some(state) = owner.get_state_mut::<ShieldState>() {
            state
        } else {
            owner.set_state(ShieldState {
                sort_id: 6000.0,
                target: Some(args.0),
                shield: 0,
            });
            owner.get_state_mut::<ShieldState>().expect("cannot get shield state from owner")
        };
        if level as i32 >= state.shield {
            let add = args.1.next_i32((1 + (level as i32 * 3 / 4)).max(1)) + 1;
            state.shield += add;
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

    fn post_defend_priority(&self) -> i32 { self.sort_id as i32 }

    fn on_post_defend(
        &mut self,
        _owner: PlrId,
        dmg: &mut i32,
        _caster: PlrId,
        _randomer: &mut crate::rc4::RC4,
        _updates: &mut crate::engine::update::RunUpdates,
        _storage: &std::sync::Arc<crate::engine::storage::Storage>,
    ) {
        if self.shield <= 0 {
            return;
        }
        if *dmg > self.shield {
            self.shield = 0;
        } else {
            self.shield -= *dmg;
            *dmg = 0;
        }
    }



    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

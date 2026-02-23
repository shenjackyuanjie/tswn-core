use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, StateTrait,
    skill::{SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct HasteSkill;

impl HasteSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for HasteSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for HasteSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        args.2.add(RunUpdate::new("[0]使用[加速术]", args.0, target_id, 60));

        let owner = args
            .3
            .just_get_player_mut(args.0)
            .expect("cannot get haste owner from storage");
        owner.set_move_point(owner.move_point() + owner.get_status().speed);

        let target = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get haste target from storage");
        if let Some(state) = target.get_state_mut::<HasteState>() {
            state.step += 4;
        } else {
            target.set_state(HasteState {
                owner: Some(args.0),
                target: Some(target_id),
                on_post_action: None,
                faster: 2,
                step: 3,
            });
        }
        args.2.add(RunUpdate::new("[1]进入[疾走]状态", args.0, target_id, 60));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HasteState {
    pub owner: Option<PlrId>,
    pub target: Option<PlrId>,
    pub on_post_action: Option<()>,
    pub faster: i32,
    pub step: i32,
}

impl Default for HasteState {
    fn default() -> Self {
        Self {
            owner: None,
            target: None,
            on_post_action: None,
            faster: 2,
            step: 3,
        }
    }
}

impl StateTrait for HasteState {
    fn meta_type(&self) -> i32 { 1 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}


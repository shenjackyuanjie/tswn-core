use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, StateTrait,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct MergeSkill;

impl MergeSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for MergeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for MergeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { self.kill_with_level(32, target, args) }

    fn kill_with_level(&mut self, level: u32, target: PlrId, args: SkillArgs) -> bool {
        if args.1.r63() >= level {
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
        let target_attr = args.3.get_player(&target).expect("cannot get merge target from storage").attr;
        let target_skill_levels = {
            let target_plr = args.3.get_player(&target).expect("cannot get merge target from storage");
            target_plr
                .skills
                .skill
                .iter()
                .map(|key| target_plr.skills.skill_by_id(*key).level())
                .collect::<Vec<u32>>()
        };
        let target_mp = args.3.get_player(&target).expect("cannot get merge target from storage").mp();
        let target_move_point = args.3.get_player(&target).expect("cannot get merge target from storage").move_point();

        let mut merged = false;
        let (transfer_mp, transfer_move_point) = {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get merge owner from storage");
            for (idx, val) in target_attr.iter().enumerate() {
                if *val > owner.attr[idx] {
                    owner.attr[idx] = *val;
                    merged = true;
                }
            }
            let upper = owner.skills.skill.len().min(target_skill_levels.len());
            for (idx, target_level) in target_skill_levels.into_iter().take(upper).enumerate() {
                let owner_skill = owner.skills.skill_by_idx_mut(idx);
                if target_level > owner_skill.level() {
                    owner_skill.set_level(target_level);
                    merged = true;
                }
            }
            let transfer_mp = target_mp > owner.mp();
            if transfer_mp {
                owner.set_mp(target_mp);
            }
            let transfer_move_point = target_move_point > owner.move_point();
            if transfer_move_point {
                owner.set_move_point(owner.move_point() + target_move_point);
            }
            let next_stack = owner.get_state::<MergeState>().map(|x| x.stacks + 1).unwrap_or(1);
            if merged {
                owner.set_state(MergeState {
                    target: Some(target),
                    stacks: next_stack,
                });
                owner.update_states();
                owner.skills.update_proc();
            }
            (transfer_mp, transfer_move_point)
        };
        {
            let target_plr = args.3.just_get_player_mut(target).expect("cannot get merge target from storage");
            if transfer_mp {
                target_plr.set_mp(0);
            }
            if transfer_move_point {
                target_plr.set_move_point(0);
            }
        }
        if !merged {
            return false;
        }
        {
            let target_plr = args.3.just_get_player_mut(target).expect("cannot get merge target from storage");
            target_plr.set_state(MergeState {
                target: Some(target),
                stacks: 1,
            });
        }
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][吞噬]了[1]", args.0, target, 60));
        args.2.add(RunUpdate::new("[0]属性上升", args.0, target, 20));
        true
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill] }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MergeState {
    pub target: Option<PlrId>,
    pub stacks: i32,
}

impl StateTrait for MergeState {
    fn meta_type(&self) -> i32 { 0 }

    fn as_any(&self) -> &dyn std::any::Any { self }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn clone_box(&self) -> Box<dyn StateTrait> { Box::new(*self) }
}

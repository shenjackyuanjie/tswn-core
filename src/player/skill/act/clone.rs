use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId, PlayerStateStore, PlayerType,
    skill::{SkillArgs, SkillExt, SkillTrait},
};
use super::minion::{MinionKind, MinionRuntimeState};

#[derive(Debug, Clone, Default)]
pub struct CloneSkill;

impl CloneSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for CloneSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CloneSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn post_act_level(&self, level: u32) -> u32 { ((level as f64) * 0.75).ceil().max(1.0) as u32 }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
            if owner.get_status().hp < 80 {
                return false;
            }
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        args.2.add(RunUpdate::new("[0]使用[分身]", args.0, args.0, 60));
        {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get clone owner from storage");
            for i in 0..6 {
                owner.attr[i] = ((owner.attr[i] as f64) * 0.6).ceil() as u32;
            }
            owner.attr[7] = ((owner.attr[7] as f64) * 0.5).ceil() as u32;
            owner.status.hp = ((owner.status.hp as f64) * 0.5).ceil() as i32;
            owner.status.hp = owner.status.hp.clamp(1, owner.status.max_hp.max(1));
            owner.update_states();
        }

        let owner_snapshot = args
            .3
            .get_player(&args.0)
            .expect("cannot get clone owner from storage")
            .clone();
        let mut cloned = owner_snapshot.clone();
        let clone_suffix = args.1.r255();
        cloned.name = format!("{}?clone{}", owner_snapshot.id_name(), clone_suffix);
        cloned.id = args.3.new_plr_id();
        cloned.player_type = PlayerType::Clone;
        cloned.sort_int = args.1.rFFFFFF() as i32;
        cloned.state = PlayerStateStore::default();
        cloned.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Clone,
        });
        cloned.status.move_point = args.1.r255() as i32 * 4 + 600;
        cloned.status.hp = owner_snapshot.get_status().hp.max(1);
        cloned.status.set_alive(true);
        cloned.status.set_frozen(false);

        let mut cloned_level = self.post_act_level(level);
        if owner_snapshot.get_status().hp + owner_snapshot.get_status().magic < args.1.r255() as i32 {
            cloned_level = (cloned_level >> 1) + 1;
        }
        if cloned.skills.skill.len() > 23 {
            cloned.skills.skill_by_idx_mut(23).set_level(cloned_level.max(1));
        }
        cloned.skills.update_proc();

        let cloned_id = cloned.as_ptr();
        args.3.queue_spawn(args.0, cloned);
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get clone owner from storage")
            .set_move_point(args.1.r255() as i32 * 4 + 1024);
        args.2.add(RunUpdate::new("出现一个新的[1]", args.0, cloned_id, 20));
    }
}

use crate::engine::update::RunUpdate;
use crate::player::{
    PlayerStateStore, PlayerType, PlrId,
    skill::store::SkillStorage,
    skill::{Skill, SkillArgs, SkillExt, SkillTrait},
};

use super::minion::{MinionKind, MinionRuntimeState};

#[derive(Debug, Clone, Default)]
pub struct ShadowSkill;

impl ShadowSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for ShadowSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for ShadowSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn post_act_level(&self, level: u32) -> u32 { ((level as f64) * 0.75).ceil().max(1.0) as u32 }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get shadow owner from storage");
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
        args.2.add(RunUpdate::new("[0]使用[幻术]", args.0, args.0, 60));
        let owner = args.3.get_player(&args.0).expect("cannot get shadow owner from storage").clone();
        let mut shadow = owner.clone();
        shadow.id = args.3.new_plr_id();
        shadow.name = format!("{}?shadow", owner.id_name());
        shadow.player_type = PlayerType::Clone;
        shadow.sort_int = 0;
        shadow.state = PlayerStateStore::default();
        shadow.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Shadow,
        });
        shadow.status.max_hp = (owner.get_status().max_hp / 2).max(1);
        shadow.status.hp = shadow.status.max_hp;
        shadow.status.set_alive(true);
        shadow.status.set_frozen(false);

        let mut skills = SkillStorage::new();
        skills.add_skill(Skill::new_with_id((level / 2 + 36).min(255), 10));
        shadow.skills = skills;
        shadow.skills.update_proc();

        shadow.status.move_point = -2048;
        args.3.queue_spawn(args.0, shadow);
        args.2.add(RunUpdate::new("召唤出幻影", args.0, args.0, 20));
    }
}

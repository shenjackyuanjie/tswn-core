use crate::engine::update::RunUpdate;
use crate::player::{
    PlayerStateStore, PlayerType, PlrId,
    skill::store::SkillStorage,
    skill::{Skill, SkillArgs, SkillExt, SkillTrait},
};

use super::minion::{MinionKind, MinionRuntimeState};

#[derive(Debug, Clone, Default)]
pub struct SummonSkill {
    pub summoned: Option<PlrId>,
}

impl SummonSkill {
    pub fn new() -> Self { Self::default() }
}

impl SkillExt for SummonSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for SummonSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn prob(&self, level: u32, smart: bool, args: SkillArgs) -> bool {
        if smart {
            let owner = args.3.get_player(&args.0).expect("cannot get summon owner from storage");
            if owner.get_status().hp < 80 {
                return false;
            }
        }
        if let Some(summoned) = self.summoned
            && args.3.get_player(&summoned).map(|p| p.alive()).unwrap_or(false)
        {
            return false;
        }
        args.1.r127() < level
    }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        args.2.add(RunUpdate::new("[0]使用[血祭]", args.0, args.0, 60));
        let owner = args.3.get_player(&args.0).expect("cannot get summon owner from storage").clone();
        let mut summoned = owner.clone();
        summoned.id = args.3.new_plr_id();
        summoned.name = format!("{}?summon{}", owner.id_name(), args.1.r255());
        summoned.player_type = PlayerType::Clone;
        summoned.sort_int = args.1.rFFFFFF() as i32;
        summoned.state = PlayerStateStore::default();
        summoned.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Summon,
        });
        summoned.status.max_hp = (owner.get_status().max_hp / 3).max(1);
        summoned.status.hp = summoned.status.max_hp;
        summoned.status.set_alive(true);
        summoned.status.set_frozen(false);

        let owner_status = owner.get_status();
        summoned.status.attack = 0;
        summoned.status.defense = owner_status.defense;
        summoned.status.magic = 0;
        summoned.status.resistance = owner_status.resistance;

        let mut skills = SkillStorage::new();
        let summon_level = (level / 2).max(1);
        skills.add_skill(Skill::new_with_id(summon_level, 0));
        skills.add_skill(Skill::new_with_id(summon_level, 0));
        summoned.skills = skills;
        summoned.skills.update_proc();

        summoned.status.move_point = args.1.r255() as i32 * 4;
        let summoned_id = summoned.as_ptr();
        self.summoned = Some(summoned_id);
        args.3.queue_spawn(args.0, summoned);
        args.2.add(RunUpdate::new("召唤出[1]", args.0, summoned_id, 20));
    }
}

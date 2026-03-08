use super::minion::{MinionKind, MinionRuntimeState};
use crate::engine::update::RunUpdate;
use crate::player::{
    PlayerStateStore, PlayerType, PlrId,
    skill::{SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct CloneSkill {
    /// JS 中 this_.f 在 v() 内部被直接修改，
    /// Rust 需要在 act_with_level 中记录最终 level，
    /// 然后在 post_act_level 中返回它。
    final_level: Option<u32>,
}

impl CloneSkill {
    pub fn new() -> Self { Self { final_level: None } }
}

impl SkillExt for CloneSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for CloneSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

    fn post_act_level(&self, level: u32) -> u32 {
        // JS 中 level 的修改全部在 v() 内部完成，没有单独的 post_act_level。
        // 所以这里返回 act_with_level 中记录的最终 level。
        self.final_level.unwrap_or(((level as f64) * 0.75).ceil().max(1.0) as u32)
    }

    fn prob(&self, level: u32, _smart: bool, args: SkillArgs) -> bool { args.1.r127() < level }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        let random_factor = (args.1.next_u8() as u32 & 63) + 64;
        let mut decayed_level = ((level as f64) * random_factor as f64 / 128.0).ceil() as u32;

        let charge_active = args
            .3
            .get_player(&args.0)
            .map(|owner| owner.get_status().at_boost >= 3.0)
            .unwrap_or(false);
        if !charge_active {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get clone owner from storage");
            for i in 0..7 {
                owner.attr[i] = ((owner.attr[i] as f64) * 0.78).ceil() as u32;
            }
            owner.attr[7] = ((owner.attr[7] as f64) * 0.5).ceil() as u32;
            owner.status.hp = ((owner.status.hp as f64) * 0.5).ceil() as i32;
            owner.status.hp = owner.status.hp.clamp(1, owner.status.max_hp.max(1));
            owner.calc_attr_sum();
            owner.update_states();
        }

        let owner_snapshot = args.3.get_player(&args.0).expect("cannot get clone owner from storage").clone();
        let mut cloned = owner_snapshot.clone();
        cloned.name = owner_snapshot.id_name();
        cloned.id = args.3.new_plr_id();
        cloned.player_type = PlayerType::Clone;
        cloned.sort_int = 0;
        cloned.state = PlayerStateStore::default();

        // JS: PlrClone 先重新 build，重置技能内部运行时状态；
        // PlrClone.addSkillsToProc 先 clamp 等级到 owner 当前等级，然后再 boost。
        cloned.build_for_clone(&owner_snapshot.skills);

        // JS PlrClone.aU: 克隆体八围直接拷贝 owner 当前八围。
        cloned.attr = owner_snapshot.attr;
        // JS: 之后 weapon.cs() (postUpgrade) 再次叠加武器 attr_bonus，
        // 导致武器属性加成被二次计入。
        if let Some(ref ws) = cloned.weapon_state {
            for i in 0..8 {
                cloned.attr[i] = (cloned.attr[i] as i32 + ws.attr_bonus[i]) as u32;
            }
        }
        cloned.state = PlayerStateStore::default();
        cloned.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Clone,
        });
        cloned.update_states();
        cloned.status.move_point = args.1.r255() as i32 * 4 + 256;
        cloned.status.hp = owner_snapshot.get_status().hp.max(1);
        // JS clone 是重新 build 的实体，mp 取 itl/2，而不是 owner 当前 mp。
        cloned.status.mp = (cloned.status.wisdom >> 1).max(0);
        cloned.status.set_alive(true);
        cloned.status.set_frozen(false);

        if owner_snapshot.get_status().hp + owner_snapshot.get_status().magic < args.1.r255() as i32 {
            decayed_level = (decayed_level >> 1) + 1;
        }
        let cloned_clone_level = (decayed_level as f64).sqrt().ceil() as u32;
        if cloned.skills.skill.len() > 23 {
            cloned.skills.skill_by_id_mut(23).set_level(cloned_clone_level.max(1));
        }
        cloned.skills.update_proc();

        let cloned_id = cloned.as_ptr();

        // JS: 先输出"使用分身"消息
        args.2.add(RunUpdate::new("[0]使用[分身]", args.0, args.0, 60));
        // 然后 addNew (queue_spawn)
        args.3.queue_spawn(args.0, cloned);
        // 最后输出"出现一个新的"消息
        args.2.add(RunUpdate::new("出现一个新的[1]", args.0, cloned_id, 20));

        // 记录最终 level，供 post_act_level 使用
        self.final_level = Some(decayed_level);
    }
}

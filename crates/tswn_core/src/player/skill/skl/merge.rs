use crate::engine::update::RunUpdate;
use crate::player::{
    PlrId,
    skill::corpse::CorpseState,
    skill::{ProcKind, SkillArgs, SkillExt, SkillTrait},
};

#[derive(Debug, Clone, Default)]
pub struct MergeSkill;

impl MergeSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for MergeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for MergeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { self.kill_with_level(32, target, args) }

    fn kill_with_level(&mut self, level: u32, target: PlrId, args: SkillArgs) -> bool {
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        if args.1.r63() >= level {
            return false;
        }
        if args.3.get_player(&args.0).is_none() {
            args.3
                .just_get_player_mut(target)
                .expect("cannot get merge target from storage")
                .set_state(CorpseState::merge());
            args.2.add(RunUpdate::new_newline());
            args.2.add(RunUpdate::new("[0][吞噬]了[1]", args.0, target, 60));
            return true;
        }
        let target_attr = args.3.get_player(&target).expect("cannot get merge target from storage").attr;
        let target_skill_levels = {
            let target_plr = args.3.get_player(&target).expect("cannot get merge target from storage");
            let mut levels = target_plr
                .skills
                .store
                .iter()
                .map(|(skill_key, skill)| (*skill_key, skill.level()))
                .collect::<Vec<(usize, u32)>>();
            levels.sort_by_key(|(skill_key, _)| *skill_key);
            levels
        };
        let target_mp = args.3.get_player(&target).expect("cannot get merge target from storage").mp();
        let target_move_point = args.3.get_player(&target).expect("cannot get merge target from storage").move_point();

        let mut merged = false;
        let (transfer_mp, transfer_move_point) = {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get merge owner from storage");
            let mut newly_enabled_skills = Vec::new();
            if debug_this {
                eprintln!(
                    "[merge] owner={} target={} owner_spd={} target_spd={} owner_mp={} target_mp={} owner_mv={} target_mv={}",
                    owner.id_name(),
                    args.3.get_player(&target).map(|p| p.id_name()).unwrap_or_else(|| format!("#{target}")),
                    owner.attr[5],
                    target_attr[5],
                    owner.mp(),
                    target_mp,
                    owner.move_point(),
                    target_move_point
                );
            }
            for (idx, val) in target_attr.iter().enumerate() {
                if *val > owner.attr[idx] {
                    owner.attr[idx] = *val;
                    merged = true;
                }
            }
            for (skill_key, target_level) in target_skill_levels {
                let mut should_add_action = false;
                if let Some(owner_skill) = owner.skills.store.get_mut(&skill_key)
                    && target_level > owner_skill.level()
                {
                    let was_zero = owner_skill.level() == 0;
                    should_add_action = was_zero && owner_skill.has_action_impl();
                    owner_skill.set_level(target_level);
                    if was_zero {
                        newly_enabled_skills.push(skill_key);
                    }
                    merged = true;
                }
                if should_add_action {
                    owner.skills.enable_action_key(skill_key);
                    if let Some(pos) = owner.skills.skill.iter().position(|key| *key == skill_key) {
                        owner.skills.skill.remove(pos);
                    }
                    owner.skills.skill.push(skill_key);
                }
            }
            for skill_key in newly_enabled_skills {
                owner.skills.register_skill_proc(skill_key);
            }
            let transfer_mp = target_mp > owner.mp();
            if transfer_mp {
                owner.set_mp(target_mp);
            }
            let transfer_move_point = target_move_point > owner.move_point();
            if transfer_move_point {
                owner.set_move_point(owner.move_point() + target_move_point);
            }
            if merged {
                owner.update_states();
            }
            (transfer_mp, transfer_move_point)
        };
        if debug_this {
            let owner = args.3.get_player(&args.0).expect("cannot get merge owner after merge");
            eprintln!(
                "[merge] merged={} transfer_mp={} transfer_mv={} owner_spd_after={} owner_mp_after={} owner_mv_after={}",
                merged,
                transfer_mp,
                transfer_move_point,
                owner.attr[5],
                owner.mp(),
                owner.move_point()
            );
        }
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
            target_plr.set_state(CorpseState::merge());
        }
        args.2.add(RunUpdate::new_newline());
        args.2.add(RunUpdate::new("[0][吞噬]了[1]", args.0, target, 60));
        args.2.add(RunUpdate::new("[0]属性上升", args.0, target, 0));
        true
    }

    fn proc_kinds(&self) -> &[ProcKind] { &[ProcKind::PostKill] }
}

//! 融合被动技能实现。
//!
//! 玩家死亡时吞并场上尸体，将其生命值并入自身，实现"融合复生"。

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
        let debug_action = crate::debug::debug_action();
        let debug_this = debug_action
            .as_deref()
            .map(|name| args.3.get_player(&args.0).map(|p| p.id_name() == name).unwrap_or(false))
            .unwrap_or(false);
        let r63_val = args.1.r63();
        if r63_val >= level {
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
        let target_slot_skills = {
            let target_plr = args.3.get_player(&target).expect("cannot get merge target from storage");
            if target_plr.skills.slot_skill.is_empty() {
                target_plr.skills.skill.clone()
            } else {
                target_plr.skills.slot_skill.clone()
            }
        };
        let target_mp = args.3.get_player(&target).expect("cannot get merge target from storage").magic_point();
        let target_move_point = args.3.get_player(&target).expect("cannot get merge target from storage").move_point();

        let mut merged = false;
        let (transfer_mp, transfer_move_point) = {
            let owner = args.3.just_get_player_mut(args.0).expect("cannot get merge owner from storage");
            let mut newly_enabled_skills = Vec::new();
            let owner_slot_skills = if owner.skills.slot_skill.is_empty() {
                owner.skills.skill.clone()
            } else {
                owner.skills.slot_skill.clone()
            };
            if debug_this {
                eprintln!(
                    "[merge] owner={} target={} owner_spd={} target_spd={} owner_mp={} target_mp={} owner_mv={} target_mv={}",
                    owner.id_name(),
                    args.3.get_player(&target).map(|p| p.id_name()).unwrap_or_else(|| format!("#{target}")),
                    owner.attr[5],
                    target_attr[5],
                    owner.magic_point(),
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
            // JS `SklMerge.bS()` 实际对齐的是“按固定槽位逐位抬 level”：
            //
            //   m = owner.k1[s]
            //   l = target.k1[s]
            //   if (l.level > m.level) {
            //     if (m.level === 0 && m instanceof ActionSkill) owner.k4.push(m)
            //     m.level = l.level
            //     m.W()
            //   }
            //
            // `md5_debug.js` 当前保留下来的实现没有一个真正生效的 runtimeType 断路，
            // 所以这里不能按“技能类型不一致就 break/continue”去推导，否则像
            // `fight_multi_7` 里“吞 Possess 幻影后把 owner 第 0 槽 Fire 抬到 76”这种
            // JS 真实行为就会丢失。
            //
            // 也就是说，merge 看的不是“同 skill id”也不是“同 runtime kind”，而是
            // `k1` 固定槽位上的对象位置。
            for (_slot_idx, (owner_skill_key, target_skill_key)) in
                owner_slot_skills.iter().copied().zip(target_slot_skills.iter().copied()).enumerate()
            {
                let Some(target_level) = args
                    .3
                    .get_player(&target)
                    .and_then(|target_plr| target_plr.skills.store.get(&target_skill_key).map(|skill| skill.level()))
                else {
                    continue;
                };
                let mut should_enable_action = false;
                if let Some(owner_skill) = owner.skills.store.get_mut(&owner_skill_key)
                    && target_level > owner_skill.level()
                {
                    let was_zero = owner_skill.level() == 0;
                    should_enable_action = was_zero && owner_skill.has_action_impl();
                    owner_skill.set_level(target_level);
                    if was_zero {
                        newly_enabled_skills.push(owner_skill_key);
                    }
                    merged = true;
                }
                #[cfg(not(feature = "no_debug"))]
                if debug_this {
                    let owner_skill_ref = owner.skills.store.get(&owner_skill_key);
                    let target_skill_ref = args
                        .3
                        .get_player(&target)
                        .and_then(|target_plr| target_plr.skills.store.get(&target_skill_key));
                    let owner_type_name = owner_skill_ref.map(|skill| skill.debug_skill_type_name()).unwrap_or("<missing>");
                    let target_type_name = target_skill_ref.map(|skill| skill.debug_skill_type_name()).unwrap_or("<missing>");
                    let owner_level = owner.skills.store.get(&owner_skill_key).map(|skill| skill.level()).unwrap_or(0);
                    eprintln!(
                        "[merge_slot] owner={} slot={} owner_skill={} target_skill={} owner_type={} target_type={} target_level={} owner_level_after={}",
                        owner.id_name(),
                        _slot_idx,
                        owner_skill_key,
                        target_skill_key,
                        owner_type_name,
                        target_type_name,
                        target_level,
                        owner_level,
                    );
                }
                if should_enable_action {
                    // JS 里 `p===0` 且是 ActionSkill 时，会把该 skill 重新放回动作队列；
                    // Rust 这里等价为重新启用 action，并把 key 放回 `skills.skill` 尾部。
                    owner.skills.enable_action_key(owner_skill_key);
                    if let Some(pos) = owner.skills.skill.iter().position(|key| *key == owner_skill_key) {
                        owner.skills.skill.remove(pos);
                    }
                    owner.skills.skill.push(owner_skill_key);
                }
            }
            let post_action_state_cursor = owner.state.post_action_registration_cursor();
            for skill_key in newly_enabled_skills {
                owner.skills.register_skill_proc_after_states(skill_key, post_action_state_cursor);
            }
            let transfer_mp = target_mp > owner.magic_point();
            if transfer_mp {
                owner.set_magic_point(target_mp);
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
                owner.magic_point(),
                owner.move_point()
            );
        }
        {
            let target_plr = args.3.just_get_player_mut(target).expect("cannot get merge target from storage");
            if transfer_mp {
                target_plr.set_magic_point(0);
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

    fn proc_kinds(&self) -> &'static [ProcKind] { &[ProcKind::PostKill] }
}

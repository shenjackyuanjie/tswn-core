use super::minion::{MinionKind, MinionRuntimeState, alloc_minion_name, root_minion_name_owner_id};
use crate::engine::update::RunUpdate;
use crate::player::{
    Player, PlayerStateStore, PlayerType, PlrId, eval_name,
    skill::{InlineCtx, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
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

    fn has_inline_act(&self) -> bool { true }

    fn act_inline(&mut self, level: u32, _targets: Vec<PlrId>, _smart: bool, ctx: &mut InlineCtx) {
        let random_factor = (ctx.randomer.next_u8() as u32 & 63) + 64;
        let mut decayed_level = ((level as f64) * random_factor as f64 / 128.0).ceil() as u32;

        let charge_active = ctx.owner.status.at_boost >= 3.0;
        if !charge_active {
            for i in 0..7 {
                ctx.owner.attr[i] = ((ctx.owner.attr[i] as f64) * 0.78).ceil() as u32;
            }
            ctx.owner.attr[7] = ((ctx.owner.attr[7] as f64) * 0.5).ceil() as u32;
            ctx.owner.status.hp = ((ctx.owner.status.hp as f64) * 0.5).ceil() as i32;
            ctx.owner.status.hp = ctx.owner.status.hp.clamp(1, ctx.owner.status.max_hp.max(1));
            ctx.owner.calc_attr_sum();
            // Clone needs the same fully refreshed owner snapshot as the legacy act path
            // before building the minion and computing the extra level decay roll.
            ctx.owner.update_states();
        }

        let root_owner_id = root_minion_name_owner_id(ctx.storage, ctx.ptr);
        let (owner_display_name, owner_base_name, owner_clan_name, owner_attr, owner_hp, owner_magic, owner_skills) = {
            let owner = ctx.storage.get_player(&ctx.ptr).expect("cannot get clone owner from storage");
            (
                owner.display_name(),
                owner.base_name(),
                owner.clan_name(),
                owner.attr,
                owner.get_status().hp,
                owner.get_status().magic,
                owner.skills.clone(),
            )
        };
        let mut cloned = ctx.owner.clone();
        cloned.set_id_name_override(Some(alloc_minion_name(ctx.storage, ctx.ptr)));
        cloned.set_display_name_override(Some(owner_display_name));
        cloned.reset_minion_name_counter();
        cloned.id = ctx.storage.new_plr_id();
        cloned.player_type = PlayerType::Clone;
        cloned.sort_int = 0;
        cloned.state = PlayerStateStore::default();
        cloned.raw_name_base = Player::normal_raw_name_base(Some(owner_clan_name.as_str()), owner_base_name.as_str());
        let factor_name = ctx
            .storage
            .get_player(&root_owner_id)
            .map(|root_owner| eval_name::eval_str_common_with_rq(root_owner.base_name().as_str(), true, ctx.storage.eval_rq()))
            .unwrap_or_else(|| eval_name::eval_str_common_with_rq(owner_base_name.as_str(), true, ctx.storage.eval_rq()));
        let factor_team = eval_name::eval_str_common_with_rq(owner_clan_name.as_str(), true, ctx.storage.eval_rq());
        cloned.name_factor = factor_name.max(factor_team - 6.0);
        cloned.build_for_clone(&owner_skills);
        cloned.attr = owner_attr;
        if let Some(ref ws) = cloned.weapon_state {
            for i in 0..8 {
                cloned.attr[i] = (cloned.attr[i] as i32 + ws.attr_bonus[i]) as u32;
            }
        }
        cloned.state = PlayerStateStore::default();
        cloned.set_state(MinionRuntimeState {
            owner: Some(root_owner_id),
            kind: MinionKind::Clone,
        });
        cloned.update_states();
        cloned.status.move_point = ctx.randomer.r255() as i32 * 4 + 256;
        cloned.status.hp = owner_hp.max(1);
        cloned.status.magic_point = (cloned.status.wisdom >> 1).max(0);
        cloned.status.set_alive(true);
        cloned.status.set_frozen(false);

        if owner_hp + owner_magic < ctx.randomer.r255() as i32 {
            decayed_level = (decayed_level >> 1) + 1;
        }
        let cloned_clone_level = (decayed_level as f64).sqrt().ceil() as u32;
        let clone_skill_was_zero = cloned.skills.skill_by_id(23).level() == 0;
        cloned.skills.skill_by_id_mut(23).set_level(cloned_clone_level.max(1));
        if clone_skill_was_zero {
            cloned.skills.disable_action_key(23);
        }
        cloned.skills.update_proc();
        if crate::debug::debug_stats() {
            let owner_snapshot = ctx.storage.get_player(&ctx.ptr).expect("cannot get clone owner from storage");
            eprintln!(
                "[CLONE_FINAL] owner={} owner_attr={:?} owner_hp={} owner_mp={} owner_atk={} owner_def={} owner_spd={} owner_agl={} owner_mag={} owner_mdf={} owner_wis={} | clone_base={} clone_attr={:?} clone_hp={} clone_mp={} clone_atk={} clone_def={} clone_spd={} clone_agl={} clone_mag={} clone_mdf={} clone_wis={} clone_move={} clone_clone_lvl={}",
                owner_snapshot.id_name(),
                owner_snapshot.attr,
                owner_snapshot.get_status().hp,
                owner_snapshot.get_status().magic_point,
                owner_snapshot.get_status().attack,
                owner_snapshot.get_status().defense,
                owner_snapshot.get_status().speed,
                owner_snapshot.get_status().agility,
                owner_snapshot.get_status().magic,
                owner_snapshot.get_status().resistance,
                owner_snapshot.get_status().wisdom,
                cloned.id_name(),
                cloned.attr,
                cloned.get_status().hp,
                cloned.get_status().magic_point,
                cloned.get_status().attack,
                cloned.get_status().defense,
                cloned.get_status().speed,
                cloned.get_status().agility,
                cloned.get_status().magic,
                cloned.get_status().resistance,
                cloned.get_status().wisdom,
                cloned.get_status().move_point,
                cloned.skills.skill_by_id(23).level(),
            );
        }

        let cloned_id = cloned.as_ptr();
        ctx.updates.add(RunUpdate::new("[0]使用[分身]", ctx.ptr, ctx.ptr, 60));
        ctx.storage.queue_spawn(ctx.ptr, cloned);
        ctx.updates.add(RunUpdate::new("出现一个新的[1]", ctx.ptr, cloned_id, 0));
        self.final_level = Some(decayed_level);
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

        let root_owner_id = root_minion_name_owner_id(args.3, args.0);
        let (mut cloned, owner_display_name, owner_base_name, owner_clan_name, owner_attr, owner_hp, owner_magic, owner_skills) = {
            let owner = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
            (
                owner.clone(),
                owner.display_name(),
                owner.base_name(),
                owner.clan_name(),
                owner.attr,
                owner.get_status().hp,
                owner.get_status().magic,
                owner.skills.clone(),
            )
        };
        cloned.set_id_name_override(Some(alloc_minion_name(args.3, args.0)));
        cloned.set_display_name_override(Some(owner_display_name));
        cloned.reset_minion_name_counter();
        cloned.id = args.3.new_plr_id();
        cloned.player_type = PlayerType::Clone;
        cloned.sort_int = 0;
        cloned.state = PlayerStateStore::default();
        cloned.raw_name_base = Player::normal_raw_name_base(Some(owner_clan_name.as_str()), owner_base_name.as_str());
        let factor_name = args
            .3
            .get_player(&root_owner_id)
            .map(|root_owner| eval_name::eval_str_common_with_rq(root_owner.base_name().as_str(), true, args.3.eval_rq()))
            .unwrap_or_else(|| eval_name::eval_str_common_with_rq(owner_base_name.as_str(), true, args.3.eval_rq()));
        let factor_team = eval_name::eval_str_common_with_rq(owner_clan_name.as_str(), true, args.3.eval_rq());
        cloned.name_factor = factor_name.max(factor_team - 6.0);

        // JS: PlrClone 先重新 build，重置技能内部运行时状态；
        // PlrClone.addSkillsToProc 先 clamp 等级到 owner 当前等级，然后再 boost。
        cloned.build_for_clone(&owner_skills);

        // JS PlrClone.aU: 克隆体八围直接拷贝 owner 当前八围。
        cloned.attr = owner_attr;
        // JS: 之后 weapon.cs() (postUpgrade) 再次叠加武器 attr_bonus，
        // 导致武器属性加成被二次计入。
        if let Some(ref ws) = cloned.weapon_state {
            for i in 0..8 {
                cloned.attr[i] = (cloned.attr[i] as i32 + ws.attr_bonus[i]) as u32;
            }
        }
        cloned.state = PlayerStateStore::default();
        cloned.set_state(MinionRuntimeState {
            owner: Some(root_owner_id),
            kind: MinionKind::Clone,
        });
        cloned.update_states();
        cloned.status.move_point = args.1.r255() as i32 * 4 + 256;
        cloned.status.hp = owner_hp.max(1);
        // JS clone 是重新 build 的实体，mp 取 itl/2，而不是 owner 当前 mp。
        cloned.status.magic_point = (cloned.status.wisdom >> 1).max(0);
        cloned.status.set_alive(true);
        cloned.status.set_frozen(false);

        if owner_hp + owner_magic < args.1.r255() as i32 {
            decayed_level = (decayed_level >> 1) + 1;
        }
        let cloned_clone_level = (decayed_level as f64).sqrt().ceil() as u32;
        // JS 在 p.az() 之后才把 clone skill 改成 sqrt(level)。
        // 这会更新共享技能对象的等级，但不会让一个 build 时为 0 的 clone skill retroactively 进入 k4。
        let clone_skill_was_zero = cloned.skills.skill_by_id(23).level() == 0;
        cloned.skills.skill_by_id_mut(23).set_level(cloned_clone_level.max(1));
        if clone_skill_was_zero {
            cloned.skills.disable_action_key(23);
        }
        cloned.skills.update_proc();
        if crate::debug::debug_stats() {
            let owner_snapshot = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
            eprintln!(
                "[CLONE_FINAL] owner={} owner_attr={:?} owner_hp={} owner_mp={} owner_atk={} owner_def={} owner_spd={} owner_agl={} owner_mag={} owner_mdf={} owner_wis={} | clone_base={} clone_attr={:?} clone_hp={} clone_mp={} clone_atk={} clone_def={} clone_spd={} clone_agl={} clone_mag={} clone_mdf={} clone_wis={} clone_move={} clone_clone_lvl={}",
                owner_snapshot.id_name(),
                owner_snapshot.attr,
                owner_snapshot.get_status().hp,
                owner_snapshot.get_status().magic_point,
                owner_snapshot.get_status().attack,
                owner_snapshot.get_status().defense,
                owner_snapshot.get_status().speed,
                owner_snapshot.get_status().agility,
                owner_snapshot.get_status().magic,
                owner_snapshot.get_status().resistance,
                owner_snapshot.get_status().wisdom,
                cloned.id_name(),
                cloned.attr,
                cloned.get_status().hp,
                cloned.get_status().magic_point,
                cloned.get_status().attack,
                cloned.get_status().defense,
                cloned.get_status().speed,
                cloned.get_status().agility,
                cloned.get_status().magic,
                cloned.get_status().resistance,
                cloned.get_status().wisdom,
                cloned.get_status().move_point,
                cloned.skills.skill_by_id(23).level(),
            );
        }

        let cloned_id = cloned.as_ptr();

        // JS: 先输出"使用分身"消息
        args.2.add(RunUpdate::new("[0]使用[分身]", args.0, args.0, 60));
        // 然后 addNew (queue_spawn)
        args.3.queue_spawn(args.0, cloned);
        // 最后输出"出现一个新的"消息
        args.2.add(RunUpdate::new("出现一个新的[1]", args.0, cloned_id, 0));

        // 记录最终 level，供 post_act_level 使用
        self.final_level = Some(decayed_level);
    }
}

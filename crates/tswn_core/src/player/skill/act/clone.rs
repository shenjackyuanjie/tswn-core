//! 克隆（分身）主动技能实现。
//!
//! 以自身为模板创建克隆召唤物，克隆体继承宿主部分属性并以独立战斗单元参战。

use super::{
    minion::{MinionKind, MinionRuntimeState, alloc_minion_name, root_minion_name_owner_id},
    summon::{SUMMON_SHARE_DAMAGE_SKILL_KEY, ensure_summon_share_damage_skill},
};
use std::sync::Arc;

use crate::engine::{storage::Storage, update::RunUpdate};
use crate::player::{
    Player, PlayerStateStore, PlayerType, PlrId, eval_name,
    skill::{SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};

// JS 产物里 Dart 的 0.78f 会落成这个精确值；分身衰减后会立刻 ceil，
// 用 0.78 会在少数整数边界把八围多保留 1 点。
const CLONE_ATTR_DECAY: f64 = 0.7799999713897705;

#[derive(Debug, Clone, Default)]
pub struct CloneSkill {
    /// `分身` 在 `act_with_level()` 内就会算出 owner 这次用完技能后的“当前熟练度”。
    ///
    /// 这里缓存的是 owner 自己的 `分身` 当前等级，供 `post_act_level()` 回写；
    /// 它不是新 clone 身上的 `分身` 等级。后者会在下面按
    /// `ceil(sqrt(decayed_level))` 单独计算。
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
        // `分身` 的熟练度衰减不是固定公式，而是以 `act_with_level()` 中算出的
        // `decayed_level` 为准。这里回写的是 owner 当前 `分身` 熟练度。
        //
        // 不要和下面的新 clone 等级混淆：
        // - `decayed_level`：owner 用完 `分身` 后自己的当前熟练度
        // - `cloned_clone_level`：新 clone 身上的 `分身` 初始等级
        self.final_level.unwrap_or(((level as f64) * 0.75).ceil().max(1.0) as u32)
    }

    fn prob(&self, level: u32, _smart: bool, args: SkillArgs) -> bool { args.1.r127() < level }

    fn select_targets_with_level(&self, _level: u32, _candidates: &[PlrId], _smart: bool, args: SkillArgs) -> Vec<PlrId> {
        vec![args.0]
    }

    fn act_with_level(&mut self, level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        // `分身` 出手时会先衰减 owner 自己当前的 `分身` 熟练度。
        // 第一段衰减公式是：`ceil(level * random_factor / 128)`，其中
        // `random_factor ∈ [64, 127]`。
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
                owner.attr[i] = ((owner.attr[i] as f64) * CLONE_ATTR_DECAY).ceil() as u32;
            }
            owner.attr[7] = ((owner.attr[7] as f64) * 0.5).ceil() as u32;
            owner.status.hp = ((owner.status.hp as f64) * 0.5).ceil() as i32;
            owner.status.hp = owner.status.hp.clamp(1, owner.status.max_hp.max(1));
            owner.calc_attr_sum();
            owner.update_states();
        }

        let root_owner_id = root_minion_name_owner_id(args.3, args.0);
        let (mut cloned, owner_display_name, owner_base_name, owner_clan_name, owner_attr, owner_hp, owner_magic) = {
            let owner = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
            (
                owner.clone(),
                owner.display_name(),
                owner.base_name(),
                owner.clan_name(),
                owner.attr,
                owner.get_status().hp,
                owner.get_status().magic,
            )
        };
        let share_damage_owner = args
            .3
            .get_player(&args.0)
            .filter(|owner| {
                owner
                    .skills
                    .store
                    .get(&SUMMON_SHARE_DAMAGE_SKILL_KEY)
                    .map(|skill| skill.level() > 0)
                    .unwrap_or(false)
            })
            .and_then(|_| root_share_damage_owner_id(args.3, args.0));
        cloned.set_id_name_override(Some(alloc_minion_name(args.3, args.0)));
        cloned.set_display_name_override(Some(owner_display_name));
        cloned.reset_minion_name_counter();
        cloned.id = args.3.new_plr_id();
        cloned.player_type = PlayerType::Clone;
        cloned.sort_int = 0;
        cloned.state = PlayerStateStore::default();
        // JS `init_PlrClone()` 会先运行普通的 `PlrClone` 构造函数，
        // 然后再用 owner 变换后的 base 覆盖 `t/name_base`。
        // 因此 `E/raw_name_base` 仍保留普通构造函数的输出。
        cloned.raw_name_base = Player::normal_raw_name_base(Some(owner_clan_name.as_str()), owner_base_name.as_str());
        let factor_name = args
            .3
            .get_player(&root_owner_id)
            .map(|root_owner| eval_name::eval_str_common_with_rq(root_owner.base_name().as_str(), true, args.3.eval_rq()))
            .unwrap_or_else(|| eval_name::eval_str_common_with_rq(owner_base_name.as_str(), true, args.3.eval_rq()));
        let factor_team = eval_name::eval_str_common_with_rq(owner_clan_name.as_str(), true, args.3.eval_rq());
        cloned.name_factor = factor_name.max(factor_team - 6.0);

        // `PlrClone` 会先重新 `build` 一次，把技能内部运行时态重置干净；
        // 但 clone build 后必须再把技能等级 clamp 到 owner 当前等级，
        // 否则像 `生命之轮` / `治愈魔法` / `苏生术` / `分身` / `幻术` 这类
        // 已在战斗中衰减过的技能，会被错误地恢复成名字初始值。
        {
            let owner = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
            cloned.build_for_clone(&owner.skills);
        }

        // `PlrClone.aU`：克隆体八围直接拷贝 owner 当前八围。
        cloned.attr = owner_attr;
        // 随后 `weapon.cs()`（postUpgrade）会再次叠加武器属性加成，
        // 因而武器的 `attr_bonus` 会被二次计入。
        if let Some(ref ws) = cloned.weapon_state {
            for i in 0..8 {
                cloned.attr[i] = (cloned.attr[i] as i32 + ws.attr_bonus[i]) as u32;
            }
        }
        if share_damage_owner.is_some() {
            ensure_summon_share_damage_skill(&mut cloned.skills, true);
        }
        cloned.state = PlayerStateStore::default();
        cloned.set_state(MinionRuntimeState {
            owner: Some(root_owner_id),
            kind: MinionKind::Clone,
            share_damage_owner,
        });
        cloned.update_states();
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_DEBUG_CLONE").is_some() {
            let owner_snapshot = args.3.get_player(&args.0).expect("cannot get clone owner from storage");
            let owner_name = owner_snapshot.id_name();
            let clone_name = cloned.id_name();
            let is_diy = owner_snapshot.skills.is_diy;
            eprintln!(
                "[clone_debug] owner={} diy={} clone={} factor={} attrs={:?} hp={} atk={} def={} spd={} agi={} mag={}",
                owner_name,
                is_diy,
                clone_name,
                cloned.name_factor,
                cloned.attr,
                cloned.status.hp,
                cloned.status.attack,
                cloned.status.defense,
                cloned.status.speed,
                cloned.status.agility,
                cloned.status.magic,
            );
            for skill_key in &cloned.skills.skill {
                let skill = cloned.skills.skill_by_id(*skill_key);
                if skill.level() > 0 {
                    let boost_info = skill.diy_boost.as_ref().map(|b| format!("{:?}", b)).unwrap_or_else(|| "none".to_string());
                    eprintln!(
                        "[clone_debug]   skill id={} level={} boosted={} diy_boost={}",
                        skill_key,
                        skill.level(),
                        skill.boosted,
                        boost_info,
                    );
                }
            }
        }
        cloned.status.move_point = args.1.r255() as i32 * 4 + 256;
        cloned.status.hp = owner_hp.max(1);
        // clone 是重新 `build` 出来的实体，所以 mp 取 `itl / 2`，
        // 而不是直接继承 owner 当前 mp。
        cloned.status.magic_point = (cloned.status.wisdom >> 1).max(0);
        cloned.status.set_alive(true);
        cloned.status.set_frozen(false);

        if owner_hp + owner_magic < args.1.r255() as i32 {
            // 资源偏低时，owner 当前 `分身` 熟练度还会再追加一次衰减：
            // `(decayed_level >> 1) + 1`。
            decayed_level = (decayed_level >> 1) + 1;
        }
        let cloned_clone_level = (decayed_level as f64).sqrt().ceil() as u32;
        // 新 clone 自己的 `分身` 等级不是直接等于 owner 的 `decayed_level`，
        // 而是 `ceil(sqrt(decayed_level))`。
        // 这只影响“新 clone 之后还能不能继续分身”，不要和 owner 当前熟练度的
        // 回写语义混淆。
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

        // 记录 owner 这次用完 `分身` 后的最终当前熟练度，供 `post_act_level()` 回写。
        self.final_level = Some(decayed_level);
    }
}

fn root_share_damage_owner_id(storage: &Arc<Storage>, start_id: PlrId) -> Option<PlrId> {
    let mut current = start_id;
    for _ in 0..32 {
        let player = storage.get_player(&current)?;
        let Some(state) = player.get_state::<MinionRuntimeState>() else {
            return Some(current);
        };
        current = state.share_damage_owner.or(state.owner)?;
    }
    Some(current)
}

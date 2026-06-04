//! `血祭` / 使魔召唤技能实现。
//!
//! 本模块负责创建、复用和刷新使魔召唤物，并处理固定槽位技能继承、
//! 伤害分摊技能以及覆盖模板应用等细节。

use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::{RunUpdate, RunUpdates};
use crate::player::{
    OnDamageFunc, PlayerStateStore, PlayerType, PlrId,
    skill::store::SkillStorage,
    skill::{Skill, SkillArgs, SkillExt, SkillTargetDomain, SkillTrait},
};
use crate::rc4::RC4;

use super::minion::{
    MinionKind, MinionRuntimeState, alloc_minion_name, apply_child_minion_overlay, apply_minion_skill_overlay,
    apply_summon_attrs, owner_minion_overlay, prepare_combat_minion,
};

pub(super) const SUMMON_SHARE_DAMAGE_SKILL_KEY: usize = 255;

pub(super) fn ensure_summon_share_damage_skill(skills: &mut SkillStorage, enabled: bool) {
    skills
        .store
        .get_or_insert_with(SUMMON_SHARE_DAMAGE_SKILL_KEY, || {
            Skill::new(1, Box::new(SummonShareDamageSkill::new()))
        })
        .set_level(if enabled { 1 } else { 0 });
}

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

    fn summon_minion_id(&self) -> Option<PlrId> { self.summoned }

    fn has_action_impl(&self) -> bool { true }

    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::SelfOnly }

    fn select_target_count(&self, _smart: bool) -> usize { 1 }

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

    fn act_with_level(&mut self, _level: u32, _targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        args.2.add(RunUpdate::new("[0]使用[血祭]", args.0, args.0, 60));
        let owner = args.3.get_player(&args.0).expect("cannot get summon owner from storage").clone();
        let charge_active = owner.get_status().at_boost >= 3.0;
        let minion_overlay = owner_minion_overlay(args.3, args.0, MinionKind::Summon);
        if let Some(summoned_id) = self.summoned
            && let Some(summoned) = args.3.just_get_player_mut(summoned_id)
            && !summoned.alive()
        {
            // JS `SklSummon.v()`：首次创建后，后续重施会复用同一个已死亡的召唤物对象，
            // 并重新执行 `bP()` / `bs()` / `cn()`。`bP()` 会清空运行时状态和 `proc` 队列，
            // 但不会重置技能 boost 标记；Rust 把所有者关联存成运行时状态，
            // 所以清空状态仓库后需要把这个标记补回去。
            summoned.state = PlayerStateStore::default();
            summoned.set_state(MinionRuntimeState {
                owner: Some(args.0),
                kind: MinionKind::Summon,
                share_damage_owner: None,
            });
            apply_child_minion_overlay(summoned, minion_overlay.as_ref());
            let reuse_overlay_skills = minion_overlay.as_ref().map(|overlay| overlay.reuse_skills_on_recast).unwrap_or(false);
            if reuse_overlay_skills || !apply_minion_skill_overlay(summoned, minion_overlay.as_ref()) {
                ensure_summon_share_damage_skill(&mut summoned.skills, !charge_active);
                summoned.skills.boost_last();
                summoned.skills.update_proc();
            } else {
                ensure_summon_share_damage_skill(&mut summoned.skills, !charge_active);
                summoned.skills.update_proc();
            }
            if apply_summon_attrs(summoned, &owner, minion_overlay.as_ref()) {
                summoned.update_states();
            }
            summoned.init_values();
            summoned.status.set_alive(true);
            summoned.status.move_point = args.1.r255() as i32 * 4;
            if charge_active {
                summoned.status.move_point = 2048;
            }
            args.3.queue_revival(summoned_id);
            args.2.add(RunUpdate::new("召唤出[1]", args.0, summoned_id, 0));
            return;
        }
        let summon_team = owner.clan_name();
        let summon_name = format!("{}?summon", owner.base_name());
        let mut summoned =
            crate::player::Player::new_minion_and_init(Some(summon_team.clone()), summon_name.clone(), None, args.3.clone())
                .expect("cannot init summon minion");
        prepare_combat_minion(&mut summoned);
        summoned.build();
        if !apply_summon_attrs(&mut summoned, &owner, minion_overlay.as_ref()) {
            summoned.attr[7] = (summoned.attr[7] / 3).max(1);
            summoned.attr[0] = 0;
            summoned.attr[1] = owner.attr[1];
            summoned.attr[4] = 0;
            summoned.attr[5] = owner.attr[5];
        }
        summoned.update_states();
        summoned.status.hp = summoned.status.max_hp;
        summoned.status.magic_point = summoned.status.wisdom >> 1;

        summoned.id = args.3.new_plr_id();
        summoned.set_id_name_override(Some(alloc_minion_name(args.3, args.0)));
        summoned.set_display_name_override(Some("使魔".to_string()));
        summoned.player_type = PlayerType::Clone;
        summoned.sort_int = 0;
        summoned.state = PlayerStateStore::default();
        summoned.set_state(MinionRuntimeState {
            owner: Some(args.0),
            kind: MinionKind::Summon,
            share_damage_owner: None,
        });
        apply_child_minion_overlay(&mut summoned, minion_overlay.as_ref());
        summoned.status.set_alive(true);
        summoned.status.set_frozen(false);

        if !apply_minion_skill_overlay(&mut summoned, minion_overlay.as_ref()) {
            let skill_level_from_slot = |slot: usize| -> u32 {
                let base = 64 + slot * 4;
                if base + 3 >= summoned.name_base.len() {
                    return 0;
                }
                let minv = summoned.name_base[base..base + 4].iter().copied().min().unwrap_or(0);
                minv.saturating_sub(10) as u32
            };
            let mut skill_order = [0usize, 1, 2];
            let team_bytes = [0_u8].iter().chain(summon_team.as_bytes()).copied().collect::<Vec<u8>>();
            let name_bytes = [0_u8].iter().chain(summon_name.as_bytes()).copied().collect::<Vec<u8>>();
            let mut skill_rand = RC4::new(&team_bytes, 1);
            skill_rand.update(&name_bytes, 2);
            skill_rand.sort_list(&mut skill_order);
            let mut skills = SkillStorage::new();
            skills.add_skill(Skill::new_with_id(0, 0));
            skills.add_skill(Skill::new_with_id(0, 0));
            skills.add_skill(Skill::new(0, Box::new(SummonExplodeSkill::new())));
            // JS `PlrSummon.ac()/dm()/bs()` 的关键点：
            //
            // 1. 固定槽位 `k1` 永远是 `[fire, fire, explode]`
            // 2. 只会打乱“哪个对象先行动/哪个对象拿到第几个等级”的遍历顺序视图
            // 3. merge 读取的是固定槽位 `k1`，不是打乱后的主动顺序
            //
            // 所以 Rust 必须把这两层分开表达：
            // - `slot_skill = [0, 1, 2]` 保留稳定的固定槽位语义
            // - `skill = skill_order` 表达当前主动技能扫描顺序
            //
            // 否则后面一旦有人吞 summon，merge 就会按错误的顺序继承等级。
            for (slot, skill_key) in skill_order.iter().copied().enumerate() {
                let level = skill_level_from_slot(slot);
                let skill = skills.skill_by_id_mut(skill_key);
                skill.set_level(level);
                // JS `Plr.dm()`：如果算出的 level > 0，就检查*原始*（raw）hash；
                // 若 raw min - 10 <= 0，则把技能标记为已 boost，让 boost_last 跳过它。
                if level > 0 {
                    let raw_base = 64 + slot * 4;
                    if raw_base + 3 < summoned.raw_name_base.len() {
                        let raw_min = summoned.raw_name_base[raw_base..raw_base + 4].iter().copied().min().unwrap_or(0);
                        if raw_min <= 10 {
                            skill.boosted = true;
                        }
                    }
                }
            }
            // 固定槽位始终不洗牌；这里只记录 JS `k1` 的稳定视图。
            skills.slot_skill = vec![0, 1, 2];
            skills.skill = skill_order.to_vec();
            ensure_summon_share_damage_skill(&mut skills, !charge_active);
            skills.boost_last();
            summoned.skills = skills;
            summoned.skills.update_proc();
        } else {
            ensure_summon_share_damage_skill(&mut summoned.skills, !charge_active);
            summoned.skills.update_proc();
        }

        // JS: this_.fr.l = a8.n() * 4 (无条件消耗 r255)
        // 然后如果 charge: this_.fr.l = 2048 (覆盖)
        summoned.status.move_point = args.1.r255() as i32 * 4;
        if charge_active {
            summoned.status.move_point = 2048;
        }
        let summoned_id = summoned.as_ptr();
        self.summoned = Some(summoned_id);
        args.3.queue_spawn(args.0, summoned);
        args.2.add(RunUpdate::new("召唤出[1]", args.0, summoned_id, 0));
    }
}

#[derive(Debug, Clone, Default)]
pub struct SummonExplodeSkill;

impl SummonExplodeSkill {
    pub fn new() -> Self { Self }
}

impl SkillExt for SummonExplodeSkill {
    fn box_new() -> Box<dyn SkillTrait> { Box::new(Self::new()) }
}

impl SkillTrait for SummonExplodeSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn has_action_impl(&self) -> bool { true }

    fn act(&mut self, targets: Vec<PlrId>, _smart: bool, args: SkillArgs) {
        if targets.is_empty() {
            return;
        }
        let target_id = targets[0];
        let fire_mag = args
            .3
            .get_player(&target_id)
            .expect("cannot get summon explode target from storage")
            .get_state::<super::fire::FireState>()
            .map(|state| state.fire_mag)
            .unwrap_or(0.0);
        let atp = args
            .3
            .get_player(&args.0)
            .expect("cannot get summon explode owner from storage")
            .get_at(true, args.1)
            * (4.0 + fire_mag);
        args.2.add(RunUpdate::new("[0]使用[自爆]", args.0, target_id, 0));
        let old_hp = {
            let owner = args
                .3
                .just_get_player_mut(args.0)
                .expect("cannot get mutable summon explode owner from storage");
            let old_hp = owner.get_status().hp;
            owner.status.hp = 0;
            old_hp
        };
        let _dmg = args
            .3
            .just_get_player_mut(target_id)
            .expect("cannot get mutable summon explode target from storage")
            .attacked(atp, true, args.0, super::fire::on_fire as OnDamageFunc, args.1, args.2, args.3);
        args.3
            .just_get_player_mut(args.0)
            .expect("cannot get mutable summon explode owner from storage")
            .on_die(old_hp, args.0, args.1, args.2, args.3);
    }
}

#[derive(Debug, Clone, Default)]
struct SummonShareDamageSkill;

impl SummonShareDamageSkill {
    fn new() -> Self { Self }
}

impl SkillTrait for SummonShareDamageSkill {
    fn destroy(&self, _plr: PlrId, _args: SkillArgs) {}

    fn clone_box(&self) -> Box<dyn SkillTrait> { Box::new(self.clone()) }

    fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        let owner_id = args
            .3
            .get_player(&args.0)
            .and_then(|player| player.get_state::<MinionRuntimeState>())
            .and_then(|state| state.share_damage_owner.or(state.owner));
        let Some(owner_id) = owner_id else {
            return;
        };
        // JS PlrSummon.aR: 在伤害分摊期间标记使魔，
        // 防止 owner 死亡时通过 linked minion 路径立即处理使魔的死亡。
        args.3.set_in_post_damage(args.0);
        if let Some(owner) = args.3.just_get_player_mut(owner_id) {
            owner.damage(dmg / 2, caster, on_summon_share_damage as OnDamageFunc, args.1, args.2, args.3);
        }
        args.3.clear_in_post_damage();
    }

    fn proc_kinds(&self) -> &'static [crate::player::skill::ProcKind] { &[crate::player::skill::ProcKind::PostDamage] }
}

fn on_summon_share_damage(
    _caster: PlrId,
    _target: PlrId,
    _dmg: i32,
    _r: &mut RC4,
    _updates: &mut RunUpdates,
    _storage: &Arc<Storage>,
) {
}

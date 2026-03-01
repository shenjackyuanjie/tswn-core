use std::any::Any;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::sync::Arc;

use crate::engine::storage::Storage;
use crate::engine::update::RunUpdates;
use crate::player::{OnDamageFunc, PlrId};
use crate::rc4::RC4;

pub mod act;
pub mod skl;
pub mod store;

pub use act::{
    absorb, accumulate, assassinate, berserk, charge, charm, clone, critical, curse, disperse, exchange, fire, half, haste, heal,
    ice, iron, poison, quake, rapid, revive, shadow, slow, summon, thunder,
};
pub use skl::{counter, defend, hide, merge, none, protect, reflect, reraise, shield, upgrade, zombie};

/// SkillArgs:
/// PlrId: player handle（稳定 ID，不是内存指针）
/// &'d mut RC4: random number generator
/// &'d mut RunUpdates: updates to be applied
/// &'d Arc<Storage>: game storage
pub type SkillArgs<'d> = (PlrId, &'d mut RC4, &'d mut RunUpdates, &'d Arc<Storage>);

/// 技能注册的流程类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcKind {
    UpdateState,
    PreStep,
    PreAction,
    PostAction,
    PreDefend,
    PostDefend,
    PostDamage,
    PostDeath,
    PostKill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillTargetDomain {
    EnemyAlive,
    AllyAlive,
    AllyAny,
    AllyDead,
    SelfOnly,
    AllAlive,
}

#[allow(unused_variables, unused_mut)]
pub trait SkillTrait: Debug {
    // ===== 必须实现的 =====
    /// 销毁这个玩意 (技能用过了)
    fn destroy(&self, plr: PlrId, args: SkillArgs);
    /// 用于实现 Clone
    fn clone_box(&self) -> Box<dyn SkillTrait>;

    // ===== 可选实现的 =====
    /// 更新状态
    fn update_state(&mut self, args: SkillArgs) {}
    fn update_state_with_level(&mut self, _level: u32, args: SkillArgs) { self.update_state(args) }
    /// 内联版更新状态 — 直接修改 PlayerStatus，不经过 Storage。
    /// 在 Player::update_states() 中调用，对齐 JS 的 F() 遍历 rx 回调。
    fn update_state_inline(&mut self, _level: u32, _status: &mut super::PlayerStatus) {}
    /// 行动!
    fn act(&mut self, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {}
    fn act_with_level(&mut self, _level: u32, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {
        self.act(targets, smart, args)
    }
    fn post_act_level(&self, level: u32) -> u32 { level }

    fn pre_step(&mut self, mut step: i32, args: SkillArgs) -> i32 { step }
    fn pre_step_with_level(&mut self, _level: u32, step: i32, args: SkillArgs) -> i32 { self.pre_step(step, args) }
    /// 行动之前
    fn pre_action(&mut self, args: SkillArgs) {}
    fn pre_action_with_level(&mut self, _level: u32, args: SkillArgs) { self.pre_action(args) }
    /// preAction 是否强制选择当前技能
    fn pre_action_select(&mut self, _smart: bool, _args: SkillArgs) -> bool { false }
    fn pre_action_select_with_level(&mut self, _level: u32, smart: bool, args: SkillArgs) -> bool {
        self.pre_action_select(smart, args)
    }
    /// preAction 是否清空当前强制动作（对齐 JS preAction 链可返回 null）。
    fn pre_action_clear_forced(&mut self, _smart: bool, _args: SkillArgs) -> bool { false }
    fn pre_action_clear_forced_with_level(&mut self, _level: u32, smart: bool, args: SkillArgs) -> bool {
        self.pre_action_clear_forced(smart, args)
    }
    /// 行动之后
    fn post_action(&mut self, args: SkillArgs) {}
    fn post_action_with_level(&mut self, _level: u32, args: SkillArgs) { self.post_action(args) }
    /// 每次 action 结束后的回调（对齐 RunUpdates.onUpdateEnd）
    fn on_update_end(&mut self, _args: SkillArgs) -> bool { false }
    fn on_update_end_with_level(&mut self, _level: u32, args: SkillArgs) -> bool { self.on_update_end(args) }
    /// 防御之前
    fn pre_defend(&mut self, mut atp: f64, caster: PlrId, is_mag: bool, on_damage: &OnDamageFunc, args: SkillArgs) -> f64 { atp }
    fn pre_defend_with_level(
        &mut self,
        _level: u32,
        atp: f64,
        caster: PlrId,
        is_mag: bool,
        on_damage: &OnDamageFunc,
        args: SkillArgs,
    ) -> f64 {
        self.pre_defend(atp, caster, is_mag, on_damage, args)
    }
    /// 防御之后
    fn post_defend(&mut self, mut dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 { dmg }
    fn post_defend_with_level(&mut self, _level: u32, dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        self.post_defend(dmg, caster, on_damage, args)
    }
    /// 伤害之后
    fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {}
    fn post_damage_with_level(&mut self, _level: u32, dmg: i32, caster: PlrId, args: SkillArgs) {
        self.post_damage(dmg, caster, args)
    }
    /// 死亡时（返回 true 表示短路，不再执行后续 die）
    fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool { false }
    fn die_with_level(&mut self, _level: u32, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool {
        self.die(oldhp, caster, args)
    }
    /// 击杀目标后（返回 true 表示短路，不再执行后续 kill）
    fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { false }
    fn kill_with_level(&mut self, _level: u32, target: PlrId, args: SkillArgs) -> bool { self.kill(target, args) }

    /// 声明该技能注册到哪些流程
    fn proc_kinds(&self) -> &[ProcKind] { &[] }

    /// 技能触发概率（默认对齐 Dart: r127 < level）
    fn prob(&self, level: u32, _smart: bool, args: SkillArgs) -> bool { args.1.r127() < level }

    /// 技能目标来源域。
    fn target_domain(&self) -> SkillTargetDomain { SkillTargetDomain::EnemyAlive }
    fn target_domain_with_level(&self, _level: u32) -> SkillTargetDomain { self.target_domain() }

    /// 技能选目标数量（默认对齐 Dart）
    fn select_target_count(&self, smart: bool) -> usize { if smart { 3 } else { 2 } }
    fn select_target_count_with_level(&self, _level: u32, smart: bool) -> usize { self.select_target_count(smart) }

    /// 技能目标合法性判定
    fn valid_target(&self, _target: PlrId, _smart: bool, _args: SkillArgs) -> bool { true }
    fn valid_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        self.valid_target(target, smart, args)
    }

    /// 技能目标打分（默认对齐 Dart 基础行为）
    fn score_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        let Some(target_plr) = args.3.get_player(&target) else {
            return f64::MIN;
        };
        if smart {
            let rate_hi_hp = |hp: i32| -> f64 {
                if hp < 20 {
                    30.0
                } else if hp > 300 {
                    300.0
                } else {
                    hp as f64
                }
            };
            let rate_low_hp = |hp: i32| -> f64 { 1.0 / rate_hi_hp(hp) };
            let alive_group_count = {
                let mut group_heads = Vec::new();
                for id in args.3.all_player_ids() {
                    let alive = args.3.get_player(&id).map(|plr| plr.alive()).unwrap_or(false);
                    if !alive {
                        continue;
                    }
                    let Some(group) = args.3.group_containing(id) else {
                        continue;
                    };
                    let Some(head) = group.first() else {
                        continue;
                    };
                    if !group_heads.contains(head) {
                        group_heads.push(*head);
                    }
                }
                group_heads.len()
            };
            let target_alive_group_len = args
                .3
                .group_containing(target)
                .map(|group| {
                    group
                        .iter()
                        .filter(|id| args.3.get_player(id).map(|plr| plr.alive()).unwrap_or(false))
                        .count()
                })
                .unwrap_or(0);
            let status = target_plr.get_status();
            if alive_group_count > 2 {
                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
            } else {
                rate_low_hp(status.hp) * status.atk_sum as f64 * status.attract
            }
        } else {
            args.1.rFFFF() as f64 + target_plr.get_status().attract
        }
    }
    fn score_target_with_level(&self, _level: u32, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        self.score_target(target, smart, args)
    }

    /// 技能选目标流程（默认：按 valid 过滤，随机采样后按 score 排序）
    fn select_targets(&self, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        let select_count = self.select_target_count(smart);
        if select_count == 0 {
            return Vec::new();
        }
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(idx) = args.1.pick(candidates) else {
                return Vec::new();
            };
            let target = candidates[idx];
            if !self.valid_target(target, smart, (args.0, args.1, args.2, args.3)) {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return Vec::new();
        }

        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_target(target, smart, (args.0, args.1, args.2, args.3))))
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.into_iter().map(|x| x.0).collect()
    }

    fn select_targets_with_level(&self, level: u32, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        let select_count = self.select_target_count_with_level(level, smart);
        if select_count == 0 {
            return Vec::new();
        }
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(idx) = args.1.pick(candidates) else {
                return Vec::new();
            };
            let target = candidates[idx];
            if !self.valid_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3)) {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return Vec::new();
        }

        let mut scored = selected
            .into_iter()
            .map(|target| {
                (
                    target,
                    self.score_target_with_level(level, target, smart, (args.0, args.1, args.2, args.3)),
                )
            })
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.into_iter().map(|x| x.0).collect()
    }

    /// 标记该技能的主动施放逻辑是否已接入当前运行链路。
    fn has_action_impl(&self) -> bool { false }

    fn is_normal_skill(&self) -> bool { true }

    fn is_boss_skill(&self) -> bool { false }

    fn is_weapon_skill(&self) -> bool { false }
}

impl Clone for Box<dyn SkillTrait> {
    fn clone(&self) -> Box<dyn SkillTrait> { self.clone_box() }
}

pub trait SkillExt: SkillTrait + Any {
    fn box_new() -> Box<dyn SkillTrait>;
}

#[derive(Debug, Clone)]
pub struct Skill {
    /// 是否被增强过
    pub boosted: bool,
    /// 等级
    level: u32,
    /// 类型
    skill_type: Box<dyn SkillTrait>,
    /// 目标
    pub target: Option<PlrId>,
}

impl Skill {
    pub fn new(level: u32, skill_type: Box<dyn SkillTrait>) -> Self {
        Self {
            boosted: false,
            level,
            skill_type,
            target: None,
        }
    }

    pub fn new_with_id(level: u32, id: u8) -> Self {
        let skill_type = {
            match id {
                0 => fire::FireSkill::box_new(),
                1 => ice::IceSkill::box_new(),
                2 => thunder::ThunderSkill::box_new(),
                3 => quake::QuakeSkill::box_new(),
                4 => absorb::AbsorbSkill::box_new(),
                5 => poison::PoisonSkill::box_new(),
                6 => rapid::RapidSkill::box_new(),
                7 => critical::CriticalSkill::box_new(),
                8 => half::HalfSkill::box_new(),
                9 => exchange::ExchangeSkill::box_new(),
                10 => berserk::BerserkSkill::box_new(),
                11 => charm::CharmSkill::box_new(),
                12 => haste::HasteSkill::box_new(),
                13 => slow::SlowSkill::box_new(),
                14 => curse::CurseSkill::box_new(),
                15 => heal::HealSkill::box_new(),
                16 => revive::ReviveSkill::box_new(),
                17 => disperse::DisperseSkill::box_new(),
                18 => iron::IronSkill::box_new(),
                19 => charge::ChargeSkill::box_new(),
                20 => accumulate::AccumulateSkill::box_new(),
                21 => assassinate::AssassinateSkill::box_new(),
                22 => summon::SummonSkill::box_new(),
                23 => clone::CloneSkill::box_new(),
                24 => shadow::ShadowSkill::box_new(),
                25 => defend::DefendSkill::box_new(),
                26 => protect::ProtectSkill::box_new(),
                27 => reflect::ReflectSkill::box_new(),
                28 => reraise::ReraiseSkill::box_new(),
                29 => shield::ShieldSkill::box_new(),
                30 => counter::CounterSkill::box_new(),
                31 => merge::MergeSkill::box_new(),
                32 => zombie::ZombieSkill::box_new(),
                33 => upgrade::UpgradeSkill::box_new(),
                34 => hide::HideSkill::box_new(),
                _ => none::NoneSkill::box_new(),
            }
        };
        Self {
            boosted: false,
            level,
            skill_type,
            target: None,
        }
    }

    pub fn set_target(&mut self, target: PlrId) { self.target = Some(target); }

    pub fn get_target(&self) -> Option<PlrId> { self.target }

    /// 如果没 boost, 那就 boost 一下
    /// true: boost 成功
    /// false: 已经 boost 过了
    pub fn boost_if_not(&mut self) -> bool {
        if self.boosted {
            false
        } else {
            self.boosted = true;
            self.level *= 2;
            true
        }
    }

    pub fn boost_level(&mut self, level: u32) -> bool {
        if self.boosted {
            self.level += level;
            false
        } else {
            self.level += level;
            self.boosted = true;
            true
        }
    }

    /// 获取技能等级
    pub fn level(&self) -> u32 { self.level }

    pub fn set_level(&mut self, level: u32) { self.level = level; }

    // ==========
    // 以下是技能 call pre/post 之类的东西
    // ==========

    pub fn update_state(&mut self, args: SkillArgs) { self.skill_type.update_state_with_level(self.level, args) }

    pub fn update_state_inline(&mut self, status: &mut super::PlayerStatus) {
        self.skill_type.update_state_inline(self.level, status)
    }

    pub fn act(&mut self, targets: Vec<PlrId>, smart: bool, args: SkillArgs) {
        let current_level = self.level;
        self.skill_type.act_with_level(current_level, targets, smart, args);
        self.level = self.skill_type.post_act_level(current_level);
    }

    pub fn pre_step(&mut self, step: i32, args: SkillArgs) -> i32 { self.skill_type.pre_step_with_level(self.level, step, args) }

    pub fn pre_action(&mut self, args: SkillArgs) { self.skill_type.pre_action_with_level(self.level, args) }

    pub fn pre_action_select(&mut self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.pre_action_select_with_level(self.level, smart, args)
    }

    pub fn pre_action_clear_forced(&mut self, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.pre_action_clear_forced_with_level(self.level, smart, args)
    }

    pub fn post_action(&mut self, args: SkillArgs) { self.skill_type.post_action_with_level(self.level, args) }

    pub fn on_update_end(&mut self, args: SkillArgs) -> bool { self.skill_type.on_update_end_with_level(self.level, args) }

    pub fn pre_defend(&mut self, atp: f64, is_mag: bool, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> f64 {
        self.skill_type.pre_defend_with_level(self.level, atp, caster, is_mag, on_damage, args)
    }

    pub fn post_defend(&mut self, dmg: i32, caster: PlrId, on_damage: &OnDamageFunc, args: SkillArgs) -> i32 {
        self.skill_type.post_defend_with_level(self.level, dmg, caster, on_damage, args)
    }

    pub fn post_damage(&mut self, dmg: i32, caster: PlrId, args: SkillArgs) {
        self.skill_type.post_damage_with_level(self.level, dmg, caster, args)
    }

    pub fn die(&mut self, oldhp: i32, caster: PlrId, args: SkillArgs) -> bool {
        self.skill_type.die_with_level(self.level, oldhp, caster, args)
    }

    pub fn kill(&mut self, target: PlrId, args: SkillArgs) -> bool { self.skill_type.kill_with_level(self.level, target, args) }

    pub fn proc_kinds(&self) -> &[ProcKind] { self.skill_type.proc_kinds() }

    pub fn prob(&self, smart: bool, args: SkillArgs) -> bool { self.skill_type.prob(self.level, smart, args) }

    pub fn target_domain(&self) -> SkillTargetDomain { self.skill_type.target_domain_with_level(self.level) }

    pub fn select_target_count(&self, smart: bool) -> usize { self.skill_type.select_target_count_with_level(self.level, smart) }

    pub fn valid_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> bool {
        self.skill_type.valid_target_with_level(self.level, target, smart, args)
    }

    pub fn score_target(&self, target: PlrId, smart: bool, args: SkillArgs) -> f64 {
        self.skill_type.score_target_with_level(self.level, target, smart, args)
    }

    pub fn select_targets(&self, candidates: &[PlrId], smart: bool, args: SkillArgs) -> Vec<PlrId> {
        self.skill_type.select_targets_with_level(self.level, candidates, smart, args)
    }

    pub fn has_action_impl(&self) -> bool { self.skill_type.has_action_impl() }

    // pub fn update_state(&self, status: &mut PlayerStatus) {
    //     match self.skill_type {
    //         SkillType::Accumulate { acc } => {
    //             status.at_boost *= acc;
    //         }
    //         SkillType::Charge => {
    //             status.at_boost *= 3.0;
    //         }
    //         SkillType::Iron { .. } => {
    //             status.attract *= 1.12;
    //         }
    //         SkillType::Hide => {
    //             status.attract /= 10.0;
    //             if self.level > 63 {
    //                 let boost_level = (self.level - 63) as i32;
    //                 status.agility += boost_level;
    //                 status.defense += boost_level;
    //                 status.resistance += boost_level;
    //             }
    //         }
    //         SkillType::Upgrade => {
    //             // 全属性 +30
    //             status.attack += 30;
    //             status.defense += 30;
    //             status.agility += 30;
    //             status.magic += 30;
    //             status.resistance += 30;
    //             // 但是这俩只加 20
    //             status.speed += 20;
    //             status.wisdom += 20;
    //         }
    //         SkillType::CharmState { charmed_group: _ } => {
    //             todo!("魅惑我还不知道咋写")
    //             /*
    //                                void updateState(Plr p) {
    //                  // 把目标拉到自己组
    //                  target.allyGroup = grp;
    //                }
    //             */
    //         }
    //         SkillType::CurseState { prob: _, multiply: _ } => {
    //             status.atk_sum *= 4;
    //         }
    //         SkillType::HasteState { faster } => {
    //             status.speed *= faster;
    //         }
    //         SkillType::IceState { .. } => {
    //             status.set_frozen(true);
    //         }

    //         _ => (),
    //     }
    // }

    // #[allow(clippy::single_match)]
    // pub fn pre_step(&mut self, step: i32, updates: &mut RunUpdates, status: &mut PlayerStatus) -> i32 {
    //     match &mut self.skill_type {
    //         SkillType::IceState { frozen_step } => {
    //             if step > 0 {
    //                 if *frozen_step > 0 {
    //                     *frozen_step -= step as u32;
    //                 } else if (step + status.move_point) >= 2048 {
    //                     // destroy
    //                     let target = self.target.expect("no target");

    //                     return 0;
    //                 }
    //             }
    //         }
    //         _ => {}
    //     }
    //     step
    // }

    // pub fn act(&mut self, args: Args) {
    //     match self.skill_type {
    //         SkillType::Iron { mut protect, mut step } => {
    //             let update = RunUpdate::new("[0]发动[铁壁]", plr, plr, 60);
    //             updates.add(update);
    //             let plr_mut = s.just_get_player_mut(plr).expect("faild to get self plr");
    //             // plr_mut.skill_store.post_defend.push();
    //             // plr_mut.skill_store.post_action.push();
    //             // plr_mut.skill_store.update
    //             // plr_mut.skill_store.meta.insert(TypeId::of::<SkillType::Iron>(), skill_idx);
    //             step = 3;
    //             if plr_mut
    //                 .skill_store
    //                 .meta
    //                 .contains_key(&SkillType::Charge.type_id())
    //                 // .iter()
    //                 // .map(|idx| plr_mut.skill_store.get_skill(*idx))
    //                 // .any(|skill| skill.skill_type == SkillType::Charge)
    //             {
    //                 step += 4;
    //                 protect += 240 + plr_mut.status.magic * 4;
    //             }
    //             plr_mut.set_move_point(plr_mut.move_point() - 256);
    //             let update = RunUpdate::new("[0]防御力大幅上升", plr, plr, 0);
    //             updates.add(update);
    //         }
    //         _ => {}
    //     }
    // }

    // pub fn pre_action(&mut self, plr: PlrId, r: &mut RC4, updates: &mut RunUpdates, s: &Arc<Storage>) {}

    // pub fn post_action(&mut self, plr: PlrId, r: &mut RC4, updates: &mut RunUpdates, s: &Arc<Storage>) {}

    // pub fn pre_defend(&mut self, plr: PlrId, mut atp: f64, r: &mut RC4, updates: &mut RunUpdates, s: &Arc<Storage>) -> f64 {
    //     atp
    // }

    // pub fn post_defend(
    //     &mut self,
    //     plr: PlrId,
    //     caster: PlrId,
    //     mut dmg: i32,
    //     r: &mut RC4,
    //     updates: &mut RunUpdates,
    //     s: &Arc<Storage>,
    // ) -> i32 {
    //     match self.skill_type {
    //         SkillType::Defend => {
    //             let plr_mut = s.just_get_player_mut(plr).expect("faild to get self plr");
    //             if r.r255() < self.level && plr_mut.mp_ready(r) {
    //                 let update = RunUpdate::new("[0][防御]", plr, plr, 40);
    //                 updates.add(update);
    //                 dmg / 2
    //             } else {
    //                 dmg
    //             }
    //         }
    //         SkillType::CurseState { prob, multiply } => {
    //             if dmg > 0 && (r.r63() as i32) < prob {
    //                 let update = RunUpdate::new("[诅咒]使伤害加倍", plr, caster, 0);
    //                 updates.add(update);
    //                 dmg * multiply
    //             } else {
    //                 dmg
    //             }
    //         }
    //         SkillType::Iron { mut protect, .. } => {
    //             if dmg > 0 {
    //                 if dmg <= protect {
    //                     dmg = 1;
    //                     protect -= dmg - 1;
    //                 } else {
    //                     dmg -= protect;
    //                     self.destroy();
    //                 }
    //                 dmg
    //             } else {
    //                 0
    //             }
    //         }
    //         _ => dmg,
    //     }
    // }

    // pub fn post_damage(&mut self, dmg: i32, caster: PlrId, r: &mut RC4, updates: &mut RunUpdates, s: &Arc<Storage>) {}

    // pub fn destroy(&self) { todo!() }
}

// ```dart
// MList<PreStepEntry> presteps = new MList<PreStepEntry>();
// MList<PreActionEntry> preactions = new MList<PreActionEntry>();
// MList<PostActionEntry> postactions = new MList<PostActionEntry>();
// MList<PreDefendEntry> predefends = new MList<PreDefendEntry>();
// MList<PostDefendEntry> postdefends = new MList<PostDefendEntry>();
// MList<PostDamageEntry> postdamages = new MList<PostDamageEntry>();
// MList<DieEntry> dies = new MList<DieEntry>();
// MList<KillEntry> kills = new MList<KillEntry>();
// ```
// #[derive(Debug, Clone, Default)]
// pub struct SkillStore {
//     /// 实际存储 skill 的地方
//     pub skill_store: Vec<Skill>,
//     /// meta??
//     pub meta: FoldHashMap<TypeId, usize>,
//     // 自己的状态 (usize: index)
//     /// 更新状态时?
//     pub update_states: Vec<usize>,
//     /// step 之前
//     pub pre_step: Vec<usize>,
//     /// 动作之前
//     pub pre_action: Vec<usize>,
//     /// 动作之后
//     pub post_action: Vec<usize>,
//     /// 防御之前
//     pub pre_defend: Vec<usize>,
//     /// 防御之后
//     pub post_defend: Vec<usize>,
//     /// 伤害之后
//     pub post_damage: Vec<usize>,
//     /// 死亡之后
//     pub post_death: Vec<usize>,
//     /// 干掉目标之后
//     pub post_kill: Vec<usize>,
//     // 别的什么东西
//     pub pending_clear_states: bool,
// }

// impl SkillStore {
//     pub fn new() -> Self {
//         Self {
//             skill_store: Vec::new(),
//             // 不再使用全局存储
//             update_states: Vec::new(),
//             meta: FoldHashMap::new(),
//             pre_step: Vec::new(),
//             pre_action: Vec::new(),
//             post_action: Vec::new(),
//             pre_defend: Vec::new(),
//             post_defend: Vec::new(),
//             post_damage: Vec::new(),
//             post_death: Vec::new(),
//             post_kill: Vec::new(),
//             pending_clear_states: false,
//         }
//     }

//     fn clear_proc(&mut self) {
//         self.pre_step.clear();
//         self.pre_action.clear();
//         self.post_action.clear();
//         self.pre_defend.clear();
//         self.post_defend.clear();
//         self.post_damage.clear();
//         self.post_death.clear();
//         self.post_kill.clear();
//     }

//     pub fn update_proc(&mut self) {
//         self.clear_proc();
//         for (idx, skill) in self.skill_store.iter().enumerate() {
//             let skill_type = &skill.skill_type;
//             match skill_type {
//                 SkillType::Counter => {
//                     self.post_damage.push(idx);
//                 }
//                 SkillType::Defend => {
//                     self.post_defend.push(idx);
//                 }
//                 SkillType::Hide => {
//                     self.post_damage.push(idx);
//                     self.pre_action.push(idx);
//                 }
//                 SkillType::Merge => {
//                     self.post_kill.push(idx);
//                 }
//                 SkillType::Protect => {
//                     self.post_action.push(idx);
//                 }
//                 SkillType::Reflect => {
//                     self.pre_defend.push(idx);
//                 }
//                 SkillType::Reraise => {
//                     self.post_death.push(idx);
//                 }
//                 SkillType::Shield => {
//                     self.pre_action.push(idx);
//                 }
//                 SkillType::Upgrade => {
//                     self.post_damage.push(idx);
//                 }
//                 SkillType::Zombie => {
//                     self.post_kill.push(idx);
//                 }
//                 // TODO: BOSS 技能
//                 SkillType::Slime => {
//                     self.post_damage.push(idx);
//                 }
//                 // TODO: 武器技能
//                 SkillType::DeathNote => {
//                     self.post_damage.push(idx);
//                 }

//                 _ => (),
//             }
//         }
//     }

//     /// 添加技能
//     pub fn add_skill(&mut self, skill: Skill) { self.skill_store.push(skill); }

//     pub fn get_skill(&self, idx: usize) -> &Skill { &self.skill_store[idx] }

//     pub fn get_skill_mut(&self, idx: usize) -> &mut Skill {
//         let slf = self as *const Self as *mut Self;
//         unsafe { &mut (&mut (*slf).skill_store)[idx] }
//     }
// }

// /// 技能类型
// /// 需要和游戏中的技能类型对应
// ///
// /// 因为不知道啥时候会加新的, 所以务必带上 `#[non_exhaustive]`
// #[derive(Debug, Clone, Copy, PartialEq)]
// #[non_exhaustive]
// pub enum SkillType {
//     /// 火球术
//     Fire,
//     /// 冰冻术
//     Ice { frozen_step: u32 },
//     /// 雷击术
//     Thunder,
//     /// 地裂术
//     Quake,
//     /// 吸血攻击
//     Absorb,
//     /// 投毒
//     Poison,
//     /// 连击
//     Rapid,
//     /// 会心一击
//     Critical,
//     /// 瘟疫
//     Plague,
//     /// 生命之轮
//     Life,
//     /// 狂暴术
//     Berserk,
//     /// 魅惑
//     Charm,
//     /// 加速术
//     Haste,
//     /// 减速术
//     Slow,
//     /// 诅咒
//     Curse,

//     /// 治愈魔法
//     Heal,
//     /// 苏生术
//     Revive,
//     /// 净化
//     Disperse,
//     /// 铁壁
//     Iron { protect: i32, step: u32 },

//     /// 蓄力
//     Charge,
//     /// 聚气
//     Accumulate { acc: f64 },

//     /// 潜行
//     Assassinate,

//     /// 血祭
//     Summon,
//     /// 分身
//     Clone,
//     /// 幻术
//     Shadow,

//     /// 防御
//     Defend,
//     /// 守护
//     Protect,
//     /// 伤害反弹
//     Reflect,
//     /// 护身符
//     Reraise,
//     /// 护盾
//     Shield,
//     /// 反击
//     Counter,
//     /// 吞噬
//     Merge,
//     /// 召唤亡灵
//     Zombie,
//     /// 垂死抗争
//     Upgrade,
//     /// 隐匿
//     Hide,

//     /// 无 (35-40)
//     None,

//     // 各种状态
//     /// 被魅惑
//     CharmState { charmed_group: u32 },
//     /// 被诅咒
//     CurseState { prob: i32, multiply: i32 },
//     /// 疾走状态
//     HasteState { faster: i32 },
//     /// 被冻结
//     IceState { frozen_step: u32 },
//     /// 被迟缓
//     SlowState { step: u32 },

//     // boss
//     /// 懒惰状态
//     LazyState,

//     // TODO: BOSS 技能
//     /// 史莱姆(分裂)
//     Slime,

//     // TODO: 武器技能
//     /// 死亡笔记
//     DeathNote,
//     /// Rinck 的修改器 (属性修改器)
//     RinickModifier,
// }

// impl SkillType {
//     pub fn new_from_skill_type_id(id: u8) -> Self {
//         match id {
//             0 => Self::Fire,
//             1 => Self::Ice { frozen_step: 1024 },
//             2 => Self::Thunder,
//             3 => Self::Quake,
//             4 => Self::Absorb,
//             5 => Self::Poison,
//             6 => Self::Rapid,
//             7 => Self::Critical,
//             8 => Self::Plague,
//             9 => Self::Life,
//             10 => Self::Berserk,
//             11 => Self::Charm,
//             12 => Self::Haste,
//             13 => Self::Slow,
//             14 => Self::Curse,

//             15 => Self::Heal,
//             16 => Self::Revive,
//             17 => Self::Disperse,
//             18 => Self::Iron { protect: 0, step: 0 },

//             19 => Self::Charge,
//             20 => Self::Accumulate { acc: 1.7 },

//             21 => Self::Assassinate,

//             22 => Self::Summon,
//             23 => Self::Clone,
//             24 => Self::Shadow,

//             25 => Self::Defend,
//             26 => Self::Protect,
//             27 => Self::Reflect,
//             28 => Self::Reraise,
//             29 => Self::Shield,
//             30 => Self::Counter,
//             31 => Self::Merge,
//             32 => Self::Summon,
//             33 => Self::Upgrade,
//             34 => Self::Hide,

//             35..40 => Self::None,
//             _ => Self::None,
//         }
//     }

//     /// 是否是普通技能
//     pub fn is_normal_skill(&self) -> bool {
//         matches!(
//             self,
//             SkillType::Fire
//                 | SkillType::Ice { .. }
//                 | SkillType::Thunder
//                 | SkillType::Quake
//                 | SkillType::Absorb
//                 | SkillType::Poison
//                 | SkillType::Rapid
//                 | SkillType::Critical
//                 | SkillType::Plague
//                 | SkillType::Life
//                 | SkillType::Berserk
//                 | SkillType::Charm
//                 | SkillType::Haste
//                 | SkillType::Slow
//                 | SkillType::Curse
//                 | SkillType::Heal
//                 | SkillType::Revive
//                 | SkillType::Disperse
//                 | SkillType::Iron { .. }
//                 | SkillType::Charge
//                 | SkillType::Accumulate { .. }
//                 | SkillType::Assassinate
//                 | SkillType::Summon
//                 | SkillType::Clone
//                 | SkillType::Shadow
//                 | SkillType::Defend
//                 | SkillType::Protect
//                 | SkillType::Reflect
//                 | SkillType::Reraise
//                 | SkillType::Shield
//                 | SkillType::Counter
//                 | SkillType::Merge
//                 | SkillType::Zombie
//                 | SkillType::Upgrade
//                 | SkillType::Hide
//         )
//     }

//     pub fn is_normal_state(&self) -> bool {
//         matches!(
//             self,
//             SkillType::SlowState { .. }
//                 | Self::CurseState { .. }
//                 | Self::IceState { .. }
//                 | Self::CharmState { .. }
//                 | Self::HasteState { .. }
//                 | Self::LazyState
//         )
//     }

//     /// 是否是 BOSS 技能
//     pub fn is_boss_skill(&self) -> bool { matches!(self, SkillType::Slime) }

//     /// 是否是武器技能
//     pub fn is_weapon_skill(&self) -> bool { matches!(self, SkillType::DeathNote) }
// }

/*
const char skillNameMap[] = {
    "火球术", "冰冻术", "雷击术", "地裂术", "吸血攻击", "投毒", "连击",
    "会心一击", "瘟疫", "生命之轮", "狂暴术", "魅惑", "加速术", "减速术",
    "诅咒", "治愈魔法", "苏生术", "净化", "铁壁", "蓄力", "聚气",
    "潜行", "血祭", "分身", "幻术", "防御", "守护", "伤害反弹",
    "护身符", "护盾", "反击", "吞噬", "召唤亡灵", "垂死抗争", "隐匿",
    "啧", "啧", "啧", "啧", "啧"};
string skillNameMap_2[35] = {
    "火球", "冰冻", "雷击", "地裂", "吸血", "投毒", "连击",
    "会心", "瘟疫", "命轮", "狂暴", "魅惑", "加速", "减速",
    "诅咒", "治愈", "苏生", "净化", "铁壁", "蓄力", "聚气",
    "潜行", "血祭", "分身", "幻术", "防御", "守护", "反弹",
    "护符", "护盾", "反击", "吞噬", "召灵", "垂死", "隐匿"};
    */

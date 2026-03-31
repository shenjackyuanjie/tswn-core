//! # 玩家运行时行为 (impl_runtime)
//!
//! 本模块实现 [`Player`] 的运行时行为，包括行动、攻击、被攻击等。
//!
//! ## 功能说明
//!
//! - **玩家行动** — `step()` 实现每回合中的玩家行动
//! - **攻击处理** — `action()` 实现玩家攻击逻辑
//! - **被攻击处理** — `attacked()` 实现被攻击逻辑
//! - **更新结束** — `on_update_end()` 处理回合结束
//! - **状态管理** — 各种状态相关的处理函数
//!
//! ## 行动流程
//!
//! `step()` 实现每回合中的玩家行动，包括：
//!
//! 1. **Pre-Step** — 行动前，计算移动点数
//! 2. **Pre-Action** — 行动前，目标选择前
//! 3. **Main Action** — 主行动（攻击、技能等）
//! 4. **Post-Action** — 行动后
//!
//! ## 攻击流程
//!
//! `attacked()` 实现被攻击逻辑，包括：
//!
//! 1. **Pre-Defends** — 被攻击前，遍历所有 predefend entry
//! 2. **Dodge** — 闪避判定
//! 3. **Defend** — 防御处理
//! 4. **Damage** — 伤害计算
//! 5. **Post-Damages** — 被攻击后，遍历所有 postdamage entry
//! 6. **OnDamaged** — 受到伤害后处理
//! 7. **OnDie** — 死亡处理
//!
//! ## 状态处理
//!
//! - **Pre-Step States** — 行动前状态处理
//! - **Pre-Action States** — 行动前状态处理
//! - **Post-Action States** — 行动后状态处理
//! - **Pre-Defend States** — 被攻击前状态处理
//! - **Post-Defend States** — 被攻击后状态处理
//! - **Post-Damage States** — 造成伤害后状态处理
//!
//! ## 示例
//!
//! ```rust,ignore
//! use tswn_core::player::Player;
//! use tswn_core::rc4::RC4;
//! use tswn_core::engine::update::RunUpdates;
//!
//! let mut player = /* ... */;
//! let mut randomer = RC4::new(&[0], 2);
//! let mut updates = RunUpdates::new();
//! let targets = /* ... */;
//!
//! player.step(&mut randomer, &mut updates, &storage, &targets);
//! ```

use super::*;
use smallvec::SmallVec;

// JS addNew 之后的新单位会立刻参与“战斗是否结束”的判断，
// Rust 侧这里也要把敌方 pending spawn 视为仍然存活的敌人。
fn has_alive_enemy_or_pending(storage: &Arc<Storage>, ally_group: &[PlrId]) -> bool {
    let all_ids = storage.all_player_ids();
    if all_ids
        .iter()
        .any(|id| !ally_group.contains(id) && storage.get_player(id).map(|plr| plr.alive()).unwrap_or(false))
    {
        return true;
    }

    all_ids
        .into_iter()
        .filter(|owner_id| !ally_group.contains(owner_id))
        .flat_map(|owner_id| storage.pending_spawn_ids_for_owner(owner_id))
        .any(|pending_id| storage.get_pending_spawn_player(pending_id).map(|plr| plr.alive()).unwrap_or(false))
}

impl Player {
    pub fn update_player(&mut self) {
        self.init_skills();
        self.update_states();
    }

    pub fn step(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        if !self.status.alive() {
            return;
        }
        let step_byte = randomer.next_u8();
        let step_roll = step_byte & 3;
        #[cfg(not(feature = "no_debug"))]
        if crate::debug::debug_action_matches(&self.id_name()) {
            eprintln!(
                "[step_roll] actor={} id={} rc4=({}, {}) byte={} roll={} speed={} step={}",
                self.id_name(),
                self.as_ptr(),
                randomer.i,
                randomer.j,
                step_byte,
                step_roll,
                self.status.speed,
                self.status.speed * step_roll as i32,
            );
        }
        let mut stp = self.status.speed * step_roll as i32;
        stp = self.apply_pre_step_states(stp, updates);
        let ptr = self.as_ptr();
        stp = self.skills.pre_step(stp, (ptr, randomer, updates, storage));
        self.status.move_point += stp;
        if self.check_move() {
            self.status.move_point -= MOVE_POINT_THRESHOLD;
            self.action(randomer, updates, storage, targets);
        }
    }

    pub fn action(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        let smart_byte = randomer.next_u8();
        let smart_roll = (smart_byte & 63) as i32;
        let smart = self.status.wisdom > smart_roll;
        let ptr = self.as_ptr();
        let pre_action_outcome = self.skills.pre_action(smart, (ptr, randomer, updates, storage));
        #[cfg(not(feature = "no_debug"))]
        let debug_action_this = crate::debug::debug_action_matches(&self.id_name());
        #[cfg(not(feature = "no_debug"))]
        if debug_action_this {
            eprintln!(
                "[action] actor={} id={} rc4=({}, {}) smart_byte={} smart_roll={} wisdom={} smart={} mp={} forced_skill={:?} clear_forced={}",
                self.id_name(),
                self.as_ptr(),
                randomer.i,
                randomer.j,
                smart_byte,
                smart_roll,
                self.status.wisdom,
                smart,
                self.status.mp,
                pre_action_outcome.forced_skill,
                pre_action_outcome.clear_forced_action,
            );
        }
        if self.status.frozed() {
            return;
        }

        let state_hijacked = self.state.on_pre_action_states(self.as_ptr(), smart, randomer, updates, storage, targets);
        if state_hijacked {
            let recover_threshold = self.status.wisdom + 64;
            if (randomer.r127() as i32) < recover_threshold {
                self.status.mp += 16;
            }
            updates.emit(RunUpdate::new_newline);
            self.run_post_action_chain(randomer, updates, storage);
            return;
        }

        let mut acted = false;
        let mut selected_skill_key: Option<usize> = pre_action_outcome.forced_skill;
        let mut selected_targets: Vec<PlrId> = Vec::new();
        let selected_from_forced_pre_action = pre_action_outcome.forced_skill.is_some();
        let forced_attack = if pre_action_outcome.clear_forced_action || pre_action_outcome.forced_skill.is_some() {
            None
        } else {
            self.state.resolve_action_mode(smart)
        };
        if let Some(forced_attack) = forced_attack {
            self.forced_attack(forced_attack, randomer, updates, storage, targets);
            // 对齐 JS：强制攻击（狂暴）击倒最后一个敌人后，battle 已结束，
            // 不再执行 forced_action_states（避免额外的"从狂暴中解除"日志）。
            let battle_ended_early = storage
                .group_containing(ptr)
                .map(|ally_group| !has_alive_enemy_or_pending(storage, ally_group))
                .unwrap_or(false);
            if !battle_ended_early {
                self.apply_forced_action_states(randomer, updates, storage);
            }
            acted = true;
        } else {
            if selected_skill_key.is_none() {
                let req_mp_byte = randomer.next_u8();
                let req_mp = (req_mp_byte & 15) as i32 + 8;
                #[cfg(not(feature = "no_debug"))]
                if debug_action_this {
                    eprintln!(
                        "[action_req_mp] actor={} id={} rc4=({}, {}) req_mp_byte={} req_mp={} mp_before={}",
                        self.id_name(),
                        self.as_ptr(),
                        randomer.i,
                        randomer.j,
                        req_mp_byte,
                        req_mp,
                        self.status.mp,
                    );
                }
                if self.status.mp >= req_mp {
                    let is_boss = self.player_type == PlayerType::Boss;
                    if !is_boss {
                        let skill_keys = self.skills.skill.clone();
                        for key in skill_keys {
                            if !self.skills.action_enabled(key) {
                                #[cfg(not(feature = "no_debug"))]
                                if debug_action_this {
                                    let skill = self.skills.skill_by_id(key);
                                    eprintln!(
                                        "[action_skill] actor={} id={} rc4=({}, {}) key={} type={} level={} enabled=false",
                                        self.id_name(),
                                        self.as_ptr(),
                                        randomer.i,
                                        randomer.j,
                                        key,
                                        skill.debug_skill_type_name(),
                                        skill.level(),
                                    );
                                }
                                continue;
                            }
                            let maybe_targets = {
                                let skill = self.skills.skill_by_id(key);
                                let action_ok = skill.has_action_impl();
                                let level_ok = skill.level() > 0;
                                let prob_ok = level_ok && action_ok && skill.prob(smart, (ptr, randomer, updates, storage));
                                #[cfg(not(feature = "no_debug"))]
                                if debug_action_this {
                                    eprintln!(
                                        "[action_skill] actor={} id={} rc4=({}, {}) key={} type={} level={} action_ok={} level_ok={} prob_ok={}",
                                        self.id_name(),
                                        self.as_ptr(),
                                        randomer.i,
                                        randomer.j,
                                        key,
                                        skill.debug_skill_type_name(),
                                        skill.level(),
                                        action_ok,
                                        level_ok,
                                        prob_ok,
                                    );
                                }
                                if !(level_ok && action_ok && prob_ok) {
                                    None
                                } else {
                                    let selected = self.select_skill_targets(skill, smart, randomer, updates, storage, targets);
                                    let allow_empty = skill.target_domain() == SkillTargetDomain::SelfOnly;
                                    #[cfg(not(feature = "no_debug"))]
                                    if debug_action_this {
                                        eprintln!(
                                            "[action_skill_targets] actor={} id={} rc4=({}, {}) key={} allow_empty={} selected={:?}",
                                            self.id_name(),
                                            self.as_ptr(),
                                            randomer.i,
                                            randomer.j,
                                            key,
                                            allow_empty,
                                            selected,
                                        );
                                    }
                                    if selected.is_empty() && !allow_empty {
                                        None
                                    } else {
                                        Some(selected)
                                    }
                                }
                            };
                            if let Some(selected) = maybe_targets {
                                selected_skill_key = Some(key);
                                selected_targets = selected;
                                break;
                            }
                        }
                    } else {
                        let prob_count = crate::player::boss::boss_action_prob_count(&self.name);
                        for _ in 0..prob_count {
                            let _ = randomer.r127();
                        }
                    }
                    self.status.mp -= req_mp;
                }
            } else if let Some(skill_key) = selected_skill_key {
                selected_targets = {
                    let skill = self.skills.skill_by_id(skill_key);
                    if selected_from_forced_pre_action {
                        self.select_forced_skill_targets(skill, smart, randomer, updates, storage, targets)
                    } else {
                        self.select_skill_targets(skill, smart, randomer, updates, storage, targets)
                    }
                };
            }

            if let Some(skill_key) = selected_skill_key {
                #[cfg(not(feature = "no_debug"))]
                if debug_action_this {
                    let skill = self.skills.skill_by_id(skill_key);
                    eprintln!(
                        "[action_choice] actor={} id={} rc4=({}, {}) selected_skill={} type={} targets={:?} forced_pre_action={}",
                        self.id_name(),
                        self.as_ptr(),
                        randomer.i,
                        randomer.j,
                        skill_key,
                        skill.debug_skill_type_name(),
                        selected_targets,
                        selected_from_forced_pre_action,
                    );
                }
                let allow_empty = {
                    let skill = self.skills.skill_by_id(skill_key);
                    skill.target_domain() == SkillTargetDomain::SelfOnly
                };
                if !selected_targets.is_empty() || allow_empty {
                    let skill = self.skills.skill_by_id_mut(skill_key);
                    skill.act(selected_targets, smart, (ptr, randomer, updates, storage));
                    acted = true;
                }
            }
        }

        if !acted {
            #[cfg(not(feature = "no_debug"))]
            if debug_action_this {
                eprintln!(
                    "[action_choice] actor={} id={} rc4=({}, {}) fallback=default_attack",
                    self.id_name(),
                    self.as_ptr(),
                    randomer.i,
                    randomer.j,
                );
            }
            self.default_attack(smart, randomer, updates, storage, targets);
        }

        // 对齐 JS：当本次行动导致战场上只剩当前阵营存活时，round 会被中断（类似 throw 退出），
        // 不再继续执行当前 actor 的 recover/newline/post_action 链路。
        let battle_ended = storage
            .group_containing(ptr)
            .map(|ally_group| !has_alive_enemy_or_pending(storage, ally_group))
            .unwrap_or(false);
        if battle_ended {
            return;
        }

        let recover_threshold = self.status.wisdom + 64;
        if (randomer.r127() as i32) < recover_threshold {
            self.status.mp += 16;
        }
        #[cfg(not(feature = "no_debug"))]
        if debug_action_this {
            eprintln!(
                "[action_after_recover] actor={} id={} rc4=({}, {}) mp={} hp={}",
                self.id_name(),
                self.as_ptr(),
                randomer.i,
                randomer.j,
                self.status.mp,
                self.status.hp,
            );
        }
        updates.emit(RunUpdate::new_newline);
        self.run_post_action_chain(randomer, updates, storage);
        #[cfg(not(feature = "no_debug"))]
        if debug_action_this {
            eprintln!(
                "[action_after_post_action] actor={} id={} rc4=({}, {}) mp={} hp={}",
                self.id_name(),
                self.as_ptr(),
                randomer.i,
                randomer.j,
                self.status.mp,
                self.status.hp,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if debug_action_this {
            eprintln!(
                "[action_end] actor={} id={} rc4=({}, {}) mp={} hp={}",
                self.id_name(),
                self.as_ptr(),
                randomer.i,
                randomer.j,
                self.status.mp,
                self.status.hp,
            );
        }
    }

    pub(super) fn run_post_action_chain(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        let ptr = self.as_ptr();
        // JS 的 x2 是一条统一 post_action 队列；
        // - 初始技能（如 Protect）先于后续 runtime 挂入的 PoisonState
        // - 但如果战斗中途才通过 Merge 获得 Protect，这个新 entry 会挂到已有 PoisonState 后面
        // - 后续再新增的 state 会继续排到这个新 Protect 后面，不能粗暴地“永远放在所有 state 后面”
        // - Charge / Haste / Slow 这类 PostActionImpl(ga4=Infinity) 则继续留在尾部
        self.skills.post_action_early((ptr, randomer, updates, storage));
        self.apply_post_action_states(randomer, updates, storage);
        self.skills.post_action_late((ptr, randomer, updates, storage));
    }

    pub fn on_update_end(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) -> bool {
        let ptr = self.as_ptr();
        self.skills.on_update_end((ptr, randomer, updates, storage))
    }

    fn pick_enemy_target(targets: &ActionTargets, randomer: &mut RC4) -> Option<PlrId> {
        if targets.all_alive.is_empty() {
            return None;
        }
        let mut skip_indices = Vec::new();
        for (idx, plr_id) in targets.all_alive.iter().enumerate() {
            if targets.ally_alive.contains(plr_id) {
                skip_indices.push(idx);
            }
        }
        if skip_indices.is_empty() {
            randomer.pick(&targets.all_alive).map(|idx| targets.all_alive[idx])
        } else {
            randomer
                .pick_skip_range(&targets.all_alive, skip_indices)
                .map(|idx| targets.all_alive[idx])
        }
    }

    fn pick_ally_target(&self, targets: &ActionTargets, randomer: &mut RC4) -> Option<PlrId> {
        if targets.ally_alive.is_empty() {
            return None;
        }
        randomer.pick(&targets.ally_alive).map(|idx| targets.ally_alive[idx])
    }

    fn pick_target_by_domain(&self, domain: SkillTargetDomain, targets: &ActionTargets, randomer: &mut RC4) -> Option<PlrId> {
        match domain {
            SkillTargetDomain::EnemyAlive => Self::pick_enemy_target(targets, randomer),
            SkillTargetDomain::AllyAlive => self.pick_ally_target(targets, randomer),
            SkillTargetDomain::AllyAny => randomer.pick(&targets.ally_all).map(|idx| targets.ally_all[idx]),
            SkillTargetDomain::AllyDead => randomer.pick(&targets.ally_dead).map(|idx| targets.ally_dead[idx]),
            SkillTargetDomain::SelfOnly => Some(self.as_ptr()),
            SkillTargetDomain::AllAlive => randomer.pick(&targets.all_alive).map(|idx| targets.all_alive[idx]),
        }
    }

    /// NOTE:
    /// 这里不能直接退化成：
    ///   skill.select_targets(candidates, smart, ...)
    ///
    /// 之前已经反复验证过，这种"看起来更统一"的改法会把当前对局随机流打歪，
    /// 典型回归就是 `fight_multi_6` 会重新失败。根因是当前 Rust 侧的主动技能选目标
    /// 语义并不完全等同于"先构出 candidate 列表，再走 trait 默认 select_targets"：
    ///
    /// 1. `EnemyAlive` 在 JS 产物里对应的是基于 `all_alive + pickSkipRange` 的抽样语义，
    ///    不是一个纯粹的 `enemy_alive` 紧凑列表。
    /// 2. `AllyAny` 需要保持 Dart 的 `group.players` 语义，也就是 team roster 视图；
    ///    它和 `AllyAlive` / `AllAlive` 是不同维度的数据，不能互相替代。
    /// 3. 现有部分技能虽然实现了 `select_targets_with_level`，但如果全量切换到统一入口，
    ///    会改变随机数消费顺序和重复/无效目标处理细节，从而造成隐藏 RC4 漂移。
    ///
    /// 因此这里先保留"按 domain 手工抽样，再按 valid/score 排序"的稳定路径。
    /// 如果后续要接入某个技能自己的特殊选目标逻辑，应该做"逐技能 opt-in" 的窄改，
    /// 而不是把整个主动技能入口一次性切到 `skill.select_targets(...)`。
    fn select_skill_targets(
        &self,
        skill: &crate::player::skill::Skill,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> Vec<PlrId> {
        let domain = skill.target_domain();
        if domain == SkillTargetDomain::SelfOnly {
            return vec![self.as_ptr()];
        }
        let select_count = skill.select_target_count(smart);
        if select_count == 0 {
            return Vec::new();
        }

        if skill.uses_custom_target_selection() {
            let candidates: &[PlrId] = match domain {
                SkillTargetDomain::EnemyAlive => &targets.enemy_alive,
                SkillTargetDomain::AllyAlive => &targets.ally_alive,
                SkillTargetDomain::AllyAny => &targets.ally_all,
                SkillTargetDomain::AllyDead => &targets.ally_dead,
                SkillTargetDomain::AllAlive => &targets.all_alive,
                SkillTargetDomain::SelfOnly => &[],
            };
            return skill.select_targets(candidates, smart, (self.as_ptr(), randomer, updates, storage));
        }

        let mut selected: SmallVec<[PlrId; 4]> = SmallVec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(target_id) = self.pick_target_by_domain(domain, targets, randomer) else {
                return Vec::new();
            };
            let valid = skill.valid_target(target_id, smart, (self.as_ptr(), randomer, updates, storage));
            if !valid {
                invalid += 1;
                continue;
            }
            if selected.contains(&target_id) {
                dup += 1;
                continue;
            }
            selected.push(target_id);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return Vec::new();
        }

        let mut scored: SmallVec<[(PlrId, f64); 4]> = selected
            .into_iter()
            .map(|target_id| {
                (
                    target_id,
                    skill.score_target(target_id, smart, (self.as_ptr(), randomer, updates, storage)),
                )
            })
            .collect();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|x| x.0).collect()
    }

    fn select_forced_skill_targets(
        &self,
        skill: &crate::player::skill::Skill,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> Vec<PlrId> {
        let domain = skill.target_domain();
        if domain == SkillTargetDomain::SelfOnly {
            return vec![self.as_ptr()];
        }
        let candidates: &[PlrId] = match domain {
            SkillTargetDomain::EnemyAlive => &targets.enemy_alive,
            SkillTargetDomain::AllyAlive => &targets.ally_alive,
            SkillTargetDomain::AllyAny => &targets.ally_all,
            SkillTargetDomain::AllyDead => &targets.ally_dead,
            SkillTargetDomain::AllAlive => &targets.all_alive,
            SkillTargetDomain::SelfOnly => &[],
        };
        if candidates.is_empty() {
            return Vec::new();
        }
        skill.select_targets(candidates, smart, (self.as_ptr(), randomer, updates, storage))
    }

    fn select_forced_attack_target(
        &self,
        config: ForcedAttackConfig,
        randomer: &mut RC4,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> Option<PlrId> {
        if config.target_domain == ForcedAttackTargetDomain::EnemyAlive && config.score_mode == ForcedAttackScoreMode::Default {
            return self.select_default_attack_target(config.smart, randomer, storage, targets);
        }
        let select_count = if config.smart { 3 } else { 2 };
        let mut selected: SmallVec<[PlrId; 4]> = SmallVec::new();
        let mut dup = 0usize;
        while dup <= select_count {
            let target_id = match config.target_domain {
                ForcedAttackTargetDomain::EnemyAlive => Self::pick_enemy_target(targets, randomer)?,
                ForcedAttackTargetDomain::AllAlive => randomer.pick(&targets.all_alive).map(|idx| targets.all_alive[idx])?,
            };
            if selected.contains(&target_id) {
                dup += 1;
                continue;
            }
            selected.push(target_id);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return None;
        }

        let mut scored: SmallVec<[(PlrId, f64); 4]> = selected
            .into_iter()
            .map(|target_id| {
                let score = storage
                    .get_player(&target_id)
                    .map(|target| match config.score_mode {
                        ForcedAttackScoreMode::Default => randomer.rFFFF() as f64 + target.get_status().attract,
                        ForcedAttackScoreMode::RandomAttract => randomer.rFFFF() as f64 * target.get_status().attract,
                    })
                    .unwrap_or(f64::MIN);
                (target_id, score)
            })
            .collect();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.first().map(|x| x.0)
    }

    fn forced_attack(
        &mut self,
        config: ForcedAttackConfig,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) {
        let Some(target_id) = self.select_forced_attack_target(config, randomer, storage, targets) else {
            return;
        };
        let atp = self.get_at(config.use_mag, randomer) * config.attack_scale;
        updates.emit(|| RunUpdate::new(config.message, self.as_ptr(), target_id, 0));
        let Some(target) = storage.just_get_player_mut(target_id) else {
            return;
        };
        target.attacked(atp, config.use_mag, self.as_ptr(), noop_on_damage, randomer, updates, storage);
    }

    pub fn select_default_attack_target(
        &self,
        smart: bool,
        randomer: &mut RC4,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) -> Option<PlrId> {
        let select_count = if smart { 3 } else { 2 };
        let mut selected: SmallVec<[PlrId; 4]> = SmallVec::new();
        let mut dup = 0usize;
        while dup <= select_count {
            let target_id = Self::pick_enemy_target(targets, randomer)?;
            if selected.contains(&target_id) {
                dup += 1;
                continue;
            }
            selected.push(target_id);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return None;
        }

        let mut scored: SmallVec<[(PlrId, f64); 4]> = selected
            .into_iter()
            .map(|target_id| {
                let score = storage
                    .get_player(&target_id)
                    .map(|target| {
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
                            let alive_group_count = storage.alive_group_count();
                            let target_alive_group_len = storage
                                .alive_group_at_team_of(target_id)
                                .map(|alive_group| alive_group.len())
                                .unwrap_or(0);
                            let status = target.get_status();
                            if alive_group_count > 2 {
                                rate_hi_hp(status.hp) * target_alive_group_len as f64 * status.attract
                            } else {
                                (1.0 / rate_hi_hp(status.hp)) * status.atk_sum as f64 * status.attract
                            }
                        } else {
                            randomer.rFFFF() as f64 + target.get_status().attract
                        }
                    })
                    .unwrap_or(f64::MIN);
                (target_id, score)
            })
            .collect();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        scored.first().map(|x| x.0)
    }

    fn default_attack(
        &mut self,
        smart: bool,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        targets: &ActionTargets,
    ) {
        if self.player_type == PlayerType::Boss {
            crate::player::boss::boss_default_action(self, smart, randomer, updates, storage, targets);
            return;
        }
        let Some(target_id) = self.select_default_attack_target(smart, randomer, storage, targets) else {
            return;
        };

        if smart && self.status.magic > self.status.attack {
            let req_mp = (self.status.magic - self.status.attack) >> 2;
            if self.status.mp >= req_mp {
                self.status.mp -= req_mp;
                let atp = self.get_at(true, randomer);
                updates.emit(|| RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
                storage
                    .just_get_player_mut(target_id)
                    .expect("cannot get default-attack target from storage")
                    .attacked(atp, true, self.as_ptr(), noop_on_damage, randomer, updates, storage);
                return;
            }
        }

        let atp = self.get_at(false, randomer);
        updates.emit(|| RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
        storage
            .just_get_player_mut(target_id)
            .expect("cannot get default-attack target from storage")
            .attacked(atp, false, self.as_ptr(), noop_on_damage, randomer, updates, storage);
    }

    #[inline]
    pub fn active(&self) -> bool { self.status.hp > 0 && !self.status.frozed() }

    #[inline]
    pub fn alive(&self) -> bool { self.status.alive() }

    #[inline]
    pub fn apply_raw_damage(&mut self, dmg: i32) {
        self.status.hp -= dmg;
        if self.status.hp < 0 {
            self.status.hp = 0;
        }
    }

    #[inline]
    pub fn heal(&mut self, amount: i32) {
        self.status.hp += amount;
        if self.status.hp > self.status.max_hp {
            self.status.hp = self.status.max_hp;
        }
    }

    #[inline]
    pub fn revive_with_hp(&mut self, hp: i32) {
        self.status.hp = hp.clamp(1, self.status.max_hp.max(1));
        self.status.set_alive(true);
        // JS 的 reraise / revive 仅设置 HP，不清除冰冻状态
    }

    #[inline]
    pub fn set_state<T: StateTrait + 'static>(&mut self, state: T) {
        self.state.set(state);
        self.update_states();
    }

    #[inline]
    pub fn get_state<T: StateTrait + 'static>(&self) -> Option<&T> { self.state.get::<T>() }

    #[inline]
    pub fn get_state_mut<T: StateTrait + 'static>(&mut self) -> Option<&mut T> { self.state.get_mut::<T>() }

    #[inline]
    pub fn has_state<T: StateTrait + 'static>(&self) -> bool { self.state.has::<T>() }

    #[inline]
    pub fn clear_state<T: StateTrait + 'static>(&mut self) {
        self.state.clear::<T>();
        self.update_states();
    }

    #[inline]
    pub fn clear_negative_states(&mut self) {
        self.state.clear_negative_states();
        self.update_states();
    }

    #[inline]
    pub fn clear_positive_states(&mut self) {
        self.state.clear_positive_states();
        self.update_states();
    }

    #[inline]
    pub fn clear_positive_states_with_messages(&mut self) -> Vec<&'static str> {
        // 对齐 JS: 净化在 onDamage 回调中触发清状态，发生在目标本次受击“死亡结算前”。
        // 若当前 hp<=0（已被这次伤害打空）则不应输出“被打消”类取消文案。
        let alive = self.alive() && self.status.hp > 0;
        let messages = self.state.clear_positive_states_with_messages(alive);
        self.update_states();
        messages
    }

    #[inline]
    pub fn clear_positive_states_with_ordered_messages(&mut self) -> Vec<(i32, &'static str)> {
        let alive = self.alive() && self.status.hp > 0;
        let messages = self.state.clear_positive_states_with_ordered_messages(alive);
        self.update_states();
        messages
    }

    pub(super) fn apply_update_state_effects(&mut self) { self.state.apply_update_state_effects(&mut self.status); }

    pub(super) fn apply_pre_step_states(&mut self, mut step: i32, updates: &mut RunUpdates) -> i32 {
        let status_snapshot = self.status;
        let clear_tags = self.state.on_pre_step_states(self.as_ptr(), &status_snapshot, &mut step, updates);
        if !clear_tags.is_empty() {
            for tag in clear_tags {
                self.state.clear_tag(tag);
            }
            self.update_states();
        }
        step
    }

    fn apply_post_action_states(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        let owner_id = self.as_ptr();
        let deferred = self.skills.post_action_after_states.clone();
        let mut deferred_idx = 0usize;
        let mut clear_tags = smallvec::SmallVec::<[crate::player::StateTag; 8]>::new();
        for (tag, state_order) in self.state.ordered_post_action_tags_with_order() {
            while deferred_idx < deferred.len() && deferred[deferred_idx].0 <= state_order {
                let skill_key = deferred[deferred_idx].1;
                self.skills.run_post_action_key(skill_key, (owner_id, randomer, updates, storage));
                deferred_idx += 1;
            }
            let current_alive = storage.get_player(&owner_id).map(|p| p.alive()).unwrap_or(self.alive());
            let should_clear = self
                .state
                .states
                .get_mut(&tag)
                .map(|state| state.on_post_action(owner_id, current_alive, randomer, updates, storage))
                .unwrap_or(false);
            if should_clear {
                clear_tags.push(tag);
            }
        }
        while deferred_idx < deferred.len() {
            let skill_key = deferred[deferred_idx].1;
            self.skills.run_post_action_key(skill_key, (owner_id, randomer, updates, storage));
            deferred_idx += 1;
        }
        if !clear_tags.is_empty() {
            for tag in clear_tags {
                self.state.clear_tag(tag);
            }
            self.update_states();
        }
    }

    fn apply_forced_action_states(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        let clear_tags = self
            .state
            .on_forced_action_states(self.as_ptr(), self.alive(), randomer, updates, storage);
        if !clear_tags.is_empty() {
            for tag in clear_tags {
                self.state.clear_tag(tag);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_pre_defend_states(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> f64 {
        let clear_tags =
            self.state
                .on_pre_defend_states(self.as_ptr(), &mut atp, is_mag, caster, on_damage, randomer, updates, storage);
        if !clear_tags.is_empty() {
            for tag in clear_tags {
                self.state.clear_tag(tag);
            }
            self.update_states();
        }
        atp
    }

    fn apply_post_defend_states(
        &mut self,
        mut dmg: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        if self
            .state
            .on_post_defend_states(self.as_ptr(), &mut dmg, caster, randomer, updates, storage)
        {
            self.update_states();
        }
        dmg
    }

    pub fn mp_ready(&mut self, randomer: &mut RC4) -> bool {
        if !self.active() {
            return false;
        }
        let require_mp = randomer.r3x3() as i32;
        if self.status.mp >= require_mp {
            self.status.mp -= require_mp;
            return true;
        }
        false
    }

    #[inline]
    pub fn id_name(&self) -> String { self.id_name_override.clone().unwrap_or_else(|| self.name.clone()) }

    #[inline]
    pub fn id_key_name(&self) -> String {
        let id_name = self.id_name();
        if let Some(team) = self.team.as_ref()
            && !team.is_empty()
            && team != &id_name
        {
            return format!("{}@{}", id_name, team);
        }
        id_name
    }

    #[inline]
    pub fn display_name(&self) -> String {
        if let Some(display_name) = self.display_name_override.as_ref() {
            return display_name.clone();
        }
        if let Some(id_name) = self.id_name_override.as_ref() {
            return id_name.split(" ").next().unwrap_or_default().to_string();
        }
        if self.player_type == PlayerType::Boss {
            return boss_display_name(&self.name).to_string();
        }
        self.name.split(" ").next().unwrap_or_default().to_string()
    }

    #[inline]
    pub fn set_id_name_override(&mut self, id_name: Option<String>) { self.id_name_override = id_name; }

    #[inline]
    pub fn set_display_name_override(&mut self, display_name: Option<String>) { self.display_name_override = display_name; }

    #[inline]
    pub(crate) fn take_next_minion_name_index(&mut self) -> usize {
        let index = self.minion_name_next_index;
        self.minion_name_next_index += 1;
        index
    }

    #[inline]
    pub(crate) fn reset_minion_name_counter(&mut self) { self.minion_name_next_index = 0; }

    #[inline]
    pub fn clan_name(&self) -> String { self.team.clone().unwrap_or(self.name.clone()) }

    #[inline]
    pub fn base_name(&self) -> String { self.name.clone() }

    #[inline]
    pub fn is_seed_plr(&self) -> bool { matches!(self.player_type, PlayerType::Boost) }

    #[inline]
    pub fn cmp_by_id_name(&self, other: &Self) -> std::cmp::Ordering { self.id_key_name().cmp(&other.id_key_name()) }

    #[inline]
    pub fn cmp_for_sort(&self, other: &Self) -> std::cmp::Ordering { self.p_cmp(other) }

    pub(super) fn p_cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.sort_int.cmp(&other.sort_int) {
            Ordering::Equal => match self.id_key_name().cmp(&other.id_key_name()) {
                Ordering::Equal => self.id.cmp(&other.id),
                ord => ord,
            },
            ord => ord,
        }
    }

    pub fn get_at(&self, use_mag: bool, randomer: &mut RC4) -> f64 {
        let atk = if use_mag { self.status.magic } else { self.status.attack };
        let a = {
            let mut temp = [
                randomer.r127() as i32,
                randomer.r127() as i32,
                randomer.r127() as i32,
                atk + 64,
                atk,
            ];
            let raw = [temp[0], temp[1], temp[2]];
            temp.sort_unstable();
            if crate::debug::debug_damage() {
                eprintln!(
                    "[GET_AT] {} atk={} r127=[{},{},{}] sorted5={:?} median={}",
                    self.id_name(),
                    atk,
                    raw[0],
                    raw[1],
                    raw[2],
                    temp,
                    temp[2]
                );
            }
            temp[2] as f64
        };
        let b = {
            let mut temp = [randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64];
            let raw = [temp[0], temp[1]];
            temp.sort_unstable();
            if crate::debug::debug_damage() {
                eprintln!(
                    "[GET_AT]   r63=[{},{}] sorted3={:?} median={} boost={:.6} result={:.4}",
                    raw[0],
                    raw[1],
                    temp,
                    temp[1],
                    self.status.at_boost,
                    a * temp[1] as f64 * self.status.at_boost
                );
            }
            temp[1] as f64
        };
        a * b * self.status.at_boost
    }

    pub fn get_df(&self, use_mag: bool) -> i32 {
        if use_mag {
            self.status.resistance + 64
        } else {
            self.status.defense + 64
        }
    }

    pub fn dodge(al_a: i32, al_d: i32, randomer: &mut RC4) -> bool {
        let ch = {
            let temp = 24 + al_d - al_a;
            if temp < 7 {
                7
            } else if temp > 64 {
                temp / 4 + 48
            } else {
                temp
            }
        };

        randomer.next_u8() as i32 <= ch
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pre_defend(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> f64 {
        use crate::player::skill::protect::ProtectState;
        use crate::player::state::state_tag;

        let atp_before = atp;
        let started_zero = atp == 0.0;
        let protect_split = self.get_state::<ProtectState>().map(|state| state.pre_defend_skill_count);
        if let Some(split) = protect_split {
            let split = split.min(self.skills.pre_defend.len());
            atp = self.skills.pre_defend_range(
                0,
                split,
                atp,
                is_mag,
                caster,
                on_damage,
                (self.as_ptr(), randomer, updates, storage),
            );
            if crate::debug::debug_damage() && (atp - atp_before).abs() > 0.001 {
                eprintln!("[PRE_DEFEND] {} atp: {:.4} -> {:.4}", self.id_name(), atp_before, atp);
            }
            // JS 的 y1/pre_defend 混合链即便在入参 atp 已经为 0 时，仍会继续跑到当前顺序里的状态 entry。
            // 这里只在“前面的 skill entry 把 atp 打成 0”时提前返回；若是一开始就为 0，需要继续让 protect/state entry 消耗 RC4。
            if atp == 0.0 && !started_zero {
                return 0.0;
            }

            let protect_tag = state_tag::<ProtectState>();
            let mut clear_tags = self.state.on_pre_defend_state_tag(
                protect_tag,
                self.as_ptr(),
                &mut atp,
                is_mag,
                caster,
                on_damage,
                randomer,
                updates,
                storage,
            );
            if atp == 0.0 {
                if !clear_tags.is_empty() {
                    for tag in clear_tags {
                        self.state.clear_tag(tag);
                    }
                    self.update_states();
                }
                return 0.0;
            }

            let atp_after_protect = atp;
            atp = self.skills.pre_defend_range(
                split,
                self.skills.pre_defend.len(),
                atp,
                is_mag,
                caster,
                on_damage,
                (self.as_ptr(), randomer, updates, storage),
            );
            if crate::debug::debug_damage() && (atp - atp_after_protect).abs() > 0.001 {
                eprintln!("[PRE_DEFEND] {} atp: {:.4} -> {:.4}", self.id_name(), atp_after_protect, atp);
            }
            if atp == 0.0 {
                if !clear_tags.is_empty() {
                    for tag in clear_tags {
                        self.state.clear_tag(tag);
                    }
                    self.update_states();
                }
                return 0.0;
            }

            let atp_before_other_states = atp;
            clear_tags.extend(self.state.on_pre_defend_states_except_tag(
                protect_tag,
                self.as_ptr(),
                &mut atp,
                is_mag,
                caster,
                on_damage,
                randomer,
                updates,
                storage,
            ));
            if !clear_tags.is_empty() {
                for tag in clear_tags {
                    self.state.clear_tag(tag);
                }
                self.update_states();
            }
            if crate::debug::debug_damage() && (atp - atp_before_other_states).abs() > 0.001 {
                eprintln!(
                    "[PRE_DEFEND_STATE] {} atp: {:.4} -> {:.4}",
                    self.id_name(),
                    atp_before_other_states,
                    atp
                );
            }
            return atp;
        }

        atp = self
            .skills
            .pre_defend(atp, is_mag, caster, on_damage, (self.as_ptr(), randomer, updates, storage));
        if crate::debug::debug_damage() && (atp - atp_before).abs() > 0.001 {
            eprintln!("[PRE_DEFEND] {} atp: {:.4} -> {:.4}", self.id_name(), atp_before, atp);
        }
        if atp == 0.0 && !started_zero {
            return 0.0;
        }
        let atp2 = self.apply_pre_defend_states(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if crate::debug::debug_damage() && (atp2 - atp).abs() > 0.001 {
            eprintln!("[PRE_DEFEND_STATE] {} atp: {:.4} -> {:.4}", self.id_name(), atp, atp2);
        }
        atp2
    }

    pub fn post_defend(
        &mut self,
        mut dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        dmg = self
            .skills
            .post_defend(dmg, caster, &on_damage, (self.as_ptr(), randomer, updates, storage));
        self.apply_post_defend_states(dmg, caster, randomer, updates, storage)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn attacked(
        &mut self,
        mut atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        atp = self.pre_defend(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if atp == 0.0 {
            return 0;
        }
        let (accure, dodgeval) = {
            let caster_plr = storage.get_player(&caster).expect("faild to get caster player");
            if is_mag {
                (
                    caster_plr.status.magic + caster_plr.status.agility,
                    self.status.resistance + self.status.agility,
                )
            } else {
                (
                    caster_plr.status.attack + caster_plr.status.agility,
                    self.status.defense + self.status.agility,
                )
            }
        };
        if self.active() && Self::dodge(accure, dodgeval, randomer) {
            updates.emit(|| RunUpdate::new("[0][回避]了攻击", self.as_ptr(), caster, 20));
            return 0;
        }
        self.defned(atp, is_mag, caster, on_damage, randomer, updates, storage)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn defned(
        &mut self,
        atp: f64,
        is_mag: bool,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        let dfp = self.get_df(is_mag);
        let mut dmg = (atp / dfp as f64).ceil() as i32;
        if crate::debug::debug_damage() {
            eprintln!(
                "[DEFNED] target={} dfp={} atp={:.4} raw_dmg={} is_mag={}",
                self.id_name(),
                dfp,
                atp,
                dmg,
                is_mag
            );
        }
        dmg = self.post_defend(dmg, caster, on_damage, randomer, updates, storage);
        self.damage(dmg, caster, on_damage, randomer, updates, storage)
    }

    pub fn damage(
        &mut self,
        dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_this = crate::debug::debug_action_matches(&self.id_name());
        #[cfg(not(feature = "no_debug"))]
        if debug_this {
            eprintln!(
                "[damage] target={} caster={} dmg={} rc4=({}, {}) hp_before={}",
                self.id_name(),
                caster,
                dmg,
                randomer.i,
                randomer.j,
                self.status.hp,
            );
        }
        if dmg < 0 {
            let _old_hp = self.status.hp;
            self.status.hp -= dmg;
            if self.status.hp > self.status.max_hp {
                self.status.hp = self.status.max_hp;
            }
            updates.emit(|| {
                let mut update = RunUpdate::new("[1]回复体力[2]点", caster, self.as_ptr(), 0);
                update.param = Some(dmg.unsigned_abs());
                update
            });
            return 0;
        }
        if dmg == 0 {
            updates.emit(|| {
                let mut update = RunUpdate::new("[0]受到[2]点伤害[s_dmg0]", self.as_ptr(), self.as_ptr(), 10);
                update.param = Some(0);
                update
            });
            return 0;
        }
        let old_hp = self.status.hp;
        self.status.hp -= dmg;
        if self.status.hp < 0 {
            self.status.hp = 0;
        }
        let mut msg = "[1]受到[2]点伤害".to_string();
        if dmg >= 160 {
            msg.push_str("[s_dmg160]");
        } else if dmg >= 120 {
            msg.push_str("[s_dmg120]");
        }
        updates.emit(|| {
            let mut update = RunUpdate::new(msg, caster, self.as_ptr(), dmg as u32);
            update.delay0 = if dmg > 250 { 1500 } else { 1000 + dmg * 2 };
            update
        });
        on_damage(caster, self.as_ptr(), dmg, randomer, updates, storage);
        let result = self.on_damaged(dmg, old_hp, caster, randomer, updates, storage);
        #[cfg(not(feature = "no_debug"))]
        if debug_this {
            eprintln!(
                "[damage_end] target={} caster={} dmg={} rc4=({}, {}) hp_after={} result={}",
                self.id_name(),
                caster,
                dmg,
                randomer.i,
                randomer.j,
                self.status.hp,
                result,
            );
        }
        result
    }

    pub fn on_damaged(
        &mut self,
        dmg: i32,
        old_hp: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_this = crate::debug::debug_action_matches(&self.id_name());
        let post_damaged_indices: Vec<_> = self.skills.post_damage.to_vec();
        for skill_idx in post_damaged_indices {
            let ptr = self.as_ptr();
            #[cfg(not(feature = "no_debug"))]
            let rc4_before = (randomer.i, randomer.j);
            let skill = self.skills.skill_by_id_mut(skill_idx);
            skill.post_damage(dmg, caster, (ptr, randomer, updates, storage));
            #[cfg(not(feature = "no_debug"))]
            if debug_this {
                eprintln!(
                    "[post_damage_skill] target={} key={} rc4 {}:{} -> {}:{}",
                    self.id_name(),
                    skill_idx,
                    rc4_before.0,
                    rc4_before.1,
                    randomer.i,
                    randomer.j,
                );
            }
        }
        self.state.on_post_damage_states(self.as_ptr(), dmg, caster, randomer, updates, storage);
        if self.status.hp <= 0 {
            #[cfg(not(feature = "no_debug"))]
            if debug_this {
                eprintln!(
                    "[on_damaged_die] target={} old_hp={} rc4=({}, {})",
                    self.id_name(),
                    old_hp,
                    randomer.i,
                    randomer.j,
                );
            }
            self.on_die_impl(old_hp, caster, randomer, updates, storage, true);
            old_hp
        } else {
            dmg
        }
    }

    fn get_die_message(&self) -> &'static str { self.state.die_message_override().unwrap_or("[1]被击倒了") }

    pub fn on_die(&mut self, old_hp: i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>) {
        self.on_die_impl(old_hp, caster, randomer, updates, storage, false);
    }

    fn on_die_impl(
        &mut self,
        old_hp: i32,
        caster: PlrId,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
        allow_dead_reentry: bool,
    ) {
        #[cfg(not(feature = "no_debug"))]
        let debug_this = crate::debug::debug_action_matches(&self.id_name());
        #[cfg(not(feature = "no_debug"))]
        if debug_this {
            eprintln!(
                "[on_die] target={} old_hp={} caster={} allow_dead_reentry={} rc4=({}, {}) alive={} hp={}",
                self.id_name(),
                old_hp,
                caster,
                allow_dead_reentry,
                randomer.i,
                randomer.j,
                self.status.alive(),
                self.status.hp,
            );
        }
        if self.status.hp > 0 {
            return;
        }

        if !self.status.alive() {
            if !allow_dead_reentry {
                return;
            }
            if caster != self.as_ptr() {
                let kill_keys = storage
                    .get_player(&caster)
                    .filter(|k| k.get_status().hp > 0)
                    .map(|k| k.skills.post_kill.clone());
                if let Some(keys) = kill_keys {
                    let target_id = self.as_ptr();
                    crate::player::skill::store::run_post_kill(keys, caster, target_id, randomer, updates, storage);
                }
            }
            return;
        }

        updates.emit(RunUpdate::new_newline);
        updates.emit(|| RunUpdate::new(self.get_die_message(), caster, self.as_ptr(), 50));

        let ptr = self.as_ptr();
        self.skills.die(old_hp, caster, (ptr, randomer, updates, storage));
        if self.status.hp > 0 {
            return;
        }
        self.status.hp = 0;
        self.status.set_alive(false);

        let owner_id = self.as_ptr();
        // JS 按队伍 roster 顺序处理 linked minion（也就是召唤/分身出现顺序），
        // 这样 owner 死亡时会先清理 `?0` 再清理 `?1`。
        // Rust 之前走 HashMap keys 顺序，会导致顺序不稳定并出现反序日志。
        let linked_group_members = storage.group_containing(owner_id).cloned().unwrap_or_else(|| {
            let mut ids = storage.all_player_ids();
            ids.sort_unstable();
            ids
        });
        let mut linked_minions_src = linked_group_members.clone();
        // JS 中如果 owner 在同一回合先生成 pending minion，随后自己立即死亡，
        // 这些 pending minion 仍会先经过 addNew 进入 round roster，
        // 然后在同一轮 sync 中随 owner 一起移除并推进 round_pos。
        // 所以这里仍要把 pending minion 标成死亡，交给后续 sync 落地并移除。
        linked_minions_src.extend(storage.pending_spawn_ids_for_group(&linked_group_members));
        let linked_minions = linked_minions_src
            .into_iter()
            .filter(|id| *id != owner_id)
            .filter(|id| {
                storage
                    .get_player_or_pending(id)
                    .map(|player| player.state.linked_to_owner(owner_id))
                    .unwrap_or(false)
            })
            .collect::<Vec<PlrId>>();
        for minion_id in linked_minions {
            // JS PlrSummon.aR: 如果使魔正在执行 post_damage（伤害分摊），
            // 只设置 HP=0，不立即处理死亡。使魔的死亡将由其自身的 on_damaged 路径处理，
            // 确保死亡顺序为 [owner, summon] 而非 [summon, owner]。
            if storage.is_in_post_damage(minion_id) {
                if let Some(minion) = storage.just_get_player_or_pending_mut(minion_id)
                    && minion.alive()
                    && minion.get_status().hp > 0
                {
                    minion.status.hp = 0;
                }
                continue;
            }
            let should_queue_remove = storage.get_player(&minion_id).is_some();
            let should_remove = if let Some(minion) = storage.just_get_player_or_pending_mut(minion_id) {
                if !minion.alive() || minion.get_status().hp <= 0 {
                    false
                } else {
                    minion.status.hp = 0;
                    minion.status.set_alive(false);
                    storage.record_death(minion_id);
                    minion.state.on_linked_owner_die(owner_id, minion_id, updates)
                }
            } else {
                false
            };
            if should_remove && should_queue_remove {
                storage.queue_remove_player(minion_id);
            }
        }

        storage.record_death(owner_id);

        #[cfg(not(feature = "no_debug"))]
        if debug_this {
            eprintln!(
                "[on_die_after_record] target={} rc4=({}, {}) alive={} hp={}",
                self.id_name(),
                randomer.i,
                randomer.j,
                self.status.alive(),
                self.status.hp,
            );
        }

        let has_enemy_alive = storage.group_containing(caster).map(|ally_group| has_alive_enemy_or_pending(storage, ally_group));
        if has_enemy_alive.unwrap_or(true) && caster != self.as_ptr() {
            // 避免在 kill 回调（如吞噬）中产生 &mut Player 别名：
            // 先获取 post_kill 键列表并检查 HP，然后释放 killer 引用，
            // 再逐个通过 storage 重新获取 &mut 来调用回调。
            let kill_keys = storage
                .get_player(&caster)
                .filter(|k| k.get_status().hp > 0)
                .map(|k| k.skills.post_kill.clone());
            if let Some(keys) = kill_keys {
                let target_id = self.as_ptr();
                crate::player::skill::store::run_post_kill(keys, caster, target_id, randomer, updates, storage);
            }
        }
    }
}

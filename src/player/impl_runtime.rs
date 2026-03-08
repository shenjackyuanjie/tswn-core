use super::*;

pub static DODGE_TRACE_ACTIVE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

impl Player {
    pub fn update_player(&mut self) {
        self.init_skills();
        self.update_states();
    }

    /// 每回合中的玩家行动
    ///
    /// 包括 pre, main, post
    pub fn step(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        if !self.status.alive() {
            return;
        }
        // Fine-grained RC4 trace between dodge 200 and 201
        let trace_fine = DODGE_TRACE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) == 1;
        if trace_fine {
            eprintln!("[step_begin] plr={} rc4=({}, {})", self.id_name(), randomer.i, randomer.j);
        }
        let debug_target = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_target.as_deref().map(|name| name == self.id_name().as_str()).unwrap_or(false);
        let move_before = self.status.move_point;
        let mut stp = self.status.speed * randomer.r3() as i32;
        if trace_fine {
            eprintln!("[step_after_r3] plr={} rc4=({}, {})", self.id_name(), randomer.i, randomer.j);
        }
        stp = self.apply_pre_step_states(stp, updates);
        if trace_fine {
            eprintln!(
                "[step_after_pre_step_states] plr={} rc4=({}, {})",
                self.id_name(),
                randomer.i,
                randomer.j
            );
        }
        let ptr = self.as_ptr();
        stp = self.skills.pre_step(stp, (ptr, randomer, updates, storage));
        if trace_fine {
            eprintln!(
                "[step_after_pre_step_skills] plr={} rc4=({}, {}) move_after={}",
                self.id_name(),
                randomer.i,
                randomer.j,
                self.status.move_point + stp
            );
        }
        self.status.move_point += stp;
        if debug_this {
            eprintln!(
                "[step] actor={} move_before={} stp={} move_after={} rc4=({}, {})",
                self.id_name(),
                move_before,
                stp,
                self.status.move_point,
                randomer.i,
                randomer.j,
            );
        }
        if self.check_move() {
            self.status.move_point -= MOVE_POINT_THRESHOLD;
            // 主动作
            self.action(randomer, updates, storage, targets);
        }
        // 结束
    }

    pub fn action(&mut self, randomer: &mut RC4, updates: &mut RunUpdates, storage: &Arc<Storage>, targets: &ActionTargets) {
        let debug_target = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_target.as_deref().map(|name| name == self.id_name().as_str()).unwrap_or(false);
        let trace_fine = DODGE_TRACE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) == 1;
        if trace_fine {
            eprintln!("[action_begin] plr={} rc4=({}, {})", self.id_name(), randomer.i, randomer.j);
        }
        let smart_roll = randomer.r63() as i32;
        let smart = self.status.wisdom > smart_roll;
        if trace_fine {
            eprintln!(
                "[action_after_smart] plr={} smart={} rc4=({}, {})",
                self.id_name(),
                smart,
                randomer.i,
                randomer.j
            );
        }
        if debug_this {
            eprintln!(
                "[action] start actor={} rc4=({}, {}) smart={} smart_roll={}",
                self.id_name(),
                randomer.i,
                randomer.j,
                smart,
                smart_roll,
            );
            let mut preview = randomer.clone();
            let peek = (0..6).map(|_| preview.next_u8()).collect::<Vec<u8>>();
            eprintln!("[action] peek_next={peek:?}");
        }
        let ptr = self.as_ptr();
        let pre_action_outcome = self.skills.pre_action(smart, (ptr, randomer, updates, storage));
        if trace_fine {
            eprintln!(
                "[action_after_preaction] plr={} rc4=({}, {})",
                self.id_name(),
                randomer.i,
                randomer.j
            );
        }
        if debug_this {
            eprintln!(
                "[action] after pre_action forced={:?} rc4=({}, {})",
                pre_action_outcome.forced_skill, randomer.i, randomer.j
            );
        }
        if self.status.frozed() {
            return;
        }

        // State-based pre-action hijack (COVID/Lazy infection replaces player's action)
        let state_hijacked = self.state.on_pre_action_states(self.as_ptr(), smart, randomer, updates, storage, targets);
        if state_hijacked {
            // State handled the entire action; skip to recovery + post_action.
            let recover_threshold = self.status.wisdom + 64;
            if (randomer.r127() as i32) < recover_threshold {
                self.status.mp += 16;
            }
            if debug_this {
                eprintln!(
                    "[action] state_hijacked end actor={} mp={} rc4=({}, {})",
                    self.id_name(),
                    self.status.mp,
                    randomer.i,
                    randomer.j,
                );
            }
            updates.add(RunUpdate::new_newline());
            let ptr = self.as_ptr();
            self.skills.post_action((ptr, randomer, updates, storage));
            self.apply_post_action_states(randomer, updates, storage);
            if debug_this {
                eprintln!("[action] after state_hijacked post_action rc4=({}, {})", randomer.i, randomer.j);
            }
            return;
        }

        let mut acted = false;
        let mut selected_skill_key: Option<usize> = pre_action_outcome.forced_skill;
        let mut selected_targets: Vec<PlrId> = Vec::new();
        let selected_from_forced_pre_action = pre_action_outcome.forced_skill.is_some();
        let forced_attack = if pre_action_outcome.clear_forced_action {
            None
        } else {
            self.state.resolve_action_mode(smart)
        };
        if let Some(forced_attack) = forced_attack {
            self.forced_attack(forced_attack, randomer, updates, storage, targets);
            self.apply_forced_action_states(randomer, updates, storage);
            acted = true;
        } else {
            if selected_skill_key.is_none() {
                let req_mp = randomer.r15() as i32 + 8;
                if debug_this {
                    eprintln!(
                        "[action] req_mp={req_mp} mp={} rc4=({}, {})",
                        self.status.mp, randomer.i, randomer.j
                    );
                }
                if self.status.mp >= req_mp {
                    let is_boss = self.player_type == PlayerType::Boss;
                    // JS PlrBoss.bs() 只把 k1 中的 ActionSkill 加入 k4。
                    // 对 COVID/Lazy 等 boss，k1 中的 SklCovidDefend/SklLazyDefend
                    // 不是 ActionSkill，因此 k4 为空，不消耗任何 prob 字节。
                    if !is_boss {
                        let skill_keys = self.skills.skill.clone();
                        for key in skill_keys {
                            let maybe_targets = {
                                let skill = self.skills.skill_by_id(key);
                                let rc4_before_prob = (randomer.i, randomer.j);
                                let action_ok = skill.has_action_impl();
                                let level_ok = skill.level() > 0;
                                let prob_ok = level_ok && action_ok && skill.prob(smart, (ptr, randomer, updates, storage));
                                if debug_this && (level_ok || action_ok) {
                                    eprintln!(
                                        "[action] skill={key} lv={} action={} prob={} rc4 {}:{} -> {}:{}",
                                        skill.level(),
                                        action_ok,
                                        prob_ok,
                                        rc4_before_prob.0,
                                        rc4_before_prob.1,
                                        randomer.i,
                                        randomer.j
                                    );
                                }
                                if !(level_ok && action_ok && prob_ok) {
                                    None
                                } else {
                                    let selected = self.select_skill_targets(skill, smart, randomer, updates, storage, targets);
                                    let allow_empty = skill.target_domain() == SkillTargetDomain::SelfOnly;
                                    if debug_this {
                                        eprintln!(
                                            "[action] skill={key} selected_len={} allow_empty={} rc4=({}, {})",
                                            selected.len(),
                                            allow_empty,
                                            randomer.i,
                                            randomer.j
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
                    } // end if !is_boss
                    self.status.mp -= req_mp;
                    if debug_this {
                        eprintln!(
                            "[action] consume mp now={} rc4=({}, {})",
                            self.status.mp, randomer.i, randomer.j
                        );
                    }
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
                let allow_empty = {
                    let skill = self.skills.skill_by_id(skill_key);
                    skill.target_domain() == SkillTargetDomain::SelfOnly
                };
                if !selected_targets.is_empty() || allow_empty {
                    let skill = self.skills.skill_by_id_mut(skill_key);
                    if debug_this {
                        eprintln!(
                            "[action] act skill={skill_key} targets={} rc4=({}, {})",
                            selected_targets.len(),
                            randomer.i,
                            randomer.j
                        );
                    }
                    skill.act(selected_targets, smart, (ptr, randomer, updates, storage));
                    acted = true;
                }
            }
        }

        if !acted {
            if trace_fine {
                eprintln!(
                    "[action_before_default_attack] plr={} rc4=({}, {})",
                    self.id_name(),
                    randomer.i,
                    randomer.j
                );
            }
            self.default_attack(smart, randomer, updates, storage, targets);
        }

        let recover_threshold = self.status.wisdom + 64;
        if (randomer.r127() as i32) < recover_threshold {
            self.status.mp += 16;
        }
        if debug_this {
            eprintln!(
                "[action] end actor={} mp={} rc4=({}, {})",
                self.id_name(),
                self.status.mp,
                randomer.i,
                randomer.j,
            );
        }
        updates.add(RunUpdate::new_newline());
        self.skills.post_action((ptr, randomer, updates, storage));
        if debug_this {
            eprintln!("[action] after skills.post_action rc4=({}, {})", randomer.i, randomer.j);
        }
        self.apply_post_action_states(randomer, updates, storage);
        if debug_this {
            eprintln!("[action] after state.post_action rc4=({}, {})", randomer.i, randomer.j);
        }
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
        if std::env::var_os("TSWN_DEBUG_PICK").is_some() {
            eprintln!(
                "[pick_enemy] all_alive_len={} skip={:?} rc4=({},{})",
                targets.all_alive.len(),
                skip_indices,
                randomer.i,
                randomer.j
            );
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
    /// 之前已经反复验证过，这种“看起来更统一”的改法会把当前对局随机流打歪，
    /// 典型回归就是 `fight_multi_6` 会重新失败。根因是当前 Rust 侧的主动技能选目标
    /// 语义并不完全等同于“先构出 candidate 列表，再走 trait 默认 select_targets”：
    ///
    /// 1. `EnemyAlive` 在 JS 产物里对应的是基于 `all_alive + pickSkipRange` 的抽样语义，
    ///    不是一个纯粹的 `enemy_alive` 紧凑列表。
    /// 2. `AllyAny` 需要保持 Dart 的 `group.players` 语义，也就是 team roster 视图；
    ///    它和 `AllyAlive` / `AllAlive` 是不同维度的数据，不能互相替代。
    /// 3. 现有部分技能虽然实现了 `select_targets_with_level`，但如果全量切换到统一入口，
    ///    会改变随机数消费顺序和重复/无效目标处理细节，从而造成隐藏 RC4 漂移。
    ///
    /// 因此这里先保留“按 domain 手工抽样，再按 valid/score 排序”的稳定路径。
    /// 如果后续要接入某个技能自己的特殊选目标逻辑，应该做“逐技能 opt-in” 的窄改，
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

        let debug_this = std::env::var("TSWN_DEBUG_ACTION")
            .ok()
            .map(|name| name == self.id_name())
            .unwrap_or(false);
        let format_targets = |ids: &[PlrId]| -> Vec<String> {
            ids.iter()
                .map(|id| storage.get_player(id).map(|plr| plr.id_name()).unwrap_or_else(|| format!("#{id}")))
                .collect::<Vec<String>>()
        };
        if debug_this {
            eprintln!(
                "[action_select] actor={} skill_level={} domain={:?} candidates ally_all={:?} ally_alive={:?} ally_dead={:?} enemy_alive={:?}",
                self.id_name(),
                skill.level(),
                domain,
                format_targets(&targets.ally_all),
                format_targets(&targets.ally_alive),
                format_targets(&targets.ally_dead),
                format_targets(&targets.enemy_alive),
            );
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
            if debug_this {
                eprintln!(
                    "[action_select] actor={} skill_level={} using_custom_selector candidates={:?}",
                    self.id_name(),
                    skill.level(),
                    format_targets(candidates),
                );
            }
            return skill.select_targets(candidates, smart, (self.as_ptr(), randomer, updates, storage));
        }

        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(target_id) = self.pick_target_by_domain(domain, targets, randomer) else {
                return Vec::new();
            };
            let valid = skill.valid_target(target_id, smart, (self.as_ptr(), randomer, updates, storage));
            if debug_this {
                let target_name = storage
                    .get_player(&target_id)
                    .map(|plr| plr.id_name())
                    .unwrap_or_else(|| format!("#{target_id}"));
                eprintln!(
                    "[action_select] actor={} skill_level={} picked={} valid={} selected_so_far={:?}",
                    self.id_name(),
                    skill.level(),
                    target_name,
                    valid,
                    format_targets(&selected),
                );
            }
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

        let mut scored = selected
            .into_iter()
            .map(|target_id| {
                (
                    target_id,
                    skill.score_target(target_id, smart, (self.as_ptr(), randomer, updates, storage)),
                )
            })
            .collect::<Vec<(PlrId, f64)>>();
        if debug_this {
            for (target_id, score) in &scored {
                eprintln!(
                    "[action_select] actor={} scored={} score={score}",
                    self.id_name(),
                    storage
                        .get_player(target_id)
                        .map(|plr| plr.id_name())
                        .unwrap_or_else(|| format!("#{target_id}")),
                );
            }
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        if debug_this {
            let sorted = scored
                .iter()
                .map(|(target_id, score)| {
                    format!(
                        "{}:{score}",
                        storage
                            .get_player(target_id)
                            .map(|plr| plr.id_name())
                            .unwrap_or_else(|| format!("#{target_id}"))
                    )
                })
                .collect::<Vec<String>>();
            eprintln!("[action_select] actor={} sorted={sorted:?}", self.id_name());
        }
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
        let mut selected = Vec::new();
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

        let mut scored = selected
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
            .collect::<Vec<(PlrId, f64)>>();
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
        updates.add(RunUpdate::new(config.message, self.as_ptr(), target_id, 0));
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
        let debug_this = std::env::var("TSWN_DEBUG_ACTION")
            .ok()
            .as_deref()
            .map(|name| name == self.id_name().as_str())
            .unwrap_or(false);
        if debug_this {
            let enemy_names = targets
                .enemy_alive
                .iter()
                .map(|id| storage.get_player(id).map(|p| p.id_name()).unwrap_or_else(|| format!("#{id}")))
                .collect::<Vec<String>>();
            let all_names = targets
                .all_alive
                .iter()
                .map(|id| storage.get_player(id).map(|p| p.id_name()).unwrap_or_else(|| format!("#{id}")))
                .collect::<Vec<String>>();
            eprintln!(
                "[default_select] smart={smart} rc4=({}, {}) all={all_names:?} enemy={enemy_names:?}",
                randomer.i, randomer.j
            );
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
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
        if debug_this {
            let selected_names = selected
                .iter()
                .map(|id| storage.get_player(id).map(|p| p.id_name()).unwrap_or_else(|| format!("#{id}")))
                .collect::<Vec<String>>();
            eprintln!(
                "[default_select] sampled={selected_names:?} rc4=({}, {})",
                randomer.i, randomer.j
            );
        }

        let mut scored = selected
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
                            let alive_group_count = {
                                let mut group_heads = Vec::new();
                                for id in storage.all_player_ids() {
                                    let alive = storage.get_player(&id).map(|plr| plr.alive()).unwrap_or(false);
                                    if !alive {
                                        continue;
                                    }
                                    let Some(group) = storage.group_containing(id) else {
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
                            let target_alive_group_len = storage
                                .group_containing(target_id)
                                .map(|group| {
                                    group
                                        .iter()
                                        .filter(|id| storage.get_player(id).map(|plr| plr.alive()).unwrap_or(false))
                                        .count()
                                })
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
                if debug_this {
                    if let Some(target) = storage.get_player(&target_id) {
                        let status = target.get_status();
                        eprintln!(
                            "[default_select] score target={} hp={} attract={} atksum={} score={} rc4=({}, {})",
                            target.id_name(),
                            status.hp,
                            status.attract,
                            status.atk_sum,
                            score,
                            randomer.i,
                            randomer.j
                        );
                    } else {
                        eprintln!(
                            "[default_select] score target=#{target_id} score={score} rc4=({}, {})",
                            randomer.i, randomer.j
                        );
                    }
                }
                (target_id, score)
            })
            .collect::<Vec<(PlrId, f64)>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(Ordering::Equal));
        if debug_this {
            if let Some((target_id, _)) = scored.first() {
                let name = storage
                    .get_player(target_id)
                    .map(|p| p.id_name())
                    .unwrap_or_else(|| format!("#{target_id}"));
                eprintln!("[default_select] chose={name} rc4=({}, {})", randomer.i, randomer.j);
            }
        }
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
        // Boss 使用专属行动逻辑
        if self.player_type == PlayerType::Boss {
            crate::player::boss::boss_default_action(self, smart, randomer, updates, storage, targets);
            return;
        }
        let trace_fine = DODGE_TRACE_ACTIVE.load(std::sync::atomic::Ordering::Relaxed) == 1;
        if trace_fine {
            eprintln!(
                "[default_attack_begin] plr={} rc4=({}, {})",
                self.id_name(),
                randomer.i,
                randomer.j
            );
        }
        let Some(target_id) = self.select_default_attack_target(smart, randomer, storage, targets) else {
            return;
        };
        if trace_fine {
            eprintln!(
                "[default_attack_after_target] plr={} target={:?} rc4=({}, {})",
                self.id_name(),
                target_id,
                randomer.i,
                randomer.j
            );
        }

        if smart && self.status.magic > self.status.attack {
            let req_mp = (self.status.magic - self.status.attack) >> 2;
            if self.status.mp >= req_mp {
                self.status.mp -= req_mp;
                let atp = self.get_at(true, randomer);
                if trace_fine {
                    eprintln!(
                        "[default_attack_before_attacked] plr={} is_mag=true atp={} rc4=({}, {})",
                        self.id_name(),
                        atp,
                        randomer.i,
                        randomer.j
                    );
                }
                updates.add(RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
                storage
                    .just_get_player_mut(target_id)
                    .expect("cannot get default-attack target from storage")
                    .attacked(atp, true, self.as_ptr(), noop_on_damage, randomer, updates, storage);
                return;
            }
        }

        let atp = self.get_at(false, randomer);
        if trace_fine {
            eprintln!(
                "[default_attack_before_attacked] plr={} is_mag=false atp={} rc4=({}, {})",
                self.id_name(),
                atp,
                randomer.i,
                randomer.j
            );
        }
        updates.add(RunUpdate::new("[0]发起攻击", self.as_ptr(), target_id, 0));
        storage
            .just_get_player_mut(target_id)
            .expect("cannot get default-attack target from storage")
            .attacked(atp, false, self.as_ptr(), noop_on_damage, randomer, updates, storage);
    }

    /// 当前玩家是否可行动
    #[inline]
    pub fn active(&self) -> bool { self.status.hp > 0 && !self.status.frozed() }
    /// 活着呢吧?
    #[inline]
    pub fn alive(&self) -> bool { self.status.alive() }

    /// 直接扣血（不走完整攻防链）。
    #[inline]
    pub fn apply_raw_damage(&mut self, dmg: i32) {
        self.status.hp -= dmg;
        if self.status.hp < 0 {
            self.status.hp = 0;
        }
    }

    /// 直接回血（不超过 max_hp）。
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
        self.status.set_frozen(false);
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
        let alive = self.alive();
        let messages = self.state.clear_positive_states_with_messages(alive);
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
        let clear_tags = self.state.on_post_action_states(self.as_ptr(), self.alive(), randomer, updates, storage);
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

    fn apply_post_defend_states(&mut self, mut dmg: i32, caster: PlrId, randomer: &mut RC4, updates: &mut RunUpdates) -> i32 {
        self.state.on_post_defend_states(self.as_ptr(), &mut dmg, caster, randomer, updates);
        dmg
    }

    /// 蓝条是不是够用
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

    // 用于兼容 namerena 的各种名字调用
    #[inline]
    pub fn id_name(&self) -> String { self.name.clone() }
    #[inline]
    pub fn id_key_name(&self) -> String {
        if let Some(team) = self.team.as_ref()
            && !team.is_empty()
            && team != &self.name
        {
            return format!("{}@{}", self.name, team);
        }
        self.name.clone()
    }
    #[inline]
    pub fn display_name(&self) -> String {
        if self.player_type == PlayerType::Boss {
            return boss_display_name(&self.name).to_string();
        }
        self.name.split(" ").next().unwrap_or_default().to_string()
    }
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

    /// getAt
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
            temp.sort_unstable();
            temp[2] as f64
        };
        let b = {
            let mut temp = [randomer.r63() as i32 + 64, randomer.r63() as i32 + 64, atk + 64];
            temp.sort_unstable();
            temp[1] as f64
        };
        a * b * self.status.at_boost
    }

    /// getDf
    pub fn get_df(&self, use_mag: bool) -> i32 {
        if use_mag {
            self.status.resistance + 64
        } else {
            self.status.defense + 64
        }
    }

    pub fn dodge(al_a: i32, al_d: i32, randomer: &mut RC4) -> bool {
        if std::env::var_os("TSWN_DEBUG_DODGE_ALL").is_some() {
            use std::sync::atomic::{AtomicU32, Ordering};
            static DODGE_COUNT: AtomicU32 = AtomicU32::new(0);
            let count = DODGE_COUNT.fetch_add(1, Ordering::Relaxed);
            eprintln!(
                "[dodge_all] #{count} accure={al_a} dodgeval={al_d} rc4=({}, {})",
                randomer.i, randomer.j
            );
            // Set/clear trace flag around dodge 200-201
            static TRACE_FLAG: AtomicU32 = AtomicU32::new(0);
            if count == 200 {
                TRACE_FLAG.store(1, Ordering::Relaxed);
            } else if count == 201 {
                TRACE_FLAG.store(0, Ordering::Relaxed);
            }
            // Export trace flag for use in step()
            DODGE_TRACE_ACTIVE.store(TRACE_FLAG.load(Ordering::Relaxed), Ordering::Relaxed);
        }
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

    /// preDefend
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
        // JS y1 列表中，技能（setup 阶段注册）排在状态（运行时注册）之前
        // 两者优先级相同（10000），按插入顺序决定迭代顺序
        atp = self
            .skills
            .pre_defend(atp, is_mag, caster, on_damage, (self.as_ptr(), randomer, updates, storage));
        if atp == 0.0 {
            return 0.0;
        }
        self.apply_pre_defend_states(atp, is_mag, caster, on_damage, randomer, updates, storage)
    }

    /// postDefend
    pub fn post_defend(
        &mut self,
        mut dmg: i32,
        caster: PlrId,
        on_damage: OnDamageFunc,
        randomer: &mut RC4,
        updates: &mut RunUpdates,
        storage: &Arc<Storage>,
    ) -> i32 {
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action.as_deref().map(|name| name == self.id_name()).unwrap_or(false);
        dmg = self
            .skills
            .post_defend(dmg, caster, &on_damage, (self.as_ptr(), randomer, updates, storage));
        if debug_this {
            let ordered = self
                .state
                .states
                .iter()
                .map(|(tag, state)| (*tag, state.post_defend_priority()))
                .collect::<Vec<(crate::player::StateTag, i32)>>();
            eprintln!(
                "[post_defend_states] owner={} after_skill dmg={} states={ordered:?} rc4=({}, {})",
                self.id_name(),
                dmg,
                randomer.i,
                randomer.j,
            );
        }
        self.apply_post_defend_states(dmg, caster, randomer, updates)
    }

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
        let debug_target = std::env::var("TSWN_DEBUG_ACTION").ok();
        let caster_name = storage.get_player(&caster).map(|p| p.id_name()).unwrap_or_default();
        let target_name = self.id_name();
        let debug_this = debug_target
            .as_deref()
            .map(|name| name == caster_name || name == target_name)
            .unwrap_or(false);
        if debug_this {
            eprintln!(
                "[damage_flow] attacked start caster={} target={} is_mag={} atp_before_pre={} rc4=({}, {})",
                caster_name, target_name, is_mag, atp, randomer.i, randomer.j,
            );
        }
        atp = self.pre_defend(atp, is_mag, caster, on_damage, randomer, updates, storage);
        if debug_this {
            eprintln!(
                "[damage_flow] after_pre_defend caster={} target={} atp_after_pre={} rc4=({}, {})",
                caster_name, target_name, atp, randomer.i, randomer.j,
            );
        }
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
        if std::env::var_os("TSWN_DEBUG_DODGE").is_some() {
            eprintln!(
                "[dodge] target={} caster={} active={} is_mag={} accure={} dodgeval={} atp={} rc4=({}, {})",
                self.id_name(),
                storage.get_player(&caster).map(|p| p.id_name()).unwrap_or_default(),
                self.active(),
                is_mag,
                accure,
                dodgeval,
                atp,
                randomer.i,
                randomer.j,
            );
        }
        if self.active() && Self::dodge(accure, dodgeval, randomer) {
            let update = RunUpdate::new("[0][回避]了攻击", self.as_ptr(), caster, 20);
            updates.add(update);
            return 0;
        }
        self.defned(atp, is_mag, caster, on_damage, randomer, updates, storage)
    }

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
        let debug_target = std::env::var("TSWN_DEBUG_ACTION").ok();
        let caster_name = storage.get_player(&caster).map(|p| p.id_name()).unwrap_or_default();
        let target_name = self.id_name();
        let debug_this = debug_target
            .as_deref()
            .map(|name| name == caster_name || name == target_name)
            .unwrap_or(false);
        let dfp = self.get_df(is_mag);
        let mut dmg = (atp / dfp as f64).ceil() as i32;
        if debug_this {
            eprintln!(
                "[damage_flow] before_post_defend caster={} target={} atp={} dfp={} raw_div={} dmg_before_post={} rc4=({}, {})",
                caster_name,
                target_name,
                atp,
                dfp,
                atp / dfp as f64,
                dmg,
                randomer.i,
                randomer.j,
            );
        }
        dmg = self.post_defend(dmg, caster, on_damage, randomer, updates, storage);
        if debug_this {
            eprintln!(
                "[damage_flow] after_post_defend caster={} target={} dmg_after_post={} rc4=({}, {})",
                caster_name, target_name, dmg, randomer.i, randomer.j,
            );
        }
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
        if dmg < 0 {
            let _old_hp = self.status.hp;
            self.status.hp -= dmg;
            if self.status.hp > self.status.max_hp {
                self.status.hp = self.status.max_hp;
            }
            let update = RunUpdate::new("[1]回复体力[2]点", caster, self.as_ptr(), dmg.unsigned_abs());
            updates.add(update);
            return 0;
        }
        if dmg == 0 {
            let update = RunUpdate::new("[0]受到[2]点伤害[s_dmg0]", self.as_ptr(), self.as_ptr(), 0);
            updates.add(update);
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
        let mut update = RunUpdate::new(msg, caster, self.as_ptr(), dmg as u32);
        update.delay0 = if dmg > 250 { 1500 } else { 1000 + dmg * 2 };
        updates.add(update);
        on_damage(caster, self.as_ptr(), dmg, randomer, updates, storage);
        self.on_damaged(dmg, old_hp, caster, randomer, updates, storage)
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
        let debug_action = std::env::var("TSWN_DEBUG_ACTION").ok();
        let debug_this = debug_action.as_deref().map(|name| name == self.id_name().as_str()).unwrap_or(false);
        let post_damaged_indices: Vec<_> = self.skills.post_damage.to_vec();
        for skill_idx in post_damaged_indices {
            let ptr = self.as_ptr();
            let rc4_before = (randomer.i, randomer.j);
            let skill = self.skills.skill_by_id_mut(skill_idx);
            skill.post_damage(dmg, caster, (ptr, randomer, updates, storage));
            if debug_this {
                eprintln!(
                    "[on_damaged_post_damage] owner={} key={} rc4 {}:{} -> {}:{}",
                    self.id_name(),
                    skill_idx,
                    rc4_before.0,
                    rc4_before.1,
                    randomer.i,
                    randomer.j,
                );
            }
        }
        // State-based post_damage hooks (e.g. SklCovidDefend/SklLazyDefend on boss)
        self.state.on_post_damage_states(self.as_ptr(), dmg, caster, randomer, updates, storage);
        if self.status.hp <= 0 {
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
        if self.status.hp > 0 {
            return;
        }

        if !self.status.alive() {
            if !allow_dead_reentry {
                return;
            }
            if caster != self.as_ptr()
                && let Some(killer) = storage.just_get_player_mut(caster)
                && killer.get_status().hp > 0
            {
                killer.skills.kill(self.as_ptr(), (caster, randomer, updates, storage));
            }
            return;
        }

        let debug_die = std::env::var("TSWN_DEBUG_DIE").ok();
        let debug_this = debug_die.as_deref().map(|name| name == self.id_name().as_str()).unwrap_or(false);
        if debug_this {
            eprintln!(
                "[on_die] start actor={} caster={} rc4=({}, {}) hp={} old_hp={}",
                self.id_name(),
                storage.get_player(&caster).map(|p| p.id_name()).unwrap_or_else(|| format!("#{caster}")),
                randomer.i,
                randomer.j,
                self.status.hp,
                old_hp
            );
        }

        updates.add(RunUpdate::new_newline());
        updates.add(RunUpdate::new(self.get_die_message(), caster, self.as_ptr(), 50));

        let ptr = self.as_ptr();
        self.skills.die(old_hp, caster, (ptr, randomer, updates, storage));
        if debug_this {
            eprintln!(
                "[on_die] after skills.die rc4=({}, {}) hp={}",
                randomer.i, randomer.j, self.status.hp
            );
        }
        if self.status.hp > 0 {
            return;
        }
        self.status.hp = 0;
        self.status.set_alive(false);

        // 对齐 Dart: 在 dies 回调中 minion 先于 owner 被移除。
        // 因此先处理 linked minions，记录它们的死亡顺序，最后再记录 owner 的死亡。
        let owner_id = self.as_ptr();
        let linked_minions = storage
            .all_player_ids()
            .into_iter()
            .filter(|id| *id != owner_id)
            .filter(|id| {
                storage
                    .get_player(id)
                    .map(|player| player.state.linked_to_owner(owner_id))
                    .unwrap_or(false)
            })
            .collect::<Vec<PlrId>>();
        if debug_this {
            let names = linked_minions
                .iter()
                .map(|id| storage.get_player(id).map(|p| p.id_name()).unwrap_or_else(|| format!("#{id}")))
                .collect::<Vec<String>>();
            eprintln!("[on_die] linked_minions={names:?} rc4=({}, {})", randomer.i, randomer.j);
        }
        for minion_id in linked_minions {
            let should_remove = if let Some(minion) = storage.just_get_player_mut(minion_id) {
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
            if should_remove {
                storage.queue_remove_player(minion_id);
            }
        }

        // 最后记录 owner 的死亡（minion 已先于 owner 入队）。
        storage.record_death(owner_id);

        let has_enemy_alive = storage.group_containing(caster).map(|ally_group| {
            storage
                .all_player_ids()
                .into_iter()
                .any(|id| !ally_group.contains(&id) && storage.get_player(&id).map(|plr| plr.alive()).unwrap_or(false))
        });
        // Dart 中 alive 是 hp > 0 的派生属性，而 Rust 使用独立的 alive flag。
        // 自爆等场景下 caster 先把 hp 设为 0 但 alive flag 还没更新，
        // 此处必须用 hp > 0 判定才能与 Dart 行为一致。
        if has_enemy_alive.unwrap_or(true)
            && caster != self.as_ptr()
            && let Some(killer) = storage.just_get_player_mut(caster)
            && killer.get_status().hp > 0
        {
            if debug_this {
                eprintln!(
                    "[on_die] killer={} before kill rc4=({}, {})",
                    killer.id_name(),
                    randomer.i,
                    randomer.j
                );
            }
            killer.skills.kill(self.as_ptr(), (caster, randomer, updates, storage));
            if debug_this {
                eprintln!("[on_die] after killer.kill rc4=({}, {})", randomer.i, randomer.j);
            }
        }
    }
}

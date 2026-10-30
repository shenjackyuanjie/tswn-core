use super::*;

impl CombatRuntime {
    pub fn run_minimal_round(&mut self) -> RoundOutcome { self.run_minimal_round_with_capture::<true>() }

    /// 批量胜率与评分只需要胜者，不保留 replay 帧。
    pub(crate) fn run_minimal_round_no_capture(&mut self) -> RoundOutcome { self.run_minimal_round_with_capture::<false>() }

    /// 将 replay 捕获策略固化为编译期常量，让批量路径消除每回合的交互模式分支。
    fn run_minimal_round_with_capture<const CAPTURE_UPDATES: bool>(&mut self) -> RoundOutcome {
        loop {
            if let Some(outcome) = self.run_minimal_round_once_with_capture::<CAPTURE_UPDATES>() {
                return outcome;
            }
        }
    }

    pub fn run_minimal_round_once(&mut self) -> Option<RoundOutcome> { self.run_minimal_round_once_with_capture::<true>() }

    fn run_minimal_round_once_with_capture<const CAPTURE_UPDATES: bool>(&mut self) -> Option<RoundOutcome> {
        let winner_team = if CAPTURE_UPDATES {
            self.world.sync_winner(&self.entities)
        } else {
            self.world.sync_winner_from_alive_views()
        };
        if let Some(winner_team) = winner_team {
            return Some(RoundOutcome {
                action: None,
                frame: None,
                winner_team: Some(winner_team),
            });
        }

        let selected_action = self.scheduler.select_action(&mut self.world, &mut self.entities, &mut self.rng);
        let mut updates = if CAPTURE_UPDATES {
            RunUpdates::new()
        } else {
            RunUpdates::new_no_capture()
        };
        for target in self.scheduler.take_ice_release_events() {
            updates.add_newline();
            updates.add(RuntimeFrame::replay_update(
                target.0 as usize,
                target.0 as usize,
                "[1]从[冰冻]中解除",
                0,
            ));
        }
        let Some(mut action) = selected_action else {
            return Some(self.finish_round_with_capture::<CAPTURE_UPDATES>(None, updates));
        };
        let legacy_plain_action = self.scheduler.uses_legacy_step_scheduler();
        self.scratch.selected_actor_round = self.round;
        #[cfg(not(feature = "no_debug"))]
        let debug_tick = crate::debug::debug_tick();
        #[cfg(not(feature = "no_debug"))]
        let tick_rng_before = (self.rng.i, self.rng.j);
        #[cfg(not(feature = "no_debug"))]
        if debug_tick {
            let actor = self
                .entities
                .get(action.actor)
                .unwrap_or_else(|| panic!("unknown runtime debug tick actor: {}", action.actor.0));
            eprintln!(
                "[runtime_tick] actor={} id={} mv={} hp={} rc4=({}, {})",
                actor.template.name,
                action.actor.0,
                actor.runtime.move_state.speed_points,
                actor.runtime.hp,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        let action_rng_before = RngCheckpoint::from_rc4(&self.rng);
        let smart = self.roll_actor_smart(action.actor);
        // 潜行 pending 的 pre-action 槽位可能在技能表重建（升级/合并/状态刷新）时被丢掉，
        // tswn 自己在施放时会 ensure 一次，这里在每次行动前补回来，
        // 之后完全交给引擎的 pre_action 规则判断要不要背刺、会不会被识破。
        if let Some(pending) = self.entities.get(action.actor).and_then(|entity| entity.runtime.assassinate) {
            if let Some(entity) = self.entities.get_mut(action.actor) {
                entity.template.skills.ensure_pre_action_lane(pending.fixed_lane);
            }
        }
        let plain_skill_pre_action = legacy_plain_action
            .then(|| self.run_plain_skill_pre_action_accumulator(action.actor))
            .unwrap_or_default();

        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_ACTION);
        let selected_target = if legacy_plain_action {
            action.target
        } else {
            self.selected_pre_action_target(&skill_plan, action.actor, smart).unwrap_or(action.target)
        };
        self.drain_skill_hook_plan_with_selected_target_into(&skill_plan, &mut updates, Some(selected_target));
        let pre_action_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_ACTION);
        let state_intercepted_action =
            self.drain_state_hook_plan_with_action_smart_into(&pre_action_state_plan, &mut updates, Some(smart));
        let mut prepared_plain_action = None;
        if legacy_plain_action && !state_intercepted_action {
            if self
                .entities
                .get(action.actor)
                .unwrap_or_else(|| panic!("unknown runtime frozen-action actor: {}", action.actor.0))
                .states
                .is_frozen()
            {
                if updates.had_updates() {
                    return Some(self.finish_round_with_capture::<CAPTURE_UPDATES>(None, updates));
                }
                return None;
            }
            let forced_pre_action_skill = plain_skill_pre_action.forced_skill.is_some();
            let Some(prepared) = self.prepare_plain_action(action.actor, smart, plain_skill_pre_action) else {
                return Some(self.finish_round_with_capture::<CAPTURE_UPDATES>(None, updates));
            };
            match &prepared {
                PreparedPlainAction::BasicAttack { target, amount, .. } => {
                    action.target = *target;
                    action.amount = *amount;
                }
                PreparedPlainAction::ForcedAttack { target, amount } => {
                    action.target = *target;
                    action.amount = *amount;
                }
                PreparedPlainAction::Saitama { target } => {
                    action.target = target.unwrap_or(action.actor);
                    action.amount = 0;
                }
                PreparedPlainAction::BuiltinSkill(prepared) => {
                    if forced_pre_action_skill {
                        action.target = action.actor;
                    } else {
                        action.target = prepared.targets.first().copied().unwrap_or(action.actor);
                    }
                    action.amount = 0;
                }
            }
            prepared_plain_action = Some(prepared);
        }
        #[cfg(not(feature = "no_debug"))]
        let action_rng_after = RngCheckpoint::from_rc4(&self.rng);
        #[cfg(not(feature = "no_debug"))]
        if let Some(trace) = &mut self.trace {
            trace.record_action(TraceAction {
                round: self.round + 1,
                actor: action.actor,
                target: action.target,
                amount: action.amount,
                rng_before: Some(action_rng_before),
                rng_after: Some(action_rng_after),
            });
        }
        let forced_plain_action = matches!(prepared_plain_action.as_ref(), Some(PreparedPlainAction::ForcedAttack { .. }));
        let builtin_plain_action = matches!(prepared_plain_action.as_ref(), Some(PreparedPlainAction::BuiltinSkill(_)));
        let terminal_plain_action = if state_intercepted_action {
            // PRE_ACTION 状态钩子接管本次行动后，legacy 仍会继续执行恢复和完整的行动后链。
            legacy_plain_action && !self.has_alive_enemy_or_pending_spawn(action.actor)
        } else if builtin_plain_action {
            let Some(PreparedPlainAction::BuiltinSkill(prepared)) = prepared_plain_action.take() else {
                unreachable!("builtin action marker must retain the prepared action")
            };
            self.drain_plain_builtin_skill_into(action.actor, prepared, &mut updates);
            legacy_plain_action && !self.has_alive_enemy_or_pending_spawn(action.actor)
        } else {
            let pre_damage_skill_plan =
                self.scheduler
                    .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_DAMAGE);
            self.drain_skill_hook_plan_into(&pre_damage_skill_plan, &mut updates);
            let pre_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_DAMAGE);
            self.drain_state_hook_plan_into(&pre_damage_state_plan, &mut updates);
            match prepared_plain_action.take() {
                Some(PreparedPlainAction::BasicAttack { use_magic, .. }) => {
                    self.drain_plain_default_attack_into(action.actor, action.target, use_magic, &mut updates);
                }
                Some(PreparedPlainAction::ForcedAttack { target, .. }) => {
                    self.drain_plain_berserk_forced_attack_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::Saitama { target }) => {
                    self.drain_plain_saitama_action_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::BuiltinSkill(_)) => {
                    unreachable!("builtin skill actions are handled before the default action branch")
                }
                None => {
                    self.effects.push(QueuedEffect::Damage {
                        caster: action.actor,
                        target: action.target,
                        amount: action.amount,
                    });
                    self.drain_effects_into(&mut updates);
                }
            }
            let terminal_plain_action = legacy_plain_action && !self.has_alive_enemy_or_pending_spawn(action.actor);
            let post_damage_skill_plan =
                self.scheduler
                    .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_DAMAGE);
            self.drain_skill_hook_plan_into(&post_damage_skill_plan, &mut updates);
            let post_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_DAMAGE);
            self.drain_state_hook_plan_into(&post_damage_state_plan, &mut updates);
            terminal_plain_action
        };
        if !terminal_plain_action {
            if forced_plain_action {
                self.drain_plain_berserk_forced_action_state_into(action.actor, &mut updates);
            }
            if legacy_plain_action {
                self.recover_plain_actor_into(action.actor, &mut updates);
            }
            if state_intercepted_action {
                updates.add_newline();
            }
            self.drain_post_action_chain_into(action.actor, &mut updates);
        }
        self.drain_plain_update_end_into(&mut updates);
        #[cfg(not(feature = "no_debug"))]
        if debug_tick {
            let actor = self
                .entities
                .get(action.actor)
                .unwrap_or_else(|| panic!("unknown runtime debug tick actor: {}", action.actor.0));
            let bytes = (self.rng.i as i32 - tick_rng_before.0 as i32).rem_euclid(256);
            eprintln!(
                "[runtime_tick_end] actor={} id={} mp_after={} hp_after={} rc4=({},{})->({},{}) bytes={} messages={:?}",
                actor.template.name,
                action.actor.0,
                actor.runtime.move_state.speed_points,
                actor.runtime.hp,
                tick_rng_before.0,
                tick_rng_before.1,
                self.rng.i,
                self.rng.j,
                bytes,
                updates
                    .updates
                    .iter()
                    .filter(|update| !matches!(update.update_type, crate::runtime::update::UpdateType::NextLine))
                    .map(|update| update.message.as_ref())
                    .collect::<Vec<_>>(),
            );
        }
        Some(self.finish_round_with_capture::<CAPTURE_UPDATES>(Some(action), updates))
    }

    fn drain_post_action_chain_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let mut skill_plans = self.scheduler.skill_post_action_plans(&self.entities, &self.registry, owner);
        self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, &skill_plans.early, updates);
        let generation_after_early = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime post-action owner entity: {}", owner.0))
            .template
            .skills
            .hook_generation();
        if generation_after_early != skill_plans.generation {
            skill_plans = self.scheduler.skill_post_action_plans(&self.entities, &self.registry, owner);
        }

        let state_plan = self.scheduler.state_hook_plan(&self.entities, owner, ProcMask::POST_ACTION);
        let deferred_entries = &skill_plans.deferred;
        let mut deferred_idx = 0usize;
        let mut deferred_owner_state_clears = Vec::new();
        for state_entry in state_plan.entries.iter().copied() {
            while deferred_entries
                .get(deferred_idx)
                .is_some_and(|(cursor, _)| *cursor <= state_entry.runtime_registration_order)
            {
                let entry = deferred_entries[deferred_idx].1;
                self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, std::slice::from_ref(&entry), updates);
                deferred_idx += 1;
            }
            // legacy 的状态循环会在每个状态执行前检查 dj()：只有行动者已经死亡且
            // 战斗同时结束时才中断；尚未执行的尾部 deferred skill 仍会在循环后继续处理。
            let owner_alive = self.entities.get(owner).is_some_and(|entity| entity.runtime.alive);
            if !owner_alive && self.world.alive_group_count() <= 1 {
                break;
            }
            self.drain_state_hook_entry_with_deferred_clears_into(
                ProcMask::POST_ACTION,
                state_entry,
                updates,
                Some(&mut deferred_owner_state_clears),
            );
        }
        while let Some((_, entry)) = deferred_entries.get(deferred_idx).copied() {
            self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, std::slice::from_ref(&entry), updates);
            deferred_idx += 1;
        }
        // legacy 会先记录本轮需要清理的状态，等状态与中途注册技能全部执行完后再统一移除。
        self.flush_deferred_owner_state_clears(owner, &deferred_owner_state_clears);

        // Charge 以及 Haste、Slow 的尾阶段必须继续晚于状态和中途注册的 early 技能。
        let generation_after_states = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime post-action owner entity: {}", owner.0))
            .template
            .skills
            .hook_generation();
        if generation_after_states != skill_plans.generation {
            skill_plans = self.scheduler.skill_post_action_plans(&self.entities, &self.registry, owner);
        }
        self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, &skill_plans.late, updates);
    }

    pub fn finish_round(&mut self, action: Option<ActionPlan>, updates: RunUpdates) -> RoundOutcome {
        if updates.capture_updates {
            self.finish_round_with_capture::<true>(action, updates)
        } else {
            self.finish_round_with_capture::<false>(action, updates)
        }
    }

    /// 批量路径不会生成 frame，也不需要重新扫描实体表判定赢家。
    fn finish_round_with_capture<const CAPTURE_UPDATES: bool>(
        &mut self,
        action: Option<ActionPlan>,
        updates: RunUpdates,
    ) -> RoundOutcome {
        debug_assert_eq!(updates.capture_updates, CAPTURE_UPDATES);
        let frame = (CAPTURE_UPDATES && updates.had_updates()).then_some(RuntimeFrame { updates });
        self.round += 1;
        let winner_team = if CAPTURE_UPDATES {
            self.world.sync_winner(&self.entities)
        } else {
            self.world.sync_winner_from_alive_views()
        };
        #[cfg(not(feature = "no_debug"))]
        if let (Some(trace), Some(frame)) = (&mut self.trace, &frame) {
            trace.record_frame(self.round, frame, winner_team, Some(RngCheckpoint::from_rc4(&self.rng)));
        }
        RoundOutcome {
            action,
            frame,
            winner_team,
        }
    }

    pub fn roll_actor_smart(&mut self, actor: EntityIdx) -> bool {
        let smart_byte = self.rng.next_u8();
        let smart_roll = (smart_byte & 63) as i32;
        self.entities.get(actor).is_some_and(|entity| entity.runtime.wisdom > smart_roll)
    }

    #[cfg(not(feature = "no_debug"))]
    pub fn probe_plain_action_matches(&self, actor: EntityIdx) -> bool {
        crate::debug::probe_action()
            .map(|needle| {
                self.entities
                    .get(actor)
                    .is_some_and(|entity| entity.template.name.contains(needle) || entity.template.display_name.contains(needle))
            })
            .unwrap_or(false)
    }

    /// 玩家操控的单位：受控队伍里的本体（默认 kind）与分身。
    /// 幻影 / 使魔 / 丧尸属于召唤物，仍由引擎自动行动。
    fn is_controlled_actor(&self, actor: EntityIdx, team: usize) -> bool {
        let Some(entity) = self.entities.get(actor) else {
            return false;
        };
        if entity.template.team != team {
            return false;
        }
        match self.registry.player_kind(entity.runtime.kind) {
            None => true,
            Some(spec) => spec.export_name == crate::runtime::profile::DEFAULT_CORE_CLONE_KIND_EXPORT,
        }
    }

    pub fn prepare_plain_action(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        pre_action: PlainSkillPreActionOutcome,
    ) -> Option<PreparedPlainAction> {
        crate::runtime::forced_action::hit();
        // 上一手的药水只持续一回合，这里先还原
        self.revert_active_potion();
        // 狂暴（含被附体）期间由引擎接管：不接收玩家指令、也不停下来要牌。
        let berserk_forced = !pre_action.clear_forced_action && self.has_plain_berserk_state(actor);
        let mut berserk_submitted = false;
        // 卡牌玩法注入：轮到指定实体时优先执行玩家排好的行动。
        // 但引擎自己有强制动作时（潜行→背刺的第二步）必须先让引擎走完，
        // 否则玩家能在这段窗口里插别的技能，背刺就永远放不出来。
        if pre_action.forced_skill.is_none() {
            if let Some((forced_skill, forced_mp)) = crate::runtime::forced_action::take(actor.0 as u32) {
                if let Some(code) = crate::runtime::forced_action::take_potion(actor.0 as u32) {
                    self.apply_potion(actor, code);
                }
                if berserk_forced {
                    // 狂暴：玩家这一手只用来推进节奏，真正动作由引擎接管
                    berserk_submitted = true;
                } else {
                    // 玩家抽到这张牌时已经掷过消耗；没有就按 tswn 现场掷。
                    let req_mp = match forced_mp {
                        Some(cost) => cost as i32,
                        None => {
                            let req_mp_byte = self.rng.next_u8();
                            (req_mp_byte & 15) as i32 + 8
                        }
                    };
                    let (mp, attack) = {
                        let entity = self
                            .entities
                            .get(actor)
                            .unwrap_or_else(|| panic!("unknown forced action actor: {}", actor.0));
                        (entity.runtime.magic_point, entity.runtime.attack)
                    };
                    // 玩家点名要用这个技能：目标选择按「不挑目标」的宽松口径，
                    // 避免因为 AI 的 smart 过滤（例如瘟疫只挑 160~400 血的目标）而放不出来。
                    let prepared = if mp >= req_mp {
                        self.prepare_forced_builtin_skill(actor, forced_skill, smart)
                    } else {
                        None
                    };
                    if let Some(prepared) = prepared {
                        if let Some(entity) = self.entities.get_mut(actor) {
                            entity.runtime.magic_point -= req_mp;
                        }
                        return Some(PreparedPlainAction::BuiltinSkill(prepared));
                    }
                    let target = self.select_plain_default_attack_target(actor, smart)?;
                    return Some(PreparedPlainAction::BasicAttack {
                        target,
                        use_magic: false,
                        amount: attack,
                    });
                }
            }
        }

        // 受控单位：只有在引擎没有强制动作、且不是「狂暴已收到玩家这一手」时才暂停。
        if pre_action.forced_skill.is_none() && !berserk_submitted {
            if let Some(team) = crate::runtime::forced_action::controlled_team() {
                // 玩家操控的单位（本体 + 分身）：没有排好的指令就停在这里，绝不代替玩家行动。
                // 召唤物（幻影 / 使魔 / 丧尸）不在此列，仍按 tswn 的规则自动行动。
                if self.is_controlled_actor(actor, team) {
                    crate::runtime::forced_action::set_pause(actor.0 as u32);
                    return None;
                }
            }
        }
        if let Some(forced_skill) = pre_action.forced_skill {
            return Some(PreparedPlainAction::BuiltinSkill(forced_skill));
        }
        if !pre_action.clear_forced_action && self.has_plain_berserk_state(actor) {
            let target = self.select_plain_berserk_forced_attack_target(smart)?;
            let amount = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime forced-attack actor: {}", actor.0))
                .runtime
                .attack;
            return Some(PreparedPlainAction::ForcedAttack {
                target,
                amount: (f64::from(amount) * 1.2000000476837158).round() as i32,
            });
        }

        #[cfg(not(feature = "no_debug"))]
        let rng_before = (self.rng.i, self.rng.j);
        let req_mp_byte = self.rng.next_u8();
        let req_mp = (req_mp_byte & 15) as i32 + 8;
        let (mp_before, attack, magic, is_boss, boss_action_prob_count) = {
            let actor_entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime default attack actor: {}", actor.0));
            let is_boss = actor_entity.runtime.flags.contains(PlayerKindFlags::BOSS);
            (
                actor_entity.runtime.magic_point,
                actor_entity.runtime.attack,
                actor_entity.runtime.magic,
                is_boss,
                if is_boss {
                    crate::namerena::boss_action_prob_count(&actor_entity.template.name)
                } else {
                    0
                },
            )
        };
        let can_scan_skills = mp_before >= req_mp;
        #[cfg(not(feature = "no_debug"))]
        let probe_action = self.probe_plain_action_matches(actor);
        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime action probe actor: {}", actor.0));
            eprintln!(
                "[action_probe:runtime:mp] round={} actor={} name={} smart={} req_mp_byte={} req_mp={} mp_before={} \
                 can_scan={} rc4=({},{}) -> ({},{})",
                self.round + 1,
                actor.0,
                entity.template.name,
                smart,
                req_mp_byte,
                req_mp,
                mp_before,
                can_scan_skills,
                rng_before.0,
                rng_before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if crate::debug::probe_possess().is_some() {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime action probe actor: {}", actor.0));
            eprintln!(
                "[possess_probe:runtime:mp] round={} actor={} name={} smart={} req_mp_byte={} req_mp={} mp_before={} \
                 can_scan={} rc4=({},{}) -> ({},{})",
                self.round + 1,
                actor.0,
                entity.template.name,
                smart,
                req_mp_byte,
                req_mp,
                mp_before,
                can_scan_skills,
                rng_before.0,
                rng_before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        let prepared_skill = if can_scan_skills {
            let selected = if is_boss {
                for _ in 0..boss_action_prob_count {
                    let _ = self.rng.r127();
                }
                None
            } else {
                self.scan_plain_action_skill_probabilities(actor, smart)
            };
            self.entities.get_mut(actor).unwrap().runtime.magic_point -= req_mp;
            selected
        } else {
            None
        };
        if let Some(prepared) = prepared_skill {
            #[cfg(not(feature = "no_debug"))]
            if probe_action {
                eprintln!(
                    "[action_probe:runtime:selected] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
                    actor.0,
                    prepared.selected.skill.export_name(),
                    prepared.selected.fixed_lane,
                    prepared.targets,
                    self.rng.i,
                    self.rng.j,
                );
            }
            return Some(PreparedPlainAction::BuiltinSkill(prepared));
        }

        if is_boss {
            if self.saitama_boss_state(actor).is_some() {
                let target = self.select_plain_default_attack_target(actor, smart);
                return Some(PreparedPlainAction::Saitama { target });
            }
            let target = self.select_plain_default_attack_target(actor, smart)?;
            return Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: false,
                amount: attack,
            });
        }

        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            eprintln!(
                "[action_probe:runtime:fallback] actor={} rc4=({}, {})",
                actor.0, self.rng.i, self.rng.j
            );
        }
        let target = self.select_plain_default_attack_target(actor, smart)?;
        let magic_cost = (magic - attack) >> 2;
        let mp_after_skill_scan = if can_scan_skills { mp_before - req_mp } else { mp_before };
        let use_magic = smart && magic > attack && mp_after_skill_scan >= magic_cost;
        if use_magic {
            self.entities.get_mut(actor).unwrap().runtime.magic_point -= magic_cost;
            Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: true,
                amount: magic,
            })
        } else {
            Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: false,
                amount: attack,
            })
        }
    }

    /// 喝药：加强当前角色一项属性一回合（不占行动）+ 少量回血
    fn apply_potion(&mut self, actor: EntityIdx, code: u32) {
        // code = 属性下标 + 1（0 表示没喝药）
        let field = ((code & 0xF) as usize).saturating_sub(1);
        let Some(entity) = self.entities.get_mut(actor) else { return };
        if field == 9 {
            // 9 = 能量药水：回复 40 点 MP（上限 100）
            entity.runtime.magic_point = (entity.runtime.magic_point + 40).min(100);
            return;
        }
        if field >= 8 {
            // 8 = 体力药水：回复 25% 最大体力
            let heal = (entity.template.max_hp / 4).max(10);
            entity.runtime.hp = (entity.runtime.hp + heal).min(entity.template.max_hp);
            return;
        }
        let slot = match field {
            0 => &mut entity.runtime.attack,
            1 => &mut entity.runtime.defense,
            2 => &mut entity.runtime.speed,
            3 => &mut entity.runtime.agility,
            4 => &mut entity.runtime.magic,
            5 => &mut entity.runtime.resistance,
            _ => &mut entity.runtime.wisdom,
        };
        let delta = (*slot / 4).max(8);
        *slot += delta;
        crate::runtime::forced_action::set_active_potion(Some((actor.0, field as u8, delta)));
    }

    /// 还原上一回合的药水加成
    fn revert_active_potion(&mut self) {
        let Some((actor_id, field, delta)) = crate::runtime::forced_action::take_active_potion() else {
            return;
        };
        let entity = self.entities.get_mut(EntityIdx(actor_id));
        let Some(entity) = entity else { return };
        match field {
            0 => entity.runtime.attack -= delta,
            1 => entity.runtime.defense -= delta,
            2 => entity.runtime.speed -= delta,
            3 => entity.runtime.agility -= delta,
            4 => entity.runtime.magic -= delta,
            5 => entity.runtime.resistance -= delta,
            6 => entity.runtime.wisdom -= delta,
            _ => {}
        }
    }

    pub fn has_plain_berserk_state(&self, actor: EntityIdx) -> bool {
        self.entities.get(actor).is_some_and(|entity| {
            entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
        })
    }

    pub fn select_plain_berserk_forced_attack_target(&mut self, smart: bool) -> Option<EntityIdx> {
        let all_alive = self.world.flat_alive();
        if all_alive.is_empty() {
            return None;
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = self.rng.pick(all_alive)?;
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        if selected.is_empty() {
            return None;
        }

        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            let attract = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("runtime forced-attack target disappeared: {}", target.0))
                .runtime
                .attract();
            scored.push((target, self.rng.rFFFF() as f64 * attract));
        }
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.first().map(|(target, _)| *target)
    }

    pub fn drain_plain_berserk_forced_attack_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0]发起[狂暴攻击]",
            0,
        ));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime forced-attack actor: {}", actor.0))
            .runtime
            .get_at(false, &mut self.rng)
            * 1.2000000476837158;
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
    }

    pub fn drain_plain_berserk_forced_action_state_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let (state_key, clear_state, actor_alive) = {
            let actor_entity = self
                .entities
                .get_mut(actor)
                .unwrap_or_else(|| panic!("unknown runtime forced-attack actor: {}", actor.0));
            let Some(entry) = actor_entity
                .states
                .entries()
                .iter()
                .find(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
            else {
                return;
            };
            let state_key = entry.legacy_order_key;
            let entry = actor_entity
                .states
                .entry_mut(state_key)
                .expect("runtime berserk state disappeared during forced action");
            let StatePayload::Berserk { step } = &mut entry.payload else {
                unreachable!("runtime berserk state key changed payload during forced action");
            };
            *step -= 1;
            (state_key, *step <= 0, actor_entity.runtime.active())
        };

        if !clear_state {
            return;
        }
        if actor_alive {
            updates.add_newline();
            updates.add(crate::runtime::update::RunUpdate::new(
                "[1]从[狂暴]中解除",
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
        }
        let removed = self
            .entities
            .get_mut(actor)
            .expect("runtime forced-attack actor disappeared before state clear")
            .states
            .clear_legacy_key(state_key);
        debug_assert!(removed, "runtime berserk state disappeared before state clear");
    }

    /// 按技能 id 构造一次主动技能行动（卡牌玩法强制出招）。
    ///
    /// 只校验该技能在当前 loadout 中的等级是否大于 0，并用与自动选择相同的
    /// 目标选择函数；队列为空时 Runtime 行为与上游完全一致。
    pub fn prepare_forced_builtin_skill(
        &mut self,
        actor: EntityIdx,
        skill_id: u32,
        smart: bool,
    ) -> Option<PreparedBuiltinSkillAction> {
        let builtin_skill = public_builtin_skill(skill_id)?;
        let fixed_lane = {
            let entity = self.entities.get(actor)?;
            let loadout = &entity.template.skills;
            let mut found = None;
            for (lane, id) in loadout.skills().iter().enumerate() {
                if self.builtin_active_skill(*id) == Some(builtin_skill) {
                    found = Some(lane);
                    break;
                }
            }
            let lane = found?;
            if loadout.level_at(lane).unwrap_or(0) == 0 {
                return None;
            }
            lane
        };
        // 目标与命中都交给 tswn：优先用引擎这一回合掷出的 smart 口径；
        // 只有当这个口径一个目标都选不出来时才退到宽松口径，避免玩家点名的技能变成普攻。
        let mut targets = self.select_forced_skill_targets(actor, builtin_skill, smart);
        if targets.is_empty() {
            targets = self.select_forced_skill_targets(actor, builtin_skill, false);
        }
        let allows_empty_targets = builtin_skill == BuiltinActiveSkill::Assassinate
            && self.entities.get(actor).is_some_and(|entity| entity.runtime.assassinate.is_some());
        if targets.is_empty() && !allows_empty_targets {
            return None;
        }
        Some(PreparedBuiltinSkillAction {
            selected: SelectedBuiltinSkill {
                skill: builtin_skill,
                fixed_lane,
            },
            targets,
        })
    }

    fn select_forced_skill_targets(
        &mut self,
        actor: EntityIdx,
        builtin_skill: BuiltinActiveSkill,
        smart: bool,
    ) -> PreparedTargetList {
        match builtin_skill {
            BuiltinActiveSkill::Fire
            | BuiltinActiveSkill::Thunder
            | BuiltinActiveSkill::Absorb
            | BuiltinActiveSkill::Poison
            | BuiltinActiveSkill::Critical
            | BuiltinActiveSkill::SummonExplode => self.select_plain_default_enemy_targets(actor, smart),
            BuiltinActiveSkill::Berserk => self.select_plain_berserk_targets(actor, smart),
            BuiltinActiveSkill::Quake => {
                self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 6 } else { 5 })
            }
            BuiltinActiveSkill::Ice => self.select_plain_ice_targets(actor, smart),
            BuiltinActiveSkill::Rapid => self.select_plain_rapid_targets(actor, smart),
            BuiltinActiveSkill::Half => self.select_plain_half_targets(actor, smart),
            BuiltinActiveSkill::Shadow => smallvec::smallvec![actor],
            BuiltinActiveSkill::Charm => self.select_plain_charm_targets(actor, smart),
            BuiltinActiveSkill::Curse => self.select_plain_curse_targets(actor, smart),
            BuiltinActiveSkill::Haste => self.select_plain_haste_targets(actor, smart),
            BuiltinActiveSkill::Heal => self.select_plain_heal_targets(actor, smart),
            BuiltinActiveSkill::Slow => self.select_plain_slow_targets(actor, smart),
            BuiltinActiveSkill::Exchange => self.select_plain_exchange_targets(actor, smart),
            BuiltinActiveSkill::Revive => self.select_plain_revive_targets(actor, smart),
            BuiltinActiveSkill::Disperse => self.select_plain_disperse_targets(actor, smart),
            BuiltinActiveSkill::Iron => smallvec::smallvec![actor],
            BuiltinActiveSkill::Clone => smallvec::smallvec![actor],
            BuiltinActiveSkill::Charge => smallvec::smallvec![actor],
            BuiltinActiveSkill::Accumulate => smallvec::smallvec![actor],
            BuiltinActiveSkill::Assassinate => self.select_plain_assassinate_targets(actor, smart),
            BuiltinActiveSkill::Summon => smallvec::smallvec![actor],
            BuiltinActiveSkill::Possess => self.select_plain_possess_targets(actor, smart),
        }
    }

    pub fn scan_plain_action_skill_probabilities(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBuiltinSkillAction> {
        let (scan_len, cached_scan) = {
            let skills = &self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime action-scan actor: {}", actor.0))
                .template
                .skills;
            match skills.cached_builtin_actions() {
                Some(cache) => (
                    cache.len(),
                    Some((cache.as_ptr(), skills.levels().as_ptr(), skills.levels().len())),
                ),
                None => (skills.active_order().len(), None),
            }
        };
        for active_index in 0..scan_len {
            let (fixed_lane, builtin_skill, level) = if let Some((cache, levels, levels_len)) = cached_scan {
                // SAFETY: 两个指针来自 actor 当前 SkillLoadout。扫描失败路径只推进 RNG，
                // plain_action_skill_probability 不修改实体或技能表；一旦目标选择开始便会立即
                // 返回，不再读取指针。缓存由 prepare_hook_cache 构造，每个 fixed_lane 均来自
                // 同一份 levels，因此 active_index 与 fixed_lane 都已分别受两段长度约束。
                let cached = unsafe { *cache.add(active_index) };
                let fixed_lane = usize::from(cached.fixed_lane);
                debug_assert!(fixed_lane < levels_len, "runtime 主动技能缓存固定槽位越界");
                let level = unsafe { *levels.add(fixed_lane) };
                (fixed_lane, cached.skill, level)
            } else {
                let loadout = &self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime action-scan actor: {}", actor.0))
                    .template
                    .skills;
                let fixed_lane = *loadout
                    .active_order()
                    .get(active_index)
                    .unwrap_or_else(|| panic!("runtime active skill order missing index {active_index}"));
                let Some(skill_id) = loadout.skills().get(fixed_lane).copied() else {
                    panic!("runtime active skill order references missing fixed lane {fixed_lane}");
                };
                let level = loadout
                    .level_at(fixed_lane)
                    .unwrap_or_else(|| panic!("runtime active skill level missing for fixed lane {fixed_lane}"));
                let Some(builtin_skill) = self.builtin_active_skill(skill_id) else {
                    continue;
                };
                (fixed_lane, builtin_skill, level)
            };
            if level == 0 {
                continue;
            }
            if self.plain_action_skill_probability(actor, builtin_skill, level, smart) {
                let selected = SelectedBuiltinSkill {
                    skill: builtin_skill,
                    fixed_lane,
                };
                let targets = match builtin_skill {
                    BuiltinActiveSkill::Fire
                    | BuiltinActiveSkill::Thunder
                    | BuiltinActiveSkill::Absorb
                    | BuiltinActiveSkill::Poison
                    | BuiltinActiveSkill::Critical
                    | BuiltinActiveSkill::SummonExplode => self.select_plain_default_enemy_targets(actor, smart),
                    BuiltinActiveSkill::Berserk => self.select_plain_berserk_targets(actor, smart),
                    BuiltinActiveSkill::Quake => {
                        self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 6 } else { 5 })
                    }
                    BuiltinActiveSkill::Ice => self.select_plain_ice_targets(actor, smart),
                    BuiltinActiveSkill::Rapid => self.select_plain_rapid_targets(actor, smart),
                    BuiltinActiveSkill::Half => self.select_plain_half_targets(actor, smart),
                    BuiltinActiveSkill::Shadow => smallvec::smallvec![actor],
                    BuiltinActiveSkill::Charm => self.select_plain_charm_targets(actor, smart),
                    BuiltinActiveSkill::Curse => self.select_plain_curse_targets(actor, smart),
                    BuiltinActiveSkill::Haste => self.select_plain_haste_targets(actor, smart),
                    BuiltinActiveSkill::Heal => self.select_plain_heal_targets(actor, smart),
                    BuiltinActiveSkill::Slow => self.select_plain_slow_targets(actor, smart),
                    BuiltinActiveSkill::Exchange => self.select_plain_exchange_targets(actor, smart),
                    BuiltinActiveSkill::Revive => self.select_plain_revive_targets(actor, smart),
                    BuiltinActiveSkill::Disperse => self.select_plain_disperse_targets(actor, smart),
                    BuiltinActiveSkill::Iron => smallvec::smallvec![actor],
                    BuiltinActiveSkill::Clone => smallvec::smallvec![actor],
                    BuiltinActiveSkill::Charge => smallvec::smallvec![actor],
                    BuiltinActiveSkill::Accumulate => smallvec::smallvec![actor],
                    BuiltinActiveSkill::Assassinate => self.select_plain_assassinate_targets(actor, smart),
                    BuiltinActiveSkill::Summon => smallvec::smallvec![actor],
                    BuiltinActiveSkill::Possess => self.select_plain_possess_targets(actor, smart),
                };
                #[cfg(not(feature = "no_debug"))]
                if self.probe_plain_action_matches(actor) {
                    eprintln!(
                        "[action_probe:runtime:targets] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
                        actor.0,
                        builtin_skill.export_name(),
                        fixed_lane,
                        targets,
                        self.rng.i,
                        self.rng.j,
                    );
                }
                let allows_empty_targets = builtin_skill == BuiltinActiveSkill::Assassinate
                    && self.entities.get(actor).is_some_and(|entity| entity.runtime.assassinate.is_some());
                if targets.is_empty() && !allows_empty_targets {
                    continue;
                }
                return Some(PreparedBuiltinSkillAction { selected, targets });
            }
        }
        None
    }

    pub fn builtin_active_skill(&self, skill_id: SkillId) -> Option<BuiltinActiveSkill> {
        self.registry.builtin_active_skill(skill_id)
    }

    pub fn plain_action_skill_probability(
        &mut self,
        actor: EntityIdx,
        builtin_skill: BuiltinActiveSkill,
        level: u32,
        smart: bool,
    ) -> bool {
        #[cfg(not(feature = "no_debug"))]
        let probe_action = self.probe_plain_action_matches(actor);
        match builtin_skill {
            BuiltinActiveSkill::Charge => {
                let actor_runtime = &self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime charge probability actor: {}", actor.0))
                    .runtime;
                if actor_runtime.charge.active || (smart && actor_runtime.hp < 100) {
                    #[cfg(not(feature = "no_debug"))]
                    if probe_action {
                        eprintln!(
                            "[action_probe:runtime:prob] actor={} skill={} level={} smart={} skipped=charge_gate active={} hp={} \
                         rc4=({}, {})",
                            actor.0,
                            builtin_skill.export_name(),
                            level,
                            smart,
                            actor_runtime.charge.active,
                            actor_runtime.hp,
                            self.rng.i,
                            self.rng.j,
                        );
                    }
                    return false;
                }
            }
            BuiltinActiveSkill::Absorb if smart => {
                let actor_entity = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime absorb probability actor: {}", actor.0));
                if actor_entity.template.max_hp - actor_entity.runtime.hp < 32 {
                    #[cfg(not(feature = "no_debug"))]
                    if probe_action {
                        eprintln!(
                            "[action_probe:runtime:prob] actor={} skill={} level={} smart={} skipped=absorb_low_missing_hp \
                         hp={} max_hp={} rc4=({}, {})",
                            actor.0,
                            builtin_skill.export_name(),
                            level,
                            smart,
                            actor_entity.runtime.hp,
                            actor_entity.template.max_hp,
                            self.rng.i,
                            self.rng.j,
                        );
                    }
                    return false;
                }
            }
            BuiltinActiveSkill::Iron => {
                if self
                    .entities
                    .get(actor)
                    .and_then(|entity| entity.states.entry(PLAIN_IRON_STATE_KEY))
                    .and_then(StateEntry::iron_value)
                    .is_some_and(|(protect, step)| protect > 0 && step > 0)
                {
                    return false;
                }
            }
            BuiltinActiveSkill::Accumulate => {
                let actor_runtime = &self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime accumulate probability actor: {}", actor.0))
                    .runtime;
                if actor_runtime.accumulate.active || (smart && actor_runtime.hp < 120) {
                    #[cfg(not(feature = "no_debug"))]
                    if probe_action {
                        eprintln!(
                            "[action_probe:runtime:prob] actor={} skill={} level={} smart={} skipped=accumulate_gate \
                         active={} hp={} rc4=({}, {})",
                            actor.0,
                            builtin_skill.export_name(),
                            level,
                            smart,
                            actor_runtime.accumulate.active,
                            actor_runtime.hp,
                            self.rng.i,
                            self.rng.j,
                        );
                    }
                    return false;
                }
            }
            BuiltinActiveSkill::Assassinate if smart => {
                if self.entities.get(actor).is_some_and(|entity| {
                    entity
                        .states
                        .entries()
                        .iter()
                        .any(|entry| matches!(entry.payload, StatePayload::Poison { .. }))
                }) {
                    return false;
                }
            }
            BuiltinActiveSkill::Summon => {
                if !self.plain_summon_probability_allowed(actor, smart) {
                    return false;
                }
            }
            // 影袭在智能模式且 HP < 80 时短路，不消耗概率字节。
            BuiltinActiveSkill::Shadow if smart => {
                if self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime shadow probability actor: {}", actor.0))
                    .runtime
                    .hp
                    < 80
                {
                    #[cfg(not(feature = "no_debug"))]
                    if probe_action {
                        eprintln!(
                            "[action_probe:runtime:prob] actor={} skill={} level={} smart={} skipped=shadow_low_hp rc4=({}, {})",
                            actor.0,
                            builtin_skill.export_name(),
                            level,
                            smart,
                            self.rng.i,
                            self.rng.j,
                        );
                    }
                    return false;
                }
            }
            _ => {}
        }
        #[cfg(not(feature = "no_debug"))]
        let before = (self.rng.i, self.rng.j);
        let roll = self.rng.r127();
        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            eprintln!(
                "[action_probe:runtime:prob] actor={} skill={} level={} smart={} roll={} pass={} rc4=({},{}) -> ({},{})",
                actor.0,
                builtin_skill.export_name(),
                level,
                smart,
                roll,
                roll < level,
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if builtin_skill == BuiltinActiveSkill::Charm && crate::debug::probe_charm().is_some() {
            eprintln!(
                "[charm_probe:runtime:prob] actor={} level={} roll={} pass={} rc4=({},{}) -> ({},{})",
                actor.0,
                level,
                roll,
                roll < level,
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        #[cfg(not(feature = "no_debug"))]
        if builtin_skill == BuiltinActiveSkill::Possess && crate::debug::probe_possess().is_some() {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime possess probe actor: {}", actor.0));
            eprintln!(
                "[possess_probe:runtime:prob] round={} actor={} name={} smart={} level={} roll={} pass={} \
                 rc4=({},{}) -> ({},{})",
                self.round + 1,
                actor.0,
                entity.template.name,
                smart,
                level,
                roll,
                roll < level,
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        roll < level
    }
}

/// 卡牌前端使用的公开技能编号（overlay 里的 0..34）→ 引擎内置主动技能。
/// 25..34 是防御/触发类技能，不参与「主动出招」，返回 None。
fn public_builtin_skill(id: u32) -> Option<BuiltinActiveSkill> {
    Some(match id {
        0 => BuiltinActiveSkill::Fire,
        1 => BuiltinActiveSkill::Ice,
        2 => BuiltinActiveSkill::Thunder,
        3 => BuiltinActiveSkill::Quake,
        4 => BuiltinActiveSkill::Absorb,
        5 => BuiltinActiveSkill::Poison,
        6 => BuiltinActiveSkill::Rapid,
        7 => BuiltinActiveSkill::Critical,
        8 => BuiltinActiveSkill::Half,
        9 => BuiltinActiveSkill::Exchange,
        10 => BuiltinActiveSkill::Berserk,
        11 => BuiltinActiveSkill::Charm,
        12 => BuiltinActiveSkill::Haste,
        13 => BuiltinActiveSkill::Slow,
        14 => BuiltinActiveSkill::Curse,
        15 => BuiltinActiveSkill::Heal,
        16 => BuiltinActiveSkill::Revive,
        17 => BuiltinActiveSkill::Disperse,
        18 => BuiltinActiveSkill::Iron,
        19 => BuiltinActiveSkill::Charge,
        20 => BuiltinActiveSkill::Accumulate,
        21 => BuiltinActiveSkill::Assassinate,
        22 => BuiltinActiveSkill::Summon,
        23 => BuiltinActiveSkill::Clone,
        24 => BuiltinActiveSkill::Shadow,
        _ => return None,
    })
}

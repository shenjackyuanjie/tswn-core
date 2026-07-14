use super::*;

impl CombatRuntime {
    pub fn run_minimal_round(&mut self) -> RoundOutcome { self.run_minimal_round_with_capture(true) }

    /// 批量胜率与评分只需要胜者，不保留 replay 帧。
    pub(crate) fn run_minimal_round_no_capture(&mut self) -> RoundOutcome { self.run_minimal_round_with_capture(false) }

    fn run_minimal_round_with_capture(&mut self, capture_updates: bool) -> RoundOutcome {
        loop {
            if let Some(outcome) = self.run_minimal_round_once_with_capture(capture_updates) {
                return outcome;
            }
        }
    }

    pub fn run_minimal_round_once(&mut self) -> Option<RoundOutcome> { self.run_minimal_round_once_with_capture(true) }

    fn run_minimal_round_once_with_capture(&mut self, capture_updates: bool) -> Option<RoundOutcome> {
        let winner_team = if capture_updates {
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
        let mut updates = if capture_updates {
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
            return Some(self.finish_round(None, updates));
        };
        let legacy_plain_action = self.scheduler.uses_legacy_step_scheduler();
        self.scratch.selected_actor_round = self.round;
        #[cfg(not(feature = "no_debug"))]
        let debug_tick = std::env::var_os("TSWN_DEBUG_TICK").is_some();
        #[cfg(not(feature = "no_debug"))]
        let tick_rng_before = (self.rng.i, self.rng.j);
        #[cfg(not(feature = "no_debug"))]
        if debug_tick {
            let actor = self
                .entities
                .get(action.actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 debug tick actor: {}", action.actor.0));
            eprintln!(
                "[v2_tick] actor={} id={} mv={} hp={} rc4=({}, {})",
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
                .unwrap_or_else(|| panic!("unknown runtime_v2 frozen-action actor: {}", action.actor.0))
                .states
                .is_frozen()
            {
                if updates.had_updates() {
                    return Some(self.finish_round(None, updates));
                }
                return None;
            }
            let forced_pre_action_skill = plain_skill_pre_action.forced_skill.is_some();
            let Some(prepared) = self.prepare_plain_action(action.actor, smart, plain_skill_pre_action) else {
                return Some(self.finish_round(None, updates));
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
                .unwrap_or_else(|| panic!("unknown runtime_v2 debug tick actor: {}", action.actor.0));
            let bytes = (self.rng.i as i32 - tick_rng_before.0 as i32).rem_euclid(256);
            eprintln!(
                "[v2_tick_end] actor={} id={} mp_after={} hp_after={} rc4=({},{})->({},{}) bytes={} messages={:?}",
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
                    .filter(|update| !matches!(update.update_type, crate::engine::update::UpdateType::NextLine))
                    .map(|update| update.message.as_ref())
                    .collect::<Vec<_>>(),
            );
        }
        Some(self.finish_round(Some(action), updates))
    }

    fn drain_post_action_chain_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let early_skill_plan =
            self.scheduler
                .skill_post_action_hook_plan(&self.entities, &self.registry, owner, SkillPostActionPhase::Early);
        self.drain_skill_hook_plan_into(&early_skill_plan, updates);

        let state_plan = self.scheduler.state_hook_plan(&self.entities, owner, ProcMask::POST_ACTION);
        let deferred_entries = self.scheduler.deferred_skill_post_action_entries(&self.entities, &self.registry, owner);
        let loadout_len = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime_v2 post-action owner entity: {}", owner.0))
            .template
            .skills
            .len();
        let mut deferred_idx = 0usize;
        let mut deferred_owner_state_clears = Vec::new();
        for state_entry in state_plan.entries.iter().copied() {
            while deferred_entries
                .get(deferred_idx)
                .is_some_and(|(cursor, _)| *cursor <= state_entry.runtime_registration_order)
            {
                let entry = deferred_entries[deferred_idx].1;
                let plan = SkillHookPlan {
                    owner,
                    hook: ProcMask::POST_ACTION,
                    loadout_len,
                    entries: smallvec::SmallVec::from_slice(&[entry]),
                };
                self.drain_skill_hook_plan_into(&plan, updates);
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
            let plan = SkillHookPlan {
                owner,
                hook: ProcMask::POST_ACTION,
                loadout_len,
                entries: smallvec::SmallVec::from_slice(&[entry]),
            };
            self.drain_skill_hook_plan_into(&plan, updates);
            deferred_idx += 1;
        }
        // legacy 会先记录本轮需要清理的状态，等状态与中途注册技能全部执行完后再统一移除。
        self.flush_deferred_owner_state_clears(owner, &deferred_owner_state_clears);

        // Charge 以及 Haste、Slow 的尾阶段必须继续晚于状态和中途注册的 early 技能。
        let late_skill_plan =
            self.scheduler
                .skill_post_action_hook_plan(&self.entities, &self.registry, owner, SkillPostActionPhase::Late);
        self.drain_skill_hook_plan_into(&late_skill_plan, updates);
    }

    pub fn finish_round(&mut self, action: Option<ActionPlan>, updates: RunUpdates) -> RoundOutcome {
        let capture_updates = updates.capture_updates;
        let frame = (capture_updates && updates.had_updates()).then_some(RuntimeFrame { updates });
        self.round += 1;
        let winner_team = if capture_updates {
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
        std::env::var("TSWN_PROBE_ACTION")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false)
    }

    pub fn prepare_plain_action(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        pre_action: PlainSkillPreActionOutcome,
    ) -> Option<PreparedPlainAction> {
        if let Some(forced_skill) = pre_action.forced_skill {
            return Some(PreparedPlainAction::BuiltinSkill(forced_skill));
        }
        if !pre_action.clear_forced_action && self.has_plain_berserk_state(actor) {
            let target = self.select_plain_berserk_forced_attack_target(smart)?;
            let amount = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 forced-attack actor: {}", actor.0))
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
        let (mp_before, is_boss, boss_action_prob_count) = {
            let actor_entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 default attack actor: {}", actor.0));
            let is_boss = actor_entity.runtime.flags.contains(PlayerKindFlags::BOSS);
            (
                actor_entity.runtime.magic_point,
                is_boss,
                if is_boss {
                    crate::player::boss::boss_action_prob_count(&actor_entity.template.name)
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
                .unwrap_or_else(|| panic!("unknown runtime_v2 action probe actor: {}", actor.0));
            eprintln!(
                "[action_probe:v2:mp] round={} actor={} name={} smart={} req_mp_byte={} req_mp={} mp_before={} \
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
        if std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 action probe actor: {}", actor.0));
            eprintln!(
                "[possess_probe:v2:mp] round={} actor={} name={} smart={} req_mp_byte={} req_mp={} mp_before={} \
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
                    "[action_probe:v2:selected] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
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
            let amount = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 boss actor: {}", actor.0))
                .runtime
                .attack;
            return Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: false,
                amount,
            });
        }

        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            eprintln!(
                "[action_probe:v2:fallback] actor={} rc4=({}, {})",
                actor.0, self.rng.i, self.rng.j
            );
        }
        let target = self.select_plain_default_attack_target(actor, smart)?;
        let actor_entity = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default attack actor: {}", actor.0));
        let attack = actor_entity.runtime.attack;
        let magic = actor_entity.runtime.magic;
        let magic_cost = (magic - attack) >> 2;
        let use_magic = smart && magic > attack && actor_entity.runtime.magic_point >= magic_cost;
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
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return None;
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = self.rng.pick(&all_alive)?;
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

        let mut scored = selected
            .into_iter()
            .map(|target| {
                let attract = self
                    .entities
                    .get(target)
                    .unwrap_or_else(|| panic!("runtime_v2 forced-attack target disappeared: {}", target.0))
                    .runtime
                    .attract();
                (target, self.rng.rFFFF() as f64 * attract)
            })
            .collect::<Vec<_>>();
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
            .unwrap_or_else(|| panic!("unknown runtime_v2 forced-attack actor: {}", actor.0))
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
                .unwrap_or_else(|| panic!("unknown runtime_v2 forced-attack actor: {}", actor.0));
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
                .expect("runtime_v2 berserk state disappeared during forced action");
            let StatePayload::Berserk { step } = &mut entry.payload else {
                unreachable!("runtime_v2 berserk state key changed payload during forced action");
            };
            *step -= 1;
            (state_key, *step <= 0, actor_entity.runtime.active())
        };

        if !clear_state {
            return;
        }
        if actor_alive {
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                "[1]从[狂暴]中解除",
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
        }
        let removed = self
            .entities
            .get_mut(actor)
            .expect("runtime_v2 forced-attack actor disappeared before state clear")
            .states
            .clear_legacy_key(state_key);
        debug_assert!(removed, "runtime_v2 berserk state disappeared before state clear");
    }

    pub fn scan_plain_action_skill_probabilities(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBuiltinSkillAction> {
        let active_order_len = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 action-scan actor: {}", actor.0))
            .template
            .skills
            .active_order()
            .len();
        for active_index in 0..active_order_len {
            let (fixed_lane, skill_id, level) = {
                let loadout = &self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime_v2 action-scan actor: {}", actor.0))
                    .template
                    .skills;
                let fixed_lane = *loadout
                    .active_order()
                    .get(active_index)
                    .unwrap_or_else(|| panic!("runtime_v2 active skill order missing index {active_index}"));
                let Some(skill_id) = loadout.skills().get(fixed_lane).copied() else {
                    panic!("runtime_v2 active skill order references missing fixed lane {fixed_lane}");
                };
                let level = loadout
                    .level_at(fixed_lane)
                    .unwrap_or_else(|| panic!("runtime_v2 active skill level missing for fixed lane {fixed_lane}"));
                (fixed_lane, skill_id, level)
            };
            if level == 0 {
                continue;
            }
            let Some(builtin_skill) = self.builtin_active_skill(skill_id) else {
                continue;
            };
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
                    BuiltinActiveSkill::Shadow => vec![actor],
                    BuiltinActiveSkill::Charm => self.select_plain_charm_targets(actor, smart),
                    BuiltinActiveSkill::Curse => self.select_plain_curse_targets(actor, smart),
                    BuiltinActiveSkill::Haste => self.select_plain_haste_targets(actor, smart),
                    BuiltinActiveSkill::Heal => self.select_plain_heal_targets(actor, smart),
                    BuiltinActiveSkill::Slow => self.select_plain_slow_targets(actor, smart),
                    BuiltinActiveSkill::Exchange => self.select_plain_exchange_targets(actor, smart),
                    BuiltinActiveSkill::Revive => self.select_plain_revive_targets(actor, smart),
                    BuiltinActiveSkill::Disperse => self.select_plain_disperse_targets(actor, smart),
                    BuiltinActiveSkill::Iron => vec![actor],
                    BuiltinActiveSkill::Clone => vec![actor],
                    BuiltinActiveSkill::Charge => vec![actor],
                    BuiltinActiveSkill::Accumulate => vec![actor],
                    BuiltinActiveSkill::Assassinate => self.select_plain_assassinate_targets(actor, smart),
                    BuiltinActiveSkill::Summon => vec![actor],
                    BuiltinActiveSkill::Possess => self.select_plain_possess_targets(actor, smart),
                };
                #[cfg(not(feature = "no_debug"))]
                if self.probe_plain_action_matches(actor) {
                    eprintln!(
                        "[action_probe:v2:targets] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
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
        if builtin_skill == BuiltinActiveSkill::Charge {
            let actor_runtime = &self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 charge probability actor: {}", actor.0))
                .runtime;
            if actor_runtime.charge.active || (smart && actor_runtime.hp < 100) {
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=charge_gate active={} hp={} \
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
        if builtin_skill == BuiltinActiveSkill::Absorb && smart {
            let actor_entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 absorb probability actor: {}", actor.0));
            if actor_entity.template.max_hp - actor_entity.runtime.hp < 32 {
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=absorb_low_missing_hp \
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
        if builtin_skill == BuiltinActiveSkill::Iron
            && self
                .entities
                .get(actor)
                .and_then(|entity| entity.states.entry(PLAIN_IRON_STATE_KEY))
                .and_then(StateEntry::iron_value)
                .is_some_and(|(protect, step)| protect > 0 && step > 0)
        {
            return false;
        }
        if builtin_skill == BuiltinActiveSkill::Accumulate {
            let actor_runtime = &self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 accumulate probability actor: {}", actor.0))
                .runtime;
            if actor_runtime.accumulate.active || (smart && actor_runtime.hp < 120) {
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=accumulate_gate \
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
        if builtin_skill == BuiltinActiveSkill::Assassinate
            && smart
            && self.entities.get(actor).is_some_and(|entity| {
                entity
                    .states
                    .entries()
                    .iter()
                    .any(|entry| matches!(entry.payload, StatePayload::Poison { .. }))
            })
        {
            return false;
        }
        if builtin_skill == BuiltinActiveSkill::Summon && !self.plain_summon_probability_allowed(actor, smart) {
            return false;
        }
        // ShadowSkill 在 smart 模式且 HP < 80 时短路，不消耗概率字节。
        if builtin_skill == BuiltinActiveSkill::Shadow
            && smart
            && self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 shadow probability actor: {}", actor.0))
                .runtime
                .hp
                < 80
        {
            #[cfg(not(feature = "no_debug"))]
            if probe_action {
                eprintln!(
                    "[action_probe:v2:prob] actor={} skill={} level={} smart={} skipped=shadow_low_hp rc4=({}, {})",
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
        #[cfg(not(feature = "no_debug"))]
        let before = (self.rng.i, self.rng.j);
        let roll = self.rng.r127();
        #[cfg(not(feature = "no_debug"))]
        if probe_action {
            eprintln!(
                "[action_probe:v2:prob] actor={} skill={} level={} smart={} roll={} pass={} rc4=({},{}) -> ({},{})",
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
        if builtin_skill == BuiltinActiveSkill::Charm && std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:prob] actor={} level={} roll={} pass={} rc4=({},{}) -> ({},{})",
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
        if builtin_skill == BuiltinActiveSkill::Possess && std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            let entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 possess probe actor: {}", actor.0));
            eprintln!(
                "[possess_probe:v2:prob] round={} actor={} name={} smart={} level={} roll={} pass={} \
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

use super::*;
use crate::runtime::control::TargetWisdom;
use crate::runtime::forced_action::ForcedFallback;
use smallvec::SmallVec;

/// 原版 MList 的优先级；任何有限 sortId 都早于 PostActionImpl。
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum PostActionPriority {
    Finite(SkillPriority),
    Infinity,
}

/// 概率抽样规则在执行前分类，普通技能可以直接消耗一个概率字节。
#[derive(Clone, Copy)]
enum ActionRollRule {
    Standard,
    Charge,
    Absorb,
    Iron,
    Accumulate,
    Assassinate,
    Summon,
    Shadow,
    WeaponS11,
    WeaponDeathNote,
    WeaponXuanYue,
}

impl ActionRollRule {
    #[inline]
    fn for_skill(skill: BuiltinActiveSkill, smart: bool) -> Self {
        match skill {
            BuiltinActiveSkill::Charge => Self::Charge,
            BuiltinActiveSkill::Absorb if smart => Self::Absorb,
            BuiltinActiveSkill::Iron => Self::Iron,
            BuiltinActiveSkill::Accumulate => Self::Accumulate,
            BuiltinActiveSkill::Assassinate if smart => Self::Assassinate,
            BuiltinActiveSkill::Summon => Self::Summon,
            BuiltinActiveSkill::Shadow if smart => Self::Shadow,
            BuiltinActiveSkill::WeaponS11 => Self::WeaponS11,
            BuiltinActiveSkill::WeaponDeathNote => Self::WeaponDeathNote,
            BuiltinActiveSkill::WeaponXuanYue => Self::WeaponXuanYue,
            _ => Self::Standard,
        }
    }
}

/// 卡牌兼容模式的降级战报；前端按这句话判断「点名的技能没生效」。
pub const FORCED_FALLBACK_MESSAGE: &str = "[1]的指定技能无法生效，改为普通攻击";

/// 组装 [`RoundOutcome`]：行动计划、可见帧与胜者队伍。
fn round_outcome(action: Option<ActionPlan>, frame: Option<RuntimeFrame>, winner_team: Option<usize>) -> RoundOutcome {
    RoundOutcome {
        action,
        frame,
        winner_team,
    }
}

#[derive(Clone, Copy)]
struct PlannedTargeting {
    smart: bool,
    followup_smart: bool,
    retry_without_wisdom: bool,
}

impl PlannedTargeting {
    /// 按计划技能、指定队伍、智慧模式与 smart 推导瞄准策略：是否自动施放、是否全程跟随 smart、智慧关闭时是否无智慧重试。
    fn for_action(skill: PlannedSkill, team: Option<usize>, wisdom: TargetWisdom, smart: bool) -> Self {
        let ally_skill = matches!(
            skill,
            PlannedSkill::Builtin(BuiltinActiveSkill::Heal | BuiltinActiveSkill::Haste | BuiltinActiveSkill::Revive)
        );
        let automatic = wisdom != TargetWisdom::Off && (team.is_some() || ally_skill);
        Self {
            smart: automatic && smart,
            followup_smart: smart && wisdom == TargetWisdom::WholeAction,
            retry_without_wisdom: !automatic,
        }
    }
}

impl CombatRuntime {
    /// 推进一个最小回合并返回 outcome；没有可见内容时继续推进直到产生。
    pub fn run_minimal_round(&mut self) -> RoundOutcome { self.run_minimal_round_with_capture::<true, _>(round_outcome) }

    #[cfg(test)]
    pub fn run_minimal_round_no_capture(&mut self) -> RoundOutcome {
        if let Some(winner) = self.world.sync_winner_from_alive_views() {
            return round_outcome(None, None, Some(winner));
        }
        self.run_minimal_round_with_capture::<false, _>(round_outcome)
    }

    /// 调用方已确认当前没有胜者；批量循环只接收本回合的胜者编号。
    pub fn run_active_round_no_capture(&mut self) -> Option<usize> {
        self.run_minimal_round_with_capture::<false, _>(|_, _, winner| winner)
    }

    /// 循环推进最小回合直到产出 outcome；未捕获模式下每轮同步胜者并提前返回。
    fn run_minimal_round_with_capture<const CAPTURE_UPDATES: bool, O>(
        &mut self,
        build: impl Fn(Option<ActionPlan>, Option<RuntimeFrame>, Option<usize>) -> O,
    ) -> O {
        loop {
            if let Some(outcome) = self.run_minimal_round_once_with_capture::<CAPTURE_UPDATES, _>(&build) {
                return outcome;
            }
            if !CAPTURE_UPDATES && let Some(winner) = self.world.winner_team() {
                return build(None, None, Some(winner));
            }
        }
    }

    /// 尝试推进一个最小回合；未选出行动时返回 None。
    pub fn run_minimal_round_once(&mut self) -> Option<RoundOutcome> {
        self.run_minimal_round_once_with_capture::<true, _>(round_outcome)
    }

    /// 尝试推进一个最小回合：同步胜者、选出行动并补发冰冻解除事件；未选出行动时直接收尾当前批次。
    fn run_minimal_round_once_with_capture<const CAPTURE_UPDATES: bool, O>(
        &mut self,
        build: impl FnOnce(Option<ActionPlan>, Option<RuntimeFrame>, Option<usize>) -> O,
    ) -> Option<O> {
        if CAPTURE_UPDATES && let Some(winner) = self.world.sync_winner(&self.entities) {
            return Some(build(None, None, Some(winner)));
        }

        let selected_action = self.scheduler.select_action(&mut self.world, &mut self.entities, &mut self.rng);
        let ice_release_events = self.scheduler.take_ice_release_events();
        if !CAPTURE_UPDATES && selected_action.is_none() && ice_release_events.is_empty() {
            RunUpdates::skip_empty_batch();
            self.round += 1;
            return Some(build(None, None, None));
        }
        let mut updates = if CAPTURE_UPDATES {
            RunUpdates::new()
        } else {
            RunUpdates::new_no_capture()
        };
        for target in ice_release_events {
            updates.add_newline();
            updates.add(RuntimeFrame::replay_update(
                target.0 as usize,
                target.0 as usize,
                "[1]从[冰冻]中解除",
                0,
            ));
            self.quilt_thaw(target, &mut updates);
        }
        let Some(mut action) = selected_action else {
            return Some(self.finish_round_with_capture::<CAPTURE_UPDATES, _>(None, updates, build));
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
        // 药水持续到服用者的下一次行动，不因其他单位行动而失效。
        self.revert_active_potion(action.actor);
        let smart = self.roll_actor_smart(action.actor);
        if legacy_plain_action {
            // 属性修改器身份的 `RinickModifierPreAction` 在行动前静默清除负向状态，
            // 被清掉的狂暴等状态无法再强制本次行动。
            self.run_plain_rinick_modifier_pre_action(action.actor, &mut updates);
        }
        // 潜行 pending 的 pre-action 槽位可能在技能表重建（升级/合并/状态刷新）时被丢掉，
        // tswn 自己在施放时会 ensure 一次，这里在每次行动前补回来，
        // 之后完全交给引擎的 pre_action 规则判断要不要背刺、会不会被识破。
        if self.is_controlled_actor(action.actor) {
            if let Some(pending) = self.entities.get(action.actor).and_then(|entity| entity.runtime.assassinate) {
                if let Some(entity) = self.entities.get_mut(action.actor) {
                    entity.template.skills.ensure_pre_action_lane(pending.fixed_lane);
                }
            }
        }
        let plain_skill_pre_action = legacy_plain_action
            .then(|| self.run_plain_skill_pre_action_accumulator(action.actor))
            .unwrap_or_default();

        let skills = &self.entities.get(action.actor).expect("runtime pre-action owner missing").template.skills;
        if !legacy_plain_action || skills.may_have_enabled_hook(ProcMask::PRE_ACTION) {
            let skill_plan =
                self.scheduler
                    .skill_hook_plan_for_loadout(skills, &self.registry, action.actor, ProcMask::PRE_ACTION);
            let selected_target = if legacy_plain_action {
                action.target
            } else {
                self.selected_pre_action_target(&skill_plan, action.actor, smart).unwrap_or(action.target)
            };
            self.drain_skill_hook_plan_with_selected_target_into(&skill_plan, &mut updates, Some(selected_target));
        }
        // 技能可能在行动前注册状态，执行完技能后再判断是否需要状态计划。
        let actor = self.entities.get(action.actor).expect("runtime pre-action state owner missing");
        let state_intercepted_action = actor.states.hook_mask().intersects(ProcMask::PRE_ACTION) && {
            let state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_ACTION);
            let forced_order = plain_skill_pre_action
                .forced_skill
                .as_ref()
                .map(|_| actor.states.assassinate_pre_action_cursor());
            self.drain_state_hook_plan_with_pre_action_order_into(&state_plan, &mut updates, Some(smart), forced_order)
        };
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
                    return Some(self.finish_round_with_capture::<CAPTURE_UPDATES, _>(None, updates, build));
                }
                return None;
            }
            // 专属计时只在真正获得行动时推进，冰冻期间保留剪枝预告。
            self.begin_squall_action(action.actor, &mut updates);
            self.begin_rosmontic_action(action.actor, &mut updates);
            let forced_pre_action_skill = plain_skill_pre_action.forced_skill.is_some();
            let prepared = self.prepare_plain_action(action.actor, smart, plain_skill_pre_action);
            // 卡牌兼容模式：点名的技能放不出来时引擎改用普攻，这里补一条说明战报。
            if let Some((actor, reason)) = crate::runtime::forced_action::take_fallback() {
                updates.add_newline();
                updates.add(RuntimeFrame::replay_update(actor as usize, actor as usize, reason, 0));
            }
            let Some(prepared) = prepared else {
                return Some(self.finish_round_with_capture::<CAPTURE_UPDATES, _>(None, updates, build));
            };
            if CAPTURE_UPDATES || cfg!(not(feature = "no_debug")) {
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
                    PreparedPlainAction::Conan { targets } => {
                        action.target = targets.first().copied().unwrap_or(action.actor);
                        action.amount = 0;
                    }
                    PreparedPlainAction::ThanosSnap { targets } => {
                        action.target = targets.first().copied().unwrap_or(action.actor);
                        action.amount = 0;
                    }
                    PreparedPlainAction::WheelExchange { target } => {
                        action.target = *target;
                        action.amount = self.plain_wheel_action_amount(action.actor, *target);
                    }
                    PreparedPlainAction::BuiltinSkill(prepared) => {
                        if forced_pre_action_skill {
                            action.target = action.actor;
                        } else {
                            action.target = prepared.targets.first().copied().unwrap_or(action.actor);
                        }
                        action.amount = 0;
                    }
                    PreparedPlainAction::BossSkill(prepared) => {
                        action.target = prepared.targets.first().copied().unwrap_or(action.actor);
                        action.amount = 0;
                    }
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
        let builtin_plain_action = matches!(
            prepared_plain_action.as_ref(),
            Some(PreparedPlainAction::BuiltinSkill(_) | PreparedPlainAction::BossSkill(_))
        );
        let terminal_plain_action = if state_intercepted_action {
            // PRE_ACTION 状态钩子接管本次行动后，legacy 仍会继续执行恢复和完整的行动后链。
            legacy_plain_action && self.plain_battle_has_ended()
        } else if builtin_plain_action {
            match prepared_plain_action.take() {
                Some(PreparedPlainAction::BuiltinSkill(prepared)) => {
                    self.drain_plain_builtin_skill_into(action.actor, prepared, &mut updates);
                }
                Some(PreparedPlainAction::BossSkill(prepared)) => {
                    self.drain_plain_boss_skill_into(action.actor, prepared, &mut updates);
                }
                _ => unreachable!("builtin action marker must retain the prepared action"),
            }
            legacy_plain_action && self.plain_battle_has_ended()
        } else {
            self.drain_action_damage_phase_into(action.actor, ProcMask::PRE_DAMAGE, &mut updates);
            match prepared_plain_action.take() {
                Some(PreparedPlainAction::BasicAttack { target, use_magic, .. }) => {
                    if self
                        .entities
                        .get(action.actor)
                        .is_some_and(|e| e.template.name == crate::runtime::combat::boss::yi::BAIXIE)
                    {
                        self.drain_baixie_action_into(action.actor, &mut updates);
                    } else if self.quilt_minion_kind(action.actor).is_some() {
                        self.drain_quilt_minion_action_into(action.actor, target, &mut updates);
                    } else if self
                        .entities
                        .get(action.actor)
                        .is_some_and(|e| e.template.name == super::boss::testsubject::ID)
                    {
                        // 实验体一阶段普攻按已行动次数爬升倍率。
                        self.drain_testsubject_default_attack_into(action.actor, target, use_magic, &mut updates);
                    } else {
                        self.drain_plain_default_attack_into(action.actor, target, use_magic, &mut updates);
                    }
                }
                Some(PreparedPlainAction::ForcedAttack { target, .. }) => {
                    self.drain_plain_berserk_forced_attack_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::Saitama { target }) => {
                    self.drain_plain_saitama_action_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::Conan { targets }) => {
                    self.drain_plain_conan_action_into(action.actor, targets, &mut updates);
                }
                Some(PreparedPlainAction::ThanosSnap { targets }) => {
                    self.drain_plain_thanos_snap_into(action.actor, targets, &mut updates);
                }
                Some(PreparedPlainAction::WheelExchange { target }) => {
                    // 追踪保留选敌时的决策；公开回合的基础威力对应 PRE_DAMAGE 之后实际执行的招式。
                    if CAPTURE_UPDATES || cfg!(not(feature = "no_debug")) {
                        action.amount = self.plain_wheel_action_amount(action.actor, target);
                    }
                    self.drain_plain_wheel_exchange_into(action.actor, target, &mut updates);
                }
                Some(PreparedPlainAction::BuiltinSkill(_)) => {
                    unreachable!("builtin skill actions are handled before the default action branch")
                }
                Some(PreparedPlainAction::BossSkill(_)) => {
                    unreachable!("boss skill actions are handled before the default action branch")
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
            let terminal_plain_action = legacy_plain_action && self.plain_battle_has_ended();
            if !terminal_plain_action {
                self.drain_action_damage_phase_into(action.actor, ProcMask::POST_DAMAGE, &mut updates);
            }
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
            self.end_rosmontic_action(action.actor, &mut updates);
            self.quilt_post_action(action.actor, &mut updates);
            if !legacy_plain_action || !self.plain_battle_has_ended() {
                self.drain_plain_update_end_into(&mut updates);
            }
        }
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
        Some(self.finish_round_with_capture::<CAPTURE_UPDATES, _>(Some(action), updates, build))
    }

    /// 空钩子不需要构造拥有型快照；缓存失效时仍走完整的注册表扫描。
    /// 技能执行完再读取状态位图，保留技能新增状态在同一阶段立即生效的顺序。
    fn drain_action_damage_phase_into(&mut self, owner: EntityIdx, hook: ProcMask, updates: &mut RunUpdates) {
        let skills = &self.entities.get(owner).expect("runtime damage phase owner missing").template.skills;
        if skills.may_have_enabled_hook(hook) {
            let plan = self.scheduler.skill_hook_plan_for_loadout(skills, &self.registry, owner, hook);
            self.drain_skill_hook_plan_into(&plan, updates);
        }
        if self
            .entities
            .get(owner)
            .expect("runtime damage phase owner missing")
            .states
            .hook_mask()
            .intersects(hook)
        {
            let plan = self.scheduler.state_hook_plan(&self.entities, owner, hook);
            self.drain_state_hook_plan_into(&plan, updates);
        }
    }

    /// 初始早段技能之后，按原版 x2 的优先级和登记序混排中途启用的技能、蓄力和状态。
    /// 反弹登记在前受击链 y1（md5.js:19302），不参与 x2 排序。
    fn drain_post_action_chain_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let actor = self.entities.get(owner).expect("runtime post-action owner missing");
        let skills = &actor.template.skills;
        if !skills.may_have_enabled_hook(ProcMask::POST_ACTION) && !actor.states.hook_mask().intersects(ProcMask::POST_ACTION) {
            return;
        }
        let mut skill_plans = self.scheduler.skill_post_action_plans(skills, &self.registry, owner);
        self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, &skill_plans.early, updates);
        let skills_after_early = &self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime post-action owner entity: {}", owner.0))
            .template
            .skills;
        if skills_after_early.hook_generation() != skill_plans.generation {
            skill_plans = self.scheduler.skill_post_action_plans(skills_after_early, &self.registry, owner);
        }

        enum PostActionHook {
            Skill(SkillHookPlanEntry),
            State(StateHookPlanEntry),
        }

        let state_plan = self.scheduler.state_hook_plan(&self.entities, owner, ProcMask::POST_ACTION);
        let owner_entity = self.entities.get(owner).expect("runtime post-action owner missing");
        let charge = if owner_entity.runtime.charge.post_action_active {
            self.registry.skill_id_by_export_name(BuiltinActiveSkill::Charge.export_name())
        } else {
            None
        };
        let mut chain = SmallVec::<[((PostActionPriority, u64), PostActionHook); 4]>::new();
        for &(cursor, entry) in &skill_plans.deferred {
            chain.push((
                (PostActionPriority::Finite(entry.priority), cursor),
                PostActionHook::Skill(entry),
            ));
        }
        let mut charge_in_chain = None;
        for &entry in skill_plans.late.iter().filter(|entry| Some(entry.skill_id) == charge) {
            charge_in_chain = Some(entry.skill_id);
            chain.push((
                (PostActionPriority::Infinity, owner_entity.states.charge_post_action_cursor()),
                PostActionHook::Skill(entry),
            ));
        }
        for entry in state_plan.entries {
            let priority =
                owner_entity
                    .states
                    .entry(entry.legacy_order_key)
                    .map_or(PostActionPriority::Finite(entry.priority), |state| {
                        match &state.payload {
                            // 中毒本体和守护都直接挂在 x2 上，使用 MEntry 默认的 10000。
                            StatePayload::Poison { .. } => PostActionPriority::Finite(SkillPriority(10_000)),
                            // PostActionImpl 包装的钩子使用 Infinity，而非有限数值。
                            StatePayload::Charm { .. }
                            | StatePayload::Haste { .. }
                            | StatePayload::Slow { .. }
                            | StatePayload::Iron { .. } => PostActionPriority::Infinity,
                            StatePayload::CovidInfection { .. } | StatePayload::LazyInfection { .. }
                                if state.is_boss_infection(&self.entities) =>
                            {
                                PostActionPriority::Infinity
                            }
                            // 无原版 x2 对应项的扩展状态沿用自身有限优先级。
                            // 例如实验体易伤（1000）先于中毒（10000）是 Rust 扩展规则，并非 JS 行为。
                            _ => PostActionPriority::Finite(entry.priority),
                        }
                    });
            chain.push(((priority, entry.runtime_registration_order), PostActionHook::State(entry)));
        }
        // 原版 MList.j 先按 sortId 插入，再按同一 sortId 的登记顺序执行。
        chain.sort_by_key(|(key, _)| *key);
        let mut deferred_owner_state_clears = Vec::new();
        let mut skip_states = false;
        for (_, hook) in chain {
            match hook {
                PostActionHook::Skill(entry) => {
                    self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, std::slice::from_ref(&entry), updates)
                }
                PostActionHook::State(entry) => {
                    let owner_alive = self.entities.get(owner).is_some_and(|entity| entity.runtime.alive);
                    skip_states |= !owner_alive && self.world.alive_group_count() <= 1;
                    if !skip_states {
                        self.drain_state_hook_entry_with_deferred_clears_into(
                            ProcMask::POST_ACTION,
                            entry,
                            updates,
                            Some(&mut deferred_owner_state_clears),
                        );
                    }
                }
            }
        }
        self.flush_deferred_owner_state_clears(owner, &deferred_owner_state_clears);

        let skills_after_states = &self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime post-action owner entity: {}", owner.0))
            .template
            .skills;
        if skills_after_states.hook_generation() != skill_plans.generation {
            skill_plans = self.scheduler.skill_post_action_plans(skills_after_states, &self.registry, owner);
        }
        if let Some(charge) = charge_in_chain {
            skill_plans.late.retain(|entry| entry.skill_id != charge);
        }
        self.drain_skill_hook_entries_into(owner, ProcMask::POST_ACTION, &skill_plans.late, updates);
    }

    /// 用既定行动与更新收尾一个回合，同步胜者并产出 outcome。
    pub fn finish_round(&mut self, action: Option<ActionPlan>, updates: RunUpdates) -> RoundOutcome {
        if updates.capture_updates {
            self.finish_round_with_capture::<true, _>(action, updates, round_outcome)
        } else {
            self.finish_round_with_capture::<false, _>(action, updates, round_outcome)
        }
    }

    /// 批量路径不会生成 frame，也不需要重新扫描实体表判定赢家。
    fn finish_round_with_capture<const CAPTURE_UPDATES: bool, O>(
        &mut self,
        action: Option<ActionPlan>,
        mut updates: RunUpdates,
        build: impl FnOnce(Option<ActionPlan>, Option<RuntimeFrame>, Option<usize>) -> O,
    ) -> O {
        // 普通回放与不捕获消息的评分路径共享同一份科研状态结算。
        self.finish_squall_action(action.as_ref().map(|a| a.actor), &mut updates);
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
        build(action, frame, winner_team)
    }

    /// 掷一次聪明判定：智慧高于随机掷值视为聪明。
    pub fn roll_actor_smart(&mut self, actor: EntityIdx) -> bool {
        let smart_byte = self.rng.next_u8();
        let smart_roll = (smart_byte & 63) as i32;
        self.entities.get(actor).is_some_and(|entity| entity.runtime.wisdom > smart_roll)
    }

    #[cfg(not(feature = "no_debug"))]
    /// 调试探针：当前行动者是否匹配探针指定的名称或编号。
    pub fn probe_plain_action_matches(&self, actor: EntityIdx) -> bool {
        crate::debug::probe_action()
            .map(|needle| {
                self.entities
                    .get(actor)
                    .is_some_and(|entity| entity.template.name.contains(needle) || entity.template.display_name.contains(needle))
            })
            .unwrap_or(false)
    }

    /// 为行动者准备本回合行动：优先消费外部脚本指令，否则按规则选择技能。
    pub fn prepare_plain_action(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        pre_action: PlainSkillPreActionOutcome,
    ) -> Option<PreparedPlainAction> {
        crate::runtime::forced_action::hit();
        // 衍生物即使暂无可攻击目标，也可以睡觉、钻被或使用自身技能。
        if self.quilt_minion_kind(actor).is_some() {
            let target = self.select_plain_default_attack_target(actor, smart).unwrap_or(actor);
            return Some(PreparedPlainAction::BasicAttack {
                target,
                use_magic: false,
                amount: 0,
            });
        }
        if self.has_rosmontic_action(actor) {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Rosmontic,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        if self.entities.get(actor).is_some_and(|e| e.runtime.cirno.is_some()) {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Cirno,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // 沙冰鱼的行动完全由专属脚本决定。
        if self
            .entities
            .get(actor)
            .is_some_and(|e| e.template.name == crate::runtime::combat::boss::fish::ID)
        {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Fish,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // TigerStar 的行动完全由专属脚本决定。
        if self
            .entities
            .get(actor)
            .is_some_and(|e| e.template.name == crate::runtime::combat::boss::tiger::ID)
        {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Tiger,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // 咦的行动完全由专属脚本决定。
        if self
            .entities
            .get(actor)
            .is_some_and(|e| e.template.name == crate::runtime::combat::boss::yi::ID)
        {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Yi,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // 新纪元的行动完全由专属脚本决定。
        if self.entities.get(actor).is_some_and(|e| e.template.name == "新纪元") {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Xjy,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // 被子的行动完全由专属脚本决定：不扫描普攻，也不消耗概率随机数。
        if self.is_quilt(actor) {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Quilt,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // 外部脚本：引擎自身没有强制动作时，优先执行调用方排好的这一手。
        // 潜行→背刺这类引擎强制动作必须先走完，指令留到该单位的下一次行动。
        let engine_forced = pre_action.forced_skill.is_some();
        let berserk_forced = !pre_action.clear_forced_action && self.has_plain_berserk_state(actor);
        let mut script_consumed = false;
        if !engine_forced && let Some((planned, use_turn_wisdom)) = self.control.take_for_with_wisdom(actor.0 as PlrId) {
            // 药水不占行动：先结算这一手附带的药水，再决定用哪个技能。
            if planned.potion > 0 {
                self.apply_potion(actor, planned.potion);
            }
            if berserk_forced {
                // 狂暴期间由引擎接管：这一手只用来推进节奏，真正动作仍按 tswn 规则。
                script_consumed = true;
            } else {
                let targeting = PlannedTargeting::for_action(planned.skill, planned.target_team, use_turn_wisdom, smart);
                match self.prepare_planned_action(actor, planned, targeting) {
                    Ok(prepared) => return Some(prepared),
                    // 严格模式把失败原样交给调用方；兼容模式按失败类型降级。
                    Err(error) => return self.resolve_planned_action_failure(actor, error, smart),
                }
            }
        }
        // 受控单位没有排好的指令就停下来等玩家，绝不代替玩家行动。
        if !engine_forced && !script_consumed && self.is_controlled_actor(actor) {
            let forced_action = berserk_forced.then_some(ForcedAction::Berserk);
            self.control.set_awaiting_forced_action(actor.0 as PlrId, forced_action, smart);
            return None;
        }
        if let Some(forced_skill) = pre_action.forced_skill {
            return Some(PreparedPlainAction::BuiltinSkill(forced_skill));
        }
        if berserk_forced {
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

        if self
            .entities
            .get(actor)
            .is_some_and(|e| e.runtime.squall.is_some() || e.runtime.squall_helper.is_some())
        {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Squall,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }
        // smdd 同样走专属脚本，不消耗概率随机数。
        if self
            .entities
            .get(actor)
            .is_some_and(|e| e.template.name == crate::runtime::combat::boss::smdd::ID)
        {
            return Some(PreparedPlainAction::BossSkill(PreparedBossSkillAction {
                selected: crate::namerena::BossActionSkill::Smdd,
                targets: PreparedTargetList::new(),
                smart,
            }));
        }

        #[cfg(not(feature = "no_debug"))]
        let rng_before = (self.rng.i, self.rng.j);
        let req_mp_byte = self.rng.next_u8();
        let req_mp = (req_mp_byte & 15) as i32 + 8;
        let (mp_before, attack, magic, is_boss) = {
            let actor_entity = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime default attack actor: {}", actor.0));
            (
                actor_entity.runtime.magic_point,
                actor_entity.runtime.attack,
                actor_entity.runtime.magic,
                actor_entity.runtime.flags.contains(PlayerKindFlags::BOSS),
            )
        };
        #[cfg(not(feature = "cards"))]
        let can_scan_skills = mp_before >= req_mp;
        // 能力卡减耗可能让原本放不出的技能进入扫描。
        #[cfg(feature = "cards")]
        let can_scan_skills = {
            let ability_team = self
                .entities
                .get(actor)
                .and_then(|e| self.entities.get(e.runtime.root_owner))
                .map_or(usize::MAX, |e| e.template.team);
            mp_before >= (req_mp - crate::runtime::forced_action::max_ability_cost(ability_team) as i32).max(1)
        };
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
            let selected = if is_boss && !self.entities.get(actor).unwrap().template.skills.uses_effective_skill_weights() {
                self.scan_plain_boss_action_skills(actor, smart).map(PreparedPlainAction::BossSkill)
            } else {
                self.scan_plain_action_skill_probabilities(actor, smart)
                    .map(PreparedPlainAction::BuiltinSkill)
            };
            // 能力卡按实际选中的技能减耗，最低仍消耗 1 点。
            #[cfg(feature = "cards")]
            let cost = (req_mp
                - match selected.as_ref() {
                    Some(PreparedPlainAction::BuiltinSkill(prepared)) => {
                        self.ability_value(actor, prepared.selected.skill.export_name()).0 as i32
                    }
                    _ => 0,
                })
            .max(1);
            #[cfg(not(feature = "cards"))]
            let cost = req_mp;
            if mp_before < cost {
                None
            } else {
                self.entities.get_mut(actor).unwrap().runtime.magic_point -= cost;
                selected
            }
        } else {
            None
        };
        if let Some(prepared_action) = prepared_skill {
            #[cfg(not(feature = "no_debug"))]
            if probe_action {
                match &prepared_action {
                    PreparedPlainAction::BuiltinSkill(prepared) => eprintln!(
                        "[action_probe:runtime:selected] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
                        actor.0,
                        prepared.selected.skill.export_name(),
                        prepared.selected.fixed_lane,
                        prepared.targets,
                        self.rng.i,
                        self.rng.j,
                    ),
                    PreparedPlainAction::BossSkill(prepared) => eprintln!(
                        "[action_probe:runtime:selected_boss] actor={} skill={:?} targets={:?} rc4=({}, {})",
                        actor.0, prepared.selected, prepared.targets, self.rng.i, self.rng.j,
                    ),
                    _ => {}
                }
            }
            return Some(prepared_action);
        }

        if is_boss {
            if self.saitama_boss_state(actor).is_some() {
                let target = self.select_plain_default_attack_target(actor, smart);
                return Some(PreparedPlainAction::Saitama { target });
            }
            let boss_name = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime boss fallback actor: {}", actor.0))
                .template
                .name
                .clone();
            if boss_name == "conan" {
                let targets = self.select_plain_conan_action_targets(actor, smart);
                if targets.is_empty() {
                    return None;
                }
                return Some(PreparedPlainAction::Conan { targets });
            }
            if boss_name == super::boss::thanos::ID {
                if let Some(targets) = self.select_plain_thanos_snap_targets(actor) {
                    return Some(PreparedPlainAction::ThanosSnap { targets });
                }
            }
            if boss_name == super::boss::wheel::ID {
                // 两种选敌方式都交给生命之轮的默认行动，执行时按当时体力决定是互换还是普攻。
                let target = self
                    .select_plain_wheel_exchange_target(actor)
                    .or_else(|| self.select_plain_default_attack_target(actor, smart))?;
                return Some(PreparedPlainAction::WheelExchange { target });
            }
            let target = self.select_plain_default_attack_target(actor, smart)?;
            // SklAttack 在 smart、魔法高于攻击且扫描技能后蓝量足够时改走魔法攻击。
            if crate::namerena::boss_uses_skill_attack(&boss_name) && smart && magic > attack {
                let magic_cost = (magic - attack) >> 2;
                let mp_after_skill_scan = if can_scan_skills { mp_before - req_mp } else { mp_before };
                if mp_after_skill_scan >= magic_cost {
                    self.entities.get_mut(actor).unwrap().runtime.magic_point -= magic_cost;
                    return Some(PreparedPlainAction::BasicAttack {
                        target,
                        use_magic: true,
                        amount: magic,
                    });
                }
            }
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

    /// 指令执行失败的两种去向：默认报错，或卡牌前端的兼容降级。
    ///
    /// 严格模式把失败记进 control，由会话层映射成 INVALID_ARGUMENT。兼容模式下技能未装备、
    /// MP 不足都改用普攻并留战报；选不出目标时暂停，整回合回滚保证行动条与随机流不被消耗。
    fn resolve_planned_action_failure(
        &mut self,
        actor: EntityIdx,
        error: PlannedActionError,
        smart: bool,
    ) -> Option<PreparedPlainAction> {
        if crate::runtime::forced_action::fallback_mode() == ForcedFallback::Strict {
            self.control.set_error(error);
            return None;
        }
        if let PlannedActionError::NoValidTarget { .. } = error {
            self.control.set_awaiting_forced_action(actor.0 as PlrId, None, smart);
            return None;
        }
        self.prepare_fallback_plain_attack(actor, smart)
    }

    /// 兼容降级：玩家点名的技能放不出来时改用普通攻击，并留一条战报说明原因。
    ///
    /// 普通攻击也选不出目标时同样暂停，行动不丢；目标口径与卡牌前端一致，
    /// 先按 smart 口径选，选不出再退到宽松口径。
    fn prepare_fallback_plain_attack(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedPlainAction> {
        let target = self
            .select_plain_default_attack_target(actor, smart)
            .or_else(|| self.select_plain_default_attack_target(actor, false));
        let Some(target) = target else {
            self.control.set_awaiting_forced_action(actor.0 as PlrId, None, smart);
            return None;
        };
        crate::runtime::forced_action::record_fallback(actor.0, FORCED_FALLBACK_MESSAGE);
        let amount = self.entities.get(actor).map_or(0, |entity| entity.runtime.attack);
        Some(PreparedPlainAction::BasicAttack {
            target,
            use_magic: false,
            amount,
        })
    }

    /// 玩家操控的单位：受控队伍里的本体（默认 kind）与分身。
    ///
    /// 幻影 / 使魔 / 丧尸属于召唤物，仍由引擎自动行动。
    fn is_controlled_actor(&self, actor: EntityIdx) -> bool {
        if self.control.controlled_teams().is_empty() {
            return false;
        }
        let Some(entity) = self.entities.get(actor) else {
            return false;
        };
        if !self.control.is_controlled_team(entity.template.team) {
            return false;
        }
        match self.registry.player_kind(entity.runtime.kind) {
            None => true,
            Some(spec) => {
                if entity.runtime.is_combat_minion() && !self.control.controlled_minions() {
                    false
                } else {
                    spec.export_name == DEFAULT_CORE_CLONE_KIND_EXPORT || entity.runtime.is_combat_minion()
                }
            }
        }
    }

    /// 执行一条脚本指令：技能必须已装备、MP 足够且选得出目标，否则整条指令失败。
    fn prepare_planned_action(
        &mut self,
        actor: EntityIdx,
        planned: PlannedAction,
        targeting: PlannedTargeting,
    ) -> Result<PreparedPlainAction, PlannedActionError> {
        let actor_id = actor.0 as PlrId;
        if planned.target_team.is_some() && !Self::supports_target_team(planned.skill) {
            return Err(PlannedActionError::NoValidTarget {
                actor: actor_id,
                skill: planned.skill,
            });
        }
        match planned.skill {
            PlannedSkill::BasicAttack => {
                let target = self.select_targeted_basic(actor, targeting.smart, planned.target_team).ok_or(
                    PlannedActionError::NoValidTarget {
                        actor: actor_id,
                        skill: planned.skill,
                    },
                )?;
                let amount = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown planned action actor: {}", actor.0))
                    .runtime
                    .attack;
                Ok(PreparedPlainAction::BasicAttack {
                    target,
                    use_magic: false,
                    amount,
                })
            }
            PlannedSkill::MagicAttack => {
                let target = self.select_targeted_basic(actor, targeting.smart, planned.target_team).ok_or(
                    PlannedActionError::NoValidTarget {
                        actor: actor_id,
                        skill: planned.skill,
                    },
                )?;
                let entity = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown planned action actor: {}", actor.0));
                let amount = entity.runtime.magic;
                // 与自动行动回退分支一致：魔攻消耗 (magic - attack) >> 2，
                // 差值不足 4 时为 0；MP 不够则整条指令失败。
                let magic_cost = ((entity.runtime.magic - entity.runtime.attack) >> 2).max(0);
                let available = entity.runtime.magic_point;
                if available < magic_cost {
                    return Err(PlannedActionError::InsufficientMagicPoint {
                        actor: actor_id,
                        required: magic_cost,
                        available,
                    });
                }
                if magic_cost > 0 {
                    self.entities
                        .get_mut(actor)
                        .unwrap_or_else(|| panic!("unknown planned action actor: {}", actor.0))
                        .runtime
                        .magic_point -= magic_cost;
                }
                Ok(PreparedPlainAction::BasicAttack {
                    target,
                    use_magic: true,
                    amount,
                })
            }
            PlannedSkill::Builtin(skill) => {
                let fixed_lane = self
                    .planned_skill_lane(actor, skill)
                    .ok_or(PlannedActionError::SkillNotEquipped { actor: actor_id, skill })?;
                // 玩家抽到这张牌时已经掷过消耗；没有就按 tswn 现场掷。
                let required = match planned.magic_cost {
                    Some(cost) => i32::from(cost),
                    None => {
                        let req_mp_byte = self.rng.next_u8();
                        i32::from(req_mp_byte & 15) + 8
                    }
                };
                let available = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown planned action actor: {}", actor.0))
                    .runtime
                    .magic_point;
                if available < required {
                    return Err(PlannedActionError::InsufficientMagicPoint {
                        actor: actor_id,
                        required,
                        available,
                    });
                }
                let targets = self.planned_builtin_targets(actor, skill, planned.target_team, targeting);
                let allows_empty_targets = skill == BuiltinActiveSkill::Assassinate
                    && self.entities.get(actor).is_some_and(|entity| entity.runtime.assassinate.is_some());
                if targets.is_empty() && !allows_empty_targets {
                    return Err(PlannedActionError::NoValidTarget {
                        actor: actor_id,
                        skill: planned.skill,
                    });
                }
                if let Some(entity) = self.entities.get_mut(actor) {
                    entity.runtime.magic_point -= required;
                }
                Ok(PreparedPlainAction::BuiltinSkill(PreparedBuiltinSkillAction {
                    selected: SelectedBuiltinSkill { skill, fixed_lane },
                    targets,
                }))
            }
        }
    }

    /// 只有主动敌对技能接受席位索敌；辅助、自身及强制动作不改变目标规则。
    pub fn supports_target_team(skill: PlannedSkill) -> bool {
        !matches!(
            skill,
            PlannedSkill::Builtin(
                BuiltinActiveSkill::Heal
                    | BuiltinActiveSkill::Revive
                    | BuiltinActiveSkill::Haste
                    | BuiltinActiveSkill::Iron
                    | BuiltinActiveSkill::Charge
                    | BuiltinActiveSkill::Accumulate
                    | BuiltinActiveSkill::Summon
                    | BuiltinActiveSkill::Clone
                    | BuiltinActiveSkill::Shadow
            )
        )
    }

    /// 指定敌队时的普攻选目：切换到该敌队的 duel 视图取样后过滤回该队；未指定队时走普通选目。
    fn select_targeted_basic(&mut self, actor: EntityIdx, smart: bool, team: Option<usize>) -> Option<EntityIdx> {
        let Some(team) = team else {
            return self.select_plain_default_attack_target(actor, false);
        };
        let actor_team = self.plain_effective_team(actor);
        if team == actor_team {
            return None;
        }
        let view = self.world.targeting_duel(actor_team, team);
        let original = std::mem::replace(&mut self.world, view);
        let selected = self.select_plain_default_attack_target(actor, smart);
        self.world = original;
        selected.filter(|id| self.entities.get(*id).is_some_and(|e| e.runtime.team == team))
    }

    /// 计划内置技能的目标列表：按 [`PlannedTargeting`] 取样，空结果且允许无智慧重试时按旧指令再取样一次，保持随机流可重演。
    fn planned_builtin_targets(
        &mut self,
        actor: EntityIdx,
        skill: BuiltinActiveSkill,
        team: Option<usize>,
        targeting: PlannedTargeting,
    ) -> PreparedTargetList {
        let targets = self.select_targeted_planned_targets(actor, skill, team, targeting.smart, targeting.followup_smart);
        if targets.is_empty() && targeting.retry_without_wisdom {
            // 旧指令保留第二次取样，确保已有战报的随机流可重演。
            return self.select_targeted_planned_targets(actor, skill, team, false, false);
        }
        targets
    }

    /// 指定敌队时的计划技能选目：duel 视图取样后过滤回该队；地裂与连击在首目标确定后按全场视图补足后续候选。
    fn select_targeted_planned_targets(
        &mut self,
        actor: EntityIdx,
        skill: BuiltinActiveSkill,
        team: Option<usize>,
        smart: bool,
        followup_smart: bool,
    ) -> PreparedTargetList {
        let Some(team) = team else {
            return self.select_planned_targets(actor, skill, smart);
        };
        let actor_team = self.plain_effective_team(actor);
        if team == actor_team {
            return PreparedTargetList::new();
        }
        let view = self.world.targeting_duel(actor_team, team);
        let original = std::mem::replace(&mut self.world, view);
        let mut selected = self.select_planned_targets(actor, skill, smart);
        self.world = original;
        selected.retain(|id| self.entities.get(*id).is_some_and(|e| e.runtime.team == team));
        let Some(first) = selected.first().copied() else {
            return selected;
        };
        if matches!(skill, BuiltinActiveSkill::Quake | BuiltinActiveSkill::Rapid) {
            // 首目标已确定；用全场队伍计算后续候选，智慧结果仍属于同一次行动。
            let mut rest = self.select_planned_targets(actor, skill, followup_smart);
            let count = rest.len().max(1);
            rest.retain(|id| *id != first);
            rest.insert(0, first);
            rest.truncate(count);
            rest
        } else {
            selected
        }
    }

    /// 找到装备了这个内置主动技能、且等级大于 0 的固定槽位。
    fn planned_skill_lane(&self, actor: EntityIdx, skill: BuiltinActiveSkill) -> Option<usize> {
        let entity = self.entities.get(actor)?;
        let loadout = &entity.template.skills;
        loadout
            .skills()
            .iter()
            .enumerate()
            .find(|(lane, skill_id)| {
                self.builtin_active_skill(**skill_id) == Some(skill) && loadout.level_at(*lane).unwrap_or(0) > 0
            })
            .map(|(lane, _)| lane)
    }

    /// 按技能沿用的目标口径选择目标，与自动施放共用同一批 `select_plain_*` 实现。
    fn select_planned_targets(&mut self, actor: EntityIdx, skill: BuiltinActiveSkill, smart: bool) -> PreparedTargetList {
        match skill {
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
            BuiltinActiveSkill::Charm => self.select_plain_charm_targets(actor, smart),
            BuiltinActiveSkill::Curse => self.select_plain_curse_targets(actor, smart),
            BuiltinActiveSkill::Haste => self.select_plain_haste_targets(actor, smart),
            BuiltinActiveSkill::Heal => self.select_plain_heal_targets(actor, smart),
            BuiltinActiveSkill::Slow => self.select_plain_slow_targets(actor, smart),
            BuiltinActiveSkill::Exchange => self.select_plain_exchange_targets(actor, smart),
            BuiltinActiveSkill::Revive => self.select_plain_revive_targets(actor, smart),
            BuiltinActiveSkill::Disperse => self.select_plain_disperse_targets(actor, smart),
            BuiltinActiveSkill::Assassinate => self.select_plain_assassinate_targets(actor, smart),
            BuiltinActiveSkill::Possess => self.select_plain_possess_targets(actor, smart),
            BuiltinActiveSkill::Iron
            | BuiltinActiveSkill::Charge
            | BuiltinActiveSkill::Accumulate
            | BuiltinActiveSkill::Summon
            | BuiltinActiveSkill::Clone
            | BuiltinActiveSkill::Shadow => smallvec::smallvec![actor],
            // 武器与属性修改器技能不在卡牌的公开编号内，玩家无法点名到这里。
            BuiltinActiveSkill::WeaponS11
            | BuiltinActiveSkill::WeaponDeathNote
            | BuiltinActiveSkill::WeaponXuanYue
            | BuiltinActiveSkill::RinickIceAge
            | BuiltinActiveSkill::RinickYuriControl => PreparedTargetList::new(),
        }
    }

    /// 预览一条指令会选出的目标；只临时推进 RNG，返回前恢复原状态。
    ///
    /// 目标选择是唯一依赖随机数的步骤，预览不会改变实体、技能表或 MP。
    pub fn preview_planned_targets(
        &mut self,
        actor: EntityIdx,
        skill: PlannedSkill,
        smart: bool,
    ) -> Result<Vec<PlrId>, PlannedActionError> {
        self.preview_targeted_planned_targets(actor, skill, smart, None)
    }

    pub fn preview_targeted_planned_targets(
        &mut self,
        actor: EntityIdx,
        skill: PlannedSkill,
        smart: bool,
        target_team: Option<usize>,
    ) -> Result<Vec<PlrId>, PlannedActionError> {
        self.preview_action_targets(
            actor,
            skill,
            target_team,
            PlannedTargeting {
                smart,
                followup_smart: smart,
                retry_without_wisdom: true,
            },
        )
    }

    pub fn preview_current_action_targets(
        &mut self,
        actor: EntityIdx,
        skill: PlannedSkill,
        team: Option<usize>,
        wisdom: TargetWisdom,
    ) -> Result<Vec<PlrId>, PlannedActionError> {
        let targeting = PlannedTargeting::for_action(skill, team, wisdom, self.control.awaiting_smart(actor.0 as PlrId));
        self.preview_action_targets(actor, skill, team, targeting)
    }

    /// 预览一次计划行动的目标：克隆 RNG 试取样后还原随机流；实体缺失或指定敌队不受支持时报无有效目标，未装备技能报 `SkillNotEquipped`。
    fn preview_action_targets(
        &mut self,
        actor: EntityIdx,
        skill: PlannedSkill,
        target_team: Option<usize>,
        targeting: PlannedTargeting,
    ) -> Result<Vec<PlrId>, PlannedActionError> {
        let actor_id = actor.0 as PlrId;
        if self.entities.get(actor).is_none() || (target_team.is_some() && !Self::supports_target_team(skill)) {
            return Err(PlannedActionError::NoValidTarget { actor: actor_id, skill });
        }
        let rng = self.rng.clone();
        let result = match skill {
            PlannedSkill::BasicAttack | PlannedSkill::MagicAttack => self
                .select_targeted_basic(actor, targeting.smart, target_team)
                .map(|target| vec![target.0 as PlrId])
                .ok_or(PlannedActionError::NoValidTarget { actor: actor_id, skill }),
            PlannedSkill::Builtin(builtin) => {
                if self.planned_skill_lane(actor, builtin).is_none() {
                    Err(PlannedActionError::SkillNotEquipped {
                        actor: actor_id,
                        skill: builtin,
                    })
                } else {
                    let targets = self.planned_builtin_targets(actor, builtin, target_team, targeting);
                    Ok(targets.iter().map(|target| target.0 as PlrId).collect())
                }
            }
        };
        self.rng = rng;
        result
    }

    /// 喝药：加强当前角色一项属性一回合（不占行动）+ 少量回血
    fn apply_potion(&mut self, actor: EntityIdx, code: u32) {
        // code = 属性下标 + 1（0 表示没喝药）
        let field = ((code & 0xF) as usize).saturating_sub(1);
        let mp_scale = self.mp_regen_scale(actor);
        let Some(entity) = self.entities.get_mut(actor) else { return };
        if field == 9 {
            // 9 = 能量药水：回复 40 点 MP（与其他返蓝共用上限；被拜谢过则减半）
            entity.runtime.magic_point = (entity.runtime.magic_point + (40.0 * mp_scale) as i32).min(MAGIC_POINT_CAP);
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
        self.control.set_active_potion(actor.0 as PlrId, field as u8, delta);
    }

    /// 还原上一回合的药水加成
    fn revert_active_potion(&mut self, actor: EntityIdx) {
        let Some((field, delta)) = self.control.take_active_potion(actor.0 as PlrId) else {
            return;
        };
        let entity = self.entities.get_mut(actor);
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
            // 位图预判：没有狂暴标签时不做 entries 遍历，行动热路径保持原开销。
            entity.states.may_contain(StatePayloadKind::Berserk)
                && entity
                    .states
                    .entries()
                    .iter()
                    .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
        })
    }

    pub fn select_plain_berserk_forced_attack_target(&mut self, smart: bool) -> Option<EntityIdx> {
        let target_pool = Self::quilt_target_pool(&self.entities, &self.world);
        let all_alive = target_pool.as_ref();
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
        self.drain_plain_attack_with_atp_and_covid_into(actor, target, false, atp, None, updates);
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

    pub fn scan_plain_action_skill_probabilities(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBuiltinSkillAction> {
        let (scan_len, cached_scan) = {
            let skills = &self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime action-scan actor: {}", actor.0))
                .template
                .skills;
            if skills.uses_compact_effective_skills() {
                return self.draw_compact_effective_skill(actor, smart);
            }
            if skills.uses_effective_skill_weights() {
                return self.draw_effective_skill(actor, smart);
            }
            match skills.cached_builtin_actions() {
                Some(cache) => (
                    cache.len(),
                    Some((cache.as_ptr(), skills.levels().as_ptr(), skills.levels().len())),
                ),
                None => (skills.active_order().len(), None),
            }
        };
        #[cfg(feature = "effective-skill-stats")]
        let mut scan = super::super::effective_skill_stats::Scan::new(self, actor, smart);
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
            #[cfg(feature = "effective-skill-stats")]
            let allowed = !scan.enabled() || self.plain_action_skill_allowed(actor, builtin_skill, level, smart);
            if self.plain_action_skill_probability(actor, builtin_skill, level, smart) {
                let selected = SelectedBuiltinSkill {
                    skill: builtin_skill,
                    fixed_lane,
                };
                if let Some(action) = self.prepare_plain_builtin_skill_action(actor, smart, selected) {
                    #[cfg(feature = "effective-skill-stats")]
                    {
                        use super::super::effective_skill_stats::ScanResult;
                        scan.record(builtin_skill, level, ScanResult::Committed);
                        scan.finish(true);
                    }
                    return Some(action);
                }
                #[cfg(feature = "effective-skill-stats")]
                scan.record(builtin_skill, level, super::super::effective_skill_stats::ScanResult::NoTarget);
            } else {
                #[cfg(feature = "effective-skill-stats")]
                {
                    use super::super::effective_skill_stats::ScanResult;
                    scan.record(
                        builtin_skill,
                        level,
                        if allowed { ScanResult::Miss } else { ScanResult::GateFailed },
                    );
                }
            }
        }
        #[cfg(feature = "effective-skill-stats")]
        scan.finish(false);
        None
    }

    /// 按内置技能类型选择目标并组装行动：自身类技能目标为施法者，武器技能目标留空到结算时解析；空目标且该技能不允许空目标时放弃。
    fn prepare_plain_builtin_skill_action(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        selected: SelectedBuiltinSkill,
    ) -> Option<PreparedBuiltinSkillAction> {
        let builtin_skill = selected.skill;
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
            // 武器技能的 `aa` 恒返回空数组，目标在技能自身结算时再解析。
            BuiltinActiveSkill::WeaponS11 | BuiltinActiveSkill::WeaponDeathNote | BuiltinActiveSkill::WeaponXuanYue => {
                smallvec::smallvec![]
            }
            // 属性修改器追加的 Boss 技能沿用其原型的选目标口径。
            BuiltinActiveSkill::RinickIceAge => {
                self.select_plain_default_enemy_targets_with_count(actor, smart, if smart { 6 } else { 5 })
            }
            BuiltinActiveSkill::RinickYuriControl => self.select_plain_yuri_control_targets(actor, smart),
        };
        #[cfg(not(feature = "no_debug"))]
        if self.probe_plain_action_matches(actor) {
            eprintln!(
                "[action_probe:runtime:targets] actor={} skill={} lane={} targets={:?} rc4=({}, {})",
                actor.0,
                builtin_skill.export_name(),
                selected.fixed_lane,
                targets,
                self.rng.i,
                self.rng.j,
            );
        }
        let allows_empty_targets = matches!(
            builtin_skill,
            BuiltinActiveSkill::WeaponS11 | BuiltinActiveSkill::WeaponDeathNote
        ) || (builtin_skill == BuiltinActiveSkill::Assassinate
            && self.entities.get(actor).is_some_and(|entity| entity.runtime.assassinate.is_some()));
        if targets.is_empty() && !allows_empty_targets {
            return None;
        }
        Some(PreparedBuiltinSkillAction { selected, targets })
    }

    /// 有效技能抽样：按行动权重与一次 24 位样本选出技能，权重下限 128 的部分留给普攻，redistribution 压缩不可用区间；中签后再过允许判定并准备目标。
    fn draw_effective_skill(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBuiltinSkillAction> {
        let loadout = &self.entities.get(actor).unwrap().template.skills;
        let redistribution = loadout.effective_skill_redistribution();
        let mut choices = smallvec::SmallVec::<[(SelectedBuiltinSkill, u32, f64); 8]>::new();
        let mut total = 0.0;
        let mut available = 0.0;
        for (fixed_lane, weight) in loadout.effective_action_weights().unwrap() {
            let Some(skill) = self.builtin_active_skill(loadout.skills()[fixed_lane]) else {
                continue;
            };
            total += weight;
            let level = loadout.level_at(fixed_lane).unwrap();
            if redistribution > 0.0 && !self.plain_action_skill_allowed(actor, skill, level, smart) {
                continue;
            }
            available += weight;
            choices.push((SelectedBuiltinSkill { skill, fixed_lane }, level, available));
        }
        // 用一个 24 位样本保留小数权重。权重不足 128 的区间留给普攻，超出时按比例缩放。
        // 移除指定比例的不可用区间，其余不可用权重仍留给普攻。全程只做一次技能抽样。
        let range = total.max(128.0) - redistribution * (total - available);
        let roll = f64::from(self.rng.rFFFFFF()) * (1.0 / 16_777_216.0) * range;
        let (selected, level, _) = choices.into_iter().find(|&(_, _, upper)| roll < upper)?;
        if !self.plain_action_skill_allowed(actor, selected.skill, level, smart) {
            return None;
        }
        self.prepare_plain_builtin_skill_action(actor, smart, selected)
    }

    /// compact 权重路径的技能抽样：普通技能视为抽样池，带 cut 的动态技能按尝试概率依次消耗池份额；候选找不到目标时按样本相对位置移除后继续抽样。
    fn draw_compact_effective_skill(&mut self, actor: EntityIdx, smart: bool) -> Option<PreparedBuiltinSkillAction> {
        let loadout = &self.entities.get(actor).unwrap().template.skills;
        let mut ordinary = smallvec::SmallVec::<[(SelectedBuiltinSkill, f64); 8]>::new();
        let mut dynamic = smallvec::SmallVec::<[(SelectedBuiltinSkill, f64, f64); 5]>::new();
        let mut ordinary_total = 0.0;
        let mut ordinary_blocked = 0.0;
        for (fixed_lane, weight) in loadout.effective_action_weights().unwrap() {
            let Some(skill) = self.builtin_active_skill(loadout.skills()[fixed_lane]) else {
                continue;
            };
            let level = loadout.level_at(fixed_lane).unwrap();
            let allowed = self.plain_action_skill_allowed(actor, skill, level, smart);
            let choice = SelectedBuiltinSkill { skill, fixed_lane };
            if let Some(cut) = loadout.effective_skill_cut(fixed_lane) {
                if allowed {
                    let probability = self.effective_skill_attempt_probability(actor, smart, skill, level, weight);
                    dynamic.push((choice, cut, probability));
                }
            } else {
                ordinary_total += weight;
                if allowed {
                    ordinary.push((choice, weight));
                } else {
                    ordinary_blocked += weight;
                }
            }
        }
        // 名字附带的武器技能仍排在玩家主动技能之后，不占特征维度。
        for &fixed_lane in loadout.active_order() {
            let Some(skill @ (BuiltinActiveSkill::WeaponS11 | BuiltinActiveSkill::WeaponDeathNote)) =
                self.builtin_active_skill(loadout.skills()[fixed_lane])
            else {
                continue;
            };
            let probability = self.effective_skill_attempt_probability(actor, smart, skill, 1, 0.0);
            if probability > 0.0 {
                dynamic.push((SelectedBuiltinSkill { skill, fixed_lane }, 1.0, probability));
            }
        }
        dynamic.sort_unstable_by(|(left, left_cut, _), (right, right_cut, _)| {
            left_cut.total_cmp(right_cut).then(left.fixed_lane.cmp(&right.fixed_lane))
        });
        let mut roll = (f64::from(self.rng.rFFFFFF()) + 0.5) / 16_777_216.0;
        let mut passed_cut = 0.0;
        loop {
            let available = ordinary.iter().map(|&(_, weight)| weight).sum::<f64>();
            let range = ordinary_total.max(128.0) - ordinary_blocked;
            let ordinary_probability = if range > 0.0 { (available / range).min(1.0) } else { 0.0 };
            let passed = passed_cut * ordinary_probability;
            let survival_before_cut = 1.0 - passed;
            let ordinary_probability = if survival_before_cut > 0.0 {
                (ordinary_probability - passed) / survival_before_cut
            } else {
                0.0
            };
            let mut previous = 0.0;
            let mut survival = 1.0;
            let mut offset = 0.0;
            let mut ordinary_mass = 0.0;
            let mut selected = None;
            for (index, &(choice, cut, probability)) in dynamic.iter().enumerate() {
                // 将普通技能看成一个抽样池，cut 是在此技能之前消耗的池份额。
                let remaining_cut = if passed_cut < 1.0 {
                    (cut - passed_cut) / (1.0 - passed_cut)
                } else {
                    0.0
                };
                let boundary = remaining_cut * ordinary_probability;
                ordinary_mass += (boundary - previous) * survival;
                let mass = (1.0 - boundary) * survival * probability;
                if roll < offset + mass {
                    selected = Some((choice, true, index, offset, mass));
                    break;
                }
                offset += mass;
                previous = boundary;
                survival *= 1.0 - probability;
            }
            if selected.is_none() && available > 0.0 {
                ordinary_mass += (ordinary_probability - previous) * survival;
                for (index, &(choice, weight)) in ordinary.iter().enumerate() {
                    let mass = ordinary_mass * weight / available;
                    if roll < offset + mass {
                        selected = Some((choice, false, index, offset, mass));
                        break;
                    }
                    offset += mass;
                }
            }
            let (choice, is_dynamic, index, lower, mass) = selected?;
            if let Some(action) = self.prepare_plain_builtin_skill_action(actor, smart, choice) {
                return Some(action);
            }
            // 目标失败则移除该候选，使用样本在失败区间中的相对位置继续抽样。
            roll = (roll - lower) / mass;
            if is_dynamic {
                // 此区域已经越过之前的双参数技能和对应的普通技能份额。
                passed_cut = dynamic[index].1;
                dynamic.drain(..=index);
            } else {
                // 普通技能找不到目标时，扣除该次选择的概率，保留普攻机会。
                ordinary.remove(index);
            }
        }
    }

    /// 计算技能的尝试概率：S11 由智慧与武器等级决定，死亡笔记由存活标记与敌我关系决定，其余按权重封顶 128 归一，玄月恒为 0。
    fn effective_skill_attempt_probability(
        &self,
        actor: EntityIdx,
        smart: bool,
        skill: BuiltinActiveSkill,
        _level: u32,
        weight: f64,
    ) -> f64 {
        match skill {
            BuiltinActiveSkill::WeaponS11 => {
                let runtime = &self.entities.get(actor).unwrap().runtime;
                runtime.weapon_s11.filter(|weapon| weapon.level > 0).map_or(0.0, |weapon| {
                    let threshold = i64::from(runtime.wisdom) - i64::from(weapon.level) + 1;
                    (64 - threshold.clamp(0, 64)) as f64 / 64.0
                })
            }
            BuiltinActiveSkill::WeaponDeathNote => {
                let Some(mark) = self.death_note_live_mark(actor) else {
                    return 0.0;
                };
                if smart {
                    f64::from(self.plain_effective_team(mark) != self.plain_effective_team(actor))
                } else {
                    0.5
                }
            }
            BuiltinActiveSkill::WeaponXuanYue => 0.0,
            _ => {
                #[cfg(feature = "cards")]
                let weight = weight * f64::from(self.card_skill_level(actor, skill, _level)) / f64::from(_level);
                weight.min(128.0) / 128.0
            }
        }
    }

    /// 返回行动者死亡笔记标记中仍然存活的实体；标记不存在或已死亡时返回 `None`。
    fn death_note_live_mark(&self, actor: EntityIdx) -> Option<EntityIdx> {
        self.entities
            .get(actor)?
            .runtime
            .death_note_mark
            .filter(|mark| self.entities.get(*mark).is_some_and(|entity| entity.runtime.hp > 0))
    }

    pub fn builtin_active_skill(&self, skill_id: SkillId) -> Option<BuiltinActiveSkill> {
        self.registry.builtin_active_skill(skill_id)
    }

    /// 行动抽样准入门槛：按技能类型检查当前是否允许尝试，如蓄力、吸收、钢铁、蓄劲、暗杀与召唤各有 gate。
    fn plain_action_skill_allowed(&self, actor: EntityIdx, builtin_skill: BuiltinActiveSkill, _level: u32, smart: bool) -> bool {
        self.action_roll_gate(
            actor,
            builtin_skill,
            ActionRollRule::for_skill(builtin_skill, smart),
            _level,
            smart,
        )
    }

    fn action_roll_gate(
        &self,
        actor: EntityIdx,
        _builtin_skill: BuiltinActiveSkill,
        rule: ActionRollRule,
        _level: u32,
        smart: bool,
    ) -> bool {
        #[cfg(not(feature = "no_debug"))]
        let probe_action = self.probe_plain_action_matches(actor);
        match rule {
            ActionRollRule::Charge => {
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
                            _builtin_skill.export_name(),
                            _level,
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
            ActionRollRule::Absorb => {
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
                            _builtin_skill.export_name(),
                            _level,
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
            ActionRollRule::Iron => {
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
            ActionRollRule::Accumulate => {
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
                            _builtin_skill.export_name(),
                            _level,
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
            ActionRollRule::Assassinate => {
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
            ActionRollRule::Summon => {
                if !self.plain_summon_probability_allowed(actor, smart) {
                    return false;
                }
            }
            // 影袭在智能模式且 HP < 80 时短路，不消耗概率字节。
            ActionRollRule::Shadow => {
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
                            _builtin_skill.export_name(),
                            _level,
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
        true
    }

    pub fn plain_action_skill_probability(
        &mut self,
        actor: EntityIdx,
        builtin_skill: BuiltinActiveSkill,
        level: u32,
        smart: bool,
    ) -> bool {
        // 卡牌玩家的幻术熟练度按 5 封顶；其它技能与普通对局保持原等级。
        #[cfg(feature = "cards")]
        let level = self.card_skill_level(actor, builtin_skill, level);
        #[cfg(not(feature = "no_debug"))]
        let probe_action = self.probe_plain_action_matches(actor);
        let rule = ActionRollRule::for_skill(builtin_skill, smart);
        #[cfg(feature = "no_debug")]
        if matches!(rule, ActionRollRule::Standard) {
            return self.rng.r127() < level;
        }
        if !self.action_roll_gate(actor, builtin_skill, rule, level, smart) {
            return false;
        }
        match rule {
            // 剁手刀：等级为 0 时直接失败且不消耗字节；否则只掷一次 r63。
            ActionRollRule::WeaponS11 => {
                let actor_entity = self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("unknown runtime weapon s11 probability actor: {}", actor.0));
                let Some(s11) = actor_entity.runtime.weapon_s11 else {
                    return false;
                };
                if s11.level == 0 {
                    return false;
                }
                #[cfg(not(feature = "no_debug"))]
                let before = (self.rng.i, self.rng.j);
                let roll = self.rng.r63() as i32;
                let pass = roll + s11.level as i32 > actor_entity.runtime.wisdom;
                #[cfg(not(feature = "no_debug"))]
                if probe_action {
                    eprintln!(
                        "[action_probe:runtime:prob] actor={} skill={} level={} smart={} roll={} pass={} rc4=({},{}) -> ({},{})",
                        actor.0,
                        builtin_skill.export_name(),
                        s11.level,
                        smart,
                        roll,
                        pass,
                        before.0,
                        before.1,
                        self.rng.i,
                        self.rng.j,
                    );
                }
                return pass;
            }
            // 死亡笔记：只有写下的目标仍在场时才可能行动；智能模式不消耗字节。
            ActionRollRule::WeaponDeathNote => {
                let Some(mark) = self.death_note_live_mark(actor) else {
                    return false;
                };
                if smart {
                    return self.plain_effective_team(mark) != self.plain_effective_team(actor);
                }
                return self.rng.c50();
            }
            // 玄月奖杯只注册受击回调，从不进入主动扫描。
            ActionRollRule::WeaponXuanYue => return false,
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

#[cfg(test)]
mod damage_phase_cache_tests {
    use super::*;

    /// 测试钩子：上报一条 `post damage` 消息，分数取技能 ID。
    fn mark(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.add_update(crate::runtime::update::RunUpdate::new("post damage", 0, 0, entry.skill_id.0));
    }

    #[test]
    fn post_damage_phase_rechecks_skills_after_intervening_mutation() {
        let mut builder = ExtensionRegistryBuilder::default();
        let post = builder
            .register_skill_with_hooks(
                "custom",
                "post",
                "custom.post",
                ProcMask::POST_DAMAGE,
                TargetPolicy::None,
                SkillPriority(0),
            )
            .unwrap();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "owner", 0, 100, 3)],
            builder.build(),
        ));
        runtime.set_skill_handler(post, mark);
        let mut updates = RunUpdates::new();
        runtime.drain_action_damage_phase_into(EntityIdx(0), ProcMask::PRE_DAMAGE, &mut updates);
        assert!(updates.updates.is_empty());
        // 两阶段之间替换技能，必须重新查询，不沿用前一阶段的空缓存。
        runtime.entities.get_mut(EntityIdx(0)).unwrap().template.skills = SkillLoadout::from_skills([post]);
        runtime.drain_action_damage_phase_into(EntityIdx(0), ProcMask::POST_DAMAGE, &mut updates);
        assert_eq!(updates.updates.len(), 1);
        assert_eq!(updates.updates[0].message, "post damage");
        assert_eq!(updates.updates[0].score, post.0);
    }

    #[test]
    #[should_panic(expected = "runtime damage phase owner missing")]
    fn damage_phase_rejects_missing_owner_even_when_no_hooks_exist() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "owner", 0, 100, 3)],
            ExtensionRegistryBuilder::default().build(),
        ));
        runtime.drain_action_damage_phase_into(EntityIdx(99), ProcMask::PRE_DAMAGE, &mut RunUpdates::new());
    }
}

use super::*;

/// 需要玩家确认的被动技能；护盾、垂死和护身符由核心自动判定。
fn passive_decision_skill(export_name: &str) -> bool {
    matches!(export_name, "core.skill.defend" | "core.skill.protect" | "core.skill.reflect")
}
use smallvec::SmallVec;

/// 内置静态 handler 的能力声明：不需要读取队友或敌人。
const NO_EXTENSION_CAPABILITIES: &[ExtensionCapability] = &[];
/// 内置静态 handler 的能力声明：需要读取队友（如保护）。
const READ_ALLIES_CAPABILITY: &[ExtensionCapability] = &[ExtensionCapability::ReadAllies];
/// 内置静态 handler 的能力声明：需要同时读取队友与敌人（如各类 Boss 感染状态）。
const READ_ALLIES_AND_ENEMIES_CAPABILITIES: &[ExtensionCapability] =
    &[ExtensionCapability::ReadAllies, ExtensionCapability::ReadEnemies];

impl CombatRuntime {
    /// 从准备模板构造 CombatRuntime，派生实体、世界、调度器与各类 handler 表。
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let PreparedCombatTemplate {
            players,
            registry,
            slots: template_slots,
        } = template;
        let entities = EntityArena::from_templates_with_registry(players, &registry);
        let world = WorldArena::from_entities(&entities);
        let scheduler = PhaseScheduler::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&registry);
        let effect_handlers = EffectHandlers::from_registry(&registry);
        let skill_handlers = SkillHandlers::from_registry(&registry);
        let state_handlers = StateHandlers::from_registry(&registry);
        let replay_renderers = ReplayRenderers::from_registry(&registry);
        let show_renderers = ShowRenderers::from_registry(&registry);
        let squall_bosses = entities
            .iter()
            .filter_map(|(idx, entity)| entity.runtime.squall.is_some().then_some(idx))
            .collect();
        Self {
            entities,
            world,
            scheduler,
            effects: EffectQueue::default(),
            effect_handlers,
            skill_handlers,
            state_handlers,
            replay_renderers,
            show_renderers,
            squall_bosses,
            scratch: BattleScratch::default(),
            template_slots,
            slots,
            registry,
            rng: RC4::default(),
            control: BattleControl::default(),
            #[cfg(not(feature = "no_debug"))]
            trace: None,
            round: 0,
        }
    }

    #[cfg(not(feature = "no_debug"))]
    /// 开启 trace 收集；仅在未启用 no_debug 的构建中存在。
    pub fn enable_trace(&mut self) { self.trace = Some(RuntimeTrace::default()); }

    #[cfg(not(feature = "no_debug"))]
    /// 读取已收集的 trace；未开启时为 None。
    pub fn trace(&self) -> Option<&RuntimeTrace> { self.trace.as_ref() }

    /// 绑定 effect handler，不声明额外能力。
    pub fn set_effect_handler(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.effect_handlers.set(id, handler); }

    /// 绑定 effect handler 并声明其上下文能力。
    pub fn set_effect_handler_with_capabilities(
        &mut self,
        id: EffectHandlerId,
        handler: EffectHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.effect_handlers.set_with_capabilities(id, handler, capabilities);
    }

    /// 绑定 skill handler，不声明额外能力。
    pub fn set_skill_handler(&mut self, id: SkillId, handler: SkillHandlerFn) { self.skill_handlers.set(id, handler); }

    /// 绑定 skill handler 并声明其上下文能力。
    pub fn set_skill_handler_with_capabilities(
        &mut self,
        id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.skill_handlers.set_with_capabilities(id, handler, capabilities);
    }

    /// 绑定 state handler，不声明额外能力。
    pub fn set_state_handler(&mut self, id: StateId, handler: StateHandlerFn) { self.state_handlers.set(id, handler); }

    /// 绑定 state handler 并声明其上下文能力。
    pub fn set_state_handler_with_capabilities(
        &mut self,
        id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.state_handlers.set_with_capabilities(id, handler, capabilities);
    }

    /// 绑定回放渲染器；ID 越界会 panic。
    pub fn set_replay_renderer(&mut self, id: ReplayRendererId, renderer: ReplayRendererFn) {
        self.replay_renderers.set(id, renderer);
    }

    /// 绑定展示渲染器；ID 越界会 panic。
    pub fn set_show_renderer(&mut self, id: ShowRendererId, renderer: ShowRendererFn) { self.show_renderers.set(id, renderer); }

    /// 检查实体与模板槽使用的技能是否都有可用 handler。
    pub fn validate_ready(&self) -> Result<(), RuntimeReadyError> {
        let mut missing_skill_handlers = Vec::new();
        for (entity_idx, entity) in self.entities.iter() {
            self.collect_missing_skill_handlers(
                entity.template.skills.skills(),
                RuntimeSkillSource::Entity(entity_idx),
                &mut missing_skill_handlers,
            );
        }
        for (slot_id, value) in self.template_slots.iter() {
            if let SlotValue::PlayerTemplate(template) = value {
                self.collect_missing_skill_handlers(
                    template.skills.skills(),
                    RuntimeSkillSource::TemplateSlot(slot_id),
                    &mut missing_skill_handlers,
                );
            }
        }
        if missing_skill_handlers.is_empty() {
            Ok(())
        } else {
            Err(RuntimeReadyError { missing_skill_handlers })
        }
    }

    /// 收集一组技能中缺失 handler 的条目，并按来源合并同类项。
    pub fn collect_missing_skill_handlers(
        &self,
        skills: &[SkillId],
        source: RuntimeSkillSource,
        missing_skill_handlers: &mut Vec<RuntimeMissingSkillHandler>,
    ) {
        for skill_id in skills {
            if self.registry.skill(*skill_id).is_some_and(|spec| spec.hook_mask.is_empty()) {
                continue;
            }
            if self.skill_uses_builtin_static_dispatch(*skill_id) {
                continue;
            }
            if self.skill_handlers.get(*skill_id).is_some() {
                continue;
            }
            if let Some(missing) = missing_skill_handlers.iter_mut().find(|missing| missing.skill_id == *skill_id) {
                if !missing.sources.contains(&source) {
                    missing.sources.push(source.clone());
                }
                continue;
            }
            missing_skill_handlers.push(RuntimeMissingSkillHandler {
                skill_id: *skill_id,
                export_name: self.registry.skill(*skill_id).map(|spec| spec.export_name.clone()),
                sources: vec![source.clone()],
            });
        }
    }

    /// 该技能是否由内置静态分发处理，无需注册 handler。
    pub fn skill_uses_builtin_static_dispatch(&self, skill_id: SkillId) -> bool {
        self.builtin_static_skill_handler(skill_id).is_some()
    }

    /// 按技能导出名返回内置静态 skill handler 及其能力要求；未命中内置表时返回 `None`。
    fn builtin_static_skill_handler(&self, skill_id: SkillId) -> Option<(SkillHandlerFn, &'static [ExtensionCapability])> {
        let export_name = self.registry.skill(skill_id)?.export_name.as_str();
        match export_name {
            DEFAULT_CORE_SHIELD_SKILL_EXPORT => Some((run_shield_pre_action_skill, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_PROTECT_SKILL_EXPORT => Some((run_protect_post_action_skill, READ_ALLIES_CAPABILITY)),
            DEFAULT_CORE_DEFEND_SKILL_EXPORT => Some((run_defend_post_defend_skill, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_REFLECT_SKILL_EXPORT => Some((run_reflect_pre_defend_skill, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_UPGRADE_SKILL_EXPORT
            | DEFAULT_CORE_HIDE_SKILL_EXPORT
            | DEFAULT_CORE_COUNTER_SKILL_EXPORT
            | DEFAULT_CORE_ZOMBIE_SKILL_EXPORT => Some((run_plain_passive_noop_skill, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_MERGE_SKILL_EXPORT => Some((run_merge_kill_skill, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_RERAISE_SKILL_EXPORT => Some((run_reraise_die_skill, NO_EXTENSION_CAPABILITIES)),
            export_name if export_name == BuiltinActiveSkill::Charge.export_name() => {
                Some((run_charge_post_action_skill, NO_EXTENSION_CAPABILITIES))
            }
            _ => None,
        }
    }

    /// 该状态是否由内置静态分发处理，无需注册 handler。
    pub fn state_uses_builtin_static_dispatch(&self, state_id: StateId) -> bool {
        self.builtin_static_state_handler(state_id).is_some()
    }

    /// 按状态导出名返回内置静态 state handler 及其能力要求；未命中内置表时返回 `None`。
    fn builtin_static_state_handler(&self, state_id: StateId) -> Option<(StateHandlerFn, &'static [ExtensionCapability])> {
        let export_name = self.registry.state(state_id)?.export_name.as_str();
        match export_name {
            DEFAULT_CORE_CHARM_STATE_EXPORT => Some((run_charm_post_action_state, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_CURSE_STATE_EXPORT => Some((run_curse_post_defend_state, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_POISON_STATE_EXPORT => Some((run_poison_post_action_state, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_HASTE_STATE_EXPORT => Some((run_haste_post_action_state, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_SLOW_STATE_EXPORT => Some((run_slow_post_action_state, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_IRON_STATE_EXPORT => Some((run_iron_post_defend_state, NO_EXTENSION_CAPABILITIES)),
            DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT => Some((run_covid_infection_state, READ_ALLIES_AND_ENEMIES_CAPABILITIES)),
            DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT => Some((run_lazy_infection_state, READ_ALLIES_AND_ENEMIES_CAPABILITIES)),
            DEFAULT_CORE_SAITAMA_BOSS_STATE_EXPORT => Some((run_saitama_boss_state, READ_ALLIES_AND_ENEMIES_CAPABILITIES)),
            DEFAULT_CORE_TESTSUBJECT_SKULL_FRACTURE_STATE_EXPORT => {
                Some((run_testsubject_skull_fracture_state, NO_EXTENSION_CAPABILITIES))
            }
            DEFAULT_CORE_TESTSUBJECT_NO_ENTITY_STATE_EXPORT => Some((run_testsubject_no_entity_state, NO_EXTENSION_CAPABILITIES)),
            _ => None,
        }
    }

    /// 按注册顺序调用回放渲染器，收集产出的事件。
    pub fn render_replay_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedReplay> {
        self.registry
            .replay_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.replay_renderers.get(spec.id) else {
                    panic!("missing runtime replay renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    /// 按注册顺序调用展示渲染器，收集产出的事件。
    pub fn render_show_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedShow> {
        self.registry
            .show_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.show_renderers.get(spec.id) else {
                    panic!("missing runtime show renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    /// 构造并执行一个 hook 的技能计划；有产出时返回帧。
    pub fn run_skill_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, owner, hook);
        self.flush_skill_hook_plan(&plan)
    }

    /// 执行既定技能计划；有产出时返回帧。
    pub fn flush_skill_hook_plan(&mut self, plan: &SkillHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_skill_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    /// 把技能计划写入调用方给定的更新序列。
    pub fn drain_skill_hook_plan_into(&mut self, plan: &SkillHookPlan, updates: &mut RunUpdates) {
        self.drain_skill_hook_plan_with_selected_target_into(plan, updates, None);
    }

    /// 带选中目标执行技能计划。
    pub fn drain_skill_hook_plan_with_selected_target_into(
        &mut self,
        plan: &SkillHookPlan,
        updates: &mut RunUpdates,
        selected_target: Option<EntityIdx>,
    ) {
        self.drain_skill_hook_entries_with_selected_target_into(plan.owner, plan.hook, &plan.entries, updates, selected_target);
    }

    /// 直接执行借用的技能条目切片，避免私有分段计划为了执行再包装成大容量容器。
    #[inline]
    pub fn drain_skill_hook_entries_into(
        &mut self,
        owner: EntityIdx,
        hook: ProcMask,
        entries: &[SkillHookPlanEntry],
        updates: &mut RunUpdates,
    ) {
        self.drain_skill_hook_entries_with_selected_target_into(owner, hook, entries, updates, None);
    }

    #[inline]
    fn drain_skill_hook_entries_with_selected_target_into(
        &mut self,
        owner: EntityIdx,
        hook: ProcMask,
        entries: &[SkillHookPlanEntry],
        updates: &mut RunUpdates,
        selected_target: Option<EntityIdx>,
    ) {
        for entry in entries {
            let skill_export = self.registry.skill(entry.skill_id).map(|skill| skill.export_name.as_str());
            let mut force_passive = false;
            let mut force_target = None;
            let export_name = skill_export.unwrap_or_default();
            // 兼容旧即时对局已经确认的护符结果，保留原随机数消费与后续决策顺序。
            // 没有匹配的旧决策时直接走自然判定，不再生成护符暂停点。
            if self.control.passive_decision_mode()
                && export_name == DEFAULT_CORE_RERAISE_SKILL_EXPORT
                && let Some(decision) = self.control.next_passive_decision_for(owner.0 as PlrId, export_name)
            {
                if !decision.activate || !decision.success {
                    continue;
                }
                force_passive = true;
            }
            // 守护候选只在被动决策模式下需要；自动模式不为每个钩子分配。
            let candidates = if self.control.passive_decision_mode() && export_name == "core.skill.protect" {
                self.protect_target_candidates(owner)
                    .into_iter()
                    .map(|entity| entity.0 as PlrId)
                    .collect::<Vec<_>>()
            } else {
                Vec::new()
            };
            if self.control.passive_decision_mode()
                && skill_export.is_some_and(passive_decision_skill)
                && !(export_name == "core.skill.protect" && candidates.is_empty())
            {
                let Some(decision) = self.control.next_passive_decision() else {
                    let level = self
                        .entities
                        .get(owner)
                        .and_then(|entity| entity.template.skills.level_at(entry.fixed_lane))
                        .unwrap_or(0);
                    let phase = match export_name {
                        "core.skill.reflect" => Some(crate::runtime::DamageDecisionPhase::Reflect),
                        "core.skill.defend" => Some(crate::runtime::DamageDecisionPhase::Defend),
                        "core.skill.protect" => Some(crate::runtime::DamageDecisionPhase::Protect),
                        _ => None,
                    };
                    let batch_id = self.control.damage_batch().map(|batch| batch.id);
                    let request = PassiveRequest {
                        initiator: self.control.damage_batch().map(|batch| batch.attacker),
                        mp_costs: self.passive_mp_costs(owner, export_name, false),
                        actor: owner.0 as PlrId,
                        target: None,
                        skill: export_name.to_owned(),
                        level,
                        candidates,
                        unavoidable: false,
                        intercept: false,
                        batch_id,
                        phase,
                    };
                    if batch_id.is_some() {
                        if let Some(phase) = phase {
                            self.control.record_damage_phase(phase);
                        }
                    }
                    self.control.set_pending_passive(request.clone());
                    self.control.set_pending_updates(updates);
                    std::panic::panic_any(PassivePause);
                };
                if !decision.activate || !decision.success {
                    continue;
                }
                force_passive = true;
                force_target = decision.target.map(|target| EntityIdx(target as u32));
            }
            // 被动技能同样使用实际发动者的独立能力。
            #[cfg(feature = "cards")]
            let _ability = self.ability_scope(
                entry.owner,
                &self.registry.skill(entry.skill_id).expect("skill exists").export_name,
            );
            let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_skill_handler(entry.skill_id) {
                static_handler
            } else {
                let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                    panic!("missing runtime skill handler implementation: {}", entry.skill_id.0);
                };
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(NO_EXTENSION_CAPABILITIES);
                (handler, capabilities)
            };
            {
                let mut context = {
                    let context = SkillContext::new(
                        &mut self.entities,
                        &mut self.world,
                        &self.registry,
                        &self.template_slots,
                        &mut self.slots,
                        &mut self.effects,
                        updates,
                        &mut self.rng,
                        *entry,
                        capabilities,
                    );
                    match selected_target {
                        Some(target) => context.with_selected_target(target),
                        None => context,
                    }
                };
                context = context.with_skill_multiplier(self.control.skill_multiplier());
                if force_passive {
                    context = context.with_force_passive(true);
                }
                if let Some(target) = force_target {
                    context = context.with_selected_target(target);
                }
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
            if hook.intersects(ProcMask::DIE) && self.entities.get(owner).is_some_and(|entity| entity.runtime.hp > 0) {
                break;
            }
        }
    }

    /// 带受击值执行技能计划，供 PRE_DEFEND / POST_DEFEND 使用。
    pub fn drain_skill_hook_plan_with_defend_value_into(
        &mut self,
        plan: &SkillHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        self.drain_skill_hook_plan_with_defend_value_and_on_damage_into(plan, updates, defend_value, PlainAttackOnDamage::None);
    }

    /// 带受击值与普攻附带效果执行技能计划。
    pub fn drain_skill_hook_plan_with_defend_value_and_on_damage_into(
        &mut self,
        plan: &SkillHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
        on_damage: PlainAttackOnDamage,
    ) {
        for entry in &plan.entries {
            self.drain_skill_hook_entry_with_defend_value_and_on_damage_into(*entry, updates, defend_value, on_damage);
        }
    }

    /// 执行单个防御技能钩子，供动态合并计划直接调用，避免为一项钩子临时构造计划容器。
    pub fn drain_skill_hook_entry_with_defend_value_and_on_damage_into(
        &mut self,
        entry: SkillHookPlanEntry,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
        on_damage: PlainAttackOnDamage,
    ) {
        let export_name = self
            .registry
            .skill(entry.skill_id)
            .map(|skill| skill.export_name.clone())
            .unwrap_or_default();
        let phase = match export_name.as_str() {
            DEFAULT_CORE_REFLECT_SKILL_EXPORT => Some(crate::runtime::DamageDecisionPhase::Reflect),
            DEFAULT_CORE_DEFEND_SKILL_EXPORT => Some(crate::runtime::DamageDecisionPhase::Defend),
            _ => None,
        };
        let reflect_caster_active = export_name != DEFAULT_CORE_REFLECT_SKILL_EXPORT
            || self.entities.get(defend_value.caster()).is_some_and(|entity| entity.runtime.active());
        let force_passive =
            if self.control.passive_decision_mode() && passive_decision_skill(&export_name) && reflect_caster_active {
                let Some(decision) = self.control.next_passive_decision() else {
                    let level = self
                        .entities
                        .get(entry.owner)
                        .and_then(|entity| entity.template.skills.level_at(entry.fixed_lane))
                        .unwrap_or(0);
                    let request = PassiveRequest {
                        initiator: Some(defend_value.caster().0 as PlrId),
                        mp_costs: self.passive_mp_costs(entry.owner, &export_name, false),
                        actor: entry.owner.0 as PlrId,
                        target: Some(defend_value.target().0 as PlrId),
                        skill: export_name,
                        level,
                        candidates: Vec::new(),
                        unavoidable: false,
                        intercept: false,
                        batch_id: self.control.damage_batch().map(|batch| batch.id),
                        phase,
                    };
                    if let Some(phase) = phase {
                        self.control.record_damage_phase(phase);
                    }
                    self.control.set_pending_passive(request);
                    self.control.set_pending_updates(updates);
                    std::panic::panic_any(PassivePause);
                };
                if !decision.activate || !decision.success {
                    return;
                }
                true
            } else {
                false
            };
        // 防御与反弹按实际发动者计算独立能力。
        #[cfg(feature = "cards")]
        let _ability = self.ability_scope(
            entry.owner,
            &self.registry.skill(entry.skill_id).expect("skill exists").export_name,
        );
        let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_skill_handler(entry.skill_id) {
            static_handler
        } else {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("缺少 runtime 技能处理器实现：{}", entry.skill_id.0);
            };
            let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(NO_EXTENSION_CAPABILITIES);
            (handler, capabilities)
        };
        {
            let context = SkillContext::new(
                &mut self.entities,
                &mut self.world,
                &self.registry,
                &self.template_slots,
                &mut self.slots,
                &mut self.effects,
                updates,
                &mut self.rng,
                entry,
                capabilities,
            )
            .with_defend_value(defend_value)
            .with_defend_on_damage(on_damage);
            let mut context = if force_passive {
                context.with_force_passive(true)
            } else {
                context
            };
            handler(&mut context, &entry);
        }
        self.drain_effects_into(updates);
    }

    /// 构造并执行一个 hook 的状态计划；有产出时返回帧。
    pub fn run_state_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.state_hook_plan(&self.entities, owner, hook);
        self.flush_state_hook_plan(&plan)
    }

    /// 执行既定状态计划；有产出时返回帧。
    pub fn flush_state_hook_plan(&mut self, plan: &StateHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_state_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    /// 把状态计划写入调用方给定的更新序列；返回是否产生更新。
    pub fn drain_state_hook_plan_into(&mut self, plan: &StateHookPlan, updates: &mut RunUpdates) -> bool {
        self.drain_state_hook_plan_with_action_smart_into(plan, updates, None)
    }

    /// 执行单个状态 hook 条目；返回是否产生更新。
    pub fn drain_state_hook_entry_into(&mut self, hook: ProcMask, entry: StateHookPlanEntry, updates: &mut RunUpdates) -> bool {
        self.drain_state_hook_entry_with_deferred_clears_into(hook, entry, updates, None)
    }

    pub fn drain_state_hook_entry_with_deferred_clears_into(
        &mut self,
        hook: ProcMask,
        entry: StateHookPlanEntry,
        updates: &mut RunUpdates,
        deferred_owner_state_clears: Option<&mut Vec<u32>>,
    ) -> bool {
        if self
            .entities
            .get(entry.owner)
            .and_then(|entity| entity.states.entry(entry.legacy_order_key))
            .is_none()
        {
            return false;
        }
        let Some(state_id) = entry.state_id else {
            return false;
        };
        let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_state_handler(state_id) {
            static_handler
        } else {
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime state handler implementation: {}", state_id.0);
            };
            let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(NO_EXTENSION_CAPABILITIES);
            (handler, capabilities)
        };
        let action_intercepted = {
            let context = StateContext::new(
                &mut self.entities,
                &mut self.world,
                &self.template_slots,
                &mut self.slots,
                &mut self.effects,
                updates,
                &mut self.rng,
                entry,
                hook,
                capabilities,
            );
            let mut context = if let Some(clears) = deferred_owner_state_clears {
                context.with_deferred_owner_state_clears(clears)
            } else {
                context
            };
            handler(&mut context, &entry);
            context.action_intercepted()
        };
        self.drain_effects_into(updates);
        action_intercepted
    }

    pub fn flush_deferred_owner_state_clears(&mut self, owner: EntityIdx, clears: &[u32]) {
        let Some(owner) = self.entities.get_mut(owner) else {
            return;
        };
        let mut changed = false;
        for legacy_order_key in clears {
            changed |= owner.states.clear_legacy_key(*legacy_order_key);
        }
        if changed {
            owner.refresh_runtime_stats_from_template();
        }
    }

    /// 带行动聪明判定执行状态计划；PRE_ACTION 状态可借此接管本次行动。
    pub fn drain_state_hook_plan_with_action_smart_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        action_smart: Option<bool>,
    ) -> bool {
        self.drain_state_hook_plan_with_pre_action_order_into(plan, updates, action_smart, None)
    }

    pub fn drain_state_hook_plan_with_pre_action_order_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        action_smart: Option<bool>,
        forced_order: Option<u64>,
    ) -> bool {
        let fold_boss_infections = action_smart.is_some()
            && plan.hook == ProcMask::PRE_ACTION
            && self.scheduler.uses_legacy_step_scheduler()
            && (forced_order.is_some() || self.has_boss_infection_pre_actions(plan));
        let mut proposed_infection_action = None;
        let mut action_intercepted = false;
        let mut rebuilt_entries = None::<SmallVec<[StateHookPlanEntry; 2]>>;
        let mut store_generation = plan.store_generation;
        let mut cursor = 0usize;
        let mut executed = SmallVec::<[(u32, u64); 8]>::new();
        let identity = |entry: &StateHookPlanEntry| (entry.legacy_order_key, entry.runtime_registration_order);
        loop {
            let entry = rebuilt_entries.as_ref().unwrap_or(&plan.entries).get(cursor).copied();
            let Some(entry) = entry else { break };
            cursor += 1;
            if executed.contains(&identity(&entry)) {
                continue;
            }
            if self
                .entities
                .get(entry.owner)
                .and_then(|entity| entity.states.entry(entry.legacy_order_key))
                .is_none()
            {
                let rebuilt = self.scheduler.state_hook_plan(&self.entities, entry.owner, plan.hook);
                store_generation = rebuilt.store_generation;
                rebuilt_entries = Some(rebuilt.entries);
                cursor = rebuilt_entries
                    .as_ref()
                    .expect("刚重建的状态计划必须存在")
                    .iter()
                    .position(|candidate| !executed.contains(&identity(candidate)))
                    .unwrap_or_else(|| rebuilt_entries.as_ref().expect("刚重建的状态计划必须存在").len());
                continue;
            }

            let Some(state_id) = entry.state_id else {
                continue;
            };
            let fold_infection = fold_boss_infections && self.is_builtin_infection_pre_action(&entry);
            let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_state_handler(state_id) {
                static_handler
            } else {
                let Some(handler) = self.state_handlers.get(state_id) else {
                    panic!("missing runtime state handler implementation: {}", state_id.0);
                };
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(NO_EXTENSION_CAPABILITIES);
                (handler, capabilities)
            };
            {
                let context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    entry,
                    plan.hook,
                    capabilities,
                );
                let mut context = if let Some(smart) = action_smart {
                    context.with_action_smart(smart)
                } else {
                    context
                };
                if fold_infection {
                    // 原版按登记顺序折叠前置钩子，前一提案不能跳过后一抽样。
                    if let Some(execute) = propose_boss_infection_pre_action(&mut context, &entry)
                        && forced_order.is_none_or(|order| entry.runtime_registration_order > order)
                    {
                        proposed_infection_action = Some((entry, execute));
                    }
                } else {
                    handler(&mut context, &entry);
                    action_intercepted |= context.action_intercepted();
                }
            }
            executed.push(identity(&entry));
            self.drain_effects_into(updates);
            let current_generation = self.entities.get(entry.owner).map(|entity| entity.states.generation());
            if current_generation != Some(store_generation) {
                let rebuilt = self.scheduler.state_hook_plan(&self.entities, entry.owner, plan.hook);
                store_generation = rebuilt.store_generation;
                rebuilt_entries = Some(rebuilt.entries);
                cursor = rebuilt_entries
                    .as_ref()
                    .expect("刚重建的状态计划必须存在")
                    .iter()
                    .position(|candidate| !executed.contains(&identity(candidate)))
                    .unwrap_or_else(|| rebuilt_entries.as_ref().expect("刚重建的状态计划必须存在").len());
            }
        }
        if let Some((entry, execute)) = proposed_infection_action {
            // 只有最终提案才选敌并行动；冻结仍阻止行动，但不退回已做完的前置抽样。
            if self.entities.get(entry.owner).is_some_and(|owner| !owner.states.is_frozen()) {
                let mut context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    entry,
                    plan.hook,
                    READ_ALLIES_AND_ENEMIES_CAPABILITIES,
                )
                .with_action_smart(action_smart.expect("mixed boss infections require the action smart roll"));
                execute(&mut context, &entry);
                action_intercepted |= context.action_intercepted();
                self.drain_effects_into(updates);
            }
        }
        action_intercepted
    }

    /// 计划中是否包含 Boss 版新冠感染的 PRE_ACTION 条目；混合感染执行时据此要求携带 action smart 判定。
    fn has_boss_infection_pre_actions(&self, plan: &StateHookPlan) -> bool {
        plan.entries
            .iter()
            .any(|entry| entry.legacy_order_key == PLAIN_COVID_INFECTION_STATE_KEY && self.is_builtin_infection_pre_action(entry))
    }

    /// 判断状态条目是否为 Boss 版新冠或懒感染的内置 PRE_ACTION 条目。
    fn is_builtin_infection_pre_action(&self, entry: &StateHookPlanEntry) -> bool {
        let export = match entry.legacy_order_key {
            PLAIN_COVID_INFECTION_STATE_KEY => DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT,
            PLAIN_LAZY_INFECTION_STATE_KEY => DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT,
            _ => return false,
        };
        entry.state_id.is_some()
            && entry.state_id == self.registry.state_id_by_export_name(export)
            && self
                .entities
                .get(entry.owner)
                .and_then(|owner| owner.states.entry(entry.legacy_order_key))
                .is_some_and(|state| state.is_boss_infection(&self.entities))
    }

    /// 带受击值执行单个状态条目，供 PRE_DEFEND / POST_DEFEND 使用。
    pub fn drain_state_hook_entry_with_defend_value_into(
        &mut self,
        hook: ProcMask,
        entry: StateHookPlanEntry,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        if self
            .entities
            .get(entry.owner)
            .and_then(|entity| entity.states.entry(entry.legacy_order_key))
            .is_none()
        {
            return;
        }
        let Some(state_id) = entry.state_id else {
            return;
        };
        let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_state_handler(state_id) {
            static_handler
        } else {
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime state handler implementation: {}", state_id.0);
            };
            let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(NO_EXTENSION_CAPABILITIES);
            (handler, capabilities)
        };
        {
            let mut context = StateContext::new(
                &mut self.entities,
                &mut self.world,
                &self.template_slots,
                &mut self.slots,
                &mut self.effects,
                updates,
                &mut self.rng,
                entry,
                hook,
                capabilities,
            )
            .with_defend_value(defend_value);
            handler(&mut context, &entry);
        }
        self.drain_effects_into(updates);
    }

    /// 带受击值执行状态计划。
    pub fn drain_state_hook_plan_with_defend_value_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        let mut rebuilt_entries = None::<SmallVec<[StateHookPlanEntry; 2]>>;
        let mut store_generation = plan.store_generation;
        let mut cursor = 0usize;
        let mut executed_legacy_keys = SmallVec::<[u32; 8]>::new();
        loop {
            let entry = rebuilt_entries.as_ref().unwrap_or(&plan.entries).get(cursor).copied();
            let Some(entry) = entry else { break };
            cursor += 1;
            if executed_legacy_keys.contains(&entry.legacy_order_key) {
                continue;
            }
            if self
                .entities
                .get(entry.owner)
                .and_then(|entity| entity.states.entry(entry.legacy_order_key))
                .is_none()
            {
                let rebuilt = self.scheduler.state_hook_plan(&self.entities, entry.owner, plan.hook);
                store_generation = rebuilt.store_generation;
                rebuilt_entries = Some(rebuilt.entries);
                cursor = rebuilt_entries
                    .as_ref()
                    .expect("刚重建的状态计划必须存在")
                    .iter()
                    .position(|candidate| !executed_legacy_keys.contains(&candidate.legacy_order_key))
                    .unwrap_or_else(|| rebuilt_entries.as_ref().expect("刚重建的状态计划必须存在").len());
                continue;
            }

            if entry.state_id.is_none() {
                continue;
            }
            self.drain_state_hook_entry_with_defend_value_into(plan.hook, entry, updates, defend_value);
            executed_legacy_keys.push(entry.legacy_order_key);
            let current_generation = self.entities.get(entry.owner).map(|entity| entity.states.generation());
            if current_generation != Some(store_generation) {
                let rebuilt = self.scheduler.state_hook_plan(&self.entities, entry.owner, plan.hook);
                store_generation = rebuilt.store_generation;
                rebuilt_entries = Some(rebuilt.entries);
                cursor = rebuilt_entries
                    .as_ref()
                    .expect("刚重建的状态计划必须存在")
                    .iter()
                    .position(|candidate| !executed_legacy_keys.contains(&candidate.legacy_order_key))
                    .unwrap_or_else(|| rebuilt_entries.as_ref().expect("刚重建的状态计划必须存在").len());
            }
        }
    }
}

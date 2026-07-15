use super::*;
use smallvec::SmallVec;

const NO_EXTENSION_CAPABILITIES: &[ExtensionCapability] = &[];
const READ_ALLIES_CAPABILITY: &[ExtensionCapability] = &[ExtensionCapability::ReadAllies];
const READ_ALLIES_AND_ENEMIES_CAPABILITIES: &[ExtensionCapability] =
    &[ExtensionCapability::ReadAllies, ExtensionCapability::ReadEnemies];

impl CombatRuntime {
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
            scratch: BattleScratch::default(),
            template_slots,
            slots,
            registry,
            rng: RC4::default(),
            #[cfg(not(feature = "no_debug"))]
            trace: None,
            round: 0,
        }
    }

    #[cfg(not(feature = "no_debug"))]
    pub fn enable_trace(&mut self) { self.trace = Some(RuntimeTrace::default()); }

    #[cfg(not(feature = "no_debug"))]
    pub fn trace(&self) -> Option<&RuntimeTrace> { self.trace.as_ref() }

    pub fn set_effect_handler(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.effect_handlers.set(id, handler); }

    pub fn set_effect_handler_with_capabilities(
        &mut self,
        id: EffectHandlerId,
        handler: EffectHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.effect_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_skill_handler(&mut self, id: SkillId, handler: SkillHandlerFn) { self.skill_handlers.set(id, handler); }

    pub fn set_skill_handler_with_capabilities(
        &mut self,
        id: SkillId,
        handler: SkillHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.skill_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_state_handler(&mut self, id: StateId, handler: StateHandlerFn) { self.state_handlers.set(id, handler); }

    pub fn set_state_handler_with_capabilities(
        &mut self,
        id: StateId,
        handler: StateHandlerFn,
        capabilities: &[ExtensionCapability],
    ) {
        self.state_handlers.set_with_capabilities(id, handler, capabilities);
    }

    pub fn set_replay_renderer(&mut self, id: ReplayRendererId, renderer: ReplayRendererFn) {
        self.replay_renderers.set(id, renderer);
    }

    pub fn set_show_renderer(&mut self, id: ShowRendererId, renderer: ShowRendererFn) { self.show_renderers.set(id, renderer); }

    pub fn validate_ready(&self) -> Result<(), RuntimeV2ReadyError> {
        let mut missing_skill_handlers = Vec::new();
        for (entity_idx, entity) in self.entities.iter() {
            self.collect_missing_skill_handlers(
                entity.template.skills.skills(),
                RuntimeV2SkillSource::Entity(entity_idx),
                &mut missing_skill_handlers,
            );
        }
        for (slot_id, value) in self.template_slots.iter() {
            if let SlotValue::PlayerTemplate(template) = value {
                self.collect_missing_skill_handlers(
                    template.skills.skills(),
                    RuntimeV2SkillSource::TemplateSlot(slot_id),
                    &mut missing_skill_handlers,
                );
            }
        }
        if missing_skill_handlers.is_empty() {
            Ok(())
        } else {
            Err(RuntimeV2ReadyError { missing_skill_handlers })
        }
    }

    pub fn collect_missing_skill_handlers(
        &self,
        skills: &[SkillId],
        source: RuntimeV2SkillSource,
        missing_skill_handlers: &mut Vec<RuntimeV2MissingSkillHandler>,
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
            missing_skill_handlers.push(RuntimeV2MissingSkillHandler {
                skill_id: *skill_id,
                export_name: self.registry.skill(*skill_id).map(|spec| spec.export_name.clone()),
                sources: vec![source.clone()],
            });
        }
    }

    pub fn skill_uses_builtin_static_dispatch(&self, skill_id: SkillId) -> bool {
        self.builtin_static_skill_handler(skill_id).is_some()
    }

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

    pub fn state_uses_builtin_static_dispatch(&self, state_id: StateId) -> bool {
        self.builtin_static_state_handler(state_id).is_some()
    }

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
            _ => None,
        }
    }

    pub fn render_replay_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedReplay> {
        self.registry
            .replay_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.replay_renderers.get(spec.id) else {
                    panic!("missing runtime_v2 replay renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    pub fn render_show_frame(&self, frame: &RuntimeFrame) -> Vec<RenderedShow> {
        self.registry
            .show_renderers_in_order()
            .into_iter()
            .filter_map(|spec| {
                let Some(renderer) = self.show_renderers.get(spec.id) else {
                    panic!("missing runtime_v2 show renderer implementation: {}", spec.id.0);
                };
                renderer(frame)
            })
            .collect()
    }

    pub fn run_skill_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.skill_hook_plan(&self.entities, &self.registry, owner, hook);
        self.flush_skill_hook_plan(&plan)
    }

    pub fn flush_skill_hook_plan(&mut self, plan: &SkillHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_skill_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    pub fn drain_skill_hook_plan_into(&mut self, plan: &SkillHookPlan, updates: &mut RunUpdates) {
        self.drain_skill_hook_plan_with_selected_target_into(plan, updates, None);
    }

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
    pub(super) fn drain_skill_hook_entries_into(
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
            let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_skill_handler(entry.skill_id) {
                static_handler
            } else {
                let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                    panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
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
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
            if hook.intersects(ProcMask::DIE) && self.entities.get(owner).is_some_and(|entity| entity.runtime.hp > 0) {
                break;
            }
        }
    }

    pub fn drain_skill_hook_plan_with_defend_value_into(
        &mut self,
        plan: &SkillHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        self.drain_skill_hook_plan_with_defend_value_and_on_damage_into(plan, updates, defend_value, PlainAttackOnDamage::None);
    }

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
    pub(super) fn drain_skill_hook_entry_with_defend_value_and_on_damage_into(
        &mut self,
        entry: SkillHookPlanEntry,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
        on_damage: PlainAttackOnDamage,
    ) {
        let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_skill_handler(entry.skill_id) {
            static_handler
        } else {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("缺少 runtime_v2 技能处理器实现：{}", entry.skill_id.0);
            };
            let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(NO_EXTENSION_CAPABILITIES);
            (handler, capabilities)
        };
        {
            let mut context = SkillContext::new(
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
            handler(&mut context, &entry);
        }
        self.drain_effects_into(updates);
    }

    pub fn run_state_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.state_hook_plan(&self.entities, owner, hook);
        self.flush_state_hook_plan(&plan)
    }

    pub fn flush_state_hook_plan(&mut self, plan: &StateHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_state_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    pub fn drain_state_hook_plan_into(&mut self, plan: &StateHookPlan, updates: &mut RunUpdates) -> bool {
        self.drain_state_hook_plan_with_action_smart_into(plan, updates, None)
    }

    pub fn drain_state_hook_entry_into(&mut self, hook: ProcMask, entry: StateHookPlanEntry, updates: &mut RunUpdates) -> bool {
        self.drain_state_hook_entry_with_deferred_clears_into(hook, entry, updates, None)
    }

    pub(crate) fn drain_state_hook_entry_with_deferred_clears_into(
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
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
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

    pub(crate) fn flush_deferred_owner_state_clears(&mut self, owner: EntityIdx, clears: &[u32]) {
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

    pub fn drain_state_hook_plan_with_action_smart_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        action_smart: Option<bool>,
    ) -> bool {
        let mut action_intercepted = false;
        let mut rebuilt_entries = None::<SmallVec<[StateHookPlanEntry; 8]>>;
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

            let Some(state_id) = entry.state_id else {
                continue;
            };
            let (handler, capabilities) = if let Some(static_handler) = self.builtin_static_state_handler(state_id) {
                static_handler
            } else {
                let Some(handler) = self.state_handlers.get(state_id) else {
                    panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
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
                handler(&mut context, &entry);
                action_intercepted |= context.action_intercepted();
            }
            executed_legacy_keys.push(entry.legacy_order_key);
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
                    .position(|candidate| !executed_legacy_keys.contains(&candidate.legacy_order_key))
                    .unwrap_or_else(|| rebuilt_entries.as_ref().expect("刚重建的状态计划必须存在").len());
            }
        }
        action_intercepted
    }

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
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
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

    pub fn drain_state_hook_plan_with_defend_value_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        let mut rebuilt_entries = None::<SmallVec<[StateHookPlanEntry; 8]>>;
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

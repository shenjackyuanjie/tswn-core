use super::*;

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
        for entry in &plan.entries {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
            };
            {
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(&[]);
                let mut context = {
                    let context = SkillContext::new(
                        &mut self.entities,
                        &mut self.world,
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
            if plan.hook.intersects(ProcMask::DIE) && self.entities.get(plan.owner).is_some_and(|entity| entity.runtime.hp > 0) {
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
        for entry in &plan.entries {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
            };
            {
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(&[]);
                let mut context = SkillContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    capabilities,
                )
                .with_defend_value(defend_value);
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
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

    pub fn drain_state_hook_plan_with_action_smart_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        action_smart: Option<bool>,
    ) -> bool {
        let mut action_intercepted = false;
        for entry in &plan.entries {
            let Some(state_id) = entry.state_id else {
                continue;
            };
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
            };
            {
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(&[]);
                let context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    plan.hook,
                    capabilities,
                );
                let mut context = if let Some(smart) = action_smart {
                    context.with_action_smart(smart)
                } else {
                    context
                };
                handler(&mut context, entry);
                action_intercepted |= context.action_intercepted();
            }
            self.drain_effects_into(updates);
        }
        action_intercepted
    }

    pub fn drain_state_hook_plan_with_defend_value_into(
        &mut self,
        plan: &StateHookPlan,
        updates: &mut RunUpdates,
        defend_value: &mut RuntimeDefendValue,
    ) {
        for entry in &plan.entries {
            let Some(state_id) = entry.state_id else {
                continue;
            };
            let Some(handler) = self.state_handlers.get(state_id) else {
                panic!("missing runtime_v2 state handler implementation: {}", state_id.0);
            };
            {
                let capabilities = self.state_handlers.capabilities(state_id).unwrap_or(&[]);
                let mut context = StateContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &self.template_slots,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    &mut self.rng,
                    *entry,
                    plan.hook,
                    capabilities,
                )
                .with_defend_value(defend_value);
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }
}

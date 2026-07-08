pub mod effect;
pub mod entity;
pub mod extension;
pub mod oracle;
pub mod scheduler;
pub mod scratch;
pub mod slot;
#[cfg(not(feature = "no_debug"))]
pub mod trace;
pub mod world;

use crate::engine::update::RunUpdates;
use crate::rc4::RC4;

pub use effect::{
    CustomEffect, CustomEffectPayload, EffectContext, EffectContextError, EffectHandlerFn, EffectHandlers, EffectQueue,
    QueuedEffect, RenderedReplay, RenderedShow, ReplayRendererFn, ReplayRenderers, RuntimeFrame, ShowRendererFn, ShowRenderers,
    SkillContext, SkillHandlerFn, SkillHandlers, StateContext, StateHandlerFn, StateHandlers,
};
pub use entity::{
    EntityArena, EntityIdx, EntityRecord, MoveState, PlayerRuntime, PlayerTemplate, SkillLoadout, StateEntry, StateStore,
};
pub use extension::{
    BattleSlotId, BattleSlotSpec, EffectHandlerId, EffectHandlerSpec, EntitySlotId, EntitySlotSpec, ExtensionCapability,
    ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder, ExtensionVersion, InstalledExtensionSpec, PlayerKindFlags,
    PlayerKindId, PlayerKindPolicies, PlayerKindSpec, ProcMask, RegistrationOrder, ReplayRendererId, ReplayRendererSpec,
    ShowRendererId, ShowRendererSpec, SkillId, SkillPriority, SkillSpec, StateId, StateSpec, TargetPolicy, TemplateSlotId,
    TemplateSlotSpec, TswnExtension,
};
pub use oracle::{NormalizedOutcome, NormalizedUpdateFrame, StrictDiff, strict_diff};
pub use scheduler::{ActionPlan, PhaseScheduler, SkillHookPlan, SkillHookPlanEntry, StateHookPlan, StateHookPlanEntry};
pub use scratch::BattleScratch;
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
#[cfg(not(feature = "no_debug"))]
pub use trace::{RngCheckpoint, RuntimeTrace, TraceAction, TraceFrame};
pub use world::WorldArena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCombatTemplate {
    pub players: Vec<PlayerTemplate>,
    pub registry: ExtensionRegistry,
    pub slots: TemplateSlotStorage,
}

impl PreparedCombatTemplate {
    pub fn new(players: Vec<PlayerTemplate>) -> Self { Self::with_registry(players, ExtensionRegistry::default()) }

    pub fn with_registry(players: Vec<PlayerTemplate>, registry: ExtensionRegistry) -> Self {
        let slots = TemplateSlotStorage::from_registry(&registry);
        Self {
            players,
            registry,
            slots,
        }
    }

    pub fn minimal_1v1(left_hp: i32, right_hp: i32, attack: i32) -> Self {
        Self::new(vec![
            PlayerTemplate::new(1, "left", 0, left_hp, attack),
            PlayerTemplate::new(2, "right", 1, right_hp, attack),
        ])
    }
}

#[derive(Debug, Clone)]
pub struct RoundOutcome {
    pub action: Option<ActionPlan>,
    pub frame: Option<RuntimeFrame>,
    pub winner_team: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct CombatRuntime {
    pub entities: EntityArena,
    pub world: WorldArena,
    pub scheduler: PhaseScheduler,
    pub effects: EffectQueue,
    pub effect_handlers: EffectHandlers,
    pub skill_handlers: SkillHandlers,
    pub state_handlers: StateHandlers,
    pub replay_renderers: ReplayRenderers,
    pub show_renderers: ShowRenderers,
    pub scratch: BattleScratch,
    pub slots: BattleSlotStorage,
    pub registry: ExtensionRegistry,
    pub rng: RC4,
    #[cfg(not(feature = "no_debug"))]
    pub trace: Option<RuntimeTrace>,
    pub round: u64,
}

impl CombatRuntime {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let entities = EntityArena::from_templates_with_registry(template.players, &template.registry);
        let world = WorldArena::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&template.registry);
        let effect_handlers = EffectHandlers::from_registry(&template.registry);
        let skill_handlers = SkillHandlers::from_registry(&template.registry);
        let state_handlers = StateHandlers::from_registry(&template.registry);
        let replay_renderers = ReplayRenderers::from_registry(&template.registry);
        let show_renderers = ShowRenderers::from_registry(&template.registry);
        Self {
            entities,
            world,
            scheduler: PhaseScheduler,
            effects: EffectQueue::default(),
            effect_handlers,
            skill_handlers,
            state_handlers,
            replay_renderers,
            show_renderers,
            scratch: BattleScratch::default(),
            slots,
            registry: template.registry,
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

    fn flush_skill_hook_plan(&mut self, plan: &SkillHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_skill_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_skill_hook_plan_into(&mut self, plan: &SkillHookPlan, updates: &mut RunUpdates) {
        for entry in &plan.entries {
            let Some(handler) = self.skill_handlers.get(entry.skill_id) else {
                panic!("missing runtime_v2 skill handler implementation: {}", entry.skill_id.0);
            };
            {
                let capabilities = self.skill_handlers.capabilities(entry.skill_id).unwrap_or(&[]);
                let mut context = SkillContext::new(
                    &mut self.entities,
                    &mut self.world,
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    *entry,
                    capabilities,
                );
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    pub fn run_state_hooks(&mut self, owner: EntityIdx, hook: ProcMask) -> Option<RuntimeFrame> {
        let plan = self.scheduler.state_hook_plan(&self.entities, owner, hook);
        self.flush_state_hook_plan(&plan)
    }

    fn flush_state_hook_plan(&mut self, plan: &StateHookPlan) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_state_hook_plan_into(plan, &mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_state_hook_plan_into(&mut self, plan: &StateHookPlan, updates: &mut RunUpdates) {
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
                    &mut self.slots,
                    &mut self.effects,
                    updates,
                    *entry,
                    capabilities,
                );
                handler(&mut context, entry);
            }
            self.drain_effects_into(updates);
        }
    }

    pub fn run_minimal_round(&mut self) -> RoundOutcome {
        if let Some(winner_team) = self.world.sync_winner(&self.entities) {
            return RoundOutcome {
                action: None,
                frame: None,
                winner_team: Some(winner_team),
            };
        }

        let Some(action) = self.scheduler.select_minimal_action(&mut self.world, &self.entities) else {
            return RoundOutcome {
                action: None,
                frame: None,
                winner_team: None,
            };
        };
        self.scratch.selected_actor_round = self.round;
        #[cfg(not(feature = "no_debug"))]
        if let Some(trace) = &mut self.trace {
            trace.record_action(TraceAction {
                round: self.round + 1,
                actor: action.actor,
                target: action.target,
                amount: action.amount,
                rng_before: Some(RngCheckpoint::from_rc4(&self.rng)),
                rng_after: Some(RngCheckpoint::from_rc4(&self.rng)),
            });
        }

        let mut updates = RunUpdates::new();
        let skill_plan = self
            .scheduler
            .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_ACTION);
        self.drain_skill_hook_plan_into(&skill_plan, &mut updates);
        let pre_damage_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::PRE_DAMAGE);
        self.drain_skill_hook_plan_into(&pre_damage_skill_plan, &mut updates);
        let pre_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::PRE_DAMAGE);
        self.drain_state_hook_plan_into(&pre_damage_state_plan, &mut updates);
        self.effects.push(QueuedEffect::Damage {
            caster: action.actor,
            target: action.target,
            amount: action.amount,
        });
        self.drain_effects_into(&mut updates);
        let post_damage_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_DAMAGE);
        self.drain_skill_hook_plan_into(&post_damage_skill_plan, &mut updates);
        let post_damage_state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_DAMAGE);
        self.drain_state_hook_plan_into(&post_damage_state_plan, &mut updates);
        let post_action_skill_plan =
            self.scheduler
                .skill_hook_plan(&self.entities, &self.registry, action.actor, ProcMask::POST_ACTION);
        self.drain_skill_hook_plan_into(&post_action_skill_plan, &mut updates);
        let state_plan = self.scheduler.state_hook_plan(&self.entities, action.actor, ProcMask::POST_ACTION);
        self.drain_state_hook_plan_into(&state_plan, &mut updates);
        let frame = updates.had_updates().then_some(RuntimeFrame { updates });
        self.round += 1;
        let winner_team = self.world.sync_winner(&self.entities);
        #[cfg(not(feature = "no_debug"))]
        if let (Some(trace), Some(frame)) = (&mut self.trace, &frame) {
            trace.record_frame(self.round, frame, winner_team);
        }
        RoundOutcome {
            action: Some(action),
            frame,
            winner_team,
        }
    }

    #[cfg(test)]
    fn flush_effects(&mut self) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_effects_into(&mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    fn drain_effects_into(&mut self, updates: &mut RunUpdates) {
        while let Some(effect) = self.effects.pop_next() {
            match effect {
                QueuedEffect::Damage { caster, target, amount } => {
                    self.ensure_effect_entity("damage", "caster", caster);
                    self.ensure_effect_entity("damage", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 damage target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
                    if target_entity.runtime.hp == 0 {
                        target_entity.runtime.alive = false;
                    }
                    updates.add(RuntimeFrame::damage_update(caster.0 as usize, target.0 as usize, amount));
                }
                QueuedEffect::Heal { caster, target, amount } => {
                    self.ensure_effect_entity("heal", "caster", caster);
                    self.ensure_effect_entity("heal", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 heal target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = (target_entity.runtime.hp + amount.max(0)).min(target_entity.template.max_hp);
                    if target_entity.runtime.hp > 0 {
                        target_entity.runtime.alive = true;
                    }
                    updates.add(RuntimeFrame::heal_update(caster.0 as usize, target.0 as usize, amount));
                }
                QueuedEffect::Spawn { caster, template } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let spawned = self.entities.spawn_from_template(template, &self.registry);
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                    updates.add(RuntimeFrame::spawn_update(caster.0 as usize, spawned.0 as usize));
                }
                QueuedEffect::AddState { target, state } => {
                    self.ensure_effect_entity("add-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 add-state target entity: {}", target.0);
                    };
                    if target_entity.states.add_entry(state) {
                        updates.add(RuntimeFrame::add_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::ClearState {
                    target,
                    legacy_order_key,
                } => {
                    self.ensure_effect_entity("clear-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 clear-state target entity: {}", target.0);
                    };
                    if target_entity.states.clear_legacy_key(legacy_order_key) {
                        updates.add(RuntimeFrame::clear_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::Revive { caster, target, hp } => {
                    self.ensure_effect_entity("revive", "caster", caster);
                    self.ensure_effect_entity("revive", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 revive target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = hp.max(1).min(target_entity.template.max_hp);
                    target_entity.runtime.alive = true;
                    let team = target_entity.runtime.team;
                    self.world.revive_round_actor(target);
                    self.world.revive_alive(target, team);
                    updates.add(RuntimeFrame::revive_update(caster.0 as usize, target.0 as usize, hp));
                }
                QueuedEffect::Remove { caster, target } => {
                    self.ensure_effect_entity("remove", "caster", caster);
                    self.ensure_effect_entity("remove", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime_v2 remove target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = 0;
                    target_entity.runtime.alive = false;
                    let team = target_entity.runtime.team;
                    self.world.remove_round_actor(target);
                    self.world.remove_alive(target, team);
                    updates.add(RuntimeFrame::remove_update(caster.0 as usize, target.0 as usize));
                }
                QueuedEffect::Replay {
                    caster,
                    target,
                    message,
                    score,
                } => {
                    self.ensure_effect_entity("replay", "caster", caster);
                    self.ensure_effect_entity("replay", "target", target);
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        message,
                        score,
                    ));
                }
                QueuedEffect::Custom(custom) => {
                    self.ensure_effect_entity("custom", "caster", custom.caster);
                    if let Some(target) = custom.target {
                        self.ensure_effect_entity("custom", "target", target);
                    }
                    let Some(handler) = self.effect_handlers.get(custom.handler) else {
                        panic!("missing runtime_v2 effect handler implementation: {}", custom.handler.0);
                    };
                    let capabilities = self.effect_handlers.capabilities(custom.handler).unwrap_or(&[]);
                    let mut context = EffectContext::new(
                        &mut self.entities,
                        &mut self.world,
                        &mut self.slots,
                        &mut self.effects,
                        updates,
                        &custom,
                        capabilities,
                    );
                    handler(&mut context, &custom);
                }
            }
        }
    }

    fn ensure_effect_entity(&self, effect: &'static str, role: &'static str, entity: EntityIdx) {
        if self.entities.get(entity).is_none() {
            panic!("unknown runtime_v2 {effect} {role} entity: {}", entity.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_1v1_template_builds_runtime() {
        let runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));

        assert_eq!(runtime.entities.len(), 2);
        assert_eq!(runtime.world.winner_team(), None);
        assert!(runtime.effects.is_empty());
        assert!(runtime.slots.is_empty());
    }

    #[test]
    fn runtime_from_template_reserves_registered_slot_storage() {
        let mut builder = ExtensionRegistryBuilder::default();
        let template_slot = builder
            .reserve_template_slot("custom", "template", "custom.template")
            .expect("template slot should reserve");
        let battle_slot = builder
            .reserve_battle_slot("custom", "battle", "custom.battle")
            .expect("battle slot should reserve");
        let entity_slot = builder
            .reserve_entity_slot("custom", "entity", "custom.entity")
            .expect("entity slot should reserve");
        let registry = builder.build();
        let mut template = PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], registry);
        template
            .slots
            .set(template_slot, SlotValue::Text("seed".to_owned()))
            .expect("template slot should write");

        let mut runtime = CombatRuntime::from_template(template);
        runtime.slots.set(battle_slot, SlotValue::U64(1)).expect("battle slot should write");
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .slots
            .set(entity_slot, SlotValue::Bool(true))
            .expect("entity slot should write");

        assert_eq!(runtime.slots.get(battle_slot), Some(&SlotValue::U64(1)));
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().slots.get(entity_slot),
            Some(&SlotValue::Bool(true))
        );
    }

    #[cfg(not(feature = "no_debug"))]
    #[test]
    fn run_minimal_round_records_trace_when_enabled() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.enable_trace();

        runtime.run_minimal_round();

        let trace = runtime.trace().expect("trace should be enabled");
        assert_eq!(trace.actions.len(), 1);
        assert_eq!(trace.actions[0].actor, EntityIdx(0));
        assert_eq!(trace.actions[0].target, EntityIdx(1));
        assert_eq!(
            trace.actions[0].rng_before,
            Some(RngCheckpoint {
                i: 0,
                j: 0,
                byte_count: 0,
            })
        );
        assert_eq!(trace.actions[0].rng_after, trace.actions[0].rng_before);
        assert_eq!(trace.frames.len(), 1);
        assert_eq!(trace.frames[0].updates[0].score, 3);
    }

    #[test]
    fn run_minimal_round_applies_damage_frame() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        let outcome = runtime.run_minimal_round();

        assert_eq!(outcome.winner_team, None);
        assert!(outcome.frame.is_some());
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(runtime.round, 1);
    }

    #[test]
    fn run_minimal_round_reports_winner_after_lethal_damage() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 3, 3));
        let outcome = runtime.run_minimal_round();

        assert_eq!(outcome.winner_team, Some(0));
        assert!(!runtime.entities.get(EntityIdx(1)).unwrap().runtime.alive);
    }

    fn custom_marks_update(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Text(message) = &effect.payload else {
            panic!("custom test effect expects text payload");
        };
        context.add_update(crate::engine::update::RunUpdate::new(
            message.clone(),
            effect.caster.0 as usize,
            effect.target.unwrap().0 as usize,
            0,
        ));
    }

    fn custom_spawns_nested_damage(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(amount) = effect.payload else {
            panic!("custom test effect expects int payload");
        };
        context.push_nested(QueuedEffect::Damage {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
        });
    }

    fn custom_spawns_nested_heal(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(amount) = effect.payload else {
            panic!("custom test effect expects int payload");
        };
        context.push_nested(QueuedEffect::Heal {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
        });
    }

    fn custom_rejects_cross_entity_read(context: &mut EffectContext<'_>, _: &CustomEffect) {
        assert_eq!(
            context.entity(EntityIdx(2)),
            Err(EffectContextError::MissingCapability(ExtensionCapability::ReadEnemies))
        );
        context.add_update(crate::engine::update::RunUpdate::new("read denied", 0, 0, 0));
    }

    fn custom_reads_cross_entity(context: &mut EffectContext<'_>, _: &CustomEffect) {
        let observed = context.entity(EntityIdx(2)).expect("capability should allow cross-entity read");
        context.add_update(crate::engine::update::RunUpdate::new(observed.template.name.clone(), 0, 2, 0));
    }

    fn custom_mutates_entity_slot(context: &mut EffectContext<'_>, effect: &CustomEffect) {
        let CustomEffectPayload::Int(slot) = effect.payload else {
            panic!("custom test effect expects entity slot id payload");
        };
        context
            .set_entity_slot(
                effect.target.expect("custom test effect needs target"),
                EntitySlotId(slot as u32),
                SlotValue::Bool(true),
            )
            .expect("capability should allow entity slot mutation");
        context.add_update(crate::engine::update::RunUpdate::new("slot set", 0, 0, 0));
    }

    fn skill_marks_update(context: &mut SkillContext<'_>, entry: &SkillHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "skill mark",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.skill_id.0,
        ));
    }

    fn skill_pushes_nested_damage(context: &mut SkillContext<'_>, _: &SkillHookPlanEntry) {
        context.push_nested(QueuedEffect::Damage {
            caster: context.owner_idx(),
            target: EntityIdx(1),
            amount: 2,
        });
    }

    fn state_marks_update(context: &mut StateContext<'_>, entry: &StateHookPlanEntry) {
        context.add_update(crate::engine::update::RunUpdate::new(
            "state mark",
            entry.owner.0 as usize,
            entry.owner.0 as usize,
            entry.legacy_order_key,
        ));
    }

    fn state_pushes_nested_heal(context: &mut StateContext<'_>, _: &StateHookPlanEntry) {
        context.push_nested(QueuedEffect::Heal {
            caster: context.owner_idx(),
            target: context.owner_idx(),
            amount: 2,
        });
    }

    fn render_first_message_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
        Some(RenderedReplay::new(
            ReplayRendererId(0),
            frame.updates.updates.first()?.message.to_string(),
        ))
    }

    fn render_update_count_replay(frame: &RuntimeFrame) -> Option<RenderedReplay> {
        Some(RenderedReplay::new(
            ReplayRendererId(1),
            frame.updates.updates.len().to_string(),
        ))
    }

    fn render_first_message_show(frame: &RuntimeFrame) -> Option<RenderedShow> {
        Some(RenderedShow::new(
            ShowRendererId(0),
            frame.updates.updates.first()?.message.to_string(),
        ))
    }

    #[test]
    fn run_skill_hooks_dispatches_registered_skill_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_skill_with_hooks(
                "custom",
                "marker",
                "custom.marker",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker])],
            registry,
        ));
        runtime.set_skill_handler(marker, skill_marks_update);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("skill handler should emit update");

        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[0].score, marker.0);
    }

    #[test]
    fn run_skill_hooks_flushes_nested_effects() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "damage",
                "custom.damage",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_pushes_nested_damage);

        let frame = runtime
            .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
            .expect("nested damage should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 8);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[0].score, 2);
    }

    #[test]
    fn run_minimal_round_dispatches_pre_action_skill_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_skill_with_hooks(
                "custom",
                "marker",
                "custom.marker",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([marker]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(marker, skill_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("skill plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_pre_action_skill_effect_before_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "damage",
                "custom.damage",
                ProcMask::PRE_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(skill, skill_pushes_nested_damage);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("skill damage plus attack should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 5);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].score, 2);
        assert_eq!(frame.updates.updates[1].score, 3);
    }

    #[test]
    fn run_minimal_round_dispatches_damage_skill_hooks_around_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_damage = builder
            .register_skill_with_hooks(
                "custom",
                "pre-damage",
                "custom.pre_damage",
                ProcMask::PRE_DAMAGE,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("pre-damage skill should register");
        let post_damage = builder
            .register_skill_with_hooks(
                "custom",
                "post-damage",
                "custom.post_damage",
                ProcMask::POST_DAMAGE,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("post-damage skill should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([pre_damage, post_damage]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_skill_handler(pre_damage, skill_marks_update);
        runtime.set_skill_handler(post_damage, skill_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("damage skill hooks plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "skill mark");
        assert_eq!(frame.updates.updates[0].score, pre_damage.0);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].message, "skill mark");
        assert_eq!(frame.updates.updates[2].score, post_damage.0);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_dispatches_post_action_skill_before_state() {
        let mut builder = ExtensionRegistryBuilder::default();
        let skill = builder
            .register_skill_with_hooks(
                "custom",
                "post-action-skill",
                "custom.post_action_skill",
                ProcMask::POST_ACTION,
                TargetPolicy::Enemy,
                SkillPriority(0),
            )
            .expect("post-action skill should register");
        let state = builder
            .register_state(
                "custom",
                "post-action-state",
                "custom.post_action_state",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 55,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_skill_handler(skill, skill_marks_update);
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-action skill and state plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "skill mark");
        assert_eq!(frame.updates.updates[1].score, skill.0);
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 55);
    }

    #[test]
    fn run_minimal_round_dispatches_post_action_state_after_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "marker", "custom.marker", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack plus state hook should emit update");

        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "state mark");
        assert_eq!(frame.updates.updates[1].score, 77);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_post_action_state_effect_after_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 88,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_pushes_nested_heal);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("attack plus state heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(frame.updates.updates.len(), 2);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[1].score, 2);
    }

    #[test]
    fn run_minimal_round_dispatches_damage_state_hooks_around_attack() {
        let mut builder = ExtensionRegistryBuilder::default();
        let pre_damage = builder
            .register_state(
                "custom",
                "pre-damage",
                "custom.pre_damage",
                ProcMask::PRE_DAMAGE,
                SkillPriority(0),
            )
            .expect("pre-damage state should register");
        let post_damage = builder
            .register_state(
                "custom",
                "post-damage",
                "custom.post_damage",
                ProcMask::POST_DAMAGE,
                SkillPriority(0),
            )
            .expect("post-damage state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 11,
                extension_state_id: Some(pre_damage),
                hook_mask: ProcMask::PRE_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
            });
            store.add_entry(StateEntry {
                legacy_order_key: 22,
                extension_state_id: Some(post_damage),
                hook_mask: ProcMask::POST_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
            });
        }
        runtime.set_state_handler(pre_damage, state_marks_update);
        runtime.set_state_handler(post_damage, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("damage state hooks plus attack should emit update");

        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(frame.updates.updates[0].score, 11);
        assert_eq!(frame.updates.updates[1].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 22);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
    }

    #[test]
    fn run_minimal_round_flushes_post_damage_state_effect_before_post_action() {
        let mut builder = ExtensionRegistryBuilder::default();
        let post_damage = builder
            .register_state(
                "custom",
                "post-damage-regen",
                "custom.post_damage_regen",
                ProcMask::POST_DAMAGE,
                SkillPriority(0),
            )
            .expect("post-damage state should register");
        let post_action = builder
            .register_state(
                "custom",
                "post-action-marker",
                "custom.post_action_marker",
                ProcMask::POST_ACTION,
                SkillPriority(0),
            )
            .expect("post-action state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_entry(StateEntry {
                legacy_order_key: 33,
                extension_state_id: Some(post_damage),
                hook_mask: ProcMask::POST_DAMAGE,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(0),
            });
            store.add_entry(StateEntry {
                legacy_order_key: 44,
                extension_state_id: Some(post_action),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
            });
        }
        runtime.set_state_handler(post_damage, state_pushes_nested_heal);
        runtime.set_state_handler(post_action, state_marks_update);

        let outcome = runtime.run_minimal_round();
        let frame = outcome.frame.expect("post-damage effect plus post-action state should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates.len(), 3);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[2].message, "state mark");
        assert_eq!(frame.updates.updates[2].score, 44);
    }

    #[test]
    fn run_state_hooks_dispatches_registered_state_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "burning", "custom.burning", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry {
            legacy_order_key: 42,
            extension_state_id: Some(state),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        });
        runtime.set_state_handler(state, state_marks_update);

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state handler should emit update");

        assert_eq!(frame.updates.updates[0].message, "state mark");
        assert_eq!(frame.updates.updates[0].score, 42);
    }

    #[test]
    fn run_state_hooks_flushes_nested_effects_and_skips_legacy_entries() {
        let mut builder = ExtensionRegistryBuilder::default();
        let state = builder
            .register_state("custom", "regen", "custom.regen", ProcMask::POST_ACTION, SkillPriority(0))
            .expect("state should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 4;
        {
            let store = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().states;
            store.add_legacy_key(11);
            store.add_entry(StateEntry {
                legacy_order_key: 22,
                extension_state_id: Some(state),
                hook_mask: ProcMask::POST_ACTION,
                priority: SkillPriority(0),
                registration_order: RegistrationOrder(1),
            });
        }
        runtime.set_state_handler(state, state_pushes_nested_heal);

        let frame = runtime
            .run_state_hooks(EntityIdx(0), ProcMask::POST_ACTION)
            .expect("state heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates.len(), 1);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[0].score, 2);
    }

    #[test]
    fn flush_effects_dispatches_custom_handlers() {
        let mut builder = ExtensionRegistryBuilder::default();
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("custom mark".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("custom handler should emit update");
        assert_eq!(frame.updates.updates[0].message, "custom mark");
    }

    #[test]
    fn flush_effects_runs_nested_custom_effect_before_older_siblings() {
        let mut builder = ExtensionRegistryBuilder::default();
        let nested_damage = builder
            .register_effect_handler("custom", "nested-damage", "custom.nested_damage", SkillPriority(0))
            .expect("handler should register");
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(nested_damage, custom_spawns_nested_damage);
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            nested_damage,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(4),
        )));
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("after nested".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("nested damage should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 6);
        assert_eq!(frame.updates.updates[0].message, "[0]攻击[1]");
        assert_eq!(frame.updates.updates[1].message, "after nested");
    }

    #[test]
    fn flush_effects_applies_heal_without_exceeding_max_hp() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 4;
        runtime.effects.push(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            amount: 20,
        });

        let frame = runtime.flush_effects().expect("heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
    }

    #[test]
    fn flush_effects_spawns_entity_and_adds_it_to_round_order() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(3, "summoned", 0, 5, 2),
        });

        let frame = runtime.flush_effects().expect("spawn should emit update");

        assert_eq!(runtime.entities.len(), 3);
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().template.name, "summoned");
        assert_eq!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.hp, 5);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
        assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
    }

    #[test]
    fn spawned_entity_can_be_selected_by_scheduler() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            ExtensionRegistry::default(),
        ));
        runtime.effects.push(QueuedEffect::Spawn {
            caster: EntityIdx(0),
            template: PlayerTemplate::new(2, "enemy", 1, 8, 4),
        });
        runtime.flush_effects().expect("spawn should emit update");

        assert_eq!(runtime.world.sync_winner(&runtime.entities), None);
        assert_eq!(
            runtime.scheduler.select_minimal_action(&mut runtime.world, &runtime.entities),
            Some(ActionPlan {
                actor: EntityIdx(0),
                target: EntityIdx(1),
                amount: 3,
            })
        );
    }

    #[test]
    fn flush_effects_adds_and_clears_state_entries() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        let state = StateEntry {
            legacy_order_key: 77,
            extension_state_id: Some(StateId(1)),
            hook_mask: ProcMask::POST_ACTION,
            priority: SkillPriority(5),
            registration_order: RegistrationOrder(2),
        };

        runtime.effects.push(QueuedEffect::AddState {
            target: EntityIdx(1),
            state,
        });
        let add_frame = runtime.flush_effects().expect("add state should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), Some(&state));
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 1);
        assert_eq!(add_frame.updates.updates[0].message, "[1]状态改变");

        runtime.effects.push(QueuedEffect::ClearState {
            target: EntityIdx(1),
            legacy_order_key: 77,
        });
        let clear_frame = runtime.flush_effects().expect("clear state should emit update");
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.entry(77), None);
        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().states.generation(), 2);
        assert_eq!(clear_frame.updates.updates[0].message, "[1]状态解除");
    }

    #[test]
    fn flush_effects_runs_nested_heal_before_older_siblings() {
        let mut builder = ExtensionRegistryBuilder::default();
        let nested_heal = builder
            .register_effect_handler("custom", "nested-heal", "custom.nested_heal", SkillPriority(0))
            .expect("handler should register");
        let marker = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(1))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 3;
        runtime.set_effect_handler(nested_heal, custom_spawns_nested_heal);
        runtime.set_effect_handler(marker, custom_marks_update);

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            nested_heal,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(4),
        )));
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            marker,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("after heal".to_owned()),
        )));

        let frame = runtime.flush_effects().expect("nested heal should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 7);
        assert_eq!(frame.updates.updates[0].message, "[1]回复体力[2]点");
        assert_eq!(frame.updates.updates[1].message, "after heal");
    }

    #[test]
    fn flush_effects_revives_dead_entity_with_capped_hp() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.world.remove_round_actor(EntityIdx(1));
        let target = runtime.entities.get_mut(EntityIdx(1)).unwrap();
        target.runtime.hp = 0;
        target.runtime.alive = false;
        runtime.effects.push(QueuedEffect::Revive {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            hp: 20,
        });

        let frame = runtime.flush_effects().expect("revive should emit update");

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.hp, 10);
        assert!(target.runtime.alive);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(frame.updates.updates[0].message, "[1][复活]了");
    }

    #[test]
    fn flush_effects_removes_entity_from_alive_set() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Remove {
            caster: EntityIdx(0),
            target: EntityIdx(1),
        });

        let frame = runtime.flush_effects().expect("remove should emit update");

        let target = runtime.entities.get(EntityIdx(1)).unwrap();
        assert_eq!(target.runtime.hp, 0);
        assert!(!target.runtime.alive);
        assert_eq!(runtime.world.round_order(), &[EntityIdx(0)]);
        assert_eq!(runtime.world.team_alive(1), Some([].as_slice()));
        assert_eq!(runtime.world.flat_alive(), &[EntityIdx(0)]);
        assert_eq!(frame.updates.updates[0].message, "[1]消失了");
    }

    #[test]
    fn flush_effects_emits_replay_effect_without_state_mutation() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(1),
            message: "custom replay".to_owned(),
            score: 7,
        });

        let frame = runtime.flush_effects().expect("replay should emit update");

        assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 10);
        assert_eq!(frame.updates.updates[0].message, "custom replay");
        assert_eq!(frame.updates.updates[0].score, 7);
    }

    #[test]
    fn flush_effects_panics_on_unknown_damage_target() {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(QueuedEffect::Damage {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            amount: 1,
        });

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(result.is_err());
    }

    fn assert_effect_panics(effect: QueuedEffect) {
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::minimal_1v1(10, 10, 3));
        runtime.effects.push(effect);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(result.is_err());
    }

    fn dummy_state_entry() -> StateEntry {
        StateEntry {
            legacy_order_key: 999,
            extension_state_id: None,
            hook_mask: ProcMask::NONE,
            priority: SkillPriority(0),
            registration_order: RegistrationOrder(0),
        }
    }

    #[test]
    fn flush_effects_panics_on_unknown_effect_entities() {
        assert_effect_panics(QueuedEffect::Damage {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Heal {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Heal {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            amount: 1,
        });
        assert_effect_panics(QueuedEffect::Spawn {
            caster: EntityIdx(99),
            template: PlayerTemplate::new(3, "ghost", 1, 1, 0),
        });
        assert_effect_panics(QueuedEffect::AddState {
            target: EntityIdx(99),
            state: dummy_state_entry(),
        });
        assert_effect_panics(QueuedEffect::ClearState {
            target: EntityIdx(99),
            legacy_order_key: 999,
        });
        assert_effect_panics(QueuedEffect::Revive {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            hp: 1,
        });
        assert_effect_panics(QueuedEffect::Revive {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            hp: 1,
        });
        assert_effect_panics(QueuedEffect::Remove {
            caster: EntityIdx(99),
            target: EntityIdx(1),
        });
        assert_effect_panics(QueuedEffect::Remove {
            caster: EntityIdx(0),
            target: EntityIdx(99),
        });
        assert_effect_panics(QueuedEffect::Replay {
            caster: EntityIdx(99),
            target: EntityIdx(1),
            message: "bad caster".to_owned(),
            score: 0,
        });
        assert_effect_panics(QueuedEffect::Replay {
            caster: EntityIdx(0),
            target: EntityIdx(99),
            message: "bad target".to_owned(),
            score: 0,
        });
    }

    #[test]
    fn flush_effects_panics_on_unknown_custom_effect_entities() {
        let mut builder = ExtensionRegistryBuilder::default();
        let handler = builder
            .register_effect_handler("custom", "mark", "custom.mark", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(handler, custom_marks_update);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(99),
            Some(EntityIdx(1)),
            CustomEffectPayload::Text("bad caster".to_owned()),
        )));

        let bad_caster = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(bad_caster.is_err());

        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            handler,
            EntityIdx(0),
            Some(EntityIdx(99)),
            CustomEffectPayload::Text("bad target".to_owned()),
        )));

        let bad_target = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| runtime.flush_effects()));

        assert!(bad_target.is_err());
    }

    #[test]
    fn custom_context_restricts_cross_entity_reads_by_capability() {
        let mut builder = ExtensionRegistryBuilder::default();
        let reader = builder
            .register_effect_handler("custom", "reader", "custom.reader", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
                PlayerTemplate::new(3, "third", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler(reader, custom_rejects_cross_entity_read);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            reader,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::None,
        )));
        let denied = runtime.flush_effects().expect("denied read handler should emit update");
        assert_eq!(denied.updates.updates[0].message, "read denied");

        runtime.set_effect_handler_with_capabilities(reader, custom_reads_cross_entity, &[ExtensionCapability::ReadEnemies]);
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            reader,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::None,
        )));
        let allowed = runtime.flush_effects().expect("allowed read handler should emit update");
        assert_eq!(allowed.updates.updates[0].message, "third");
    }

    #[test]
    fn custom_context_requires_capability_for_entity_slot_mutation() {
        let mut builder = ExtensionRegistryBuilder::default();
        let slot = builder
            .reserve_entity_slot("custom", "flag", "custom.flag")
            .expect("entity slot should reserve");
        let mutator = builder
            .register_effect_handler("custom", "mutator", "custom.mutator", SkillPriority(0))
            .expect("handler should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![
                PlayerTemplate::new(1, "left", 0, 10, 3),
                PlayerTemplate::new(2, "right", 1, 10, 3),
            ],
            registry,
        ));
        runtime.set_effect_handler_with_capabilities(
            mutator,
            custom_mutates_entity_slot,
            &[ExtensionCapability::MutateEntitySlots],
        );
        runtime.effects.push(QueuedEffect::Custom(CustomEffect::new(
            mutator,
            EntityIdx(0),
            Some(EntityIdx(1)),
            CustomEffectPayload::Int(slot.0 as i32),
        )));

        let frame = runtime.flush_effects().expect("slot mutation should emit update");

        assert_eq!(frame.updates.updates[0].message, "slot set");
        assert_eq!(
            runtime.entities.get(EntityIdx(1)).unwrap().slots.get(slot),
            Some(&SlotValue::Bool(true))
        );
    }

    #[test]
    fn runtime_dispatches_replay_renderers_in_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let late = builder
            .register_replay_renderer("custom", "late", "custom.late_replay", SkillPriority(10))
            .expect("late replay renderer should register");
        let early = builder
            .register_replay_renderer("custom", "early", "custom.early_replay", SkillPriority(1))
            .expect("early replay renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_replay_renderer(late, render_update_count_replay);
        runtime.set_replay_renderer(early, render_first_message_replay);
        let frame = RuntimeFrame::single_damage(0, 0, 3);

        let rendered = runtime.render_replay_frame(&frame);

        assert_eq!(
            rendered,
            vec![
                RenderedReplay::new(ReplayRendererId(0), "[0]攻击[1]"),
                RenderedReplay::new(ReplayRendererId(1), "1")
            ]
        );
    }

    #[test]
    fn runtime_dispatches_show_renderers_in_registry_order() {
        let mut builder = ExtensionRegistryBuilder::default();
        let show = builder
            .register_show_renderer("custom", "show", "custom.show", SkillPriority(0))
            .expect("show renderer should register");
        let registry = builder.build();
        let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
            vec![PlayerTemplate::new(1, "left", 0, 10, 3)],
            registry,
        ));
        runtime.set_show_renderer(show, render_first_message_show);
        let frame = RuntimeFrame::single_damage(0, 0, 3);

        let rendered = runtime.render_show_frame(&frame);

        assert_eq!(rendered, vec![RenderedShow::new(ShowRendererId(0), "[0]攻击[1]")]);
    }
}

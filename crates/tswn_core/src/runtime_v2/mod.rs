pub mod effect;
pub mod entity;
pub mod extension;
pub mod oracle;
pub mod scheduler;
pub mod scratch;
pub mod slot;
pub mod world;

use crate::engine::update::RunUpdates;

pub use effect::{
    CustomEffect, CustomEffectPayload, EffectContext, EffectHandlerFn, EffectHandlers, EffectQueue, QueuedEffect, RenderedReplay,
    RenderedShow, ReplayRendererFn, ReplayRenderers, RuntimeFrame, ShowRendererFn, ShowRenderers,
};
pub use entity::{EntityArena, EntityIdx, EntityRecord, PlayerRuntime, PlayerTemplate, StateEntry, StateStore};
pub use extension::{
    BattleSlotId, BattleSlotSpec, EffectHandlerId, EffectHandlerSpec, EntitySlotId, EntitySlotSpec, ExtensionCapability,
    ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder, ExtensionVersion, InstalledExtensionSpec, PlayerKindId,
    PlayerKindSpec, ProcMask, RegistrationOrder, ReplayRendererId, ReplayRendererSpec, ShowRendererId, ShowRendererSpec, SkillId,
    SkillPriority, SkillSpec, StateId, StateSpec, TargetPolicy, TemplateSlotId, TemplateSlotSpec, TswnExtension,
};
pub use oracle::{NormalizedOutcome, NormalizedUpdateFrame, StrictDiff, strict_diff};
pub use scheduler::{ActionPlan, PhaseScheduler};
pub use scratch::BattleScratch;
pub use slot::{BattleSlotStorage, EntitySlotStorage, SlotError, SlotValue, TemplateSlotStorage};
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
    pub frame: Option<RuntimeFrame>,
    pub winner_team: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatRuntime {
    pub entities: EntityArena,
    pub world: WorldArena,
    pub scheduler: PhaseScheduler,
    pub effects: EffectQueue,
    pub effect_handlers: EffectHandlers,
    pub replay_renderers: ReplayRenderers,
    pub show_renderers: ShowRenderers,
    pub scratch: BattleScratch,
    pub slots: BattleSlotStorage,
    pub registry: ExtensionRegistry,
    pub round: u64,
}

impl CombatRuntime {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let entities = EntityArena::from_templates_with_registry(template.players, &template.registry);
        let world = WorldArena::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&template.registry);
        let effect_handlers = EffectHandlers::from_registry(&template.registry);
        let replay_renderers = ReplayRenderers::from_registry(&template.registry);
        let show_renderers = ShowRenderers::from_registry(&template.registry);
        Self {
            entities,
            world,
            scheduler: PhaseScheduler,
            effects: EffectQueue::default(),
            effect_handlers,
            replay_renderers,
            show_renderers,
            scratch: BattleScratch::default(),
            slots,
            registry: template.registry,
            round: 0,
        }
    }

    pub fn set_effect_handler(&mut self, id: EffectHandlerId, handler: EffectHandlerFn) { self.effect_handlers.set(id, handler); }

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

    pub fn run_minimal_round(&mut self) -> RoundOutcome {
        if let Some(winner_team) = self.world.sync_winner(&self.entities) {
            return RoundOutcome {
                frame: None,
                winner_team: Some(winner_team),
            };
        }

        let Some(action) = self.scheduler.select_minimal_action(&mut self.world, &self.entities) else {
            return RoundOutcome {
                frame: None,
                winner_team: None,
            };
        };
        self.scratch.selected_actor_round = self.round;

        self.effects.push(QueuedEffect::Damage {
            caster: action.actor,
            target: action.target,
            amount: action.amount,
        });
        let frame = self.flush_effects();
        self.round += 1;
        let winner_team = self.world.sync_winner(&self.entities);
        RoundOutcome { frame, winner_team }
    }

    fn flush_effects(&mut self) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        while let Some(effect) = self.effects.pop_next() {
            match effect {
                QueuedEffect::Damage { caster, target, amount } => {
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        continue;
                    };
                    target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
                    if target_entity.runtime.hp == 0 {
                        target_entity.runtime.alive = false;
                    }
                    updates.add(RuntimeFrame::damage_update(caster.0 as usize, target.0 as usize, amount));
                }
                QueuedEffect::Custom(custom) => {
                    let Some(handler) = self.effect_handlers.get(custom.handler) else {
                        panic!("missing runtime_v2 effect handler implementation: {}", custom.handler.0);
                    };
                    let mut context = EffectContext {
                        entities: &mut self.entities,
                        world: &mut self.world,
                        slots: &mut self.slots,
                        queue: &mut self.effects,
                        updates: &mut updates,
                    };
                    handler(&mut context, &custom);
                }
            }
        }
        updates.had_updates().then_some(RuntimeFrame { updates })
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
        context.updates.add(crate::engine::update::RunUpdate::new(
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
        context.queue.push_nested(QueuedEffect::Damage {
            caster: effect.caster,
            target: effect.target.expect("custom test effect needs target"),
            amount,
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

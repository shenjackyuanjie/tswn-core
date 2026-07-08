pub mod effect;
pub mod entity;
pub mod extension;
pub mod oracle;
pub mod scheduler;
pub mod scratch;
pub mod slot;
pub mod world;

pub use effect::{EffectQueue, QueuedEffect, RuntimeFrame};
pub use entity::{EntityArena, EntityIdx, EntityRecord, PlayerRuntime, PlayerTemplate, StateEntry, StateStore};
pub use extension::{
    BattleSlotId, BattleSlotSpec, EntitySlotId, EntitySlotSpec, ExtensionError, ExtensionRegistry, ExtensionRegistryBuilder,
    PlayerKindId, PlayerKindSpec, ProcMask, RegistrationOrder, SkillId, SkillPriority, SkillSpec, StateId, StateSpec,
    TargetPolicy, TemplateSlotId, TemplateSlotSpec,
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
    pub scratch: BattleScratch,
    pub slots: BattleSlotStorage,
    pub round: u64,
}

impl CombatRuntime {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let entities = EntityArena::from_templates_with_registry(template.players, &template.registry);
        let world = WorldArena::from_entities(&entities);
        let slots = BattleSlotStorage::from_registry(&template.registry);
        Self {
            entities,
            world,
            scheduler: PhaseScheduler,
            effects: EffectQueue::default(),
            scratch: BattleScratch::default(),
            slots,
            round: 0,
        }
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
        let mut frame = None;
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
                    frame = Some(RuntimeFrame::single_damage(caster.0 as usize, target.0 as usize, amount));
                }
            }
        }
        frame
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
}

pub mod effect;
pub mod entity;
pub mod scratch;
pub mod world;

pub use effect::{EffectQueue, QueuedEffect, RuntimeFrame};
pub use entity::{EntityArena, EntityIdx, EntityRecord, PlayerRuntime, PlayerTemplate};
pub use scratch::BattleScratch;
pub use world::WorldArena;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedCombatTemplate {
    pub players: Vec<PlayerTemplate>,
}

impl PreparedCombatTemplate {
    pub fn new(players: Vec<PlayerTemplate>) -> Self { Self { players } }

    pub fn minimal_1v1(left_hp: i32, right_hp: i32, attack: i32) -> Self {
        Self {
            players: vec![
                PlayerTemplate::new(1, "left", 0, left_hp, attack),
                PlayerTemplate::new(2, "right", 1, right_hp, attack),
            ],
        }
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
    pub effects: EffectQueue,
    pub scratch: BattleScratch,
    pub round: u64,
}

impl CombatRuntime {
    pub fn from_template(template: PreparedCombatTemplate) -> Self {
        let entities = EntityArena::from_templates(template.players);
        let world = WorldArena::from_entities(&entities);
        Self {
            entities,
            world,
            effects: EffectQueue::default(),
            scratch: BattleScratch::default(),
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

        let Some(actor) = self.world.next_actor(&self.entities) else {
            return RoundOutcome {
                frame: None,
                winner_team: None,
            };
        };
        self.scratch.selected_actor_round = self.round;

        let Some(target) = self.world.first_alive_enemy(actor, &self.entities) else {
            let winner_team = self.world.sync_winner(&self.entities);
            return RoundOutcome {
                frame: None,
                winner_team,
            };
        };

        let amount = self.entities.get(actor).map_or(0, |entity| entity.template.attack);
        self.effects.push(QueuedEffect::Damage {
            caster: actor,
            target,
            amount,
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

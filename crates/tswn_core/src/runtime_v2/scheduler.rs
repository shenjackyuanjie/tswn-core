use crate::runtime_v2::entity::{EntityArena, EntityIdx};
use crate::runtime_v2::world::WorldArena;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionPlan {
    pub actor: EntityIdx,
    pub target: EntityIdx,
    pub amount: i32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PhaseScheduler;

impl PhaseScheduler {
    pub fn select_minimal_action(&mut self, world: &mut WorldArena, entities: &EntityArena) -> Option<ActionPlan> {
        let actor = world.next_actor(entities)?;
        let target = world.first_alive_enemy(actor, entities)?;
        let amount = entities.get(actor).map_or(0, |entity| entity.template.attack);
        Some(ActionPlan { actor, target, amount })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::{PlayerTemplate, PreparedCombatTemplate};

    #[test]
    fn scheduler_selects_next_actor_first_alive_enemy_and_amount() {
        let entities = EntityArena::from_templates(PreparedCombatTemplate::minimal_1v1(10, 10, 4).players);
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler;

        assert_eq!(
            scheduler.select_minimal_action(&mut world, &entities),
            Some(ActionPlan {
                actor: EntityIdx(0),
                target: EntityIdx(1),
                amount: 4,
            })
        );
    }

    #[test]
    fn scheduler_skips_dead_actor_in_round_order() {
        let mut entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "dead-left", 0, 10, 4),
            PlayerTemplate::new(2, "right", 1, 10, 4),
            PlayerTemplate::new(3, "alive-left", 0, 10, 4),
        ]);
        entities.get_mut(EntityIdx(0)).unwrap().runtime.alive = false;
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler;

        assert_eq!(
            scheduler.select_minimal_action(&mut world, &entities),
            Some(ActionPlan {
                actor: EntityIdx(1),
                target: EntityIdx(2),
                amount: 4,
            })
        );
    }

    #[test]
    fn scheduler_returns_none_without_alive_enemy() {
        let mut entities = EntityArena::from_templates(PreparedCombatTemplate::minimal_1v1(10, 10, 4).players);
        entities.get_mut(EntityIdx(1)).unwrap().runtime.alive = false;
        let mut world = WorldArena::from_entities(&entities);
        let mut scheduler = PhaseScheduler;

        assert_eq!(scheduler.select_minimal_action(&mut world, &entities), None);
    }
}

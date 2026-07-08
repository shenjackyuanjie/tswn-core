use crate::runtime_v2::entity::{EntityArena, EntityIdx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldArena {
    round_order: Vec<EntityIdx>,
    cursor: usize,
    winner_team: Option<usize>,
}

impl WorldArena {
    pub fn from_entities(entities: &EntityArena) -> Self {
        let round_order = entities.iter().map(|(idx, _)| idx).collect();
        Self {
            round_order,
            cursor: 0,
            winner_team: None,
        }
    }

    pub fn next_actor(&mut self, entities: &EntityArena) -> Option<EntityIdx> {
        if self.round_order.is_empty() {
            return None;
        }

        for _ in 0..self.round_order.len() {
            let actor = self.round_order[self.cursor];
            self.cursor = (self.cursor + 1) % self.round_order.len();
            if entities.get(actor).is_some_and(|entity| entity.runtime.alive) {
                return Some(actor);
            }
        }
        None
    }

    pub fn first_alive_enemy(&self, actor: EntityIdx, entities: &EntityArena) -> Option<EntityIdx> {
        let actor_team = entities.get(actor)?.template.team;
        entities
            .iter()
            .find(|(_, entity)| entity.runtime.alive && entity.template.team != actor_team)
            .map(|(idx, _)| idx)
    }

    pub fn append_round_actor(&mut self, actor: EntityIdx) { self.round_order.push(actor); }

    pub fn round_order(&self) -> &[EntityIdx] { &self.round_order }

    pub fn sync_winner(&mut self, entities: &EntityArena) -> Option<usize> {
        let mut alive_team = None;
        for (_, entity) in entities.iter() {
            if !entity.runtime.alive {
                continue;
            }
            match alive_team {
                None => alive_team = Some(entity.template.team),
                Some(team) if team == entity.template.team => {}
                Some(_) => {
                    self.winner_team = None;
                    return None;
                }
            }
        }
        self.winner_team = alive_team;
        self.winner_team
    }

    pub fn winner_team(&self) -> Option<usize> { self.winner_team }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_v2::{EntityArena, PlayerTemplate};

    #[test]
    fn world_appends_spawned_actor_to_round_order() {
        let entities = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3)]);
        let mut world = WorldArena::from_entities(&entities);

        world.append_round_actor(EntityIdx(1));

        assert_eq!(world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
    }
}

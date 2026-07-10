use crate::runtime_v2::entity::{EntityArena, EntityIdx};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldArena {
    round_order: Vec<EntityIdx>,
    team_roster: Vec<Vec<EntityIdx>>,
    team_alive: Vec<Vec<EntityIdx>>,
    flat_alive: Vec<EntityIdx>,
    alive_group_count: usize,
    round_pos: i32,
    winner_team: Option<usize>,
}

impl WorldArena {
    pub fn from_entities(entities: &EntityArena) -> Self {
        let round_order = entities.iter().map(|(idx, _)| idx).collect();
        let team_count = entities.iter().map(|(_, entity)| entity.runtime.team).max().map_or(0, |team| team + 1);
        let mut team_roster = vec![Vec::new(); team_count];
        let mut team_alive = vec![Vec::new(); team_count];
        let mut flat_alive = Vec::new();
        for (idx, entity) in entities.iter() {
            team_roster[entity.runtime.team].push(idx);
            if !entity.runtime.alive {
                continue;
            }
            team_alive[entity.runtime.team].push(idx);
            flat_alive.push(idx);
        }
        let alive_group_count = team_alive.iter().filter(|team| !team.is_empty()).count();
        Self {
            round_order,
            team_roster,
            team_alive,
            flat_alive,
            alive_group_count,
            round_pos: -1,
            winner_team: None,
        }
    }

    pub fn sync_initial_views(
        &mut self,
        entities: &EntityArena,
        round_order: Vec<EntityIdx>,
        team_roster: Vec<Vec<EntityIdx>>,
        team_alive: Vec<Vec<EntityIdx>>,
        flat_alive: Vec<EntityIdx>,
    ) {
        debug_assert!(round_order.iter().all(|idx| entities.get(*idx).is_some()));
        debug_assert!(team_roster.iter().enumerate().all(|(team, roster)| {
            roster
                .iter()
                .all(|idx| entities.get(*idx).is_some_and(|entity| entity.runtime.team == team))
        }));
        debug_assert!(flat_alive.iter().all(|idx| entities.get(*idx).is_some_and(|entity| entity.runtime.alive)));
        debug_assert!(team_alive.iter().enumerate().all(|(team, alive)| alive.iter().all(|idx| {
            entities
                .get(*idx)
                .is_some_and(|entity| entity.runtime.alive && entity.runtime.team == team)
        })));

        self.round_order = round_order;
        self.team_roster = team_roster;
        self.team_alive = team_alive;
        self.flat_alive = flat_alive;
        self.alive_group_count = self.team_alive.iter().filter(|team| !team.is_empty()).count();
        self.round_pos = -1;
    }

    pub fn next_actor(&mut self, entities: &EntityArena) -> Option<EntityIdx> {
        if self.round_order.is_empty() {
            return None;
        }

        for _ in 0..self.round_order.len() {
            self.round_pos = (self.round_pos + 1).rem_euclid(self.round_order.len() as i32);
            let actor = self.round_order[self.round_pos as usize];
            if entities.get(actor).is_some_and(|entity| entity.runtime.alive) {
                #[cfg(not(feature = "no_debug"))]
                if std::env::var_os("TSWN_DEBUG_TICK_ORDER").is_some() {
                    eprintln!(
                        "[v2_tick_order] round_pos={} actor={} order={:?}",
                        self.round_pos,
                        actor.0,
                        self.round_order.iter().map(|idx| idx.0).collect::<Vec<_>>()
                    );
                }
                return Some(actor);
            }
        }
        None
    }

    pub fn first_alive_enemy(&self, actor: EntityIdx, entities: &EntityArena) -> Option<EntityIdx> {
        let actor_team = entities.get(actor)?.runtime.team;
        self.flat_alive
            .iter()
            .copied()
            .find(|idx| entities.get(*idx).is_some_and(|entity| entity.runtime.team != actor_team))
    }

    pub fn append_round_actor(&mut self, actor: EntityIdx) { self.round_order.push(actor); }

    pub fn add_spawned_alive(&mut self, actor: EntityIdx, team: usize) {
        if self.team_roster.len() <= team {
            self.team_roster.resize_with(team + 1, Vec::new);
        }
        if !self.team_roster[team].contains(&actor) {
            self.team_roster[team].push(actor);
        }
        self.append_round_actor(actor);
        self.revive_alive(actor, team);
    }

    pub fn remove_round_actor(&mut self, actor: EntityIdx) -> bool {
        let Some(pos) = self.round_order.iter().position(|idx| *idx == actor) else {
            return false;
        };
        #[cfg(not(feature = "no_debug"))]
        let debug_order = std::env::var_os("TSWN_DEBUG_TICK_ORDER").is_some();
        #[cfg(not(feature = "no_debug"))]
        if debug_order {
            eprintln!(
                "[v2_round_remove:before] actor={} idx={} round_pos={} order={:?}",
                actor.0,
                pos,
                self.round_pos,
                self.round_order.iter().map(|idx| idx.0).collect::<Vec<_>>()
            );
        }
        if pos as i32 >= self.round_pos {
            self.round_pos -= 1;
        }
        self.round_order.remove(pos);
        if self.round_order.is_empty() {
            self.round_pos = -1;
        }
        #[cfg(not(feature = "no_debug"))]
        if debug_order {
            eprintln!(
                "[v2_round_remove:after] actor={} round_pos={} order={:?}",
                actor.0,
                self.round_pos,
                self.round_order.iter().map(|idx| idx.0).collect::<Vec<_>>()
            );
        }
        true
    }

    pub fn revive_round_actor(&mut self, actor: EntityIdx) -> bool {
        if self.round_order.contains(&actor) {
            return false;
        }
        self.round_order.push(actor);
        true
    }

    pub fn round_order(&self) -> &[EntityIdx] { &self.round_order }

    pub fn team_roster(&self, team: usize) -> Option<&[EntityIdx]> { self.team_roster.get(team).map(Vec::as_slice) }

    pub fn team_alive(&self, team: usize) -> Option<&[EntityIdx]> { self.team_alive.get(team).map(Vec::as_slice) }

    pub fn flat_alive(&self) -> &[EntityIdx] { &self.flat_alive }

    pub fn alive_group_count(&self) -> usize { self.alive_group_count }

    pub fn alive_group_len_containing(&self, actor: EntityIdx) -> usize {
        let Some(team) = self.team_alive.iter().find(|team| team.contains(&actor)) else {
            return 0;
        };
        team.len()
    }

    pub fn revive_alive(&mut self, actor: EntityIdx, team: usize) {
        if self.flat_alive.contains(&actor) {
            return;
        }
        let new_team = self.team_alive.len() <= team;
        if self.team_alive.len() <= team {
            self.team_alive.resize_with(team + 1, Vec::new);
        }
        let last_teammate_pos = self.team_alive[team]
            .iter()
            .rev()
            .find_map(|idx| self.flat_alive.iter().position(|alive| alive == idx));
        self.team_alive[team].push(actor);
        if let Some(pos) = last_teammate_pos {
            self.flat_alive.insert(pos + 1, actor);
        } else {
            self.flat_alive.push(actor);
        }
        if new_team {
            self.alive_group_count += 1;
        }
    }

    pub fn remove_alive(&mut self, actor: EntityIdx, team: usize) -> bool {
        let was_flat_alive = if let Some(pos) = self.flat_alive.iter().position(|idx| *idx == actor) {
            self.flat_alive.remove(pos);
            true
        } else {
            false
        };
        if let Some(team_alive) = self.team_alive.get_mut(team) {
            let was_team_alive = team_alive.contains(&actor);
            team_alive.retain(|idx| *idx != actor);
            if was_team_alive && team_alive.is_empty() {
                self.alive_group_count = self.alive_group_count.saturating_sub(1);
            }
        }
        was_flat_alive
    }

    pub fn mark_dead(&mut self, actor: EntityIdx, team: usize) -> bool {
        let was_alive = self.remove_alive(actor, team);
        self.remove_round_actor(actor);
        was_alive
    }

    pub fn sync_winner(&mut self, entities: &EntityArena) -> Option<usize> {
        let mut alive_team = None;
        for (_, entity) in entities.iter() {
            if !entity.runtime.alive {
                continue;
            }
            match alive_team {
                None => alive_team = Some(entity.runtime.team),
                Some(team) if team == entity.runtime.team => {}
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
    fn world_initializes_alive_views_in_team_order() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
            PlayerTemplate::new(3, "ally", 0, 10, 3),
        ]);
        let world = WorldArena::from_entities(&entities);

        assert_eq!(world.round_order(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(world.team_roster(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
        assert_eq!(world.team_roster(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
        assert_eq!(world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(world.flat_alive(), &[EntityIdx(0), EntityIdx(1), EntityIdx(2)]);
        assert_eq!(world.alive_group_count(), 2);
    }

    #[test]
    fn world_reports_alive_group_len_containing_entity() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
            PlayerTemplate::new(3, "ally", 0, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert_eq!(world.alive_group_len_containing(EntityIdx(0)), 2);
        assert_eq!(world.alive_group_len_containing(EntityIdx(1)), 1);
        assert_eq!(world.alive_group_len_containing(EntityIdx(99)), 0);

        assert!(world.remove_alive(EntityIdx(2), 0));

        assert_eq!(world.alive_group_len_containing(EntityIdx(0)), 1);
        assert_eq!(world.alive_group_len_containing(EntityIdx(2)), 0);
    }

    #[test]
    fn world_sync_initial_views_replaces_seed_sorted_orders() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 1, 10, 3),
            PlayerTemplate::new(2, "right", 0, 10, 3),
            PlayerTemplate::new(3, "ally", 1, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        world.sync_initial_views(
            &entities,
            vec![EntityIdx(2), EntityIdx(0), EntityIdx(1)],
            vec![vec![EntityIdx(1)], vec![EntityIdx(2), EntityIdx(0)]],
            vec![vec![EntityIdx(1)], vec![EntityIdx(2), EntityIdx(0)]],
            vec![EntityIdx(1), EntityIdx(2), EntityIdx(0)],
        );

        assert_eq!(world.round_order(), &[EntityIdx(2), EntityIdx(0), EntityIdx(1)]);
        assert_eq!(world.team_roster(0), Some([EntityIdx(1)].as_slice()));
        assert_eq!(world.team_roster(1), Some([EntityIdx(2), EntityIdx(0)].as_slice()));
        assert_eq!(world.team_alive(0), Some([EntityIdx(1)].as_slice()));
        assert_eq!(world.team_alive(1), Some([EntityIdx(2), EntityIdx(0)].as_slice()));
        assert_eq!(world.flat_alive(), &[EntityIdx(1), EntityIdx(2), EntityIdx(0)]);
        assert_eq!(world.alive_group_count(), 2);
    }

    #[test]
    fn world_appends_spawned_actor_to_round_and_alive_views() {
        let entities = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3)]);
        let mut world = WorldArena::from_entities(&entities);

        world.add_spawned_alive(EntityIdx(1), 0);

        assert_eq!(world.round_order(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(world.team_roster(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
        assert_eq!(world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
        assert_eq!(world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        assert_eq!(world.alive_group_count(), 1);
    }

    #[test]
    fn world_removes_round_actor_and_keeps_cursor_on_next_actor() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "middle", 1, 10, 3),
            PlayerTemplate::new(3, "right", 2, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert_eq!(world.next_actor(&entities), Some(EntityIdx(0)));
        assert!(world.remove_round_actor(EntityIdx(0)));
        assert_eq!(world.round_order(), &[EntityIdx(1), EntityIdx(2)]);
        assert_eq!(world.next_actor(&entities), Some(EntityIdx(1)));
    }

    #[test]
    fn world_revives_round_actor_by_entity_order_without_duplicates() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "middle", 1, 10, 3),
            PlayerTemplate::new(3, "right", 2, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert!(world.remove_round_actor(EntityIdx(1)));
        assert_eq!(world.round_order(), &[EntityIdx(0), EntityIdx(2)]);
        assert!(world.revive_round_actor(EntityIdx(1)));
        assert!(!world.revive_round_actor(EntityIdx(1)));
        assert_eq!(world.round_order(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
    }

    #[test]
    fn world_removing_future_actor_matches_legacy_round_pos_adjustment() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "middle", 1, 10, 3),
            PlayerTemplate::new(3, "right", 2, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert_eq!(world.next_actor(&entities), Some(EntityIdx(0)));
        assert!(world.remove_round_actor(EntityIdx(1)));
        assert_eq!(world.round_order(), &[EntityIdx(0), EntityIdx(2)]);
        assert_eq!(world.next_actor(&entities), Some(EntityIdx(0)));
    }

    #[test]
    fn world_revives_alive_after_last_team_member_in_flat_order() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "enemy", 1, 10, 3),
            PlayerTemplate::new(3, "ally", 0, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert!(world.remove_alive(EntityIdx(2), 0));
        assert_eq!(world.flat_alive(), &[EntityIdx(0), EntityIdx(1)]);
        world.revive_alive(EntityIdx(2), 0);

        assert_eq!(world.team_alive(0), Some([EntityIdx(0), EntityIdx(2)].as_slice()));
        assert_eq!(world.flat_alive(), &[EntityIdx(0), EntityIdx(2), EntityIdx(1)]);
        assert_eq!(world.alive_group_count(), 2);
    }

    #[test]
    fn world_remove_alive_updates_team_flat_and_group_count() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert!(world.remove_alive(EntityIdx(1), 1));

        assert_eq!(world.team_roster(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(world.team_alive(1), Some([].as_slice()));
        assert_eq!(world.flat_alive(), &[EntityIdx(0)]);
        assert_eq!(world.alive_group_count(), 1);
    }

    #[test]
    fn world_revive_does_not_restore_legacy_alive_group_count() {
        let entities = EntityArena::from_templates(vec![
            PlayerTemplate::new(1, "left", 0, 10, 3),
            PlayerTemplate::new(2, "right", 1, 10, 3),
        ]);
        let mut world = WorldArena::from_entities(&entities);

        assert!(world.remove_alive(EntityIdx(1), 1));
        world.revive_alive(EntityIdx(1), 1);

        assert_eq!(world.team_alive(1), Some([EntityIdx(1)].as_slice()));
        assert_eq!(world.alive_group_count(), 1);
    }
}

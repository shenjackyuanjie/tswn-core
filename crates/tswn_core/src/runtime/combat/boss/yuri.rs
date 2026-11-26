//! 尤里（yuri）Boss：心灵控制（魅惑）的目标选择与层数叠加。
use super::super::*;
use crate::runtime::lang::keys;

impl CombatRuntime {
    /// 尤里的心灵控制目标：不看 smart，只要目标不在自身当前队伍、没有魅惑。
    pub fn select_plain_yuri_control_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let target_pool = Self::quilt_target_pool(&self.entities, &self.world);
        let all_alive = target_pool.as_ref();
        let mut candidates = smallvec::SmallVec::<[EntityIdx; 8]>::new();
        let mut enemy_skip_indices = smallvec::SmallVec::<[usize; 8]>::new();
        for (idx, target) in all_alive.iter().copied().enumerate() {
            if self.entities.get(target).is_some_and(|entity| entity.runtime.team == actor_team) {
                enemy_skip_indices.push(idx);
            } else {
                candidates.push(target);
            }
        }
        if candidates.is_empty() {
            return PreparedTargetList::new();
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            let valid = target != actor
                && self
                    .entities
                    .get(target)
                    .is_some_and(|entity| entity.states.entry(PLAIN_CHARM_STATE_KEY).is_none());
            if !valid {
                invalid += 1;
                continue;
            }
            if selected.contains(&target) {
                dup += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_charm_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    /// `SklYuriControl`：不做免疫与闪避判定，层数取目标队伍初始人数下限 3。
    pub fn drain_plain_yuri_control_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(RunUpdate::new_lang(
            keys::SKL_YURI_CONTROL,
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let step = self.plain_group_initial_player_count(target).max(3) as i32;
        let caster_effective_team_idx = self.plain_effective_team(actor);
        let existing = self
            .entities
            .get(target)
            .and_then(|entity| entity.states.entry(PLAIN_CHARM_STATE_KEY))
            .and_then(StateEntry::charm_value);
        #[cfg(not(feature = "no_debug"))]
        if crate::debug::probe_charm().is_some() {
            eprintln!(
                "[charm_probe:runtime:yuri_control] actor={} target={} source={caster_effective_team_idx} existing={existing:?}",
                actor.0, target.0,
            );
        }
        if let Some((_, effective_team_idx, _, state_target, old_step)) = existing {
            // 心灵控制的重施增加整段步数，但同普通魅惑一样等待目标重算有效队伍。
            let effective_team_idx = effective_team_idx.or_else(|| Some(self.plain_effective_team(target)));
            assert!(
                self.entities.get_mut(target).unwrap().states.set_payload(
                    PLAIN_CHARM_STATE_KEY,
                    StatePayload::Charm {
                        group_id: actor.0 as usize,
                        effective_team_idx,
                        source_team_idx: Some(caster_effective_team_idx),
                        target: state_target,
                        step: old_step + step,
                    },
                ),
                "runtime yuri charm state disappeared during recharm"
            );
        } else {
            let charm_state_id = self
                .registry
                .state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT)
                .expect("default runtime profile must register core charm state");
            let charm_priority = self
                .registry
                .state(charm_state_id)
                .expect("default runtime core charm state disappeared")
                .priority;
            assert!(
                self.entities.get_mut(target).unwrap().states.add_entry(StateEntry::charm(
                    PLAIN_CHARM_STATE_KEY,
                    charm_state_id,
                    actor.0 as usize,
                    Some(caster_effective_team_idx),
                    Some(caster_effective_team_idx),
                    Some(target.0),
                    step,
                    charm_priority,
                )),
                "runtime yuri charm state should be inserted"
            );
            self.entities.get_mut(target).unwrap().refresh_runtime_stats_from_template();
        }
        updates.add(RunUpdate::new_lang(
            keys::SKL_CHARM_HIT,
            actor.0 as usize,
            target.0 as usize,
            120,
        ));
    }

    /// 目标所在队伍的初始成员数，对应 0.5.5 `group.initPlayers.length`。
    ///
    /// arena 里的初始成员都指向自己；分身、召唤与僵尸的 owner 指向召唤者，不会被计入。
    fn plain_group_initial_player_count(&self, target: EntityIdx) -> usize {
        let Some(entity) = self.entities.get(target) else {
            return 0;
        };
        let team = entity.runtime.team;
        self.entities
            .iter()
            .filter(|(idx, entity)| entity.runtime.team == team && entity.runtime.owner == *idx)
            .count()
    }
}

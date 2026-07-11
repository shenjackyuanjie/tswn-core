use super::*;

impl CombatRuntime {
    pub fn plain_effective_team(&self, actor: EntityIdx) -> usize {
        let actor_entity = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 effective-team actor: {}", actor.0));
        actor_entity
            .states
            .entries()
            .iter()
            .find_map(|entry| match entry.payload {
                StatePayload::Charm {
                    group_id,
                    effective_team_idx,
                    ..
                } => effective_team_idx.or_else(|| {
                    u32::try_from(group_id)
                        .ok()
                        .and_then(|group_entity| self.entities.get(EntityIdx(group_entity)))
                        .map(|entity| entity.runtime.team)
                }),
                _ => None,
            })
            .unwrap_or(actor_entity.runtime.team)
    }

    pub fn select_plain_charm_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        #[cfg(not(feature = "no_debug"))]
        let before = (self.rng.i, self.rng.j);
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        let mut candidates = Vec::new();
        let mut enemy_skip_indices = Vec::new();
        for (idx, target) in all_alive.iter().copied().enumerate() {
            if self.entities.get(target).is_some_and(|entity| entity.runtime.team == actor_team) {
                enemy_skip_indices.push(idx);
            } else {
                candidates.push(target);
            }
        }
        if candidates.is_empty() {
            return Vec::new();
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            let valid = !smart
                || self
                    .entities
                    .get(target)
                    .and_then(|entity| entity.states.entry(76))
                    .and_then(StateEntry::charm_value)
                    .is_none_or(|(_, _, _, _, step)| step <= 1);
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
        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_charm_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        let targets = scored.into_iter().map(|(target, _)| target).collect::<Vec<_>>();
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:select] actor={} smart={} candidates={:?} targets={:?} rc4=({},{}) -> ({},{})",
                actor.0,
                smart,
                candidates.iter().map(|target| target.0).collect::<Vec<_>>(),
                targets.iter().map(|target| target.0).collect::<Vec<_>>(),
                before.0,
                before.1,
                self.rng.i,
                self.rng.j,
            );
        }
        targets
    }

    pub fn score_plain_charm_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 charm target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        let mut score = if smart {
            if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity.states.entry(76).and_then(StateEntry::charm_value).is_some()
            || entity
                .states
                .entries()
                .iter()
                .any(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
        {
            score /= 2.0;
        }
        score
    }

    pub fn drain_plain_charm_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:act_before] actor={} target={} rc4=({},{})",
                actor.0, target.0, self.rng.i, self.rng.j,
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[魅惑]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let (owner_magic, charge_active, caster_effective_team_idx) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 charm actor: {}", actor.0));
            (
                owner.runtime.magic,
                owner.runtime.at_boost_millionths >= 3_000_000,
                self.plain_effective_team(actor),
            )
        };
        let (target_flags, target_name, target_dodge, target_active) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 charm target: {}", target.0));
            (
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.runtime.agility + target_entity.runtime.resistance,
                target_entity.is_active(),
            )
        };
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "charm");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        if immune || (target_active && PlayerRuntime::dodge(owner_magic, target_dodge, &mut self.rng)) {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            #[cfg(not(feature = "no_debug"))]
            if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
                eprintln!(
                    "[charm_probe:v2:act_after] actor={} target={} dodged=true rc4=({},{})",
                    actor.0, target.0, self.rng.i, self.rng.j,
                );
            }
            return;
        }

        let existing = self
            .entities
            .get(target)
            .and_then(|entity| entity.states.entry(76))
            .and_then(StateEntry::charm_value);
        if let Some((mut group_id, effective_team_idx, mut source_team_idx, state_target, mut step)) = existing {
            let existing_team_idx = source_team_idx.or_else(|| {
                u32::try_from(group_id)
                    .ok()
                    .and_then(|group_entity| self.entities.get(EntityIdx(group_entity)))
                    .map(|entity| entity.runtime.team)
            });
            if existing_team_idx == Some(caster_effective_team_idx) {
                step += 1;
            } else {
                group_id = actor.0 as usize;
                source_team_idx = Some(caster_effective_team_idx);
            }
            if charge_active {
                step += 3;
            }
            assert!(
                self.entities.get_mut(target).unwrap().states.set_payload(
                    76,
                    StatePayload::Charm {
                        group_id,
                        effective_team_idx,
                        source_team_idx,
                        target: state_target,
                        step,
                    },
                ),
                "runtime_v2 charm state disappeared during recharm"
            );
        } else {
            let charm_state_id = self
                .registry
                .state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT)
                .expect("default runtime v2 profile must register core charm state");
            let charm_priority = self
                .registry
                .state(charm_state_id)
                .expect("default runtime v2 core charm state disappeared")
                .priority;
            assert!(
                self.entities.get_mut(target).unwrap().states.add_entry(StateEntry::charm(
                    76,
                    charm_state_id,
                    actor.0 as usize,
                    Some(caster_effective_team_idx),
                    Some(caster_effective_team_idx),
                    Some(target.0),
                    if charge_active { 4 } else { 1 },
                    charm_priority,
                )),
                "runtime_v2 charm state should be inserted"
            );
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]被[魅惑]了",
            actor.0 as usize,
            target.0 as usize,
            120,
        ));
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_CHARM").is_some() {
            eprintln!(
                "[charm_probe:v2:act_after] actor={} target={} dodged=false rc4=({},{})",
                actor.0, target.0, self.rng.i, self.rng.j,
            );
        }
    }

    pub fn select_plain_heal_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let candidates = self
            .world
            .team_roster(actor_team)
            .unwrap_or_default()
            .iter()
            .copied()
            .filter(|target| self.entities.get(*target).is_some_and(|entity| entity.runtime.alive))
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Vec::new();
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(picked) = self.rng.pick(&candidates) else {
                return Vec::new();
            };
            let target = candidates[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                if smart {
                    entity.runtime.hp + 80 < entity.template.max_hp
                } else {
                    entity.runtime.hp < entity.template.max_hp
                }
            });
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

        let mut scored = selected
            .into_iter()
            .map(|target| (target, self.score_plain_heal_target(target, smart)))
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_heal_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if !smart {
            return self.rng.rFFFF() as f64;
        }
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 heal target: {}", target.0));
        let negative_state_count = entity
            .states
            .entries()
            .iter()
            .filter(|entry| {
                matches!(
                    entry.payload,
                    StatePayload::FireMagHalfSteps(_)
                        | StatePayload::Ice { .. }
                        | StatePayload::Curse { .. }
                        | StatePayload::Poison { .. }
                        | StatePayload::Berserk { .. }
                        | StatePayload::Charm { .. }
                        | StatePayload::Slow { .. }
                )
            })
            .count() as i32;
        let damaged = (entity.template.max_hp - entity.runtime.hp).max(0) + negative_state_count * 64;
        damaged as f64 * entity.runtime.attr_sum.max(1) as f64
    }

    pub fn drain_plain_heal_skill_into(
        &mut self,
        actor: EntityIdx,
        fixed_lane: usize,
        target: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 heal level missing for fixed lane {fixed_lane}"));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 heal actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        let missing_hp = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 heal target: {}", target.0));
            (target_entity.template.max_hp - target_entity.runtime.hp).max(0)
        };
        if missing_hp <= 0 {
            return;
        }
        let heal = ((atp / 60.0).ceil() as i32).clamp(1, missing_hp);
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[治愈魔法]",
            actor.0 as usize,
            target.0 as usize,
            heal as u32,
        ));

        let (had_berserk, had_charm, had_curse, had_ice, had_poison, had_slow) = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 heal target: {}", target.0));
            target_entity.runtime.hp = (target_entity.runtime.hp + heal).min(target_entity.template.max_hp);

            let mut had_berserk = false;
            let mut had_charm = false;
            let mut had_curse = false;
            let mut had_ice = false;
            let mut had_poison = false;
            let mut had_slow = false;
            let negative_keys = target_entity
                .states
                .entries()
                .iter()
                .filter_map(|entry| {
                    let negative = match entry.payload {
                        StatePayload::FireMagHalfSteps(_) => true,
                        StatePayload::Ice { .. } => {
                            had_ice = true;
                            true
                        }
                        StatePayload::Curse { .. } => {
                            had_curse = true;
                            true
                        }
                        StatePayload::Poison { .. } => {
                            had_poison = true;
                            true
                        }
                        StatePayload::Berserk { .. } => {
                            had_berserk = true;
                            true
                        }
                        StatePayload::Charm { .. } => {
                            had_charm = true;
                            true
                        }
                        StatePayload::Slow { .. } => {
                            had_slow = true;
                            true
                        }
                        _ => false,
                    };
                    negative.then_some(entry.legacy_order_key)
                })
                .collect::<Vec<_>>();
            for legacy_order_key in negative_keys {
                assert!(
                    target_entity.states.clear_legacy_key(legacy_order_key),
                    "runtime_v2 negative state disappeared during heal"
                );
            }

            if had_curse || had_ice || had_charm || had_slow {
                target_entity.runtime.atk_sum = target_entity.template.atk_sum;
                target_entity.runtime.speed = target_entity.template.speed;
            }
            (had_berserk, had_charm, had_curse, had_ice, had_poison, had_slow)
        };

        let mut recover_update =
            crate::engine::update::RunUpdate::new("[1]回复体力[2]点", actor.0 as usize, target.0 as usize, 0);
        recover_update.param = Some(heal as u32);
        updates.add(recover_update);

        for (had_state, message) in [
            (had_berserk, "[1]从[狂暴]中解除"),
            (had_charm, "[1]从[魅惑]中解除"),
            (had_curse, "[1]从[诅咒]中解除"),
            (had_ice, "[1]从[冰冻]中解除"),
            (had_poison, "[1]从[中毒]中解除"),
            (had_slow, "[1]从[迟缓]中解除"),
        ] {
            if had_state {
                updates.add_newline();
                updates.add(RuntimeFrame::replay_update(actor.0 as usize, target.0 as usize, message, 0));
            }
        }

        let next_level = if current_level > 8 { current_level - 1 } else { current_level };
        assert!(
            self.entities.get_mut(actor).unwrap().template.skills.set_level_at(fixed_lane, next_level),
            "runtime_v2 heal fixed lane disappeared during action"
        );
    }

    pub fn select_plain_disperse_targets(&mut self, actor: EntityIdx, smart: bool) -> Vec<EntityIdx> {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return Vec::new();
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, target)| {
                self.entities
                    .get(*target)
                    .is_some_and(|entity| entity.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return Vec::new();
            };
            let target = all_alive[picked];
            if self.entities.get(target).is_none() {
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
        let mut scored = selected
            .into_iter()
            .map(|target| {
                (
                    target,
                    score_disperse_target(&self.entities, &self.world, target, smart, &mut self.rng),
                )
            })
            .collect::<Vec<_>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn drain_plain_disperse_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.effects.push(QueuedEffect::DisperseAttack { caster: actor, target });
        self.drain_effects_into(updates);
    }
}

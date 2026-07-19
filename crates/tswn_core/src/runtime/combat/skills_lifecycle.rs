use super::*;

impl CombatRuntime {
    pub fn select_plain_revive_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let candidates = self.world.team_roster(actor_team).unwrap_or_default();
        if candidates.is_empty() {
            return PreparedTargetList::new();
        }

        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut dup = 0usize;
        let mut invalid = -(select_count as i32);
        while dup <= select_count && invalid <= select_count as i32 {
            let Some(picked) = self.rng.pick(candidates) else {
                return PreparedTargetList::new();
            };
            let target = candidates[picked];
            let valid = self.entities.get(target).is_some_and(|entity| {
                !entity.runtime.alive && !entity.runtime.is_combat_minion() && entity.runtime.corpse == RuntimeCorpseKind::None
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

        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_revive_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_revive_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        if smart {
            self.entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime revive target: {}", target.0))
                .runtime
                .attr_sum as f64
        } else {
            self.rng.rFFFF() as f64
        }
    }

    pub fn drain_plain_revive_skill_into(
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
            .unwrap_or_else(|| panic!("runtime revive level missing for fixed lane {fixed_lane}"));
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime revive actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng);
        let max_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime revive target: {}", target.0))
            .template
            .max_hp;
        let heal = ((atp / 75.0).ceil() as i32).clamp(1, max_hp.max(1));

        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[苏生术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));

        let team = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime revive target: {}", target.0));
            if target_entity.runtime.alive {
                return;
            }
            target_entity.runtime.hp = heal;
            target_entity.runtime.alive = true;
            target_entity.runtime.team
        };
        self.world.revive_round_actor(target);
        self.world.revive_alive(target, team);

        updates.add(crate::runtime::update::RunUpdate::new(
            "[1][复活]了",
            actor.0 as usize,
            target.0 as usize,
            (heal + 60) as u32,
        ));
        let mut recover_update =
            crate::runtime::update::RunUpdate::new("[1]回复体力[2]点", actor.0 as usize, target.0 as usize, 0);
        recover_update.param = Some(heal as u32);
        updates.add(recover_update);

        assert!(
            self.entities
                .get_mut(actor)
                .unwrap()
                .template
                .skills
                .set_level_at(fixed_lane, (current_level + 1) >> 1),
            "runtime revive fixed lane disappeared during action"
        );
    }

    pub fn select_plain_slow_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        if all_alive.is_empty() {
            return PreparedTargetList::new();
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
            .collect::<smallvec::SmallVec<[usize; 8]>>();
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
            let valid = self.entities.get(target).is_some_and(|entity| {
                !smart
                    || (entity.runtime.hp >= 80
                        && entity.states.entry(78).and_then(StateEntry::slow_value).is_none_or(|step| step <= 1))
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
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_slow_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_slow_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime slow target: {}", target.0));
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
        if entity.states.entry(78).and_then(StateEntry::slow_value).is_some() {
            score /= 2.0;
        }
        score
    }

    pub fn drain_plain_slow_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[减速术]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));
        let (owner_magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime slow actor: {}", actor.0));
            (owner.runtime.magic, owner.runtime.at_boost_millionths >= 3_000_000)
        };
        let (target_flags, target_name, target_resistance, target_active) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime slow target: {}", target.0));
            (
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.runtime.resistance,
                target_entity.is_active(),
            )
        };
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::namerena::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::namerena::boss_immune_threshold(&target_name, "slow");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        if immune || (target_active && PlayerRuntime::dodge(owner_magic, target_resistance, &mut self.rng)) {
            updates.add(crate::runtime::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        let slow_state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_SLOW_STATE_EXPORT)
            .expect("default runtime profile must register core slow state");
        let slow_priority = self
            .registry
            .state(slow_state_id)
            .expect("default runtime core slow state disappeared")
            .priority;
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime slow target: {}", target.0));
        let reduce_move_point = target_entity.effective_speed() + 64;
        target_entity.runtime.move_state.speed_points -= reduce_move_point;
        let next_step = target_entity.states.entry(78).and_then(StateEntry::slow_value).map_or(2, |step| step + 2)
            + if charge_active { 4 } else { 0 };
        if target_entity.states.entry(78).is_some() {
            assert!(
                target_entity.states.set_payload(78, StatePayload::Slow { step: next_step }),
                "runtime slow state disappeared during extension"
            );
        } else {
            assert!(
                target_entity
                    .states
                    .add_entry(StateEntry::slow(78, slow_state_id, next_step, slow_priority)),
                "runtime slow state should be inserted"
            );
            // legacy 首次 set_state 会调用 update_states；必须同时提交疾走等
            // 等待属性刷新才生效的倍率。已有迟缓只延长 step，不触发刷新。
            target_entity.refresh_runtime_stats_from_template();
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[1]进入[迟缓]状态",
            actor.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    pub fn drain_plain_shadow_skill_into(&mut self, actor: EntityIdx, fixed_lane: usize, updates: &mut RunUpdates) {
        self.ensure_plain_minion_blueprint(actor, crate::namerena::MinionKind::Shadow);
        let blueprint_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT)
            .expect("default runtime profile must register core shadow blueprint slot");
        let counter_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default runtime profile must register core minion counter slot");
        let mut shadow_template = match self.entities.get(actor).and_then(|entity| entity.slots.get(blueprint_slot)) {
            Some(SlotValue::PlayerTemplate(template)) => template.as_ref().clone(),
            Some(_) => panic!("runtime core shadow blueprint slot has invalid value"),
            None => panic!("runtime core shadow blueprint missing for entity {}", actor.0),
        };
        let root_owner = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime shadow actor: {}", actor.0))
            .runtime
            .root_owner;
        let (root_name, next_minion_index) = {
            let root = self
                .entities
                .get(root_owner)
                .unwrap_or_else(|| panic!("unknown runtime shadow root owner: {}", root_owner.0));
            let next = match root.slots.get(counter_slot) {
                Some(SlotValue::U64(next)) => *next,
                Some(_) => panic!("runtime core minion counter slot has invalid value"),
                None => 0,
            };
            (root.template.name.clone(), next)
        };
        self.entities
            .get_mut(root_owner)
            .unwrap()
            .slots
            .set(counter_slot, SlotValue::U64(next_minion_index + 1))
            .expect("runtime core minion counter slot must exist");
        shadow_template.name = format!("{root_name}?{next_minion_index}");
        shadow_template.move_state.speed_points = if self
            .entities
            .get(actor)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000)
        {
            2048
        } else {
            -2048
        };

        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            actor.0 as usize,
            "[0]使用[幻术]",
            60,
        ));
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template: shadow_template,
            message: "召唤出[1]".to_owned(),
        });
        self.drain_effects_into(updates);

        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime shadow level missing for fixed lane {fixed_lane}"));
        let next_level = current_level.saturating_mul(3).div_ceil(4).max(1);
        assert!(
            self.entities.get_mut(actor).unwrap().template.skills.set_level_at(fixed_lane, next_level),
            "runtime shadow fixed lane disappeared during action"
        );
    }

    pub fn select_plain_possess_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
        if all_alive.is_empty() {
            return PreparedTargetList::new();
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
            .collect::<smallvec::SmallVec<[usize; 8]>>();
        let has_enemy = all_alive
            .iter()
            .copied()
            .any(|target| self.entities.get(target).is_some_and(|entity| entity.runtime.team != actor_team));
        if !has_enemy {
            return PreparedTargetList::new();
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut duplicate_count = 0usize;
        let invalid_count = -(select_count as i32);
        while duplicate_count <= select_count && invalid_count <= select_count as i32 {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(all_alive)
            } else {
                self.rng.pick_skip_range(all_alive, &enemy_skip_indices)
            };
            let Some(picked) = picked else {
                return PreparedTargetList::new();
            };
            let target = all_alive[picked];
            if selected.contains(&target) {
                duplicate_count += 1;
                continue;
            }
            selected.push(target);
            if selected.len() >= select_count {
                break;
            }
        }
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_possess_target(target, smart)));
        }
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_possess_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime possess target: {}", target.0));
        if smart {
            let hp = if entity.runtime.hp < 20 {
                30.0
            } else if entity.runtime.hp > 300 {
                300.0
            } else {
                entity.runtime.hp as f64
            };
            if self.world.alive_group_count() > 2 {
                hp * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                (1.0 / hp) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    pub fn drain_plain_possess_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]使用[附体]",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        let (caster_magic, target_flags, target_name, target_resistance, target_active) = {
            let caster = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime possess actor: {}", actor.0));
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime possess target: {}", target.0));
            (
                caster.runtime.magic,
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.runtime.resistance,
                target_entity.is_active(),
            )
        };
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            eprintln!(
                "[possess_probe:runtime:act_before] actor={} target={} target_name={} flags={:?} rc4=({},{})",
                actor.0, target.0, target_name, target_flags, self.rng.i, self.rng.j,
            );
        }
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::namerena::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::namerena::boss_immune_threshold(&target_name, "berserk");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        let dodged = immune || (target_active && PlayerRuntime::dodge(caster_magic, target_resistance, &mut self.rng));
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_POSSESS").is_some() {
            eprintln!(
                "[possess_probe:runtime:act_after] actor={} target={} immune={} dodged={} rc4=({},{})",
                actor.0, target.0, immune, dodged, self.rng.i, self.rng.j,
            );
        }
        if dodged {
            updates.add(crate::runtime::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime possess target disappeared: {}", target.0));
        let next_step = target_entity
            .states
            .entry(10)
            .and_then(|entry| match entry.payload {
                StatePayload::Berserk { step } => Some(step + 4),
                _ => None,
            })
            .unwrap_or(4);
        if !target_entity.states.set_payload(10, StatePayload::Berserk { step: next_step }) {
            target_entity.states.add_entry(StateEntry::berserk(10, next_step));
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[1]进入[狂暴]状态",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime possess actor disappeared: {}", actor.0))
            .runtime
            .hp = 0;
        self.drain_plain_lethal_damage_into(actor, actor, updates);
    }
}

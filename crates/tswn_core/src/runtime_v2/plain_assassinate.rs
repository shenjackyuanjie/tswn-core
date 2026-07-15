use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlainSkillPreActionOutcome {
    pub forced_skill: Option<PreparedBuiltinSkillAction>,
    pub clear_forced_action: bool,
}

impl CombatRuntime {
    pub fn run_plain_skill_pre_action_accumulator(&mut self, actor: EntityIdx) -> PlainSkillPreActionOutcome {
        let pre_action_order = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 pre-action owner: {}", actor.0))
            .template
            .skills
            .pre_action_order()
            .iter()
            .copied()
            .collect::<smallvec::SmallVec<[usize; 4]>>();
        let mut outcome = PlainSkillPreActionOutcome::default();
        for fixed_lane in pre_action_order {
            let skill_id = self
                .entities
                .get(actor)
                .and_then(|entity| entity.template.skills.skills().get(fixed_lane))
                .copied()
                .unwrap_or_else(|| panic!("runtime_v2 pre-action order references missing fixed lane {fixed_lane}"));
            let export_name = self
                .registry
                .skill(skill_id)
                .unwrap_or_else(|| panic!("unknown runtime_v2 pre-action skill id: {}", skill_id.0))
                .export_name
                .as_str();
            match export_name {
                DEFAULT_CORE_HIDE_SKILL_EXPORT => {
                    self.clear_plain_hide_before_action(actor);
                    if self.has_plain_berserk_state(actor) {
                        outcome.clear_forced_action = true;
                    }
                    outcome.forced_skill = None;
                }
                export_name if export_name == BuiltinActiveSkill::Assassinate.export_name() => {
                    let pending = self.entities.get(actor).and_then(|entity| entity.runtime.assassinate);
                    let Some(pending) = pending else {
                        self.entities
                            .get_mut(actor)
                            .expect("runtime_v2 assassinate owner disappeared")
                            .template
                            .skills
                            .remove_pre_action_lane(fixed_lane);
                        continue;
                    };
                    if self.entities.get(pending.target).is_some_and(|entity| entity.runtime.active()) {
                        outcome.forced_skill = Some(PreparedBuiltinSkillAction {
                            selected: SelectedBuiltinSkill {
                                skill: BuiltinActiveSkill::Assassinate,
                                fixed_lane: pending.fixed_lane,
                            },
                            targets: smallvec::smallvec![pending.target],
                        });
                    } else {
                        self.clear_plain_assassinate_pending(actor);
                        outcome.forced_skill = None;
                        outcome.clear_forced_action = true;
                    }
                }
                _ => {}
            }
            if outcome.forced_skill.is_some() {
                outcome.clear_forced_action = false;
            }
        }
        outcome
    }

    pub(super) fn select_plain_assassinate_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        if self.entities.get(actor).is_some_and(|entity| entity.runtime.assassinate.is_some()) {
            return PreparedTargetList::new();
        }
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
            let valid = self.entities.get(target).is_some_and(|entity| !smart || entity.runtime.hp > 160);
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
            scored.push((target, self.score_plain_assassinate_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    fn score_plain_assassinate_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 assassinate target: {}", target.0));
        if !smart {
            return self.rng.rFFFF() as f64 + entity.runtime.attract();
        }
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        if self.world.alive_group_count() > 2 {
            rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
        } else {
            rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
        }
    }

    fn clear_plain_assassinate_pending(&mut self, actor: EntityIdx) -> Option<AssassinateRuntime> {
        let entity = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 assassinate owner: {}", actor.0));
        let pending = entity.runtime.assassinate.take();
        if let Some(pending) = pending {
            entity.template.skills.remove_pre_action_lane(pending.fixed_lane);
        }
        pending
    }

    pub(super) fn drain_plain_assassinate_skill_into(
        &mut self,
        actor: EntityIdx,
        fixed_lane: usize,
        target: EntityIdx,
        updates: &mut RunUpdates,
    ) {
        if let Some(pending) = self.clear_plain_assassinate_pending(actor) {
            let target = pending.target;
            if !self.entities.get(target).is_some_and(|entity| entity.runtime.active()) {
                return;
            }
            updates.add(RuntimeFrame::replay_update(
                actor.0 as usize,
                target.0 as usize,
                "[0]发动[背刺]",
                1,
            ));
            let atp = {
                let owner = &self
                    .entities
                    .get(actor)
                    .unwrap_or_else(|| panic!("runtime_v2 assassinate owner disappeared: {}", actor.0))
                    .runtime;
                let at1 = owner.get_at(true, &mut self.rng);
                let at2 = owner.get_at(true, &mut self.rng);
                let at3 = owner.get_at(true, &mut self.rng);
                at1.max(at2).max(at3) * 4.0
            };
            if self.status_immune(target, "assassinate") {
                updates.add(RuntimeFrame::replay_update(
                    target.0 as usize,
                    actor.0 as usize,
                    "[0][回避]了攻击",
                    0,
                ));
                return;
            }
            self.drain_plain_attack_from_defense_into(actor, target, true, atp, updates);
            return;
        }

        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 assassinate owner: {}", actor.0));
        let charge_active = owner.runtime.charge.active;
        owner.runtime.move_state.speed_points += owner.runtime.magic * 3 + if charge_active { 1600 } else { 0 };
        owner.runtime.assassinate = Some(AssassinateRuntime {
            fixed_lane,
            target,
            break_on_damage: !charge_active,
        });
        owner.template.skills.ensure_pre_action_lane(fixed_lane);
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0][潜行]到[1]身后",
            1,
        ));
    }

    pub(super) fn run_plain_assassinate_post_damage_into(&mut self, owner: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0
            || !self
                .entities
                .get(owner)
                .and_then(|entity| entity.runtime.assassinate)
                .is_some_and(|pending| pending.break_on_damage)
        {
            return;
        }
        let pending = self
            .clear_plain_assassinate_pending(owner)
            .expect("runtime_v2 assassinate pending disappeared during post-damage");
        updates.add_newline();
        updates.add(RuntimeFrame::replay_update(
            owner.0 as usize,
            pending.target.0 as usize,
            "[0]的[潜行]被识破",
            0,
        ));
    }
}

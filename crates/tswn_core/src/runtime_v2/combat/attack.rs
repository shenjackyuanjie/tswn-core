use super::*;

impl CombatRuntime {
    pub fn select_plain_default_attack_target(&mut self, actor: EntityIdx, smart: bool) -> Option<EntityIdx> {
        self.entities.get(actor)?;
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive().to_vec();
        if all_alive.is_empty() {
            return None;
        }
        let enemy_skip_indices = all_alive
            .iter()
            .enumerate()
            .filter_map(|(index, entity)| {
                self.entities
                    .get(*entity)
                    .is_some_and(|record| record.runtime.team == actor_team)
                    .then_some(index)
            })
            .collect::<Vec<_>>();
        let select_count = if smart { 3 } else { 2 };
        let mut selected = Vec::with_capacity(select_count);
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = if enemy_skip_indices.is_empty() {
                self.rng.pick(&all_alive)
            } else {
                self.rng.pick_skip_range(&all_alive, &enemy_skip_indices)
            }?;
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
        if selected.is_empty() {
            return None;
        }

        let mut scored = Vec::with_capacity(selected.len());
        for target in selected {
            let target_entity = self.entities.get(target).unwrap();
            let score = if smart {
                let hp = if target_entity.runtime.hp < 20 {
                    30.0
                } else if target_entity.runtime.hp > 300 {
                    300.0
                } else {
                    target_entity.runtime.hp as f64
                };
                let alive_group_len = self.world.alive_group_len_containing(target) as f64;
                if self.world.alive_group_count() > 2 {
                    hp * alive_group_len * target_entity.runtime.attract()
                } else {
                    (1.0 / hp) * target_entity.runtime.atk_sum as f64 * target_entity.runtime.attract()
                }
            } else {
                self.rng.rFFFF() as f64 + target_entity.runtime.attract()
            };
            scored.push((target, score));
        }
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        #[cfg(not(feature = "no_debug"))]
        if std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false)
        {
            let entity_name = |entity: EntityIdx| {
                self.entities
                    .get(entity)
                    .map(|record| format!("{}#{}(hp={})", record.template.name, entity.0, record.runtime.hp))
                    .unwrap_or_else(|| format!("#{}", entity.0))
            };
            let ranked = scored
                .iter()
                .map(|(entity, score)| format!("{}:{score}", entity_name(*entity)))
                .collect::<Vec<_>>();
            eprintln!(
                "[probe_default_attack:v2] actor={}#{} smart={} effective_team={} rc4=({}, {}) all_alive={:?} ranked={:?}",
                self.entities.get(actor).unwrap().template.name,
                actor.0,
                smart,
                actor_team,
                self.rng.i,
                self.rng.j,
                all_alive.iter().copied().map(entity_name).collect::<Vec<_>>(),
                ranked,
            );
        }
        scored.first().map(|(target, _)| *target)
    }

    pub fn drain_plain_default_attack_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        updates: &mut RunUpdates,
    ) {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false);
        if let Some(at_boost) = self.lazy_boss_at_boost(actor)
            && self.has_lazy_infection(target)
            && self.rng.next_u8() < 128
        {
            self.emit_lazy_activity_into(actor, updates);
            self.set_lazy_boss_at_boost(actor, at_boost + 0.5);
            return;
        }
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0]发起攻击",
            0,
        ));
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:atp_before] actor={} target={} use_magic={} rc4=({}, {})",
                actor.0, target.0, use_magic, self.rng.i, self.rng.j,
            );
        }
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default attack actor: {}", actor.0))
            .runtime
            .get_at(use_magic, &mut self.rng)
            * self.lazy_boss_at_boost(actor).unwrap_or(1.0);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:atp_after] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        self.drain_plain_attack_with_atp_into(actor, target, use_magic, atp, updates);
    }

    pub fn saitama_boss_state(&self, actor: EntityIdx) -> Option<(i32, i32, usize, usize)> {
        self.entities.get(actor)?.states.entries().iter().find_map(|entry| {
            let StatePayload::SaitamaBoss {
                turns,
                damages,
                hitters,
                minions,
            } = &entry.payload
            else {
                return None;
            };
            Some((*turns, *damages, hitters.len(), minions.len()))
        })
    }

    pub fn drain_plain_saitama_action_into(
        &mut self,
        actor: EntityIdx,
        selected_target: Option<EntityIdx>,
        updates: &mut RunUpdates,
    ) {
        let (turns, damages, hitter_count, minion_count) = self
            .saitama_boss_state(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor lacks saitama state: {}", actor.0));
        let hunger_denominator = hitter_count as i32 + minion_count as i32 / 3 + 1;
        if damages / hunger_denominator.max(1) > 255 {
            let display_name = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
                .template
                .display_name
                .clone();
            let mut hungry_update =
                crate::engine::update::RunUpdate::new(format!("{display_name}觉得有点饿"), actor.0 as usize, actor.0 as usize, 0);
            hungry_update.delay1 = 2000;
            updates.add(hungry_update);
            updates.add_newline();
            updates.add(crate::engine::update::RunUpdate::new(
                format!(" {display_name}离开了战场"),
                actor.0 as usize,
                actor.0 as usize,
                0,
            ));
            let team = {
                let actor_entity = self
                    .entities
                    .get_mut(actor)
                    .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0));
                actor_entity.runtime.hp = 0;
                actor_entity.runtime.alive = false;
                actor_entity.runtime.team
            };
            self.world.mark_dead(actor, team);
            return;
        }

        if turns < 10 {
            let actor_entity = self
                .entities
                .get_mut(actor)
                .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0));
            let entry = actor_entity
                .states
                .entry_mut(PLAIN_SAITAMA_BOSS_STATE_KEY)
                .expect("runtime_v2 saitama state disappeared");
            let StatePayload::SaitamaBoss { turns, .. } = &mut entry.payload else {
                panic!("runtime_v2 saitama state key is occupied by another state");
            };
            *turns += 1;
            return;
        }

        let Some(target) = selected_target else {
            return;
        };
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
            .runtime
            .get_at(false, &mut self.rng)
            * 12.0;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]发起攻击",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);

        let actor_team = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
            .runtime
            .team;
        let team_members = self
            .entities
            .iter()
            .filter_map(|(member, entity)| (entity.runtime.team == actor_team).then_some(member))
            .collect::<Vec<_>>();
        for member in team_members {
            self.entities
                .get_mut(member)
                .expect("runtime_v2 saitama team member disappeared")
                .runtime
                .move_state
                .speed_points = 0;
        }
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime_v2 saitama actor disappeared: {}", actor.0))
            .runtime
            .move_state
            .speed_points = 1700;
    }

    pub fn drain_plain_attack_with_atp_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_with_atp_covid_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    pub fn drain_plain_attack_with_atp_and_on_damage_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_with_atp_covid_and_on_damage_into(actor, target, use_magic, atp, covid_source, on_damage, updates)
    }

    pub fn drain_plain_attack_with_atp_and_covid_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        updates: &mut RunUpdates,
    ) -> i32 {
        self.drain_plain_attack_with_atp_covid_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    pub fn drain_plain_attack_with_atp_covid_and_on_damage_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false);
        let mut defend_value = RuntimeDefendValue::Atp {
            value: atp,
            caster: actor,
            target,
            is_magic: use_magic,
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:pre_defend_before] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
        let Some(atp) = defend_value.atp() else {
            panic!("runtime_v2 PRE_DEFEND hooks must leave an atp value");
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:pre_defend_after] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        if atp == 0.0 {
            return 0;
        }

        let (accuracy, dodge_value, target_active) = {
            let actor_runtime = &self.entities.get(actor).unwrap().runtime;
            let target_entity = self.entities.get(target).unwrap();
            let target_runtime = &target_entity.runtime;
            (
                if use_magic {
                    actor_runtime.magic + actor_runtime.agility
                } else {
                    actor_runtime.attack + actor_runtime.agility
                },
                if use_magic {
                    target_runtime.resistance + target_runtime.agility
                } else {
                    target_runtime.defense + target_runtime.agility
                },
                target_entity.is_active(),
            )
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:dodge_before] actor={} target={} accuracy={} dodge={} active={} rc4=({}, {})",
                actor.0, target.0, accuracy, dodge_value, target_active, self.rng.i, self.rng.j,
            );
        }
        let dodged = target_active && PlayerRuntime::dodge(accuracy, dodge_value, &mut self.rng);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:dodge_after] actor={} target={} dodged={} rc4=({}, {})",
                actor.0, target.0, dodged, self.rng.i, self.rng.j,
            );
        }
        if dodged {
            updates.add(RuntimeFrame::replay_update(
                target.0 as usize,
                actor.0 as usize,
                "[0][回避]了攻击",
                20,
            ));
            return 0;
        }

        self.drain_plain_attack_after_dodge_into(actor, target, use_magic, atp, covid_source, on_damage, updates)
    }

    /// 从防御值换算阶段开始结算攻击。
    ///
    /// 这对应 legacy `Player::defned`：调用方已经决定跳过 `PRE_DEFEND`
    /// 和普通闪避，只保留伤害换算、`POST_DEFEND` 与后续伤害链。
    pub fn drain_plain_attack_from_defense_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_after_dodge_into(actor, target, use_magic, atp, covid_source, PlainAttackOnDamage::None, updates)
    }

    pub fn drain_plain_attack_after_dodge_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = std::env::var("TSWN_PROBE_DEFAULT_ATTACK")
            .map(|needle| {
                self.entities.get(actor).is_some_and(|entity| {
                    entity.template.name.contains(&needle) || entity.template.display_name.contains(&needle)
                })
            })
            .unwrap_or(false);
        let defense = {
            let target_runtime = &self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 attack target: {}", target.0))
                .runtime;
            if use_magic {
                target_runtime.resistance + 64
            } else {
                target_runtime.defense + 64
            }
        };
        let amount = (atp / defense as f64).ceil() as i32;
        let mut defend_value = RuntimeDefendValue::Damage {
            value: amount,
            caster: actor,
            target,
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:post_defend_before] actor={} target={} damage={} rc4=({}, {})",
                actor.0, target.0, amount, self.rng.i, self.rng.j,
            );
        }
        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
        let Some(amount) = defend_value.damage() else {
            panic!("runtime_v2 POST_DEFEND hooks must leave a damage value");
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:v2:post_defend_after] actor={} target={} damage={} rc4=({}, {})",
                actor.0, target.0, amount, self.rng.i, self.rng.j,
            );
        }
        if self.apply_plain_attack_damage_with_covid_and_on_damage_into(actor, target, amount, covid_source, on_damage, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
        amount
    }

    pub fn apply_plain_attack_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let covid_source = self.covid_boss_mutation(caster).map(|mutation| (caster, mutation));
        self.apply_plain_attack_damage_with_covid_into(caster, target, amount, covid_source, updates)
    }

    pub fn apply_plain_attack_damage_with_covid_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        covid_source: Option<(EntityIdx, i32)>,
        updates: &mut RunUpdates,
    ) -> bool {
        self.apply_plain_attack_damage_with_covid_and_on_damage_into(
            caster,
            target,
            amount,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    pub fn apply_plain_attack_damage_with_covid_and_on_damage_into(
        &mut self,
        caster: EntityIdx,
        target: EntityIdx,
        amount: i32,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> bool {
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 default attack target: {}", target.0));
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let killed = target_entity.runtime.hp == 0 && target_entity.runtime.alive;
        updates.add(RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
        if amount == 0 {
            return false;
        }
        if let Some((boss, mutation)) = covid_source {
            self.try_covid_spread_on_damage_into(boss, target, mutation, amount, updates);
        }
        if self.lazy_boss_at_boost(caster).is_some() {
            self.infect_with_lazy_into(caster, target, updates);
            if amount > 0 {
                self.set_lazy_boss_at_boost(caster, 1.0);
            }
        }
        match on_damage {
            PlainAttackOnDamage::None => {}
            PlainAttackOnDamage::Absorb => self.apply_absorb_on_damage(caster, amount, updates),
            PlainAttackOnDamage::Berserk => self.apply_berserk_on_damage(caster, target, amount, updates),
            PlainAttackOnDamage::Curse => self.apply_curse_on_damage(caster, target, amount, updates),
            PlainAttackOnDamage::Ice if amount > 0 => self.apply_ice_on_damage(caster, target, updates),
            PlainAttackOnDamage::Ice => {}
            PlainAttackOnDamage::Poison => self.apply_poison_on_damage(caster, target, amount, updates),
        }
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        killed
    }

    pub fn apply_absorb_on_damage(&mut self, caster: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        let owner = self
            .entities
            .get_mut(caster)
            .unwrap_or_else(|| panic!("unknown runtime_v2 absorb caster: {}", caster.0));
        if owner.runtime.hp <= 0 {
            return;
        }
        let healed = ((damage + 1) / 2).min(owner.template.max_hp - owner.runtime.hp);
        if healed > 0 {
            owner.runtime.hp = (owner.runtime.hp + healed).min(owner.template.max_hp);
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]回复体力[2]点",
            caster.0 as usize,
            caster.0 as usize,
            healed as u32,
        ));
    }

    pub fn apply_berserk_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        if self.entities.get(target).is_none_or(|entity| entity.runtime.hp <= 0) || self.status_immune(target, "berserk") {
            return;
        }
        let charge_active = self
            .entities
            .get(caster)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);
        let existing_key = self.entities.get(target).and_then(|entity| {
            entity
                .states
                .entries()
                .iter()
                .find(|entry| matches!(entry.payload, StatePayload::Berserk { .. }))
                .map(|entry| entry.legacy_order_key)
        });
        if let Some(state_key) = existing_key {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 berserk target: {}", target.0));
            let StatePayload::Berserk { step } = &mut target_entity
                .states
                .entry_mut(state_key)
                .expect("runtime_v2 berserk state disappeared during extension")
                .payload
            else {
                unreachable!("runtime_v2 berserk state key changed payload during extension");
            };
            *step += 1 + i32::from(charge_active);
            return;
        }

        assert!(
            self.entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 berserk target: {}", target.0))
                .states
                .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 1 + i32::from(charge_active),)),
            "runtime_v2 berserk state should be inserted"
        );
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]进入[狂暴]状态",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    pub fn apply_curse_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        let (target_hp, target_flags, charge_active, existing) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 curse target: {}", target.0));
            let existing = target_entity.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| match entry.payload {
                StatePayload::Curse { prob, multiply } => (prob, multiply),
                _ => panic!("runtime_v2 curse state key is occupied by another payload"),
            });
            (
                target_entity.runtime.hp,
                target_entity.runtime.flags,
                target_entity.runtime.at_boost_millionths >= 3_000_000,
                existing,
            )
        };
        if target_hp <= 0 || target_flags.intersects(PlayerKindFlags::BOSS | PlayerKindFlags::BOOST) {
            return;
        }

        let curse_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_CURSE_STATE_EXPORT)
            .expect("default runtime v2 profile must register curse state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 curse target: {}", target.0));
        let charge_prob = if charge_active { 10 } else { 0 };
        let charge_multiply = if charge_active { 1 } else { 0 };
        if let Some((prob, multiply)) = existing {
            assert!(
                target_entity.states.set_payload(
                    PLAIN_CURSE_STATE_KEY,
                    StatePayload::Curse {
                        prob: prob + 10 + charge_prob,
                        multiply: multiply + 1 + charge_multiply,
                    },
                ),
                "runtime_v2 curse state disappeared while stacking"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::curse(
                    PLAIN_CURSE_STATE_KEY,
                    curse_state,
                    42 + charge_prob,
                    2 + charge_multiply,
                    SkillPriority(10_000),
                )),
                "runtime_v2 curse state key should be vacant"
            );
            target_entity.runtime.atk_sum = target_entity.runtime.atk_sum.saturating_mul(4);
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]被[诅咒]了",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }
}

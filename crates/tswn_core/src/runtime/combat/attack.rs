use super::*;

impl CombatRuntime {
    #[cfg(not(feature = "no_debug"))]
    fn probe_default_attack_matches(&self, actor: EntityIdx) -> bool {
        crate::debug::probe_default_attack()
            .map(|needle| {
                if let Some(raw_idx) = needle.strip_prefix("idx:") {
                    return raw_idx.parse::<u32>().is_ok_and(|idx| actor.0 == idx);
                }
                self.entities
                    .get(actor)
                    .is_some_and(|entity| entity.template.name.contains(needle) || entity.template.display_name.contains(needle))
            })
            .unwrap_or(false)
    }

    pub fn select_plain_default_attack_target(&mut self, actor: EntityIdx, smart: bool) -> Option<EntityIdx> {
        self.entities.get(actor)?;
        let actor_team = self.plain_effective_team(actor);
        let all_alive = self.world.flat_alive();
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
            .collect::<crate::rc4::PickSkipRange>();
        #[cfg(feature = "no_debug")]
        if let Some(index) = enemy_skip_indices.single_remaining_index(all_alive.len()) {
            if all_alive.len() > 1 {
                // 排除队友后每次抽样消耗两字节。聪明时抽五次，普通时抽四次再取两字节评分。
                self.rng.discard_bytes::<10>();
            } else if !smart {
                // 无排除项的单元素列表抽样不消耗字节，只有普通评分推进两字节。
                self.rng.discard_bytes::<2>();
            }
            let target = all_alive[index];
            self.entities.get(target).unwrap();
            return Some(target);
        }
        #[cfg(feature = "no_debug")]
        if let Some([left, right]) = enemy_skip_indices.two_remaining_indices(all_alive.len()) {
            let candidates = [all_alive[left], all_alive[right]];
            if self.world.alive_group_count() <= 2 && candidates[0] != candidates[1] {
                let select_count = if smart { 3 } else { 2 };
                return Some(self.rank_two_plain_enemy_targets(candidates, smart, select_count).0);
            }
        }
        let select_count = if smart { 3 } else { 2 };
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut duplicate_count = 0usize;
        while duplicate_count <= select_count {
            let picked = self.rng.pick_skip_span(all_alive, enemy_skip_indices)?;
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

        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            let target_entity = self.entities.get(target).unwrap();
            let score = if smart {
                Self::plain_smart_attack_score(&self.world, target, &target_entity.runtime)
            } else {
                self.rng.rFFFF() as f64 + target_entity.runtime.attract()
            };
            scored.push((target, score));
        }
        scored.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(std::cmp::Ordering::Equal));
        #[cfg(not(feature = "no_debug"))]
        if self.probe_default_attack_matches(actor) {
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
                "[probe_default_attack:runtime] actor={}#{} smart={} effective_team={} rc4=({}, {}) all_alive={:?} ranked={:?}",
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

    pub(super) fn plain_smart_attack_score(world: &WorldArena, target: EntityIdx, runtime: &PlayerRuntime) -> f64 {
        let hp = match runtime.hp {
            ..=19 => 30.0,
            301.. => 300.0,
            hp => hp as f64,
        };
        if world.alive_group_count() > 2 {
            hp * world.alive_group_len_containing(target) as f64 * runtime.attract()
        } else {
            (1.0 / hp) * runtime.atk_sum as f64 * runtime.attract()
        }
    }

    #[cfg(feature = "no_debug")]
    fn sample_two_plain_enemy_targets(
        &mut self,
        candidates: [EntityIdx; 2],
        select_count: usize,
    ) -> (EntityIdx, Option<EntityIdx>) {
        debug_assert!(select_count >= 2);
        // 上界为 2 的抽样消费两字节，取末字节最低位等同于求余。
        let first = (self.rng.rFFFF() & 1) as usize;
        let duplicate_limit = select_count + 1;
        for duplicates in 0..duplicate_limit {
            if (self.rng.rFFFF() & 1) as usize != first {
                if select_count > 2 {
                    // 两个位置都已入选时，剩余抽样只会增加重复计数。
                    for _ in duplicates..duplicate_limit {
                        self.rng.discard_bytes::<2>();
                    }
                }
                return (candidates[first], Some(candidates[first ^ 1]));
            }
        }
        (candidates[first], None)
    }

    #[cfg(feature = "no_debug")]
    pub(super) fn rank_two_plain_enemy_targets(
        &mut self,
        candidates: [EntityIdx; 2],
        smart: bool,
        select_count: usize,
    ) -> (EntityIdx, Option<EntityIdx>) {
        let (first, other) = self.sample_two_plain_enemy_targets(candidates, select_count);
        let first_runtime = &self.entities.get(first).unwrap().runtime;
        let Some(second) = other else {
            if !smart {
                self.rng.discard_bytes::<2>();
            }
            return (first, None);
        };
        let second_runtime = &self.entities.get(second).unwrap().runtime;
        let (first_score, second_score) = if smart {
            (
                Self::plain_smart_attack_score(&self.world, first, first_runtime),
                Self::plain_smart_attack_score(&self.world, second, second_runtime),
            )
        } else {
            (
                self.rng.rFFFF() as f64 + first_runtime.attract(),
                self.rng.rFFFF() as f64 + second_runtime.attract(),
            )
        };
        if second_score > first_score {
            (second, Some(first))
        } else {
            (first, Some(second))
        }
    }

    /// 保留受限技能的两个停止条件：重复抽中与抽中无效目标各自计数。
    pub(in crate::runtime) fn select_plain_filtered_enemy_targets(
        &mut self,
        actor: EntityIdx,
        smart: bool,
        eligible: impl Fn(Option<&EntityRecord>, bool) -> bool,
        score: impl Fn(&mut Self, EntityIdx, bool) -> f64,
    ) -> PreparedTargetList {
        let team = self.plain_effective_team(actor);
        let alive = self.world.flat_alive();
        let skips = alive
            .iter()
            .enumerate()
            .filter_map(|(index, &id)| self.entities.get(id).is_some_and(|entity| entity.runtime.team == team).then_some(index))
            .collect::<crate::rc4::PickSkipRange>();
        let limit = if smart { 3 } else { 2 };
        #[cfg(feature = "no_debug")]
        if let Some(index) = skips.single_remaining_index(alive.len())
            && let Some(entity) = self.entities.get(alive[index])
        {
            let target = alive[index];
            let valid = eligible(Some(entity), smart);
            if alive.len() > 1 {
                match (smart, valid) {
                    (true, true) => self.rng.discard_bytes::<10>(),
                    (true, false) => self.rng.discard_bytes::<14>(),
                    (false, true) => self.rng.discard_bytes::<8>(),
                    (false, false) => self.rng.discard_bytes::<10>(),
                }
            }
            if !valid {
                return PreparedTargetList::new();
            }
            score(self, target, smart);
            return smallvec::smallvec![target];
        }
        #[cfg(feature = "no_debug")]
        if let Some([left, right]) = skips.two_remaining_indices(alive.len())
            && alive[left] != alive[right]
            && let (Some(left_entity), Some(right_entity)) = (self.entities.get(alive[left]), self.entities.get(alive[right]))
        {
            let candidates = [alive[left], alive[right]];
            let valid = [eligible(Some(left_entity), smart), eligible(Some(right_entity), smart)];
            return self.select_two_plain_filtered_targets(candidates, valid, smart, limit, score);
        }
        let mut selected = smallvec::SmallVec::<[EntityIdx; 3]>::new();
        let mut duplicates = 0;
        let mut invalid = 0;
        while duplicates <= limit && invalid <= limit * 2 {
            let Some(index) = self.rng.pick_skip_span(alive, skips) else {
                return PreparedTargetList::new();
            };
            let target = alive[index];
            if !eligible(self.entities.get(target), smart) {
                invalid += 1;
            } else if selected.contains(&target) {
                duplicates += 1;
            } else {
                selected.push(target);
                if selected.len() == limit {
                    break;
                }
            }
        }
        let mut scored = selected
            .into_iter()
            .map(|target| (target, score(self, target, smart)))
            .collect::<smallvec::SmallVec<[_; 3]>>();
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    #[cfg(feature = "no_debug")]
    fn select_two_plain_filtered_targets(
        &mut self,
        candidates: [EntityIdx; 2],
        valid: [bool; 2],
        smart: bool,
        limit: usize,
        score: impl Fn(&mut Self, EntityIdx, bool) -> f64,
    ) -> PreparedTargetList {
        match valid {
            [true, true] => {
                let (first, second) = self.sample_two_plain_enemy_targets(candidates, limit);
                let first_score = score(self, first, smart);
                if let Some(second) = second {
                    if score(self, second, smart) > first_score {
                        return smallvec::smallvec![second, first];
                    }
                    return smallvec::smallvec![first, second];
                }
                smallvec::smallvec![first]
            }
            [false, false] => {
                if smart {
                    self.rng.discard_bytes::<14>();
                } else {
                    self.rng.discard_bytes::<10>();
                }
                PreparedTargetList::new()
            }
            _ => {
                let valid_index = usize::from(valid[1]);
                let mut hits = 0;
                let mut invalid = 0;
                while hits < limit + 2 && invalid <= limit * 2 {
                    if (self.rng.rFFFF() & 1) as usize == valid_index {
                        hits += 1;
                    } else {
                        invalid += 1;
                    }
                }
                if hits == 0 {
                    PreparedTargetList::new()
                } else {
                    let target = candidates[valid_index];
                    score(self, target, smart);
                    smallvec::smallvec![target]
                }
            }
        }
    }

    pub fn drain_plain_default_attack_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        updates: &mut RunUpdates,
    ) {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = self.probe_default_attack_matches(actor);
        if let Some(at_boost) = self.lazy_boss_at_boost(actor)
            && self.has_lazy_infection(target)
            && self.rng.next_u8() < 128
        {
            self.emit_lazy_activity_into(actor, updates);
            self.set_lazy_boss_at_boost(actor, at_boost + 0.5);
            return;
        }
        updates.add_with(|| RuntimeFrame::replay_update(actor.0 as usize, target.0 as usize, "[0]发起攻击", 0));
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:atp_before] actor={} target={} use_magic={} rc4=({}, {})",
                actor.0, target.0, use_magic, self.rng.i, self.rng.j,
            );
        }
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime default attack actor: {}", actor.0))
            .runtime
            .get_at(use_magic, &mut self.rng)
            * self.lazy_boss_at_boost(actor).unwrap_or(1.0);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:atp_after] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        // 青雉只免疫 SklAttack 的物理回调；普通玩家、青雉和斑鸠使用它，
        // 其他 Boss 的 SklSimpleAttack 与物理技能都不使用该回调。
        let kind = if !use_magic
            && self.entities.get(actor).is_some_and(|entity| {
                if entity.runtime.flags.contains(PlayerKindFlags::BOSS) {
                    crate::namerena::boss_uses_skill_attack(&entity.template.name)
                } else {
                    self.entities.get(target).is_some_and(|target| {
                        target.runtime.flags.contains(PlayerKindFlags::BOSS) && target.template.name == "aokiji"
                    })
                }
            }) {
            DamageKind::Physical
        } else {
            DamageKind::Default
        };
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_with_atp_covid_kind_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            kind,
            PlainAttackOnDamage::None,
            updates,
        );
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
            .unwrap_or_else(|| panic!("runtime saitama actor lacks saitama state: {}", actor.0));
        // 原版把每只召唤物计作 1/3，比较前也不截断平均伤害。
        let hunger_denominator = hitter_count as f64 + minion_count as f64 / 3.0 + 1.0;
        if f64::from(damages) / hunger_denominator > 255.0 {
            let display_name = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("runtime saitama actor disappeared: {}", actor.0))
                .template
                .display_name
                .clone();
            let mut hungry_update = crate::runtime::update::RunUpdate::new(
                format!("{display_name}觉得有点饿"),
                actor.0 as usize,
                actor.0 as usize,
                0,
            );
            hungry_update.delay1 = 2000;
            updates.add(hungry_update);
            updates.add_newline();
            updates.add_with(|| {
                crate::runtime::update::RunUpdate::new(
                    format!(" {display_name}离开了战场"),
                    actor.0 as usize,
                    actor.0 as usize,
                    0,
                )
            });
            let team = {
                let actor_entity = self
                    .entities
                    .get_mut(actor)
                    .unwrap_or_else(|| panic!("runtime saitama actor disappeared: {}", actor.0));
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
                .unwrap_or_else(|| panic!("runtime saitama actor disappeared: {}", actor.0));
            let entry = actor_entity
                .states
                .entry_mut(PLAIN_SAITAMA_BOSS_STATE_KEY)
                .expect("runtime saitama state disappeared");
            let StatePayload::SaitamaBoss { turns, .. } = &mut entry.payload else {
                panic!("runtime saitama state key is occupied by another state");
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
            .unwrap_or_else(|| panic!("runtime saitama actor disappeared: {}", actor.0))
            .runtime
            .get_at(false, &mut self.rng)
            * 12.0;
        updates.add(crate::runtime::update::RunUpdate::new(
            "[0]发起攻击",
            actor.0 as usize,
            target.0 as usize,
            0,
        ));
        self.drain_plain_attack_with_atp_into(actor, target, false, atp, updates);
        // 原版在最后一组对手退场时直接结束本次动作，不执行后面的清条。
        if self.world.sync_winner_from_alive_views().is_some() {
            return;
        }

        // 非终局出拳清空全场存活者的行动值，再给一拳超人自己留下 1700。
        for &member in self.world.flat_alive() {
            self.entities.get_mut(member).unwrap().runtime.move_state.speed_points = 0;
        }
        self.entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("runtime saitama actor disappeared: {}", actor.0))
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
        self.drain_plain_attack_with_atp_covid_kind_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            DamageKind::Default,
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
        kind: DamageKind,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        let covid_source = self.covid_boss_mutation(actor).map(|mutation| (actor, mutation));
        self.drain_plain_attack_with_atp_covid_kind_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            kind,
            on_damage,
            updates,
        )
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
        self.drain_plain_attack_with_atp_covid_kind_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            DamageKind::Default,
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
        self.drain_plain_attack_with_atp_covid_kind_and_on_damage_into(
            actor,
            target,
            use_magic,
            atp,
            covid_source,
            DamageKind::Default,
            on_damage,
            updates,
        )
    }

    /// 命中/回避判定口径：攻击方命中值、目标闪避值与目标当前是否活跃。
    ///
    /// 只在真正需要判定时调用，取值时机决定语义：PRE_DEFEND 钩子可能改变存活与攻防敏，
    /// 因此实际闪避判定必须在钩子之后重新取值。
    fn plain_dodge_values(&self, actor: EntityIdx, target: EntityIdx, use_magic: bool) -> (i32, i32, bool) {
        let actor_runtime = &self.entities.get(actor).unwrap().runtime;
        let target_entity = self.entities.get(target).unwrap();
        Self::plain_dodge_values_for(actor_runtime, target_entity, use_magic)
    }

    #[inline]
    fn plain_dodge_values_for(actor_runtime: &PlayerRuntime, target_entity: &EntityRecord, use_magic: bool) -> (i32, i32, bool) {
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
    }

    /// 完整攻击入口，携带 0.5.5 `a3` 的伤害类型标记。
    ///
    /// `kind` 只在双方都携带时才有可观察差异：青雉的 POST_DEFEND 吸收依赖
    /// `T.mE()`（冰冻术/冰河时代）与 `T.oH()`（`SklAttack` 物理分支）。
    ///
    /// 伤害决策批次在进入 PRE_DEFEND 之前开启：回避预演要挂在该批次上，退出时成对关闭。
    pub(crate) fn drain_plain_attack_with_atp_covid_kind_and_on_damage_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        covid_source: Option<(EntityIdx, i32)>,
        kind: DamageKind,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        let passive_decisions = self.control.passive_decision_mode();
        let batch_id = if cfg!(feature = "no_debug") && !updates.capture_updates && !passive_decisions {
            // 自动结算不会读取伤害决策栈，只推进编号以便后续交互沿用相同批次号。
            self.control.advance_damage_batch_id();
            None
        } else {
            Some(self.control.begin_damage_batch(actor.0 as PlrId, target.0 as PlrId))
        };
        #[cfg(feature = "no_debug")]
        if !updates.capture_updates
            && !passive_decisions
            && let Some(defender) = self.entities.get(target)
            && !defender.runtime.flags.contains(PlayerKindFlags::BOSS)
            && defender.runtime.shield <= 0
            && (defender.runtime.protect_from.is_empty() || defender.runtime.protect_pre_defend_skill_count.is_none())
            && !defender.states.hook_mask().intersects(ProcMask::PRE_DEFEND | ProcMask::POST_DEFEND)
            && !defender
                .template
                .skills
                .may_have_enabled_hook(ProcMask::PRE_DEFEND | ProcMask::POST_DEFEND)
        {
            // 两个防御阶段均为空，可从同一个目标记录读取闪避与基础防御。命中后沿用正式伤害链。
            let amount = if atp == 0.0 {
                0
            } else {
                let actor_runtime = &self.entities.get(actor).unwrap().runtime;
                let (accuracy, dodge, active) = Self::plain_dodge_values_for(actor_runtime, defender, use_magic);
                if active && PlayerRuntime::dodge(accuracy, dodge, &mut self.rng) {
                    updates.add_with(|| RuntimeFrame::replay_update(target.0 as usize, actor.0 as usize, "[0][回避]了攻击", 20));
                    0
                } else {
                    let defense = if use_magic {
                        defender.runtime.resistance
                    } else {
                        defender.runtime.defense
                    } + 64;
                    let amount = ceil_damage_to_i32(atp / defense as f64);
                    self.settle_plain_attack_damage_into(actor, target, amount, covid_source, on_damage, updates);
                    amount
                }
            };
            if let Some(batch_id) = batch_id {
                self.control.finish_damage_batch(batch_id);
            }
            return amount;
        }
        // 回避预演必须在 PRE_DEFEND 之前询问（被动决策协议），且只在决策模式下需要判定口径。
        if passive_decisions {
            let (accuracy, dodge_value, target_active) = self.plain_dodge_values(actor, target, use_magic);
            if target_active {
                self.prepare_plain_dodge(target, accuracy, dodge_value, false, updates);
            }
        }
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = self.probe_default_attack_matches(actor);
        let mut defend_value = RuntimeDefendValue::Atp {
            value: atp,
            caster: actor,
            target,
            is_magic: use_magic,
            kind,
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:pre_defend_before] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        self.drain_pre_defend_hooks_with_on_damage_into(target, updates, &mut defend_value, on_damage);
        let Some(atp) = defend_value.atp() else {
            panic!("runtime PRE_DEFEND hooks must leave an atp value");
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:pre_defend_after] actor={} target={} atp={} rc4=({}, {})",
                actor.0, target.0, atp, self.rng.i, self.rng.j,
            );
        }
        if atp == 0.0 {
            if let Some(batch_id) = batch_id {
                self.control.finish_damage_batch(batch_id);
            }
            return 0;
        }
        // 实际判定沿用 PRE_DEFEND 之后的状态：反弹/守护钩子可能改变目标存活或攻防敏，
        // main 一直在这里重算，提前取值会让命中与随机数消耗偏离开源语义。
        let (accuracy, dodge_value, target_active) = self.plain_dodge_values(actor, target, use_magic);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:dodge_before] actor={} target={} accuracy={} dodge={} active={} rc4=({}, {})",
                actor.0, target.0, accuracy, dodge_value, target_active, self.rng.i, self.rng.j,
            );
        }
        let dodged = target_active && self.roll_plain_dodge(target, accuracy, dodge_value, false);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:dodge_after] actor={} target={} dodged={} rc4=({}, {})",
                actor.0, target.0, dodged, self.rng.i, self.rng.j,
            );
        }
        if dodged {
            updates.add_with(|| RuntimeFrame::replay_update(target.0 as usize, actor.0 as usize, "[0][回避]了攻击", 20));
            if let Some(batch_id) = batch_id {
                self.control.finish_damage_batch(batch_id);
            }
            return 0;
        }

        // 伤害链本身可能嵌套别的攻击（反弹/反击），批次由外层释放。
        let damage =
            self.drain_plain_attack_after_dodge_into(actor, target, use_magic, atp, kind, covid_source, on_damage, updates);
        if let Some(batch_id) = batch_id {
            self.control.finish_damage_batch(batch_id);
        }
        damage
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
        self.drain_plain_attack_after_dodge_into(
            actor,
            target,
            use_magic,
            atp,
            DamageKind::Default,
            covid_source,
            PlainAttackOnDamage::None,
            updates,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drain_plain_attack_after_dodge_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        use_magic: bool,
        atp: f64,
        kind: DamageKind,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) -> i32 {
        #[cfg(not(feature = "no_debug"))]
        let debug_attack = self.probe_default_attack_matches(actor);
        let defense = {
            let target_runtime = &self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime attack target: {}", target.0))
                .runtime;
            if use_magic {
                target_runtime.resistance + 64
            } else {
                target_runtime.defense + 64
            }
        };
        let amount = ceil_damage_to_i32(atp / defense as f64);
        let mut defend_value = RuntimeDefendValue::Damage {
            value: amount,
            caster: actor,
            target,
            kind,
        };
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:post_defend_before] actor={} target={} damage={} rc4=({}, {})",
                actor.0, target.0, amount, self.rng.i, self.rng.j,
            );
        }
        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
        let Some(amount) = defend_value.damage() else {
            panic!("runtime POST_DEFEND hooks must leave a damage value");
        };
        self.apply_wheel_post_defend_spsum(target, amount);
        #[cfg(not(feature = "no_debug"))]
        if debug_attack {
            eprintln!(
                "[probe_default_attack:runtime:post_defend_after] actor={} target={} damage={} rc4=({}, {})",
                actor.0, target.0, amount, self.rng.i, self.rng.j,
            );
        }
        self.settle_plain_attack_damage_into(actor, target, amount, covid_source, on_damage, updates);
        amount
    }

    fn settle_plain_attack_damage_into(
        &mut self,
        actor: EntityIdx,
        target: EntityIdx,
        amount: i32,
        covid_source: Option<(EntityIdx, i32)>,
        on_damage: PlainAttackOnDamage,
        updates: &mut RunUpdates,
    ) {
        if self.apply_plain_attack_damage_with_covid_and_on_damage_into(actor, target, amount, covid_source, on_damage, updates) {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
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
        // 0.5.5 `aF`：负数伤害表示 Boss 防御吸收，只回血并输出恢复消息，
        // 不触发 covid/lazy 扩散、命中回调或 POST_DAMAGE 链。
        if amount < 0 {
            self.apply_plain_absorb_heal_into(caster, target, amount.saturating_neg(), updates);
            return false;
        }
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime default attack target: {}", target.0));
        target_entity.runtime.hp = (target_entity.runtime.hp - amount).max(0);
        let was_alive = target_entity.runtime.alive;
        updates.add_with(|| RuntimeFrame::legacy_damage_update(caster.0 as usize, target.0 as usize, amount));
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
            PlainAttackOnDamage::Fire(fire_state_key) if amount > 0 => self.apply_fire_on_damage(target, fire_state_key),
            PlainAttackOnDamage::Fire(_) => {}
            PlainAttackOnDamage::Ice if amount > 0 => self.apply_ice_on_damage(caster, target, updates),
            PlainAttackOnDamage::Ice => {}
            PlainAttackOnDamage::Poison => self.apply_poison_on_damage(caster, target, amount, updates),
            PlainAttackOnDamage::CovidSpread { boss, mutation } => {
                self.infect_with_covid_into(boss, target, mutation, updates);
            }
        }
        self.drain_plain_post_damage_skill_chain_into(target, amount, caster, updates);
        was_alive
            && self
                .entities
                .get(target)
                .is_some_and(|entity| entity.runtime.hp == 0 && entity.runtime.alive)
    }

    pub fn apply_absorb_on_damage(&mut self, caster: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 0 {
            return;
        }
        let owner = self
            .entities
            .get_mut(caster)
            .unwrap_or_else(|| panic!("unknown runtime absorb caster: {}", caster.0));
        if owner.runtime.hp <= 0 {
            return;
        }
        let healed = ((damage + 1) / 2).min(owner.template.max_hp - owner.runtime.hp);
        if healed > 0 {
            owner.runtime.hp = (owner.runtime.hp + healed).min(owner.template.max_hp);
        }
        updates.add(crate::runtime::update::RunUpdate::new(
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
        let charge_active = self.entities.get(caster).is_some_and(|entity| entity.runtime.charge.active);
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
                .unwrap_or_else(|| panic!("unknown runtime berserk target: {}", target.0));
            let StatePayload::Berserk { step } = &mut target_entity
                .states
                .entry_mut(state_key)
                .expect("runtime berserk state disappeared during extension")
                .payload
            else {
                unreachable!("runtime berserk state key changed payload during extension");
            };
            *step += 1 + i32::from(charge_active);
            return;
        }

        assert!(
            self.entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime berserk target: {}", target.0))
                .states
                .add_entry(StateEntry::berserk(PLAIN_BERSERK_STATE_KEY, 1 + i32::from(charge_active),)),
            "runtime berserk state should be inserted"
        );
        updates.add(crate::runtime::update::RunUpdate::new(
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
        let target_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime curse target: {}", target.0))
            .runtime
            .hp;
        if target_hp <= 0 || self.status_immune(target, "curse") {
            return;
        }
        // 默认沿用上游口径：读目标的蓄力状态。
        // `curse_caster_charge` 打开后改读施法者，与狂暴、冰冻等其它受击效果一致。
        #[cfg(not(feature = "curse_caster_charge"))]
        let charge_active = self.entities.get(target).is_some_and(|entity| entity.runtime.charge.active);
        #[cfg(feature = "curse_caster_charge")]
        let charge_active = self.entities.get(caster).is_some_and(|entity| entity.runtime.charge.active);

        let curse_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_CURSE_STATE_EXPORT)
            .expect("default runtime profile must register curse state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime curse target: {}", target.0));
        let charge_prob = if charge_active { 10 } else { 0 };
        let charge_multiply = if charge_active { 1 } else { 0 };
        if let Some(entry) = target_entity.states.entry(PLAIN_CURSE_STATE_KEY) {
            let StatePayload::Curse { prob, multiply } = entry.payload else {
                panic!("runtime curse state key is occupied by another payload");
            };
            assert!(
                target_entity.states.set_payload(
                    PLAIN_CURSE_STATE_KEY,
                    StatePayload::Curse {
                        prob: prob + 10 + charge_prob,
                        multiply: multiply + 1 + charge_multiply,
                    },
                ),
                "runtime curse state disappeared while stacking"
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
                "runtime curse state key should be vacant"
            );
            // legacy 的 `set_state(CurseState)` 会立即调用 `update_states()`。
            // 除了让诅咒的 atk_sum 倍率生效，这次完整重算也会提交疾走中
            // 等待下一次属性刷新的 faster 值。
            target_entity.refresh_runtime_stats_from_template();
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[1]被[诅咒]了",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::RuntimeRunner;

    #[test]
    fn aokiji_only_nullifies_the_physical_default_attack_callback() {
        // 原版首回合：d 的物理普攻命中但清零，alice 的魔法普攻和索尼克连击实伤。
        for (opponent, expected_hp, expected_move_point, expected_mp) in
            [("d", 394, 1618, 31), ("alice", 368, 1116, 35), ("sonic@!", 326, 815, 10)]
        {
            let mut runner = RuntimeRunner::new_from_namerena_raw(format!("aokiji@!\n\n{opponent}")).unwrap();
            let aokiji = runner.input_groups[0][0];
            let attacker = runner.input_groups[1][0];
            let outcome = (0..32)
                .map(|_| runner.run_round())
                .find(|outcome| outcome.frame.is_some())
                .expect("应产生首次可见的对战帧");
            let action = outcome.action.expect("对手应完成首次行动");
            assert_eq!(action.actor, attacker, "{opponent}");
            let frame = outcome.frame.unwrap();
            let zero_damage = frame
                .updates
                .updates
                .iter()
                .any(|update| update.target == aokiji.0 as usize && update.message == "[0]受到[2]点伤害[s_dmg0]");
            assert_eq!(zero_damage, opponent == "d", "{opponent}");

            let boss = &runner.runtime.entities.get(aokiji).unwrap().runtime;
            assert_eq!(
                (boss.hp, boss.move_state.speed_points),
                (expected_hp, expected_move_point),
                "{opponent}"
            );
            let player = &runner.runtime.entities.get(attacker).unwrap().runtime;
            assert_eq!(player.magic_point, expected_mp, "{opponent}");
        }
    }

    #[test]
    fn saitama_only_resets_speed_points_when_his_punch_does_not_end_the_battle() {
        // 原版最后一组敌人退场时直接结束动作；马里奥前两次复活仍会触发清条。
        for (opponents, expected) in [("Bob\nAlice", &[1700, 164][..]), ("mario@!", &[1700, 1700, 420][..])] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(format!("saitama@!\n\n{opponents}")).unwrap();
            let actor = runner.input_groups[0][0];
            let mut points = Vec::new();
            for _ in 0..300 {
                let outcome = runner.run_round();
                if outcome.frame.is_some_and(|frame| {
                    frame
                        .updates
                        .updates
                        .iter()
                        .any(|update| update.caster == actor.0 as usize && update.message == "[0]发起攻击")
                }) {
                    let boss = &runner.runtime.entities.get(actor).unwrap().runtime;
                    points.push(boss.move_state.speed_points);
                }
                if outcome.winner_team.is_some() {
                    break;
                }
            }
            assert_eq!(points, expected, "{opponents}");
        }
    }
}

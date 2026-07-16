use super::*;

impl CombatRuntime {
    pub fn drain_plain_half_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[瘟疫]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));

        let (owner_wisdom, owner_magic, charge_active) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 half actor: {}", actor.0));
            (
                owner.runtime.wisdom,
                owner.runtime.magic,
                owner.runtime.at_boost_millionths >= 3_000_000,
            )
        };
        let (target_hp, target_resistance, target_agility, target_flags, target_name, target_active) = {
            let target_entity = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 half target: {}", target.0));
            (
                target_entity.runtime.hp,
                target_entity.runtime.resistance,
                target_entity.runtime.agility,
                target_entity.runtime.flags,
                target_entity.template.name.clone(),
                target_entity.is_active(),
            )
        };
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "half");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        let chance = (owner_wisdom + ((360 - target_hp) / 3)).max(0);
        if immune
            || (target_active
                && !charge_active
                && PlayerRuntime::dodge(chance, target_resistance + target_agility, &mut self.rng))
        {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        let mut percent = ((owner_magic - (target_resistance / 2)) / 2) + 47;
        if charge_active {
            percent = owner_magic + 50;
        }
        percent = percent.min(99);
        let new_hp = ((target_hp as f64) * (100 - percent) as f64 / 100.0).ceil() as i32;
        let damage = (target_hp - new_hp).max(0);
        let mut update =
            crate::engine::update::RunUpdate::new("[1]体力减少[2]%", actor.0 as usize, target.0 as usize, damage as u32);
        update.param = Some(percent.max(0) as u32);
        updates.add(update);
        if damage <= 0 {
            return;
        }

        self.entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime_v2 half target disappeared: {}", target.0))
            .runtime
            .hp = new_hp;
        self.drain_plain_post_damage_skill_chain_into(target, damage, actor, updates);
        // 瘟疫本身按百分比保底留下 1 HP，但 on_damaged 中的使魔分摊可能击倒 owner，
        // 并把当前活动使魔标记为 0 HP，等待外层伤害调用方完成致死链。
        let target_needs_lethal = self
            .entities
            .get(target)
            .is_some_and(|entity| entity.runtime.hp <= 0 && entity.runtime.alive);
        if target_needs_lethal {
            self.drain_plain_lethal_damage_into(actor, target, updates);
        }
    }

    pub fn select_plain_ice_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
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
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_ice_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_ice_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 ice target: {}", target.0));
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
                (1.0 / rate_hi_hp(entity.runtime.hp)) * entity.runtime.atk_sum as f64 * entity.runtime.attract()
            }
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        };
        if entity.states.ice_frozen_step(PLAIN_ICE_STATE_KEY).is_some() {
            score /= 2.0;
        }
        score
    }

    pub fn drain_plain_ice_skill_into(&mut self, actor: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let atp = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 ice actor: {}", actor.0))
            .runtime
            .get_at(true, &mut self.rng)
            * crate::player::skill::act::ice::ICE_DAMAGE_MULTIPLIER;
        updates.add(RuntimeFrame::replay_update(
            actor.0 as usize,
            target.0 as usize,
            "[0]使用[冰冻术]",
            1,
        ));
        self.drain_plain_attack_with_atp_and_on_damage_into(actor, target, true, atp, PlainAttackOnDamage::Ice, updates);
    }

    pub fn drain_plain_charge_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 charge owner: {}", actor.0));
        owner.activate_charge_runtime();
        owner.runtime.magic_point += 32;
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]开始[蓄力]",
            actor.0 as usize,
            actor.0 as usize,
            1,
        ));
    }

    pub fn drain_plain_accumulate_skill_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let owner = self
            .entities
            .get_mut(actor)
            .unwrap_or_else(|| panic!("unknown runtime_v2 accumulate owner: {}", actor.0));
        if !owner.activate_accumulate_runtime() {
            return;
        }
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]开始[聚气]",
            actor.0 as usize,
            actor.0 as usize,
            1,
        ));
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]攻击力上升",
            actor.0 as usize,
            actor.0 as usize,
            0,
        ));
    }

    pub fn drain_plain_clone_skill_into(&mut self, actor: EntityIdx, fixed_lane: usize, updates: &mut RunUpdates) {
        let current_level = self
            .entities
            .get(actor)
            .and_then(|entity| entity.template.skills.level_at(fixed_lane))
            .unwrap_or_else(|| panic!("runtime_v2 clone level missing for fixed lane {fixed_lane}"));
        // eager 路径已经有三类蓝图；score 的延迟路径必须在本体属性衰减前补齐，
        // 克隆体随后才能继承与旧初始化顺序完全相同的模板。
        self.ensure_plain_minion_blueprint(actor, crate::player::skill::act::minion::MinionKind::Shadow);
        self.ensure_plain_minion_blueprint(actor, crate::player::skill::act::minion::MinionKind::Summon);
        self.ensure_plain_minion_blueprint(actor, crate::player::skill::act::minion::MinionKind::Zombie);
        let shadow_blueprint_slot = self.registry.entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT);
        let summon_blueprint_slot = self.registry.entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT);
        let zombie_blueprint_slot = self.registry.entity_slot_id_by_export_name(DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT);
        let clone_kind = self
            .registry
            .player_kind_id_by_export_name(DEFAULT_CORE_CLONE_KIND_EXPORT)
            .expect("default runtime v2 profile must register core clone kind");
        let random_factor = (u32::from(self.rng.next_u8()) & 63) + 64;
        let mut decayed_level = ((current_level as f64) * random_factor as f64 / 128.0).ceil() as u32;
        let charge_active = self
            .entities
            .get(actor)
            .is_some_and(|entity| entity.runtime.at_boost_millionths >= 3_000_000);

        if !charge_active {
            let owner = self
                .entities
                .get_mut(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 clone actor: {}", actor.0));
            let old_max_hp = owner.template.max_hp.max(1);
            let next_hp = (((owner.runtime.hp as f64) * 0.5).ceil() as i32).clamp(1, old_max_hp);
            let build = owner
                .template
                .clone_build
                .as_mut()
                .unwrap_or_else(|| panic!("runtime_v2 clone build data missing for entity {}", actor.0));
            build.decay_owner();
            let stats = build.derive_stats();
            owner.apply_derived_stats(stats);
            owner.runtime.hp = next_hp;
        }

        let (
            root_owner,
            root_name,
            owner_display_name,
            owner_team,
            owner_hp,
            owner_magic,
            mut clone_skills,
            clone_build,
            shadow_blueprint,
            summon_blueprint,
            zombie_blueprint,
        ) = {
            let owner = self
                .entities
                .get(actor)
                .unwrap_or_else(|| panic!("unknown runtime_v2 clone actor: {}", actor.0));
            let root_owner = owner.runtime.root_owner;
            let root_name = self
                .entities
                .get(root_owner)
                .unwrap_or_else(|| panic!("unknown runtime_v2 clone root owner: {}", root_owner.0))
                .template
                .name
                .clone();
            let clone_build = owner
                .template
                .clone_build
                .as_ref()
                .unwrap_or_else(|| panic!("runtime_v2 clone build data missing for entity {}", actor.0))
                .child();
            let shadow_blueprint = shadow_blueprint_slot.and_then(|slot| match owner.slots.get(slot) {
                Some(SlotValue::PlayerTemplate(template)) => Some(template.as_ref().clone()),
                Some(_) => panic!("runtime_v2 core shadow blueprint slot has invalid value"),
                None => None,
            });
            let summon_blueprint = summon_blueprint_slot.and_then(|slot| match owner.slots.get(slot) {
                Some(SlotValue::PlayerTemplate(template)) => Some(template.as_ref().clone()),
                Some(_) => panic!("runtime_v2 core summon blueprint slot has invalid value"),
                None => None,
            });
            let zombie_blueprint = zombie_blueprint_slot.and_then(|slot| match owner.slots.get(slot) {
                Some(SlotValue::PlayerTemplate(template)) => Some(template.as_ref().clone()),
                Some(_) => panic!("runtime_v2 core zombie blueprint slot has invalid value"),
                None => None,
            });
            (
                root_owner,
                root_name,
                owner.template.display_name.clone(),
                owner.runtime.team,
                owner.runtime.hp,
                owner.runtime.magic,
                owner.template.skills.rebuilt_for_clone(),
                clone_build,
                shadow_blueprint,
                summon_blueprint,
                zombie_blueprint,
            )
        };
        let clone_move_points = self.rng.r255() as i32 * 4 + 256;
        if owner_hp + owner_magic < self.rng.r255() as i32 {
            decayed_level = (decayed_level >> 1) + 1;
        }
        let cloned_clone_level = (decayed_level as f64).sqrt().ceil() as u32;
        let clone_skill_was_zero = clone_skills.level_at(fixed_lane) == Some(0);
        assert!(
            clone_skills.set_level_at(fixed_lane, cloned_clone_level.max(1)),
            "runtime_v2 clone fixed lane disappeared while building child"
        );
        if clone_skill_was_zero {
            clone_skills.disable_action_lane(fixed_lane);
        }
        assert!(
            self.entities
                .get_mut(actor)
                .unwrap()
                .template
                .skills
                .set_level_at(fixed_lane, decayed_level.max(1)),
            "runtime_v2 clone fixed lane disappeared while updating owner"
        );

        let counter_slot = self
            .registry
            .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
            .expect("default runtime v2 profile must register core minion counter slot");
        let next_minion_index = match self
            .entities
            .get(root_owner)
            .unwrap_or_else(|| panic!("unknown runtime_v2 clone root owner: {}", root_owner.0))
            .slots
            .get(counter_slot)
        {
            Some(SlotValue::U64(next)) => *next,
            Some(_) => panic!("runtime_v2 core minion counter slot has invalid value"),
            None => 0,
        };
        self.entities
            .get_mut(root_owner)
            .unwrap()
            .slots
            .set(counter_slot, SlotValue::U64(next_minion_index + 1))
            .expect("runtime_v2 core minion counter slot must exist");

        let clone_stats = clone_build.derive_stats();
        let next_entity = self.entities.len();
        let mut clone_template = PlayerTemplate::with_kind(
            next_entity + 1,
            format!("{root_name}?{next_minion_index}"),
            clone_kind,
            owner_team,
            clone_stats.max_hp.max(1),
            clone_stats.attack.max(0),
        )
        .with_display_name(owner_display_name)
        .with_magic(clone_stats.magic.max(0))
        .with_magic_point((clone_stats.wisdom >> 1).max(0))
        .with_wisdom(clone_stats.wisdom.max(0))
        .with_speed(clone_stats.speed.max(0))
        .with_def_res(clone_stats.defense.max(0), clone_stats.resistance.max(0))
        .with_agility(clone_stats.agility.max(0))
        .with_at_boost(f64::from_bits(clone_stats.at_boost_bits).max(0.0))
        .with_target_score_stats(
            clone_stats.attr_sum,
            clone_stats.atk_sum,
            f64::from_bits(clone_stats.attract_bits),
        )
        .with_speed_points(clone_move_points)
        .with_skill_loadout(clone_skills);
        clone_template.clone_build = Some(clone_build);
        let clone_idx = EntityIdx(next_entity.try_into().expect("runtime_v2 clone entity index overflow"));

        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[分身]",
            actor.0 as usize,
            actor.0 as usize,
            60,
        ));
        self.effects.push(QueuedEffect::SpawnWithMessage {
            caster: actor,
            template: clone_template,
            message: "出现一个新的[1]".to_owned(),
        });
        self.drain_effects_into(updates);
        let clone_entity = self
            .entities
            .get_mut(clone_idx)
            .unwrap_or_else(|| panic!("runtime_v2 clone spawn missing entity {}", clone_idx.0));
        clone_entity.runtime.hp = owner_hp.max(1);
        if let (Some(slot), Some(template)) = (shadow_blueprint_slot, shadow_blueprint) {
            clone_entity
                .slots
                .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                .expect("runtime_v2 core shadow blueprint slot must exist");
        }
        if let (Some(slot), Some(template)) = (summon_blueprint_slot, summon_blueprint) {
            clone_entity
                .slots
                .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                .expect("runtime_v2 core summon blueprint slot must exist");
        }
        if let (Some(slot), Some(template)) = (zombie_blueprint_slot, zombie_blueprint) {
            clone_entity
                .slots
                .set(slot, SlotValue::PlayerTemplate(Box::new(template)))
                .expect("runtime_v2 core zombie blueprint slot must exist");
        }
    }

    pub fn select_plain_exchange_targets(&mut self, actor: EntityIdx, smart: bool) -> PreparedTargetList {
        let actor_hp = self
            .entities
            .get(actor)
            .map(|entity| entity.runtime.hp)
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange actor: {}", actor.0));
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
                if smart {
                    entity.runtime.hp - actor_hp > 32
                } else {
                    entity.runtime.hp > actor_hp
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
        let mut scored = smallvec::SmallVec::<[(EntityIdx, f64); 3]>::new();
        for target in selected {
            scored.push((target, self.score_plain_exchange_target(target, smart)));
        }
        scored.sort_by(|lhs, rhs| rhs.1.partial_cmp(&lhs.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(target, _)| target).collect()
    }

    pub fn score_plain_exchange_target(&mut self, target: EntityIdx, smart: bool) -> f64 {
        let entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange target: {}", target.0));
        let rate_hi_hp = |hp: i32| -> f64 {
            if hp < 20 {
                30.0
            } else if hp > 300 {
                300.0
            } else {
                hp as f64
            }
        };
        if smart {
            let base = if self.world.alive_group_count() > 2 {
                rate_hi_hp(entity.runtime.hp) * self.world.alive_group_len_containing(target) as f64 * entity.runtime.attract()
            } else {
                rate_hi_hp(entity.runtime.hp) * entity.runtime.attr_sum as f64 * entity.runtime.attract()
            };
            base * entity.runtime.hp as f64
        } else {
            self.rng.rFFFF() as f64 + entity.runtime.attract()
        }
    }

    pub fn drain_plain_exchange_skill_into(
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
            .unwrap_or_else(|| panic!("runtime_v2 exchange level missing for fixed lane {fixed_lane}"));
        assert!(
            self.entities
                .get_mut(actor)
                .unwrap()
                .template
                .skills
                .set_level_at(fixed_lane, (current_level + 1) >> 1),
            "runtime_v2 exchange fixed lane disappeared during action"
        );
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]使用[生命之轮]",
            actor.0 as usize,
            target.0 as usize,
            1,
        ));

        let (owner_magic, charge_active, owner_hp, owner_max_hp) = self
            .entities
            .get(actor)
            .map(|owner| {
                (
                    owner.runtime.magic,
                    owner.runtime.at_boost_millionths >= 3_000_000,
                    owner.runtime.hp,
                    owner.template.max_hp,
                )
            })
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange actor: {}", actor.0));
        let (target_flags, target_name, target_res, target_def, target_agl, target_hp, target_active) = self
            .entities
            .get(target)
            .map(|target_entity| {
                (
                    target_entity.runtime.flags,
                    target_entity.template.name.clone(),
                    target_entity.runtime.resistance,
                    target_entity.runtime.defense,
                    target_entity.runtime.agility,
                    target_entity.runtime.hp,
                    target_entity.is_active(),
                )
            })
            .unwrap_or_else(|| panic!("unknown runtime_v2 exchange target: {}", target.0));
        let immune = if target_flags.contains(PlayerKindFlags::BOOST) {
            self.rng.r127() < crate::player::boost_value(&target_name)
        } else if target_flags.contains(PlayerKindFlags::BOSS) {
            let threshold = crate::player::boss::boss_immune_threshold(&target_name, "exchange");
            (self.rng.next_u8() as i32) < threshold
        } else {
            false
        };
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_EXCHANGE").is_some() {
            let owner = self.entities.get(actor).expect("runtime_v2 exchange owner missing for probe");
            let target_entity = self.entities.get(target).expect("runtime_v2 exchange target missing for probe");
            eprintln!(
                "[exchange_probe:v2:before] round={} owner={} target={} owner_hp={} target_hp={} owner_max_hp={} \
                 owner_magic={} owner_boost={} charge={} owner_move={} target_move={} target_active={} immune={} rc4=({}, {})",
                self.round + 1,
                owner.template.name,
                target_entity.template.name,
                owner_hp,
                target_hp,
                owner_max_hp,
                owner_magic,
                owner.runtime.at_boost(),
                charge_active,
                owner.runtime.move_state.speed_points,
                target_entity.runtime.move_state.speed_points,
                target_active,
                immune,
                self.rng.i,
                self.rng.j,
            );
        }
        if immune
            || (target_active
                && !charge_active
                && PlayerRuntime::dodge(owner_magic, target_res + target_def + target_agl, &mut self.rng))
        {
            updates.add(crate::engine::update::RunUpdate::new(
                "[0][回避]了攻击",
                target.0 as usize,
                actor.0 as usize,
                20,
            ));
            return;
        }

        if charge_active {
            let target_move_points = self
                .entities
                .get(target)
                .unwrap_or_else(|| panic!("unknown runtime_v2 exchange target: {}", target.0))
                .runtime
                .move_state
                .speed_points;
            self.entities.get_mut(actor).unwrap().runtime.move_state.speed_points += target_move_points;
            self.entities.get_mut(target).unwrap().runtime.move_state.speed_points = 0;
        }

        self.entities.get_mut(actor).unwrap().runtime.hp = target_hp.min(owner_max_hp);
        self.entities.get_mut(target).unwrap().runtime.hp = owner_hp;
        updates.add(crate::engine::update::RunUpdate::new(
            "[1]的体力值与[0]互换",
            actor.0 as usize,
            target.0 as usize,
            ((target_hp - owner_hp) * 2).max(0) as u32,
        ));
        if target_hp > owner_hp {
            self.drain_plain_post_damage_skill_chain_into(target, target_hp - owner_hp, actor, updates);
            // 生命之轮直接改写 HP 后仍会调用 legacy on_damaged；若伤害分摊等
            // post_damage 回调把目标压到 0 HP，必须在当前技能内完成死亡与 KILL 链。
            let target_needs_lethal = self
                .entities
                .get(target)
                .is_some_and(|entity| entity.runtime.hp <= 0 && entity.runtime.alive);
            if target_needs_lethal {
                self.drain_plain_lethal_damage_into(actor, target, updates);
            }
        }
        #[cfg(not(feature = "no_debug"))]
        if std::env::var_os("TSWN_PROBE_EXCHANGE").is_some() {
            let owner = self.entities.get(actor).expect("runtime_v2 exchange owner missing after probe");
            let target_entity = self.entities.get(target).expect("runtime_v2 exchange target missing after probe");
            eprintln!(
                "[exchange_probe:v2:after] round={} owner_hp={} target_hp={} owner_move={} target_move={} rc4=({}, {})",
                self.round + 1,
                owner.runtime.hp,
                target_entity.runtime.hp,
                owner.runtime.move_state.speed_points,
                target_entity.runtime.move_state.speed_points,
                self.rng.i,
                self.rng.j,
            );
        }
    }
}

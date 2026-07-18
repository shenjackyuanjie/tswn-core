use super::*;

impl CombatRuntime {
    pub fn selected_pre_action_target(&mut self, plan: &SkillHookPlan, actor: EntityIdx, smart: bool) -> Option<EntityIdx> {
        let has_disperse = plan.entries.iter().any(|entry| {
            self.registry
                .skill(entry.skill_id)
                .is_some_and(|spec| spec.export_name == "core.disperse")
        });
        if !has_disperse {
            return None;
        }
        select_disperse_targets(&self.entities, &self.world, actor, smart, &mut self.rng)
            .into_iter()
            .next()
    }

    #[cfg(test)]
    pub fn flush_effects(&mut self) -> Option<RuntimeFrame> {
        let mut updates = RunUpdates::new();
        self.drain_effects_into(&mut updates);
        updates.had_updates().then_some(RuntimeFrame { updates })
    }

    pub fn drain_effects_into(&mut self, updates: &mut RunUpdates) {
        while let Some(effect) = self.effects.pop_next() {
            match effect {
                QueuedEffect::Damage { caster, target, amount } => {
                    self.ensure_effect_entity("damage", "caster", caster);
                    self.ensure_effect_entity("damage", "target", target);
                    let resolved_target = self.resolve_damage_target(target);
                    self.ensure_effect_entity("damage", "resolved target", resolved_target);
                    let share_targets = self.resolve_damage_share_targets(target, resolved_target);
                    if self.apply_damage_into(caster, resolved_target, amount, updates) {
                        self.drain_lethal_damage_hooks_into(caster, resolved_target, updates);
                    }
                    for share_target in share_targets {
                        self.ensure_effect_entity("damage", "share target", share_target);
                        if self.apply_damage_into(caster, share_target, amount, updates) {
                            self.drain_lethal_damage_hooks_into(caster, share_target, updates);
                        }
                    }
                }
                QueuedEffect::ReflectedAttack {
                    caster,
                    target,
                    atp_bits,
                    on_damage,
                } => {
                    self.ensure_effect_entity("reflected attack", "caster", caster);
                    self.ensure_effect_entity("reflected attack", "target", target);
                    self.drain_plain_attack_with_atp_and_on_damage_into(
                        caster,
                        target,
                        true,
                        f64::from_bits(atp_bits),
                        on_damage,
                        updates,
                    );
                    self.entities
                        .get_mut(caster)
                        .expect("runtime reflected attack caster disappeared")
                        .runtime
                        .move_state
                        .speed_points -= 480;
                }
                QueuedEffect::PoisonTick { caster, target, amount } => {
                    self.ensure_effect_entity("poison tick", "caster", caster);
                    self.ensure_effect_entity("poison tick", "target", target);
                    if self.apply_poison_tick_damage_into(caster, target, amount, updates) {
                        self.drain_plain_lethal_damage_into(caster, target, updates);
                    }
                    // 最后一跳毒伤即使一度致死，也可能被护身符在 DIE 钩子中救回。
                    // legacy 会在整条死亡链结束后按最终存活状态决定是否输出中毒解除。
                    if self.entities.get(target).map(|entity| entity.runtime.alive).unwrap_or(false) {
                        self.emit_poison_release_if_cleared(target, updates);
                    }
                }
                QueuedEffect::FireAttack {
                    caster,
                    target,
                    fire_state_key,
                } => {
                    self.ensure_effect_entity("fire-attack", "caster", caster);
                    self.ensure_effect_entity("fire-attack", "target", target);
                    let fire_mag = self.entities.get(target).unwrap().states.fire_mag(fire_state_key);
                    let atp = self.entities.get(caster).unwrap().runtime.get_at(true, &mut self.rng);
                    let mut defend_value = RuntimeDefendValue::Atp {
                        value: atp * (1.5 + fire_mag),
                        caster,
                        target,
                        is_magic: true,
                    };
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        "[0]使用[火球术]",
                        1,
                    ));
                    self.drain_pre_defend_hooks_with_on_damage_into(
                        target,
                        updates,
                        &mut defend_value,
                        PlainAttackOnDamage::Fire(fire_state_key),
                    );
                    let Some(atp) = defend_value.atp() else {
                        panic!("runtime PRE_DEFEND hooks must leave an atp value");
                    };
                    if atp == 0.0 {
                        continue;
                    }
                    if self.magic_attack_dodged(caster, target) {
                        updates.add(RuntimeFrame::replay_update(
                            target.0 as usize,
                            caster.0 as usize,
                            "[0][回避]了攻击",
                            20,
                        ));
                    } else {
                        let amount = (atp / self.entities.get(target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
                        let mut defend_value = RuntimeDefendValue::Damage {
                            value: amount,
                            caster,
                            target,
                        };
                        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
                        let Some(amount) = defend_value.damage() else {
                            panic!("runtime POST_DEFEND hooks must leave a damage value");
                        };
                        if self.apply_plain_legacy_damage_into(caster, target, amount, updates) {
                            self.drain_plain_lethal_damage_into(caster, target, updates);
                        } else if amount > 0 {
                            self.apply_fire_on_damage(target, fire_state_key);
                        }
                    }
                }
                QueuedEffect::SummonExplode {
                    caster,
                    target,
                    fire_state_key,
                } => {
                    self.ensure_effect_entity("summon-explode", "caster", caster);
                    self.ensure_effect_entity("summon-explode", "target", target);
                    let fire_mag = self.entities.get(target).unwrap().states.fire_mag(fire_state_key);
                    let atp = self.entities.get(caster).unwrap().runtime.get_at(true, &mut self.rng);
                    let mut defend_value = RuntimeDefendValue::Atp {
                        value: atp * (4.0 + fire_mag),
                        caster,
                        target,
                        is_magic: true,
                    };
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        "[0]使用[自爆]",
                        0,
                    ));
                    let killed_caster = self.kill_entity_without_damage_mark_only_into(caster);
                    self.drain_pre_defend_hooks_into(target, updates, &mut defend_value);
                    let Some(atp) = defend_value.atp() else {
                        panic!("runtime PRE_DEFEND hooks must leave an atp value");
                    };
                    if atp == 0.0 {
                        if killed_caster {
                            self.drain_summon_explode_self_death_into(caster, updates);
                        }
                        continue;
                    }
                    if self.magic_attack_dodged(caster, target) {
                        updates.add(RuntimeFrame::replay_update(
                            target.0 as usize,
                            caster.0 as usize,
                            "[0][回避]了攻击",
                            20,
                        ));
                    } else {
                        let amount = (atp / self.entities.get(target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
                        let mut defend_value = RuntimeDefendValue::Damage {
                            value: amount,
                            caster,
                            target,
                        };
                        self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
                        let Some(amount) = defend_value.damage() else {
                            panic!("runtime POST_DEFEND hooks must leave a damage value");
                        };
                        if self.apply_plain_legacy_damage_into(caster, target, amount, updates) {
                            self.drain_plain_lethal_damage_into(caster, target, updates);
                        } else if amount > 0 {
                            self.apply_fire_on_damage(target, fire_state_key);
                        }
                    }
                    if killed_caster {
                        self.drain_summon_explode_self_death_into(caster, updates);
                    }
                }
                QueuedEffect::DisperseAttack { caster, target } => {
                    self.ensure_effect_entity("disperse-attack", "caster", caster);
                    self.ensure_effect_entity("disperse-attack", "target", target);
                    let mut atp = self.entities.get(caster).unwrap().runtime.get_at(true, &mut self.rng);
                    if self.entities.get(target).unwrap().runtime.is_combat_minion() {
                        atp *= 2.0;
                    }
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        "[0]使用[净化]",
                        20,
                    ));
                    // legacy 的 `DisperseSkill::act_with_level` 会刻意调用 `Player::defned`，
                    // 而不是 `Player::attacked`。因此驱散完全跳过 PRE_DEFEND 和闪避，
                    // 但仍会在结算伤害前执行 POST_DEFEND。
                    let amount = (atp / self.entities.get(target).unwrap().runtime.magic_defense() as f64).ceil() as i32;
                    let mut defend_value = RuntimeDefendValue::Damage {
                        value: amount,
                        caster,
                        target,
                    };
                    self.drain_post_defend_hooks_into(target, updates, &mut defend_value);
                    let Some(amount) = defend_value.damage() else {
                        panic!("runtime POST_DEFEND hooks must leave a damage value");
                    };
                    if self.apply_disperse_attack_damage_into(caster, target, amount, updates) {
                        self.drain_plain_lethal_damage_into(caster, target, updates);
                    }
                }
                QueuedEffect::DisperseHit { caster, target, damage } => {
                    self.ensure_effect_entity("disperse-hit", "caster", caster);
                    self.ensure_effect_entity("disperse-hit", "target", target);
                    if damage > 0 {
                        self.apply_disperse_hit_into(caster, target, updates);
                    }
                }
                QueuedEffect::CovidContact {
                    owner,
                    candidate,
                    boss,
                    mutation,
                } => {
                    self.ensure_effect_entity("covid-contact", "owner", owner);
                    self.ensure_effect_entity("covid-contact", "candidate", candidate);
                    self.ensure_effect_entity("covid-contact", "boss", boss);
                    let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
                    let candidate_entity = self.entities.get(candidate).unwrap();
                    let candidate_name = candidate_entity.template.display_name.clone();
                    let threshold = candidate_entity.runtime.wisdom >> 1;
                    updates.add(crate::engine::update::RunUpdate::new(
                        format!("{owner_name}和{candidate_name}近距离接触"),
                        owner.0 as usize,
                        candidate.0 as usize,
                        0,
                    ));
                    if i32::from(self.rng.next_u8()) < threshold {
                        updates.add(crate::engine::update::RunUpdate::new(
                            format!("但{candidate_name}没被感染"),
                            owner.0 as usize,
                            candidate.0 as usize,
                            0,
                        ));
                    } else {
                        self.infect_with_covid_into(boss, candidate, mutation, updates);
                    }
                }
                QueuedEffect::CovidAttack {
                    owner,
                    candidate,
                    boss,
                    mutation,
                } => {
                    self.ensure_effect_entity("covid-attack", "owner", owner);
                    self.ensure_effect_entity("covid-attack", "candidate", candidate);
                    self.ensure_effect_entity("covid-attack", "boss", boss);
                    updates.add(RuntimeFrame::replay_update(
                        owner.0 as usize,
                        candidate.0 as usize,
                        "[0]发起攻击",
                        0,
                    ));
                    let atp = self.entities.get(owner).unwrap().runtime.get_at(false, &mut self.rng);
                    self.drain_plain_attack_with_atp_and_covid_into(
                        owner,
                        candidate,
                        false,
                        atp,
                        Some((boss, mutation)),
                        updates,
                    );
                }
                QueuedEffect::CovidPneumonia { owner, boss, mutation } => {
                    self.ensure_effect_entity("covid-pneumonia", "owner", owner);
                    self.ensure_effect_entity("covid-pneumonia", "boss", boss);
                    self.drain_covid_pneumonia_into(owner, boss, mutation, updates);
                }
                QueuedEffect::LazyFlare { owner, boss } => {
                    self.ensure_effect_entity("lazy-flare", "owner", owner);
                    self.ensure_effect_entity("lazy-flare", "boss", boss);
                    self.drain_lazy_flare_into(owner, boss, updates);
                }
                QueuedEffect::Heal { caster, target, amount } => {
                    self.ensure_effect_entity("heal", "caster", caster);
                    self.ensure_effect_entity("heal", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime heal target entity: {}", target.0);
                    };
                    let was_alive = target_entity.runtime.alive;
                    target_entity.runtime.hp = (target_entity.runtime.hp + amount.max(0)).min(target_entity.template.max_hp);
                    if target_entity.runtime.hp > 0 {
                        target_entity.runtime.alive = true;
                    }
                    let team = target_entity.runtime.team;
                    updates.add(RuntimeFrame::heal_update(caster.0 as usize, target.0 as usize, amount));
                    if !was_alive && target_entity.runtime.alive {
                        self.world.revive_round_actor(target);
                        self.world.revive_alive(target, team);
                    }
                }
                QueuedEffect::Spawn { caster, template } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                    updates.add(RuntimeFrame::spawn_update(caster.0 as usize, spawned.0 as usize));
                }
                QueuedEffect::SpawnSilent { caster, template } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                }
                QueuedEffect::SpawnWithMessage {
                    caster,
                    template,
                    message,
                } => {
                    self.ensure_effect_entity("spawn", "caster", caster);
                    let root_owner = self.entities.get(caster).unwrap().runtime.root_owner;
                    let spawned =
                        self.entities
                            .spawn_from_template_with_owner(template, &self.registry, Some(caster), Some(root_owner));
                    let team = self.entities.get(spawned).unwrap().runtime.team;
                    self.world.add_spawned_alive(spawned, team);
                    updates.add(RuntimeFrame::replay_update(caster.0 as usize, spawned.0 as usize, message, 0));
                }
                QueuedEffect::AddState { target, state } => {
                    self.ensure_effect_entity("add-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime add-state target entity: {}", target.0);
                    };
                    if target_entity.states.add_entry(state) {
                        updates.add(RuntimeFrame::add_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::AddBerserkState {
                    target,
                    legacy_order_key,
                    step,
                } => {
                    self.ensure_effect_entity("add-berserk-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime add-berserk-state target entity: {}", target.0);
                    };
                    let next_step = target_entity
                        .states
                        .entry(legacy_order_key)
                        .and_then(|entry| match entry.payload {
                            StatePayload::Berserk { step: existing_step } => Some(existing_step + step),
                            _ => None,
                        })
                        .unwrap_or(step);
                    if !target_entity
                        .states
                        .set_payload(legacy_order_key, StatePayload::Berserk { step: next_step })
                    {
                        target_entity.states.add_entry(StateEntry::berserk(legacy_order_key, next_step));
                    }
                }
                QueuedEffect::ClearState {
                    target,
                    legacy_order_key,
                } => {
                    self.ensure_effect_entity("clear-state", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime clear-state target entity: {}", target.0);
                    };
                    if target_entity.states.clear_legacy_key(legacy_order_key) {
                        updates.add(RuntimeFrame::clear_state_update(target.0 as usize));
                    }
                }
                QueuedEffect::Revive { caster, target, hp } => {
                    self.ensure_effect_entity("revive", "caster", caster);
                    self.ensure_effect_entity("revive", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime revive target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = hp.max(1).min(target_entity.template.max_hp);
                    target_entity.runtime.alive = true;
                    let team = target_entity.runtime.team;
                    self.world.revive_round_actor(target);
                    self.world.revive_alive(target, team);
                    updates.add(RuntimeFrame::revive_update(caster.0 as usize, target.0 as usize, hp));
                }
                QueuedEffect::ReviveWithMessage {
                    caster,
                    target,
                    hp,
                    message,
                } => {
                    self.ensure_effect_entity("revive", "caster", caster);
                    self.ensure_effect_entity("revive", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime revive target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = hp.max(1).min(target_entity.template.max_hp);
                    target_entity.runtime.alive = true;
                    let team = target_entity.runtime.team;
                    self.world.revive_round_actor(target);
                    self.world.revive_alive(target, team);
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        message,
                        hp.max(0) as u32,
                    ));
                }
                QueuedEffect::Remove { caster, target } => {
                    self.ensure_effect_entity("remove", "caster", caster);
                    self.ensure_effect_entity("remove", "target", target);
                    let Some(target_entity) = self.entities.get_mut(target) else {
                        panic!("unknown runtime remove target entity: {}", target.0);
                    };
                    target_entity.runtime.hp = 0;
                    target_entity.runtime.alive = false;
                    let team = target_entity.runtime.team;
                    updates.add(RuntimeFrame::remove_update(caster.0 as usize, target.0 as usize));
                    self.mark_dead_with_linked_minions_into(target, team, updates);
                }
                QueuedEffect::Merge { caster, target } => {
                    self.apply_plain_merge_into(caster, target, updates);
                }
                QueuedEffect::Replay {
                    caster,
                    target,
                    message,
                    score,
                } => {
                    self.ensure_effect_entity("replay", "caster", caster);
                    self.ensure_effect_entity("replay", "target", target);
                    updates.add(RuntimeFrame::replay_update(
                        caster.0 as usize,
                        target.0 as usize,
                        message,
                        score,
                    ));
                }
                QueuedEffect::Custom(custom) => {
                    self.ensure_effect_entity("custom", "caster", custom.caster);
                    if let Some(target) = custom.target {
                        self.ensure_effect_entity("custom", "target", target);
                    }
                    let Some(handler) = self.effect_handlers.get(custom.handler) else {
                        panic!("missing runtime effect handler implementation: {}", custom.handler.0);
                    };
                    let capabilities = self.effect_handlers.capabilities(custom.handler).unwrap_or(&[]);
                    let mut context = EffectContext::new(
                        &mut self.entities,
                        &mut self.world,
                        &self.template_slots,
                        &mut self.slots,
                        &mut self.effects,
                        updates,
                        &mut self.rng,
                        &custom,
                        capabilities,
                    );
                    handler(&mut context, &custom);
                }
            }
        }
    }

    pub fn apply_plain_merge_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        self.ensure_effect_entity("merge", "caster", caster);
        self.ensure_effect_entity("merge", "target", target);
        let target_skills = self.entities.get(target).unwrap().template.skills.clone();
        let target_build = self.entities.get(target).unwrap().template.clone_build.clone();
        let target_magic_point = self.entities.get(target).unwrap().runtime.magic_point;
        let target_move_points = self.entities.get(target).unwrap().runtime.move_state.speed_points;
        let (merged, transfer_magic_point, transfer_move_points) = {
            let caster_entity = self
                .entities
                .get_mut(caster)
                .unwrap_or_else(|| panic!("unknown runtime merge caster entity: {}", caster.0));
            let merged_attrs = match (caster_entity.template.clone_build.as_mut(), target_build.as_ref()) {
                (Some(owner_build), Some(target_build)) => owner_build.merge_attrs_from(target_build),
                _ => false,
            };
            if merged_attrs {
                let stats = caster_entity
                    .template
                    .clone_build
                    .as_ref()
                    .expect("runtime merge owner build disappeared")
                    .derive_stats();
                caster_entity.apply_derived_stats(stats);
            }
            let post_action_state_cursor = caster_entity.states.post_action_registration_cursor();
            let before_levels = caster_entity.template.skills.levels().to_vec();
            let merged_skills = caster_entity
                .template
                .skills
                .merge_fixed_lanes_from(&target_skills, caster_entity.runtime.policies.merge);
            if merged_skills {
                let skills = caster_entity.template.skills.skills().to_vec();
                let after_levels = caster_entity.template.skills.levels().to_vec();
                for (lane, (before, after)) in before_levels.iter().zip(after_levels.iter()).enumerate() {
                    if *before != 0 || *after == 0 {
                        continue;
                    }
                    let Some(skill_id) = skills.get(lane).copied() else {
                        continue;
                    };
                    let Some(skill) = self.registry.skill(skill_id) else {
                        continue;
                    };
                    // JS 的 x2 会把 Merge 新启用的 early POST_ACTION 钩子插到当前状态队列尾部，
                    // 后续新增状态仍排在它后面；这里保存当时的状态注册游标，不能只按固定槽位重排。
                    if skill.hook_mask.intersects(ProcMask::POST_ACTION) && skill.post_action_phase == SkillPostActionPhase::Early
                    {
                        caster_entity
                            .template
                            .skills
                            .register_post_action_after_states(lane, post_action_state_cursor);
                    }
                    if skill.export_name == DEFAULT_CORE_HIDE_SKILL_EXPORT {
                        caster_entity.template.skills.ensure_pre_action_lane(lane);
                    }
                }
            }
            let transfer_magic_point = target_magic_point > caster_entity.runtime.magic_point;
            if transfer_magic_point {
                caster_entity.runtime.magic_point = target_magic_point;
            }
            let transfer_move_points = target_move_points > caster_entity.runtime.move_state.speed_points;
            if transfer_move_points {
                caster_entity.runtime.move_state.speed_points += target_move_points;
            }
            (merged_attrs || merged_skills, transfer_magic_point, transfer_move_points)
        };
        if transfer_magic_point || transfer_move_points {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime merge target entity: {}", target.0));
            if transfer_magic_point {
                target_entity.runtime.magic_point = 0;
            }
            if transfer_move_points {
                target_entity.runtime.move_state.speed_points = 0;
            }
        }
        if !merged {
            return false;
        }
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("runtime merge target disappeared: {}", target.0));
        target_entity.states.register_compressed_legacy_state(CompressedLegacyState::Corpse);
        target_entity.runtime.corpse = RuntimeCorpseKind::Merge;
        updates.add_newline();
        updates.add(crate::engine::update::RunUpdate::new(
            "[0][吞噬]了[1]",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
        updates.add(crate::engine::update::RunUpdate::new(
            "[0]属性上升",
            caster.0 as usize,
            target.0 as usize,
            0,
        ));
        true
    }

    pub fn drain_summon_explode_self_death_into(&mut self, caster: EntityIdx, updates: &mut RunUpdates) {
        if self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("runtime summon explode caster disappeared: {}", caster.0))
            .runtime
            .is_combat_minion()
        {
            self.emit_plain_lethal_replay_into(caster, caster, updates);
        }
        self.drain_die_hooks_into(caster, updates);
        let (hp, team) = self
            .entities
            .get(caster)
            .map(|entity| (entity.runtime.hp, entity.runtime.team))
            .unwrap_or_else(|| panic!("runtime summon explode caster disappeared after die hooks: {}", caster.0));
        if hp <= 0 {
            self.entities.get_mut(caster).unwrap().runtime.alive = false;
            self.mark_dead_with_linked_minions_into(caster, team, updates);
        }
    }
}

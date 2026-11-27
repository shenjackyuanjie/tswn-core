use super::*;

impl CombatRuntime {
    /// 预读被动的真实 MP 消耗，不推进随机流；被动使用 r3x3，而非主动技的 8~23。
    pub fn passive_mp_costs(&self, owner: EntityIdx, skill: &str, intercept: bool) -> std::collections::BTreeMap<String, u32> {
        let payable = match skill {
            "core.skill.reflect" | "core.skill.defend" | "core.skill.counter" => Some(skill),
            "core.skill.protect" if intercept => Some(skill),
            "core.skill.merge" | "core.skill.zombie"
                if self.preview_plain_minion_blueprint(owner, crate::namerena::MinionKind::Zombie).is_some() =>
            {
                Some("core.skill.zombie")
            }
            _ => None,
        };
        payable.map_or_else(Default::default, |skill| {
            let mut rng = self.rng.clone();
            [(skill.to_owned(), rng.r3x3())].into_iter().collect()
        })
    }

    /// 检查并支付实体 MP：未激活或不足时返回 false。
    pub fn entity_mp_ready(&mut self, owner: EntityIdx) -> bool {
        let Some(entity) = self.entities.get(owner) else {
            return false;
        };
        if !entity.is_active() {
            return false;
        }
        self.entities
            .get_mut(owner)
            .expect("runtime entity disappeared during mp_ready")
            .runtime
            .mp_ready(&mut self.rng)
    }

    /// 命中伤害超过阈值时叠加中毒：威力累加、层数重置为 4。
    pub fn apply_poison_on_damage(&mut self, caster: EntityIdx, target: EntityIdx, damage: i32, updates: &mut RunUpdates) {
        if damage <= 4 {
            return;
        }
        let target_hp = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime poison target: {}", target.0))
            .runtime
            .hp;
        if target_hp <= 0 || self.status_immune(target, "piston") {
            return;
        }

        let poison_atp = self
            .entities
            .get(caster)
            .unwrap_or_else(|| panic!("unknown runtime poison caster: {}", caster.0))
            .runtime
            .get_at(true, &mut self.rng)
            * 1.2000000476837158;
        let poison_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_POISON_STATE_EXPORT)
            .expect("default runtime profile must register poison state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime poison target: {}", target.0));
        let existing = target_entity.states.entry(PLAIN_POISON_STATE_KEY).map(|entry| match entry.payload {
            StatePayload::Poison {
                caster,
                target,
                atp_bits,
                ..
            } => (caster, target, f64::from_bits(atp_bits)),
            _ => panic!("runtime poison state key is occupied by another payload"),
        });
        if let Some((_, existing_target, existing_atp)) = existing {
            assert!(
                target_entity.states.set_payload(
                    PLAIN_POISON_STATE_KEY,
                    StatePayload::Poison {
                        caster: Some(caster.0),
                        target: existing_target.or(Some(target.0)),
                        atp_bits: (existing_atp + poison_atp).to_bits(),
                        count: 4,
                    },
                ),
                "runtime poison state disappeared while stacking"
            );
        } else {
            assert!(
                target_entity.states.add_entry(StateEntry::poison(
                    PLAIN_POISON_STATE_KEY,
                    poison_state,
                    Some(caster.0),
                    Some(target.0),
                    poison_atp,
                    4,
                    SkillPriority(150),
                )),
                "runtime poison state key should be vacant"
            );
        }
        updates.add(crate::runtime::update::RunUpdate::new(
            "[1][中毒]",
            caster.0 as usize,
            target.0 as usize,
            60,
        ));
    }

    /// 读取新冠 Boss 的当前变异值；未感染或状态缺失返回 None。
    pub fn covid_boss_mutation(&self, boss: EntityIdx) -> Option<i32> {
        let states = &self.entities.get(boss)?.states;
        if !states.may_contain(StatePayloadKind::CovidBoss) {
            return None;
        }
        states.entries().iter().find_map(|entry| {
            let StatePayload::CovidBoss { mutation } = &entry.payload else {
                return None;
            };
            Some(*mutation)
        })
    }

    /// Boss 的主动和被击传播检查感染历史，而非当前活跃株。
    pub fn has_covid_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity.states.may_contain(StatePayloadKind::CovidInfection)
                && entity
                    .states
                    .entries()
                    .iter()
                    .any(|entry| matches!(&entry.payload, StatePayload::CovidInfection(_)))
        })
    }

    /// 读取懒癌 Boss 当前的攻击加成；未设置返回 None。
    pub fn lazy_boss_at_boost(&self, boss: EntityIdx) -> Option<f64> {
        let states = &self.entities.get(boss)?.states;
        if !states.may_contain(StatePayloadKind::LazyBoss) {
            return None;
        }
        states.entries().iter().find_map(|entry| {
            let StatePayload::LazyBoss { at_boost_bits } = &entry.payload else {
                return None;
            };
            Some(f64::from_bits(*at_boost_bits))
        })
    }

    /// 写入懒癌 Boss 的攻击加成；状态缺失会 panic。
    pub fn set_lazy_boss_at_boost(&mut self, boss: EntityIdx, at_boost: f64) {
        let boss_entity = self
            .entities
            .get_mut(boss)
            .unwrap_or_else(|| panic!("unknown runtime lazy boss entity: {}", boss.0));
        assert!(
            boss_entity.states.set_payload(
                PLAIN_LAZY_BOSS_STATE_KEY,
                StatePayload::LazyBoss {
                    at_boost_bits: at_boost.to_bits(),
                },
            ),
            "runtime lazy boss state disappeared"
        );
    }

    /// 目标是否携带懒癌传染。
    pub fn has_lazy_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity.states.may_contain(StatePayloadKind::LazyInfection)
                && entity
                    .states
                    .entries()
                    .iter()
                    .any(|entry| matches!(&entry.payload, StatePayload::LazyInfection { .. }))
        })
    }

    /// 给目标附加懒癌传染；已携带或状态已存在时返回 false。
    pub fn infect_with_lazy_into(&mut self, boss: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        if self.has_lazy_infection(target) {
            return false;
        }
        let state_id = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_LAZY_INFECTION_STATE_EXPORT)
            .expect("default runtime profile must register lazy infection state");
        let target_entity = self
            .entities
            .get_mut(target)
            .unwrap_or_else(|| panic!("unknown runtime lazy infection target: {}", target.0));
        if !target_entity.states.add_entry(StateEntry::lazy_infection(
            PLAIN_LAZY_INFECTION_STATE_KEY,
            state_id,
            boss,
            SkillPriority(1000),
        )) {
            return false;
        }
        target_entity.refresh_runtime_stats_from_template();
        updates.add_with(|| RunUpdate::new_lang(crate::runtime::lang::keys::SKL_LAZY_HIT, boss.0 as usize, target.0 as usize, 0));
        true
    }

    /// 播报懒癌跳回合的活动文案与「什么也没做」。
    pub fn emit_lazy_activity_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        use crate::runtime::lang::{keys, lazy_activity_key};
        let activity = lazy_activity_key(self.rng.next_u8());
        let owner = owner.0 as usize;
        for key in [activity, keys::SKL_LAZY_SKIP_TURN_0] {
            updates.add_with(|| RunUpdate::new_lang(key, owner, owner, 0).without_target());
        }
    }

    /// 新冠 Boss 命中时尝试把病毒传播给目标。
    pub fn try_covid_spread_on_damage_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        damage: i32,
        updates: &mut RunUpdates,
    ) {
        self.try_covid_spread_on_damage_with_threshold_into(boss, target, mutation, damage, updates, false);
    }

    pub fn try_covid_spread_on_damage_with_threshold_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        damage: i32,
        updates: &mut RunUpdates,
        legacy_callback: bool,
    ) {
        if self.has_covid_infection(target) {
            return;
        }
        let from_covid_boss = legacy_callback
            || self
                .entities
                .get(boss)
                .is_some_and(|source| source.runtime.flags.contains(PlayerKindFlags::BOSS) && source.template.name == "covid");
        // Vite 的主动传播使用 1..=64；扩展来源沿用旧的零基阈值。两者都只抽一个字节。
        let roll = self.rng.r64() as i32 - i32::from(!from_covid_boss);
        if roll < damage {
            self.infect_with_covid_into(boss, target, mutation, updates);
        }
    }

    /// Boss 受击后的被动传播：裸脸直接感染，仅口罩需要抽一次 c75。
    pub fn try_covid_spread_to_attacker_into(
        &mut self,
        boss: EntityIdx,
        attacker: EntityIdx,
        mutation: i32,
        updates: &mut RunUpdates,
    ) {
        if self.has_covid_infection(attacker) {
            return;
        }
        let attacker_entity = self.entities.get(attacker).expect("runtime covid attacker must exist");
        if attacker_entity.template.covid_mask && self.rng.next_u8() < 192 {
            return;
        }
        self.infect_with_covid_into(boss, attacker, mutation, updates);
    }

    /// 给目标附加新冠传染并登记新株；首次康复后仍接纳未见过的株。
    pub fn infect_with_covid_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let infection_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT)
            .expect("default runtime profile must register covid infection state");

        let infected = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime covid target entity: {}", target.0));
            if let Some(entry) = target_entity.states.entry(PLAIN_COVID_INFECTION_STATE_KEY) {
                let StatePayload::CovidInfection(covid) = &entry.payload else {
                    panic!("runtime covid infection key is occupied by a different state");
                };
                // 首次康复后永久接纳此前未见的新株，即使已有其他活跃株。
                if !covid.recovered || covid.mutation_set.contains(&mutation) {
                    false
                } else {
                    let order = target_entity.states.reserve_runtime_registration_order();
                    let entry = target_entity.states.entry_mut(PLAIN_COVID_INFECTION_STATE_KEY).unwrap();
                    let StatePayload::CovidInfection(covid) = &mut entry.payload else {
                        unreachable!();
                    };
                    covid.entries.push(CovidInfectionEntry {
                        boss,
                        mutation,
                        days: 0,
                        registration_order: Some(order),
                    });
                    covid.mutation_set.push(mutation);
                    true
                }
            } else {
                target_entity.states.add_entry(StateEntry::covid_infection(
                    PLAIN_COVID_INFECTION_STATE_KEY,
                    infection_state,
                    boss,
                    mutation,
                    SkillPriority(1000),
                ))
            }
        };
        if !infected {
            return false;
        }

        self.entities.get_mut(target).unwrap().refresh_runtime_stats_from_template();
        updates.add_with(|| RunUpdate::new_lang(lang::keys::SKL_COVID_HIT, boss.0 as usize, target.0 as usize, 0));
        let all_alive = self.world.flat_alive().to_vec();
        for entity_idx in all_alive {
            let delta = if entity_idx == target { 2048 } else { -256 };
            self.entities
                .get_mut(entity_idx)
                .unwrap_or_else(|| panic!("runtime covid alive entity disappeared: {}", entity_idx.0))
                .runtime
                .move_state
                .speed_points += delta;
        }
        true
    }

    pub fn drain_covid_strains_into(&mut self, owner: EntityIdx, order: Option<u64>, updates: &mut RunUpdates) {
        let mut index = 0;
        loop {
            let StatePayload::CovidInfection(covid) = &self
                .entities
                .get(owner)
                .unwrap()
                .states
                .entry(PLAIN_COVID_INFECTION_STATE_KEY)
                .expect("runtime covid state must still exist")
                .payload
            else {
                unreachable!("runtime covid infection key has an unexpected payload");
            };
            let Some(infection) = covid.entries.get(index).copied() else {
                return;
            };
            if let Some(order) = order {
                let original_order = self
                    .entities
                    .get(owner)
                    .unwrap()
                    .states
                    .runtime_registration_order(PLAIN_COVID_INFECTION_STATE_KEY)
                    .unwrap();
                if infection.registration_order.unwrap_or(original_order) != order {
                    index += 1;
                    continue;
                }
            }
            if infection.days > 1 && self.drain_covid_pneumonia_into(owner, infection.boss, infection.mutation, updates) {
                // group.die 在终局中止原版回调，连当前株的康复解绑也不执行。
                return;
            }
            if infection.days > 6 {
                let target = self.entities.get_mut(owner).unwrap();
                let StatePayload::CovidInfection(covid) = &mut target
                    .states
                    .entry_mut(PLAIN_COVID_INFECTION_STATE_KEY)
                    .expect("runtime covid state must still exist")
                    .payload
                else {
                    unreachable!("runtime covid infection key has an unexpected payload");
                };
                covid.entries.remove(index);
                covid.recovered = true;
                target.refresh_runtime_stats_from_template();
            } else {
                index += 1;
            }
            if order.is_some() {
                return;
            }
        }
    }

    /// 返回肺炎是否导致战斗结束；非终局死亡仍允许逐株解绑。
    pub fn drain_covid_pneumonia_into(
        &mut self,
        owner: EntityIdx,
        boss: EntityIdx,
        mutation: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        let target = self.entities.get(owner).unwrap();
        let old_hp = target.runtime.hp;
        if !target.runtime.alive || old_hp <= 0 {
            return false;
        }
        let atp = target.runtime.get_at(true, &mut self.rng);
        let defense = target.runtime.magic_defense();
        let damage = ((atp + f64::from(mutation * 80)) / f64::from(defense)).ceil() as i32;
        if damage <= 0 {
            return false;
        }

        updates.add_with(|| RunUpdate::new_lang(lang::keys::SKL_COVID_DAMAGE, boss.0 as usize, owner.0 as usize, 0));
        let killed = self.apply_plain_attack_damage_with_covid_into(boss, owner, damage, None, updates);
        if killed {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
            if self.plain_battle_has_ended() {
                return true;
            }
        }

        let boss_entity = self.entities.get_mut(boss).unwrap();
        if !boss_entity.runtime.alive || boss_entity.runtime.hp <= 0 {
            return false;
        }
        let heal_amount = if boss_entity.runtime.hp >= boss_entity.template.max_hp {
            (damage >> 3) + 1
        } else {
            damage >> 1
        }
        .min(old_hp);
        // 满血时使用较小的恢复量，但允许溢出体力上限。
        let before = boss_entity.runtime.hp;
        let after = before.saturating_add(heal_amount);
        boss_entity.runtime.hp = after;
        updates.add_with(|| {
            let id = boss.0 as usize;
            let mut update =
                RunUpdate::new("[1]回复体力[2]点", id, id, 0).with_hp_changes([crate::runtime::update::RunUpdateHp {
                    player: id,
                    before,
                    after,
                }]);
            update.param = Some(heal_amount as u32);
            update
        });
        false
    }

    /// 懒癌发作：按 Boss 威力对感染者造成直接伤害，不走新普攻。
    pub fn drain_lazy_flare_into(&mut self, owner: EntityIdx, boss: EntityIdx, updates: &mut RunUpdates) {
        if !self.entities.get(owner).is_some_and(|entity| entity.runtime.alive) {
            return;
        }
        let boss_atp = self.entities.get(boss).unwrap().runtime.get_at(true, &mut self.rng);
        let target_defense = self.entities.get(owner).unwrap().runtime.magic_defense();
        let damage = (boss_atp / f64::from(target_defense)).ceil() as i32;
        if damage <= 0 {
            return;
        }
        updates.add_with(|| {
            RunUpdate::new_lang(
                crate::runtime::lang::keys::SKL_LAZY_DAMAGE,
                boss.0 as usize,
                owner.0 as usize,
                0,
            )
        });
        // 发作是感染者身上的直接伤害，不是来源的新普攻，不重置来源的攻击倍率。
        if self.apply_plain_legacy_damage_into(boss, owner, damage, updates) {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
        }
    }

    /// 致死链：播报死亡、走 die hook，移出战场并按条件触发击杀回调。
    pub fn drain_plain_lethal_damage_into(&mut self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        self.emit_plain_lethal_replay_into(caster, target, updates);
        self.drain_die_hooks_into(target, updates);

        let (hp, team) = self
            .entities
            .get(target)
            .map(|entity| (entity.runtime.hp, entity.runtime.team))
            .unwrap_or_else(|| panic!("runtime lethal target disappeared: {}", target.0));
        if hp > 0 {
            return;
        }

        self.entities.get_mut(target).unwrap().runtime.alive = false;
        self.mark_dead_with_linked_minions_into(target, team, updates);
        if self.should_run_kill_hooks(caster, target) {
            self.drain_kill_hooks_into(caster, target, updates);
        }
    }

    /// 播报通用死亡消息；战斗已结束时抑制使魔自身的死亡日志。
    pub fn emit_plain_lethal_replay_into(&self, caster: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) {
        let target_entity = self
            .entities
            .get(target)
            .unwrap_or_else(|| panic!("unknown runtime lethal replay target: {}", target.0));
        let is_combat_minion = target_entity.runtime.is_combat_minion();
        if is_combat_minion
            && !self
                .entities
                .iter()
                .any(|(_, entity)| entity.runtime.alive && entity.runtime.team != target_entity.runtime.team)
        {
            // legacy 在战斗已结束时抑制战斗召唤物自身的死亡日志；典型路径是使魔自爆击倒最后敌人。
            return;
        }
        let die_message = if is_combat_minion { "[1]消失了" } else { "[1]被击倒了" };
        updates.add_newline();
        updates.add(crate::runtime::update::RunUpdate::new(
            die_message,
            caster.0 as usize,
            target.0 as usize,
            50,
        ));
    }

    /// 行动后恢复：按智慧概率回复 16 点 MP，并追加换行分隔。
    pub fn recover_plain_actor_into(&mut self, actor: EntityIdx, updates: &mut RunUpdates) {
        let recover_threshold = self
            .entities
            .get(actor)
            .unwrap_or_else(|| panic!("unknown runtime recovery actor: {}", actor.0))
            .runtime
            .wisdom
            + 64;
        if (self.rng.r127() as i32) < recover_threshold {
            self.entities.get_mut(actor).unwrap().runtime.magic_point += 16;
        }
        updates.add_newline();
    }
}

#[cfg(test)]
mod tests;

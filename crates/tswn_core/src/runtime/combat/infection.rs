use super::*;

impl CombatRuntime {
    /// 预读被动的真实 MP 消耗，不推进随机流；被动使用 r3x3，而非主动技的 8~23。
    pub(crate) fn passive_mp_costs(
        &self,
        owner: EntityIdx,
        skill: &str,
        intercept: bool,
    ) -> std::collections::BTreeMap<String, u32> {
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

    pub fn has_covid_infection(&self, target: EntityIdx) -> bool {
        self.entities.get(target).is_some_and(|entity| {
            entity.states.may_contain(StatePayloadKind::CovidInfection)
                && entity
                    .states
                    .entries()
                    .iter()
                    .any(|entry| matches!(&entry.payload, StatePayload::CovidInfection { .. }))
        })
    }

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

    pub fn infect_with_lazy_into(&mut self, boss: EntityIdx, target: EntityIdx, updates: &mut RunUpdates) -> bool {
        if target == boss || self.has_lazy_infection(target) {
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
        let boss_display = self
            .entities
            .get(boss)
            .unwrap_or_else(|| panic!("unknown runtime lazy boss entity: {}", boss.0))
            .template
            .display_name
            .clone();
        updates.add_with(|| {
            crate::runtime::update::RunUpdate::new(format!("[1]感染了{boss_display}"), boss.0 as usize, target.0 as usize, 0)
        });
        true
    }

    pub fn emit_lazy_activity_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        let activity = match self.rng.next_u8() {
            0..=49 => "Steam",
            50..=99 => "守望先锋",
            100..=149 => "文明6",
            150..=189 => "英雄联盟",
            190..=229 => "微博",
            _ => "朋友圈",
        };
        let owner_name = self
            .entities
            .get(owner)
            .unwrap_or_else(|| panic!("unknown runtime lazy activity owner: {}", owner.0))
            .template
            .display_name
            .clone();
        updates.add_with(|| {
            crate::runtime::update::RunUpdate::new(
                format!("{owner_name}打开了{activity}, 这回合什么也没做"),
                owner.0 as usize,
                owner.0 as usize,
                0,
            )
        });
    }

    pub fn try_covid_spread_on_damage_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        damage: i32,
        updates: &mut RunUpdates,
    ) {
        if self.has_covid_infection(target) {
            return;
        }
        if i32::from(self.rng.next_u8() & 63) < damage {
            self.infect_with_covid_into(boss, target, mutation, updates);
        }
    }

    /// Boss 受击后的被动传播：裸脸直接感染，仅口罩需要抽一次 c75。
    pub(super) fn try_covid_spread_to_attacker_into(
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

    pub fn infect_with_covid_into(
        &mut self,
        boss: EntityIdx,
        target: EntityIdx,
        mutation: i32,
        updates: &mut RunUpdates,
    ) -> bool {
        if target == boss {
            return false;
        }
        let boss_display = self
            .entities
            .get(boss)
            .unwrap_or_else(|| panic!("unknown runtime covid boss entity: {}", boss.0))
            .template
            .display_name
            .clone();
        let infection_state = self
            .registry
            .state_id_by_export_name(DEFAULT_CORE_COVID_INFECTION_STATE_EXPORT)
            .expect("default runtime profile must register covid infection state");

        let infected = {
            let target_entity = self
                .entities
                .get_mut(target)
                .unwrap_or_else(|| panic!("unknown runtime covid target entity: {}", target.0));
            if let Some(entry) = target_entity.states.entry_mut(PLAIN_COVID_INFECTION_STATE_KEY) {
                let StatePayload::CovidInfection {
                    entries,
                    mutation_set,
                    recovered,
                } = &mut entry.payload
                else {
                    panic!("runtime covid infection key is occupied by a different state");
                };
                if !*recovered || mutation_set.contains(&mutation) {
                    false
                } else {
                    *recovered = false;
                    entries.push(CovidInfectionEntry { boss, mutation, days: 0 });
                    mutation_set.push(mutation);
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

        updates.add_with(|| {
            crate::runtime::update::RunUpdate::new(format!("[1]感染了{boss_display}"), boss.0 as usize, target.0 as usize, 0)
        });
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

    pub fn drain_covid_pneumonia_into(&mut self, owner: EntityIdx, boss: EntityIdx, mutation: i32, updates: &mut RunUpdates) {
        if !self.entities.get(owner).is_some_and(|entity| entity.runtime.alive) {
            return;
        }
        let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
        let atp = self.entities.get(owner).unwrap().runtime.get_at(true, &mut self.rng);
        let defense = self.entities.get(owner).unwrap().runtime.magic_defense();
        let damage = ((atp + f64::from(mutation * 80)) / f64::from(defense)).ceil() as i32;
        if damage <= 0 {
            return;
        }

        updates.add_with(|| {
            crate::runtime::update::RunUpdate::new(format!(" {owner_name}肺炎发作"), boss.0 as usize, owner.0 as usize, 0)
        });
        let old_hp = self.entities.get(owner).unwrap().runtime.hp;
        let killed = self.apply_plain_attack_damage_with_covid_into(boss, owner, damage, None, updates);
        let actual_damage = if killed { old_hp } else { damage };
        if killed {
            self.drain_plain_lethal_damage_into(boss, owner, updates);
        }

        let boss_entity = self.entities.get_mut(boss).unwrap();
        if !boss_entity.runtime.alive || boss_entity.runtime.hp <= 0 || actual_damage <= 0 {
            return;
        }
        let heal_amount = if boss_entity.runtime.hp >= boss_entity.template.max_hp {
            ((damage >> 3) + 1).min(actual_damage)
        } else {
            (damage >> 1).min(actual_damage)
        };
        // 肺炎回血不是普通治疗：满血时使用较小的恢复量，但允许溢出体力上限。
        boss_entity.runtime.hp = boss_entity.runtime.hp.saturating_add(heal_amount);
        let boss_display = boss_entity.template.display_name.clone();
        updates.add_with(|| {
            crate::runtime::update::RunUpdate::new(
                format!("{boss_display}回复体力{heal_amount}点"),
                boss.0 as usize,
                boss.0 as usize,
                0,
            )
        });
    }

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
        let boss_display = self.entities.get(boss).unwrap().template.display_name.clone();
        let owner_name = self.entities.get(owner).unwrap().template.display_name.clone();
        updates.add_with(|| {
            crate::runtime::update::RunUpdate::new(
                format!(" {owner_name}{boss_display}发作"),
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
mod tests {
    use super::*;

    fn set_next_roll(runtime: &mut CombatRuntime, roll: u8) {
        for _ in 0..4096 {
            if runtime.rng.peek_next_u8() == roll {
                return;
            }
            runtime.rng.next_u8();
        }
        panic!("未找到预期随机字节 {roll}");
    }

    fn assert_rng(runtime: &CombatRuntime, expected: &crate::rc4::RC4) {
        assert_eq!((runtime.rng.i, runtime.rng.j), (expected.i, expected.j));
        assert_eq!(runtime.rng.main_val, expected.main_val);
    }

    #[test]
    fn first_public_hit_infects_the_unmasked_attacker_but_not_either_mask_suffix() {
        // 同为实际命中；原版首帧只感染裸脸 a，两种口罩武器名都阻挡。
        for (attacker_input, damage, infected) in [("a", 54, true), ("a+mask", 54, false), ("a+防尘口罩", 31, false)] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(format!("covid@!\n\n{attacker_input}")).unwrap();
            let boss = runner.input_groups[0][0];
            let attacker = runner.input_groups[1][0];
            let first = (0..32).find_map(|_| runner.run_round().frame).expect("应有可见战斗帧");
            assert_eq!(runner.runtime.entities.get(boss).unwrap().runtime.hp, 326 - damage);
            assert_eq!(runner.runtime.has_covid_infection(attacker), infected, "{attacker_input}");
            assert_eq!(
                first.updates.updates.iter().any(|update| update.message.contains("感染了")),
                infected
            );
        }
    }

    #[test]
    fn public_clone_keeps_its_owners_mask_when_hitting_the_boss() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("covid@!\n\nmasked67+mask".to_owned()).unwrap();
        let sixth = (0..96).filter_map(|_| runner.run_round().frame).nth(5).expect("应产生第六个可见帧");
        let hit = sixth
            .updates
            .updates
            .iter()
            .find(|update| update.score == 77 && update.message.contains("点伤害"))
            .unwrap();
        let clone = EntityIdx(hit.caster as u32);
        let entity = runner.runtime.entities.get(clone).unwrap();
        assert_eq!(entity.template.id_key_name, "masked67?0@masked67");
        assert!(!runner.runtime.has_covid_infection(clone));
        assert_eq!(entity.runtime.move_state.speed_points, 351);
    }

    #[test]
    fn contact_mask_uses_wisdom_plus_192_and_always_draws_one_byte() {
        for (candidate_input, roll, infected) in [
            ("candidate", 19, false),
            ("candidate", 20, true),
            ("candidate+防尘口罩", 231, false),
            ("candidate+防尘口罩", 232, true),
            ("candidate+fabricmask", 231, false),
            ("candidate+MASK", 20, true),
            ("candidate+口罩夹", 20, true),
            ("mask", 20, true),
        ] {
            let raw = format!("covid@!\n\ncarrier\n{candidate_input}");
            let mut runner = RuntimeRunner::new_from_namerena_raw(raw).unwrap();
            let boss = runner.input_groups[0][0];
            let (owner, candidate) = (EntityIdx(1), EntityIdx(2));
            let runtime = &mut runner.runtime;
            runtime.entities.get_mut(candidate).unwrap().runtime.wisdom = 40;
            set_next_roll(runtime, roll);
            let mut expected = runtime.rng.clone();
            expected.next_u8();
            runtime.effects.push(QueuedEffect::CovidContact {
                owner,
                candidate,
                boss,
                mutation: 40,
            });
            let frame = runtime.flush_effects().expect("应有接触回放");
            assert!(frame.updates.updates[0].message.contains("近距离接触"));
            assert_eq!(runtime.has_covid_infection(candidate), infected, "{candidate_input} / {roll}");
            assert_eq!(frame.updates.updates[1].message.contains("没被感染"), !infected);
            assert_rng(runtime, &expected);
        }
    }

    #[test]
    fn infected_cross_team_attack_uses_boss_damage_source_without_the_boss_attack_roll() {
        // 原版第 9 帧：a 发起攻击，新冠给 c 造成 59 伤害并感染，随后 a 肺炎 63。
        let mut runner = RuntimeRunner::new_from_namerena_raw("covid@!\n\na\nb\n\nc".to_owned()).unwrap();
        let boss = runner.input_groups[0][0];
        let carrier = EntityIdx(1);
        let target = runner.input_groups[2][0];
        let cross_team = (0..64).filter_map(|_| runner.run_round().frame).nth(8).expect("应出现跨队攻击帧");
        let events = &cross_team.updates.updates;
        assert_eq!((events[0].caster, events[0].target), (carrier.0 as usize, target.0 as usize));
        let damage: Vec<_> = events.iter().filter(|update| update.message.contains("点伤害")).collect();
        assert_eq!((damage[0].caster, damage[0].score), (boss.0 as usize, 59));
        assert_eq!(damage[1].score, 63, "跨队传播命中后不抽 Boss 主动攻击的 r64");
        assert!(runner.runtime.has_covid_infection(target));
    }

    #[test]
    fn only_a_masked_attacker_draws_the_c75_roll_when_hitting_the_boss() {
        for (input, roll, damage, already_infected, draws, infected) in [
            ("a", 191, 1, false, false, true),
            ("a+mask", 191, 1, false, true, false),
            ("a+口罩", 192, 1, false, true, true),
            ("a+mask", 192, 0, false, false, false),
            ("a+mask", 192, 1, true, false, true),
        ] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(format!("covid@!\n\n{input}")).unwrap();
            let (boss, attacker) = (runner.input_groups[0][0], runner.input_groups[1][0]);
            let runtime = &mut runner.runtime;
            let mut updates = RunUpdates::new();
            if already_infected {
                assert!(runtime.infect_with_covid_into(boss, attacker, 40, &mut updates));
                updates = RunUpdates::new();
            }
            set_next_roll(runtime, roll);
            let mut expected = runtime.rng.clone();
            if draws {
                expected.next_u8();
            }
            assert!(!runtime.apply_plain_legacy_damage_into(attacker, boss, damage, &mut updates));
            assert_eq!(runtime.has_covid_infection(attacker), infected, "{input} / {roll} / {damage}");
            assert_eq!(
                updates.updates.iter().any(|update| update.message.contains("感染了")),
                infected && !already_infected
            );
            assert_rng(runtime, &expected);
        }
    }
}

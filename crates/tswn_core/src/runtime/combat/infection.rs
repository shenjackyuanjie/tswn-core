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

    /// Boss 的主动和被击传播检查感染历史，而非当前活跃株。
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

    pub fn emit_lazy_activity_into(&mut self, owner: EntityIdx, updates: &mut RunUpdates) {
        use crate::runtime::lang::{keys, lazy_activity_key};
        let activity = lazy_activity_key(self.rng.next_u8());
        let owner = owner.0 as usize;
        for key in [activity, keys::SKL_LAZY_SKIP_TURN_0] {
            updates.add_with(|| RunUpdate::new_lang(key, owner, owner, 0).without_target());
        }
    }

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

    pub(super) fn try_covid_spread_on_damage_with_threshold_into(
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
                let StatePayload::CovidInfection {
                    mutation_set, recovered, ..
                } = &entry.payload
                else {
                    panic!("runtime covid infection key is occupied by a different state");
                };
                // 首次康复后永久接纳此前未见的新株，即使已有其他活跃株。
                if !*recovered || mutation_set.contains(&mutation) {
                    false
                } else {
                    let order = target_entity.states.reserve_runtime_registration_order();
                    let entry = target_entity.states.entry_mut(PLAIN_COVID_INFECTION_STATE_KEY).unwrap();
                    let StatePayload::CovidInfection {
                        entries, mutation_set, ..
                    } = &mut entry.payload
                    else {
                        unreachable!();
                    };
                    entries.push(CovidInfectionEntry {
                        boss,
                        mutation,
                        days: 0,
                        registration_order: Some(order),
                    });
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

    pub(super) fn drain_covid_strains_into(&mut self, owner: EntityIdx, order: Option<u64>, updates: &mut RunUpdates) {
        let mut index = 0;
        loop {
            let StatePayload::CovidInfection { entries, .. } = &self
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
            let Some(infection) = entries.get(index).copied() else {
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
                let StatePayload::CovidInfection { entries, recovered, .. } = &mut target
                    .states
                    .entry_mut(PLAIN_COVID_INFECTION_STATE_KEY)
                    .expect("runtime covid state must still exist")
                    .payload
                else {
                    unreachable!("runtime covid infection key has an unexpected payload");
                };
                entries.remove(index);
                *recovered = true;
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
    fn public_plant_recovers_then_accepts_two_strains_and_uses_the_latest() {
        let names = (0..12).map(|index| format!("plant4x{index}")).collect::<Vec<_>>().join("\n");
        let mut runner = RuntimeRunner::new_from_namerena_raw(format!("covid@!\n\n{names}")).unwrap();
        let target = by_name(&runner.runtime, "plant4x11");
        for number in 1..=40 {
            let frame = next_visible(&mut runner);
            if number < 39 {
                continue;
            }
            let entity = runner.runtime.entities.get(target).unwrap();
            let state = entity.states.entry(PLAIN_COVID_INFECTION_STATE_KEY).unwrap();
            let StatePayload::CovidInfection {
                entries,
                mutation_set,
                recovered,
            } = &state.payload
            else {
                panic!("应有新冠状态");
            };
            assert!(*recovered);
            assert_eq!(mutation_set.as_slice(), &[40, 29, 57]);
            let active: Vec<_> = entries.iter().map(|strain| (strain.mutation, strain.days)).collect();
            assert_eq!(active, [(29, 0), (57, i32::from(number == 40))]);
            if number == 39 {
                assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), (33, 11));
                assert_eq!(entity.runtime.move_state.speed_points, 3606);
                assert!(
                    frame
                        .updates
                        .updates
                        .iter()
                        .any(|update| update.message.contains("感染了") && update.target == target.0 as usize)
                );
            } else {
                assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), (57, 113));
                assert_eq!(
                    (
                        entity.runtime.hp,
                        entity.runtime.move_state.speed_points,
                        entity.runtime.magic_point
                    ),
                    (57, 1558, 54)
                );
            }
        }
    }

    #[test]
    fn public_infection_action_skips_frozen_carriers_and_samples_the_sources_effective_team() {
        let raw = "covid@!\n\nyuri@!\ncharm5x0\n\ncharm5x1\ncharm5x2\ncharm5x3\ncharm5x4\ncharm5x5\ncharm5x6\ncharm5x7\ncharm5x8";
        let mut runner = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
        let carrier = by_name(&runner.runtime, "charm5x0");
        let contact = by_name(&runner.runtime, "charm5x7");
        for number in 1..=40 {
            let frame = next_visible(&mut runner);
            if number == 14 {
                assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), (205, 23));
                assert!(
                    frame
                        .updates
                        .updates
                        .iter()
                        .any(|update| update.message.contains("感染了") && update.target == contact.0 as usize)
                );
            } else if number == 40 {
                assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), (228, 130));
                let entity = runner.runtime.entities.get(carrier).unwrap();
                assert_eq!(
                    (
                        entity.runtime.hp,
                        entity.runtime.move_state.speed_points,
                        entity.runtime.magic_point
                    ),
                    (97, 136, 71)
                );
                assert!(
                    frame
                        .updates
                        .updates
                        .iter()
                        .any(|update| update.message.contains("重症监护室") && update.target == carrier.0 as usize)
                );
            }
        }
    }

    #[test]
    fn public_charmed_covid_can_infect_itself() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("yuri@!\n\ncovid@!".to_owned()).unwrap();
        let boss = runner.input_groups[1][0];
        next_visible(&mut runner);
        let frame = next_visible(&mut runner);
        assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), (83, 30));
        assert_eq!(runner.runtime.entities.get(boss).unwrap().runtime.hp, 255);
        assert!(runner.runtime.has_covid_infection(boss));
        assert!(
            frame
                .updates
                .updates
                .iter()
                .any(|update| update.message.contains("感染了") && update.target == boss.0 as usize)
        );
    }

    #[test]
    fn public_terminal_pneumonia_stops_before_heal_and_recovery() {
        let mut runner = RuntimeRunner::new_from_namerena_raw("covid@!\n\nd".to_owned()).unwrap();
        let (boss, target) = (runner.input_groups[0][0], runner.input_groups[1][0]);
        let frame = (0..4).map(|_| next_visible(&mut runner)).last().unwrap();
        assert_eq!((runner.runtime.rng.i, runner.runtime.rng.j), (162, 194));
        assert_eq!(runner.runtime.entities.get(boss).unwrap().runtime.hp, 355);
        assert!(!frame.updates.updates.iter().any(|update| update.message.contains("回复体力")));
        let state = runner
            .runtime
            .entities
            .get(target)
            .unwrap()
            .states
            .entry(PLAIN_COVID_INFECTION_STATE_KEY)
            .unwrap();
        let StatePayload::CovidInfection { entries, recovered, .. } = &state.payload else {
            panic!("应有新冠状态");
        };
        assert!(!recovered);
        assert_eq!(entries[0].days, 11);
    }

    #[test]
    fn strain_recovery_preserves_history_and_ticks_in_registration_order() {
        for (raw, dying, recovery_hp, recovery_rng, tick_hp, tick_rng, scores) in [
            (
                "covid@!\n\na",
                false,
                (9885, 382),
                (32, 102),
                (9707, 406),
                (42, 158),
                &[80, 98][..],
            ),
            ("covid@!\n\na\nb", true, (9915, 367), (39, 218), (0, 368), (44, 203), &[85][..]),
        ] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
            let runtime = &mut runner.runtime;
            let (boss, target) = (by_name(runtime, "covid@!"), by_name(runtime, "a"));
            let mut updates = RunUpdates::new();
            assert!(!runtime.apply_plain_legacy_damage_into(target, boss, 1, &mut updates));
            runtime.entities.get_mut(target).unwrap().runtime.hp = 10_000;
            assert!(
                !runtime.infect_with_covid_into(boss, target, 41, &mut updates),
                "首次康复前拒绝不同株"
            );
            set_strain_days(runtime, target, &[7]);
            covid_post_action(runtime, target);
            assert_eq!((hp(runtime, target), hp(runtime, boss)), recovery_hp);
            assert_eq!((runtime.rng.i, runtime.rng.j), recovery_rng);
            assert!(
                !runtime.infect_with_covid_into(boss, target, 40, &mut updates),
                "康复后仍拒绝见过的株"
            );
            assert!(runtime.infect_with_covid_into(boss, target, 41, &mut updates));
            assert!(runtime.infect_with_covid_into(boss, target, 42, &mut updates));
            assert!(!runtime.infect_with_covid_into(boss, target, 42, &mut updates));
            set_strain_days(runtime, target, &[7, if dying { 7 } else { 2 }]);
            if dying {
                runtime.entities.get_mut(target).unwrap().runtime.hp = 1;
            }
            let updates = covid_post_action(runtime, target);
            let damage: Vec<_> = updates.updates.iter().filter(|u| u.message.contains("点伤害")).map(|u| u.score).collect();
            assert_eq!(damage, scores);
            assert_eq!((hp(runtime, target), hp(runtime, boss)), tick_hp);
            assert_eq!((runtime.rng.i, runtime.rng.j), tick_rng);
            let state = runtime
                .entities
                .get(target)
                .unwrap()
                .states
                .entry(PLAIN_COVID_INFECTION_STATE_KEY)
                .unwrap();
            let StatePayload::CovidInfection {
                entries,
                mutation_set,
                recovered,
            } = &state.payload
            else {
                panic!("应有新冠状态");
            };
            assert!(*recovered);
            assert_eq!(mutation_set.as_slice(), &[40, 41, 42]);
            let remaining: Vec<_> = entries.iter().map(|strain| (strain.mutation, strain.days)).collect();
            // 非终局死亡会跳过第二次伤害及抽样，但照常注销两条过期钩子。
            assert_eq!(remaining, if dying { &[][..] } else { &[(42, 2)][..] });
        }
    }

    #[test]
    fn charmed_carrier_spreads_by_original_team_not_effective_team() {
        for (raw, same_team, message, candidate_hp, rng) in [
            ("covid@!\n\ncarrier\ncandidate2", true, "[0]和[1]近距离接触", 265, (81, 129)),
            ("covid@!\n\ncarrier\n\ncandidate2", false, "[0]发起攻击", 228, (88, 184)),
        ] {
            let mut runner = RuntimeRunner::new_from_namerena_raw(raw.to_owned()).unwrap();
            let runtime = &mut runner.runtime;
            let (boss, carrier, candidate) = (
                by_name(runtime, "covid@!"),
                by_name(runtime, "carrier"),
                by_name(runtime, "candidate2"),
            );
            let mut updates = RunUpdates::new();
            assert!(!runtime.apply_plain_legacy_damage_into(carrier, boss, 1, &mut updates));
            let charm_source = if same_team { boss } else { candidate };
            let effective = runtime.entities.get(charm_source).unwrap().runtime.team;
            let charm = runtime.registry.state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT).unwrap();
            assert!(runtime.entities.get_mut(carrier).unwrap().states.add_entry(StateEntry::charm(
                PLAIN_CHARM_STATE_KEY,
                charm,
                charm_source.0 as usize,
                Some(effective),
                Some(effective),
                Some(carrier.0),
                2,
                SkillPriority(100),
            )));
            let plan = runtime.scheduler.state_hook_plan(&runtime.entities, carrier, ProcMask::PRE_ACTION);
            let mut updates = RunUpdates::new();
            assert!(runtime.drain_state_hook_plan_with_action_smart_into(&plan, &mut updates, Some(true)));
            let first = &updates.updates[0];
            assert_eq!(
                (first.message.as_ref(), first.caster, first.target),
                (message, carrier.0 as usize, candidate.0 as usize)
            );
            assert_eq!(hp(runtime, candidate), candidate_hp);
            assert_eq!((runtime.rng.i, runtime.rng.j), rng);
        }
    }

    fn by_name(runtime: &CombatRuntime, name: &str) -> EntityIdx {
        runtime.entities.iter().find(|(_, entity)| entity.template.id_key_name == name).unwrap().0
    }

    fn hp(runtime: &CombatRuntime, owner: EntityIdx) -> i32 { runtime.entities.get(owner).unwrap().runtime.hp }

    fn set_strain_days(runtime: &mut CombatRuntime, owner: EntityIdx, days: &[i32]) {
        let state = runtime
            .entities
            .get_mut(owner)
            .unwrap()
            .states
            .entry_mut(PLAIN_COVID_INFECTION_STATE_KEY)
            .unwrap();
        let StatePayload::CovidInfection { entries, .. } = &mut state.payload else {
            panic!("应有新冠状态");
        };
        assert_eq!(entries.len(), days.len());
        for (strain, &days) in entries.iter_mut().zip(days) {
            strain.days = days;
        }
    }

    fn covid_post_action(runtime: &mut CombatRuntime, owner: EntityIdx) -> RunUpdates {
        let plan = runtime.scheduler.state_hook_plan(&runtime.entities, owner, ProcMask::POST_ACTION);
        let mut updates = RunUpdates::new();
        runtime.drain_state_hook_plan_into(&plan, &mut updates);
        updates
    }

    fn next_visible(runner: &mut RuntimeRunner) -> RuntimeFrame {
        (0..512)
            .find_map(|_| {
                runner
                    .run_round()
                    .frame
                    .filter(|frame| frame.updates.updates.iter().any(|update| !update.message.trim().is_empty()))
            })
            .expect("应有下一场战斗可见帧")
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

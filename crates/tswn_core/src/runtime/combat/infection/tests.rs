use super::super::*;

/// 测试辅助：推进 RNG 直到下一随机字节等于 `roll`，最多尝试 4096 次。
fn set_next_roll(runtime: &mut CombatRuntime, roll: u8) {
    for _ in 0..4096 {
        if runtime.rng.peek_next_u8() == roll {
            return;
        }
        runtime.rng.next_u8();
    }
    panic!("未找到预期随机字节 {roll}");
}

/// 测试辅助：断言 runtime 的 RC4 游标与主值和期望值一致。
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

/// 测试辅助：按模板 `id_key_name` 查找实体索引。
fn by_name(runtime: &CombatRuntime, name: &str) -> EntityIdx {
    runtime.entities.iter().find(|(_, entity)| entity.template.id_key_name == name).unwrap().0
}

/// 测试辅助：读取指定实体的当前体力。
fn hp(runtime: &CombatRuntime, owner: EntityIdx) -> i32 { runtime.entities.get(owner).unwrap().runtime.hp }

/// 测试辅助：改写新冠感染状态中各毒株的剩余天数，数量须与现有条目一致。
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

/// 测试辅助：执行指定单位的 POST_ACTION 状态钩子计划，并返回产生的帧更新。
fn covid_post_action(runtime: &mut CombatRuntime, owner: EntityIdx) -> RunUpdates {
    let plan = runtime.scheduler.state_hook_plan(&runtime.entities, owner, ProcMask::POST_ACTION);
    let mut updates = RunUpdates::new();
    runtime.drain_state_hook_plan_into(&plan, &mut updates);
    updates
}

/// 测试辅助：推进至多 512 步，返回下一场含非空消息的可见战斗帧。
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

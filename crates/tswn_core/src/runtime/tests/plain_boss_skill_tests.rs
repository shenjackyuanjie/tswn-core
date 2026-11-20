use super::*;

use crate::namerena::BossActionSkill;
use crate::runtime::combat::PlainAttackOnDamage;
use crate::runtime::lang::{get_lang, keys};

fn boss_test_registry() -> (PlayerKindId, ExtensionRegistry) {
    let mut builder = ExtensionRegistryBuilder::default();
    let boss_kind = builder
        .register_player_kind_with_policies(
            "core",
            "boss",
            "core.boss",
            PlayerKindFlags::BOSS,
            PlayerKindPolicies::default(),
        )
        .expect("boss kind should register");
    builder
        .register_state(
            "core",
            "charm",
            DEFAULT_CORE_CHARM_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("charm state should register");
    builder
        .register_state(
            "core",
            "curse",
            DEFAULT_CORE_CURSE_STATE_EXPORT,
            ProcMask::POST_DEFEND,
            SkillPriority(10_000),
        )
        .expect("curse state should register");
    (boss_kind, builder.build())
}

/// 一个 Boss 带两名敌方英雄；英雄血量很低，用来确认多目标技能不因首个目标倒下而中断。
fn boss_runtime(name: &str) -> CombatRuntime {
    let (boss_kind, registry) = boss_test_registry();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, name, boss_kind, 0, 1000, 3).with_magic(1_000_000),
            PlayerTemplate::new(2, "hero-a", 1, 5, 3),
            PlayerTemplate::new(3, "hero-b", 1, 5, 3),
        ],
        registry,
    ))
}

#[test]
fn curse_damage_checks_boss_immunity_before_applying_the_state() {
    for (name, roll, damage, immune_roll, cursed) in [
        ("ikaruga", 0, 1, false, true),
        ("aokiji", 191, 1, true, false),
        ("aokiji", 192, 1, true, true),
        ("aokiji", 192, 0, false, false),
        ("aokiji", 192, 1000, false, false),
    ] {
        let mut runtime = boss_runtime(name);
        for _ in 0..4096 {
            if runtime.rng.clone().next_u8() == roll {
                break;
            }
            runtime.rng.next_u8();
        }
        let mut expected_rng = runtime.rng.clone();
        assert_eq!(expected_rng.clone().next_u8(), roll);
        if immune_roll {
            expected_rng.next_u8();
        }
        let mut updates = RunUpdates::new();
        runtime.apply_plain_attack_damage_with_covid_and_on_damage_into(
            EntityIdx(1),
            EntityIdx(0),
            damage,
            None,
            PlainAttackOnDamage::Curse,
            &mut updates,
        );

        let boss = runtime.entities.get(EntityIdx(0)).unwrap();
        assert_eq!(boss.runtime.hp, 1000 - damage);
        assert_eq!(
            boss.states.entry(PLAIN_CURSE_STATE_KEY).map(|entry| entry.payload.clone()),
            cursed.then_some(StatePayload::Curse { prob: 42, multiply: 2 }),
            "{name} roll={roll} damage={damage}"
        );
        assert_eq!(messages_of(&updates).iter().any(|message| message == "[1]被[诅咒]了"), cursed);
        assert_rng_state_eq(&runtime.rng, &expected_rng);
    }
}

/// 尤里队伍外有两组目标：四人组验证层数取初始人数，单人组验证下限 3。
fn yuri_runtime() -> CombatRuntime {
    let (boss_kind, registry) = boss_test_registry();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "yuri", boss_kind, 0, 1000, 3),
            PlayerTemplate::new(2, "alpha", 1, 100, 3),
            PlayerTemplate::new(3, "beta", 1, 100, 3),
            PlayerTemplate::new(4, "gamma", 1, 100, 3),
            PlayerTemplate::new(5, "delta", 1, 100, 3),
            PlayerTemplate::new(6, "loner", 2, 100, 3),
        ],
        registry,
    ))
}

fn add_mario_growth(runtime: &mut CombatRuntime, grow: i32, fire_level: u32) {
    assert!(
        runtime.entities.get_mut(EntityIdx(0)).unwrap().states.add_entry(StateEntry::mario_boss(
            PLAIN_MARIO_BOSS_STATE_KEY,
            grow,
            fire_level
        )),
        "mario growth state should be inserted"
    );
}

fn mario_growth_value(runtime: &CombatRuntime) -> Option<(i32, u32)> {
    runtime
        .entities
        .get(EntityIdx(0))
        .unwrap()
        .states
        .entry(PLAIN_MARIO_BOSS_STATE_KEY)
        .and_then(StateEntry::mario_boss_value)
}

fn advance_until_r127_below(runtime: &mut CombatRuntime, level: u32) {
    while {
        let mut probe = runtime.rng.clone();
        probe.r127() >= level
    } {
        runtime.rng.next_u8();
    }
}

fn advance_until_next_u8_below(runtime: &mut CombatRuntime, max: u8) {
    while {
        let mut probe = runtime.rng.clone();
        probe.next_u8() >= max
    } {
        runtime.rng.next_u8();
    }
}

fn messages_of(updates: &RunUpdates) -> Vec<String> { updates.updates.iter().map(|update| update.message.to_string()).collect() }

/// 推进 RNG，直到 Boss `k4` 扫描依次消耗的 r127 都不通过概率。
fn advance_until_boss_scan_misses(runtime: &mut CombatRuntime, levels: &[u32]) {
    loop {
        let mut probe = runtime.rng.clone();
        probe.next_u8();
        if levels.iter().all(|&level| probe.r127() >= level) {
            break;
        }
        runtime.rng.next_u8();
    }
}

#[test]
fn boss_scan_saitama_only_burns_one_probe_byte() {
    let mut runtime = boss_runtime("saitama");
    let mut expected = runtime.rng.clone();
    expected.r127();

    assert!(runtime.scan_plain_boss_action_skills(EntityIdx(0), false).is_none());
    assert_rng_state_eq(&runtime.rng, &expected);
}

#[test]
fn saitama_hunger_preserves_fractional_damage_and_minion_weight() {
    for (damages, has_minion, leaves) in [(510, false, false), (511, false, true), (520, true, false)] {
        let mut runner = RuntimeRunner::new_from_namerena_raw(
            "saitama@!

enemy-a

enemy-b"
                .to_owned(),
        )
        .unwrap();
        let actor = runner.input_groups[0][0];
        let enemy = runner.input_groups[1][0];
        let runtime = &mut runner.runtime;
        let entry = runtime
            .entities
            .get_mut(actor)
            .unwrap()
            .states
            .entry_mut(PLAIN_SAITAMA_BOSS_STATE_KEY)
            .unwrap();
        entry.payload = StatePayload::SaitamaBoss {
            turns: 0,
            damages,
            hitters: smallvec::smallvec![enemy],
            minions: if has_minion {
                smallvec::smallvec![EntityIdx(99)]
            } else {
                Default::default()
            },
        };
        let mut updates = RunUpdates::new();
        runtime.drain_plain_saitama_action_into(actor, Some(enemy), &mut updates);

        assert_eq!(
            runtime.world.flat_alive().contains(&actor),
            !leaves,
            "damage={damages} minion={has_minion}"
        );
        let actor_runtime = &runtime.entities.get(actor).unwrap().runtime;
        assert!(actor_runtime.hp > 0);
        assert_eq!(actor_runtime.alive, !leaves);
        assert_eq!(runtime.saitama_boss_state(actor).unwrap().0, if leaves { 0 } else { 1 });
    }
}

#[test]
fn saitama_punch_resets_speed_points_of_all_living_teams() {
    let mut runner = RuntimeRunner::new_from_namerena_raw(
        "saitama@!
friend

enemy-a

enemy-b"
            .to_owned(),
    )
    .unwrap();
    let enemy = runner.input_groups[1][0];
    let runtime = &mut runner.runtime;
    let actor = runtime.entities.iter().find(|(_, entity)| entity.template.name == "saitama").unwrap().0;
    let alive = runtime.world.flat_alive().to_vec();
    assert_eq!(alive.len(), 4);
    for &id in &alive {
        runtime.entities.get_mut(id).unwrap().runtime.move_state.speed_points = 999;
    }
    runtime.entities.get_mut(enemy).unwrap().runtime.hp = 100_000;
    let mut updates = RunUpdates::new();
    for _ in 0..10 {
        runtime.drain_plain_saitama_action_into(actor, Some(enemy), &mut updates);
    }
    assert!(updates.updates.is_empty());
    assert_eq!(runtime.entities.get(enemy).unwrap().runtime.move_state.speed_points, 999);

    runtime.drain_plain_saitama_action_into(actor, Some(enemy), &mut updates);

    assert_eq!(runtime.world.flat_alive(), alive);
    assert_eq!(updates.updates[0].message, "[0]发起攻击");
    for id in alive {
        let entity = runtime.entities.get(id).unwrap();
        assert_eq!(
            entity.runtime.move_state.speed_points,
            if id == actor { 1700 } else { 0 },
            "{}",
            entity.template.name
        );
    }
}

#[test]
fn covid_pneumonia_can_heal_above_max_hp_but_not_a_dead_source() {
    for source_alive in [true, false] {
        let mut runtime = boss_runtime("covid");
        let boss = EntityIdx(0);
        let target = EntityIdx(1);
        let initial_boss_hp = if source_alive {
            1000
        } else {
            assert!(runtime.kill_entity_without_damage_into(boss, &mut RunUpdates::new()));
            0
        };
        runtime.entities.get_mut(target).unwrap().runtime.hp = 1000;
        let mut updates = RunUpdates::new();
        runtime.drain_covid_pneumonia_into(target, boss, 40, &mut updates);

        let damage = 1000 - runtime.entities.get(target).unwrap().runtime.hp;
        assert!((1..1000).contains(&damage));
        let expected_heal = if source_alive { (damage >> 3) + 1 } else { 0 };
        let source = runtime.entities.get(boss).unwrap();
        assert_eq!(source.runtime.hp, initial_boss_hp + expected_heal);
        assert_eq!(source.runtime.alive, source_alive);
        assert_eq!(
            messages_of(&updates).iter().any(|message| message.contains("回复体力")),
            source_alive
        );
    }
}

#[test]
fn lazy_flare_keeps_running_after_its_source_is_defeated() {
    for source_alive in [true, false] {
        let mut runner = RuntimeRunner::new_from_namerena_raw(
            "lazy@!

enemy-a

enemy-b"
                .to_owned(),
        )
        .unwrap();
        let boss = runner.input_groups[0][0];
        let target = runner.input_groups[1][0];
        let runtime = &mut runner.runtime;
        runtime.entities.get_mut(target).unwrap().runtime.hp = 10_000;
        assert!(runtime.infect_with_lazy_into(boss, target, &mut RunUpdates::new()));
        runtime.set_lazy_boss_at_boost(boss, 2.0);
        if !source_alive {
            runtime.entities.get_mut(boss).unwrap().runtime.hp = 0;
            runtime.drain_plain_lethal_damage_into(runner.input_groups[2][0], boss, &mut RunUpdates::new());
            assert_eq!(runtime.world.alive_group_count(), 2);
        }

        let frame = runtime.run_state_hooks(target, ProcMask::POST_ACTION).expect("感染者仍会在行动后发作");
        assert!(messages_of(&frame.updates).iter().any(|message| message.contains("懒癌发作")));
        assert!(runtime.entities.get(target).unwrap().runtime.hp < 10_000);
        assert_eq!(runtime.lazy_boss_at_boost(boss), Some(2.0), "发作并非懒癌的当前普攻");
    }
}

#[test]
fn boss_scan_prefers_ice_age_and_uses_selected_target_count() {
    let mut runtime = boss_runtime("aokiji");
    advance_until_r127_below(&mut runtime, 70);
    let mut expected = runtime.rng.clone();
    expected.r127();

    let prepared = runtime
        .scan_plain_boss_action_skills(EntityIdx(0), false)
        .expect("aokiji should pick ice age when its roll passes");
    assert_eq!(prepared.selected, BossActionSkill::IceAge);

    let mut reference = boss_runtime("aokiji");
    reference.rng = expected;
    let expected_targets = reference.select_plain_default_enemy_targets_with_count(EntityIdx(0), false, 5);
    assert_eq!(prepared.targets, expected_targets);
    assert_rng_state_eq(&runtime.rng, &reference.rng);
}

#[test]
fn boss_scan_skips_mario_get_rng_when_grown_with_spare_lives() {
    let mut runtime = boss_runtime("mario");
    add_mario_growth(&mut runtime, 3, 120);
    advance_until_r127_below(&mut runtime, 120);
    let mut expected = runtime.rng.clone();
    expected.r127();

    let prepared = runtime
        .scan_plain_boss_action_skills(EntityIdx(0), false)
        .expect("mario should fall through to fire when the flower roll passes");
    assert_eq!(prepared.selected, BossActionSkill::Fire);

    let mut reference = boss_runtime("mario");
    add_mario_growth(&mut reference, 3, 120);
    reference.rng = expected;
    let expected_targets = reference.select_plain_default_enemy_targets(EntityIdx(0), false);
    assert_eq!(prepared.targets, expected_targets);
    assert_rng_state_eq(&runtime.rng, &reference.rng);
}

#[test]
fn boss_scan_mario_get_seven_percent_roll_selects_empty_targets() {
    let mut runtime = boss_runtime("mario");
    add_mario_growth(&mut runtime, 2, 120);
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.mario_reraise_lives = Some(1);
    advance_until_next_u8_below(&mut runtime, 7);
    let mut expected = runtime.rng.clone();
    expected.next_u8();

    let prepared = runtime
        .scan_plain_boss_action_skills(EntityIdx(0), false)
        .expect("mario should pick the mushroom when the seven-percent roll passes");
    assert_eq!(prepared.selected, BossActionSkill::MarioGet);
    assert!(prepared.targets.is_empty());
    assert_rng_state_eq(&runtime.rng, &expected);
}

#[test]
fn boss_skill_probability_absorb_smart_gates_missing_hp_without_rng() {
    let mut runtime = boss_runtime("mosquito");
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 980;
    let expected = runtime.rng.clone();

    assert!(!runtime.plain_boss_action_skill_probability(EntityIdx(0), BossActionSkill::Absorb, 100, true));
    assert_rng_state_eq(&runtime.rng, &expected);

    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 900;
    let mut expected = runtime.rng.clone();
    let expected_result = expected.r127() < 100;
    assert_eq!(
        runtime.plain_boss_action_skill_probability(EntityIdx(0), BossActionSkill::Absorb, 100, true),
        expected_result
    );
    assert_rng_state_eq(&runtime.rng, &expected);

    // 非 smart 路径不做血量差检查，直接走基础概率。
    let mut expected = runtime.rng.clone();
    let expected_result = expected.r127() < 100;
    assert_eq!(
        runtime.plain_boss_action_skill_probability(EntityIdx(0), BossActionSkill::Absorb, 100, false),
        expected_result
    );
    assert_rng_state_eq(&runtime.rng, &expected);
}

#[test]
fn boss_mario_get_grows_through_mushroom_flower_and_life() {
    let mut runtime = boss_runtime("mario");
    let mut updates = RunUpdates::new();

    runtime.drain_plain_mario_get_into(EntityIdx(0), &mut updates);
    let entity = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(mario_growth_value(&runtime), Some((1, 0)));
    assert_eq!(entity.runtime.at_boost(), 1.5);
    assert_eq!(entity.runtime.mario_reraise_lives, Some(3));
    assert_eq!(entity.runtime.move_state.speed_points, 2000);
    assert_eq!(
        messages_of(&updates),
        [get_lang(keys::BOSS_MARIO_GROW_10), get_lang(keys::BOSS_MARIO_GROW_11)]
    );

    updates.reset();
    runtime.drain_plain_mario_get_into(EntityIdx(0), &mut updates);
    let entity = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(mario_growth_value(&runtime), Some((2, 120)));
    assert_eq!(entity.runtime.at_boost(), 1.5);
    assert_eq!(entity.runtime.move_state.speed_points, 4000);
    assert_eq!(
        messages_of(&updates),
        [get_lang(keys::BOSS_MARIO_GROW_20), get_lang(keys::BOSS_MARIO_GROW_21)]
    );

    updates.reset();
    runtime.drain_plain_mario_get_into(EntityIdx(0), &mut updates);
    let entity = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(mario_growth_value(&runtime), Some((3, 120)));
    assert_eq!(entity.runtime.mario_reraise_lives, Some(4));
    assert_eq!(entity.runtime.move_state.speed_points, 6000);
    let messages = messages_of(&updates);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], get_lang(keys::BOSS_MARIO_GROW_30));
    assert_eq!(messages[1], get_lang(keys::BOSS_MARIO_LIFE));
    assert_eq!(updates.updates[1].param, Some(4));
}

#[test]
fn boss_yuri_control_charms_with_initial_group_floor_and_stacks_on_recharm() {
    let mut runtime = yuri_runtime();
    let mut updates = RunUpdates::new();

    runtime.drain_plain_yuri_control_into(EntityIdx(0), EntityIdx(1), &mut updates);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(PLAIN_CHARM_STATE_KEY)
            .and_then(StateEntry::charm_value),
        Some((0, Some(0), Some(0), Some(1), 4))
    );
    assert_eq!(
        messages_of(&updates),
        [get_lang(keys::SKL_YURI_CONTROL), get_lang(keys::SKL_CHARM_HIT)]
    );
    assert_eq!(updates.updates[0].score, 1);
    assert_eq!(updates.updates[1].score, 120);

    updates.reset();
    runtime.drain_plain_yuri_control_into(EntityIdx(0), EntityIdx(1), &mut updates);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .states
            .entry(PLAIN_CHARM_STATE_KEY)
            .and_then(StateEntry::charm_value),
        Some((0, Some(0), Some(0), Some(1), 8))
    );

    // 单人组按 0.5.5 的初始人数下限取 3。
    updates.reset();
    runtime.drain_plain_yuri_control_into(EntityIdx(0), EntityIdx(5), &mut updates);
    assert_eq!(
        runtime
            .entities
            .get(EntityIdx(5))
            .unwrap()
            .states
            .entry(PLAIN_CHARM_STATE_KEY)
            .and_then(StateEntry::charm_value),
        Some((0, Some(0), Some(0), Some(5), 3))
    );
}

#[test]
fn boss_skill_attack_uses_magic_only_for_aokiji_and_ikaruga() {
    for (name, levels) in [("aokiji", &[70u32, 80][..]), ("ikaruga", &[48][..])] {
        let mut runtime = boss_runtime(name);
        {
            let boss = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            boss.runtime.attack = 100;
            boss.runtime.magic = 1000;
            boss.runtime.magic_point = 500;
        }
        advance_until_boss_scan_misses(&mut runtime, levels);
        let mut probe = runtime.rng.clone();
        let req_mp = (probe.next_u8() & 15) as i32 + 8;
        let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
        let prepared = runtime
            .prepare_plain_action(EntityIdx(0), true, pre_action)
            .expect("boss fallback should produce an action");
        let PreparedPlainAction::BasicAttack {
            use_magic,
            amount,
            target,
        } = prepared
        else {
            panic!("{name} should fall back to SklAttack: {prepared:?}");
        };
        assert!(use_magic, "{name} should use magic when smart and mp allows");
        assert_eq!(amount, 1000);
        assert_ne!(target, EntityIdx(0));
        let magic_cost = (1000 - 100) >> 2;
        assert_eq!(
            runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point,
            500 - req_mp - magic_cost
        );
    }
}

#[test]
fn boss_skill_attack_stays_physical_without_smart_or_mp() {
    for (name, levels) in [("aokiji", &[70u32, 80][..]), ("ikaruga", &[48][..])] {
        let mut runtime = boss_runtime(name);
        {
            let boss = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            boss.runtime.attack = 100;
            boss.runtime.magic = 1000;
            boss.runtime.magic_point = 500;
        }
        advance_until_boss_scan_misses(&mut runtime, levels);
        let mut probe = runtime.rng.clone();
        let req_mp = (probe.next_u8() & 15) as i32 + 8;
        let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
        let prepared = runtime
            .prepare_plain_action(EntityIdx(0), false, pre_action)
            .expect("boss fallback should produce an action");
        assert!(
            matches!(
                prepared,
                PreparedPlainAction::BasicAttack {
                    use_magic: false,
                    amount: 100,
                    ..
                }
            ),
            "{name} without smart should stay physical: {prepared:?}"
        );
        assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_point, 500 - req_mp);

        let mut runtime = boss_runtime(name);
        {
            let boss = runtime.entities.get_mut(EntityIdx(0)).unwrap();
            boss.runtime.attack = 100;
            boss.runtime.magic = 1000;
            boss.runtime.magic_point = (1000 - 100) >> 2;
        }
        advance_until_boss_scan_misses(&mut runtime, levels);
        let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
        let prepared = runtime
            .prepare_plain_action(EntityIdx(0), true, pre_action)
            .expect("boss fallback should produce an action");
        assert!(
            matches!(
                prepared,
                PreparedPlainAction::BasicAttack {
                    use_magic: false,
                    amount: 100,
                    ..
                }
            ),
            "{name} without enough mp should stay physical: {prepared:?}"
        );
    }
}

/// 柯南 Boss 使用固定 4 目标的 smart 选敌，并跳过召唤物。
fn conan_runtime(heroes: usize, minions: usize) -> CombatRuntime {
    let mut builder = ExtensionRegistryBuilder::default();
    let boss_kind = builder
        .register_player_kind_with_policies(
            "core",
            "boss",
            "core.boss",
            PlayerKindFlags::BOSS,
            PlayerKindPolicies::default(),
        )
        .expect("boss kind should register");
    let minion_kind = builder
        .register_player_kind_with_policies(
            "core",
            "minion",
            "core.minion",
            PlayerKindFlags::MINION | PlayerKindFlags::COMBAT_MINION,
            PlayerKindPolicies::default(),
        )
        .expect("minion kind should register");
    let mut templates = vec![PlayerTemplate::with_kind(1, "conan", boss_kind, 0, 1000, 3)];
    for index in 0..heroes {
        templates.push(PlayerTemplate::new(2 + index, format!("hero-{index}"), 1, 100, 3));
    }
    for index in 0..minions {
        templates.push(PlayerTemplate::with_kind(2 + heroes + index, "summon", minion_kind, 1, 100, 3));
    }
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(templates, builder.build()))
}

fn next_conan_duel_frame(runner: &mut RuntimeRunner) -> RoundOutcome {
    (0..64)
        .map(|_| runner.run_round())
        .find(|outcome| outcome.frame.is_some())
        .expect("应产生可见回合")
}

#[test]
fn boss_conan_public_duel_can_target_a_clone() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("conan@!\n\nfighter22".to_owned(), config).unwrap();
    next_conan_duel_frame(&mut runner);
    next_conan_duel_frame(&mut runner);
    let runtime = runner.runtime();
    let (clone, _) = runtime
        .entities
        .iter()
        .find(|(_, entity)| entity.template.id_key_name == "fighter22?0@fighter22")
        .unwrap();
    assert_eq!((runtime.rng.i, runtime.rng.j), (91, 202));

    let frame = next_conan_duel_frame(&mut runner).frame.unwrap();
    let hit = &frame.updates.updates[0];
    assert_eq!(hit.message, get_lang(keys::SKL_CONAN_KILL));
    assert_eq!(hit.target, clone.0 as usize);
    let runtime = runner.runtime();
    assert!(!runtime.entities.get(clone).unwrap().runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 141);
    assert_eq!((runtime.rng.i, runtime.rng.j), (120, 209));
    assert_eq!(
        runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points,
        1035
    );
    assert!(runner.run_until_winner(64).winner_team.is_some());
    assert!(runner.input_group_won(0));
}

#[test]
fn boss_conan_retries_smart_after_empty_ordinary_sample() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("conan@!\n\nhero".to_owned(), config).unwrap();
    let runtime = runner.runtime_mut();
    let (conan, hero) = (EntityIdx(0), EntityIdx(1));
    let summon_kind = runtime.registry.player_kind_id_by_export_name(DEFAULT_CORE_SUMMON_KIND_EXPORT).unwrap();
    let team = runtime.entities.get(hero).unwrap().runtime.team;
    // 与原版受控战斗相同：敌队先生成八只使魔，再推进四字节。
    for idx in 0..8 {
        let speed_points = runtime.rng.r255() as i32 * 4;
        let summon = runtime.entities.spawn_from_template_with_owner(
            PlayerTemplate::with_kind(0, format!("hero?{idx}"), summon_kind, team, 20, 1).with_speed_points(speed_points),
            &runtime.registry,
            Some(hero),
            Some(hero),
        );
        runtime.world.add_spawned_alive(summon, team);
    }
    for _ in 0..4 {
        runtime.rng.r255();
    }
    assert_eq!((runtime.rng.i, runtime.rng.j), (45, 3));

    // 原版普通样本只抽到使魔，智能重抽才抽到 hero。
    assert_eq!(runtime.select_plain_conan_action_targets(conan, false).as_slice(), &[hero]);
    assert_eq!((runtime.rng.i, runtime.rng.j), (79, 45));
}

#[test]
fn conan_public_duel_counts_mario_after_reraise() {
    let config = default_custom_runtime_import_config().unwrap();
    let mut runner = RuntimeRunner::from_custom_mixed_namerena_raw("conan@!\n\nmario@!".to_owned(), config).unwrap();
    let (conan, mario) = (EntityIdx(0), EntityIdx(1));

    // 原版公开对局：马里奥先吃蘑菇，柯南在首次受击之前累计了五份速度。
    next_conan_duel_frame(&mut runner);
    let runtime = runner.runtime();
    assert_eq!(runtime.entities.get(conan).unwrap().runtime.move_state.speed_points, 1083);
    assert_eq!((runtime.rng.i, runtime.rng.j), (50, 125));

    // 初始速度过高会让柯南在马里奥第二次普通攻击前开始推理，并改变整个后续随机流。
    for hp in [152, 91] {
        assert_eq!(next_conan_duel_frame(&mut runner).action.unwrap().actor, mario);
        assert_eq!(runner.runtime().entities.get(conan).unwrap().runtime.hp, hp);
    }
    assert_eq!(next_conan_duel_frame(&mut runner).action.unwrap().actor, conan);
    let growth = next_conan_duel_frame(&mut runner).frame.unwrap();
    assert_eq!(growth.updates.updates[0].caster, mario.0 as usize);
    assert_eq!(growth.updates.updates[0].message, get_lang(keys::BOSS_MARIO_GROW_20));

    let outcome = next_conan_duel_frame(&mut runner);
    assert_eq!(outcome.action.unwrap().actor, conan);
    let messages = messages_of(&outcome.frame.unwrap().updates);
    assert!(
        messages
            .windows(3)
            .any(|window| window == ["[1]被击倒了", "[0]满血复活", "[0]还剩[2]条命"])
    );
    let runtime = runner.runtime();
    let revived = &runtime.entities.get(mario).unwrap().runtime;
    assert_eq!((revived.hp, revived.mario_reraise_lives), (386, Some(2)));
    assert_eq!(revived.move_state.speed_points, 2109);
    assert_eq!(runtime.entities.get(conan).unwrap().runtime.move_state.speed_points, 1083);
    assert_eq!((runtime.rng.i, runtime.rng.j), (147, 29));
    assert!(next_conan_duel_frame(&mut runner).winner_team.is_some());
    assert!(runner.input_group_won(1));
}

#[test]
fn boss_conan_kills_first_multi_target_and_charges_speed_by_group_size() {
    let mut runtime = conan_runtime(2, 0);
    let targets = runtime.select_plain_conan_targets(EntityIdx(0));
    assert_eq!(targets.len(), 2, "conan should sample up to four targets");
    let first = targets[0];
    let mut updates = RunUpdates::new();

    runtime.drain_plain_conan_action_into(EntityIdx(0), targets, &mut updates);

    let victim = runtime.entities.get(first).unwrap();
    assert_eq!(victim.runtime.hp, 0);
    assert!(!victim.runtime.alive);
    assert_eq!(
        messages_of(&updates),
        [
            get_lang(keys::SKL_CONAN_KILL).to_owned(),
            "\n".to_owned(),
            "[1]被击倒了".to_owned()
        ]
    );
    assert_eq!(updates.updates[0].score, 180);
    assert_eq!(updates.updates[0].target, first.0 as usize);
    let conan = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(conan.runtime.conan_thinking, Some(1));
    assert_eq!(conan.runtime.move_state.speed_points, 1000);
}

#[test]
fn boss_conan_single_target_thinks_once_before_final_kill() {
    let mut runtime = conan_runtime(1, 0);
    let targets = runtime.select_plain_conan_targets(EntityIdx(0));
    assert_eq!(targets.as_slice(), &[EntityIdx(1)]);
    let mut updates = RunUpdates::new();

    runtime.drain_plain_conan_action_into(EntityIdx(0), targets, &mut updates);

    assert_eq!(
        messages_of(&updates),
        [
            get_lang(keys::SKL_CONAN_KILL_UNKNOWN).to_owned(),
            "\n".to_owned(),
            get_lang(keys::SKL_CONAN_THINKING).to_owned(),
        ]
    );
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.conan_thinking, Some(0));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 100);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 0);

    updates.reset();
    let targets = runtime.select_plain_conan_targets(EntityIdx(0));
    runtime.drain_plain_conan_action_into(EntityIdx(0), targets, &mut updates);

    let messages = messages_of(&updates);
    assert_eq!(messages[0], get_lang(keys::SKL_CONAN_THINKING_FINISH));
    assert_eq!(messages[1], get_lang(keys::SKL_CONAN_THINKING_FINISH2));
    assert_eq!(messages[2], get_lang(keys::SKL_CONAN_THINKING_FINISH3));
    assert_eq!(messages[3], get_lang(keys::SKL_CONAN_KILL_LAST));
    assert_eq!(messages[4], "\n");
    assert_eq!(messages[5], "[1]被击倒了");
    assert_eq!(updates.updates[1].delay1, 2000);
    assert_eq!(updates.updates[3].score, 180);
    assert_eq!(updates.updates[3].target, 1);
    let victim = runtime.entities.get(EntityIdx(1)).unwrap();
    assert_eq!(victim.runtime.hp, 0);
    assert!(!victim.runtime.alive);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.conan_thinking, Some(0));
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 0);
}

#[test]
fn boss_conan_target_selection_skips_minions() {
    let mut runtime = conan_runtime(1, 1);
    for _ in 0..40 {
        let targets = runtime.select_plain_conan_targets(EntityIdx(0));
        assert_eq!(
            targets.as_slice(),
            &[EntityIdx(1)],
            "only the hero should be selectable: {targets:?}"
        );
        runtime.rng.next_u8();
    }
}

#[test]
fn boss_prepare_routes_conan_to_custom_action() {
    let mut runtime = conan_runtime(2, 0);
    let pre_action = runtime.run_plain_skill_pre_action_accumulator(EntityIdx(0));
    let prepared = runtime
        .prepare_plain_action(EntityIdx(0), true, pre_action)
        .expect("conan fallback should produce an action");
    match prepared {
        PreparedPlainAction::Conan { targets } => assert_eq!(targets.len(), 2),
        other => panic!("conan should route to its custom action: {other:?}"),
    }
}

#[test]
fn boss_ice_age_hits_every_selected_target_with_shared_atp() {
    let mut runtime = boss_runtime("aokiji");
    let targets = smallvec::smallvec![EntityIdx(1), EntityIdx(2)];
    let mut updates = RunUpdates::new();

    runtime.drain_plain_boss_ice_age_into(EntityIdx(0), targets, &mut updates);

    assert_eq!(updates.updates[0].message, get_lang(keys::SKL_AOKIJI_ICE_AGE));
    assert_eq!(updates.updates[0].score, 1);
    assert_eq!(updates.updates[0].target, 1);
    for target in [EntityIdx(1), EntityIdx(2)] {
        assert!(
            runtime.entities.get(target).unwrap().runtime.hp <= 0,
            "ice age should keep attacking after the first target falls: hp={} updates={:?}",
            runtime.entities.get(target).unwrap().runtime.hp,
            messages_of(&updates)
        );
    }
}

#[test]
fn boss_aokiji_defend_absorbs_ice_without_rng() {
    let mut runtime = boss_runtime("aokiji");
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 900;
    let expected_rng = runtime.rng.clone();
    let mut updates = RunUpdates::new();

    let amount = runtime.drain_plain_attack_after_dodge_into(
        EntityIdx(1),
        EntityIdx(0),
        true,
        640.0,
        DamageKind::Ice,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );

    assert_eq!(amount, -10);
    assert_rng_state_eq(&runtime.rng, &expected_rng);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 910);
    assert_eq!(messages_of(&updates), [get_lang(keys::SKL_AOKIJI_DEFEND), "[1]回复体力[2]点"]);
    assert_eq!(updates.updates[0].caster, 0);
    assert_eq!(updates.updates[0].score, 10);
    assert_eq!(updates.updates[0].param, None);
    assert_eq!(updates.updates[1].caster, 1);
    assert_eq!(updates.updates[1].target, 0);
    assert_eq!(updates.updates[1].score, 0);
    assert_eq!(updates.updates[1].param, Some(10));
}

#[test]
fn boss_aokiji_defend_nullifies_physical_and_passes_default() {
    let mut runtime = boss_runtime("aokiji");
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 900;
    let mut updates = RunUpdates::new();

    let amount = runtime.drain_plain_attack_after_dodge_into(
        EntityIdx(1),
        EntityIdx(0),
        false,
        640.0,
        DamageKind::Physical,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );
    assert_eq!(amount, 0);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 900);
    assert_eq!(messages_of(&updates), ["[0]受到[2]点伤害[s_dmg0]"]);
    assert_eq!(updates.updates[0].param, Some(0));

    updates.reset();
    let amount = runtime.drain_plain_attack_after_dodge_into(
        EntityIdx(1),
        EntityIdx(0),
        false,
        640.0,
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );
    assert_eq!(amount, 10);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 890);
    assert_eq!(messages_of(&updates), ["[1]受到[2]点伤害"]);
}

#[test]
fn boss_ikaruga_defend_absorbs_odd_and_passes_even() {
    let mut runtime = boss_runtime("ikaruga");
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 900;
    let mut updates = RunUpdates::new();

    // 10 点偶数伤害直接通过，斑鸠不吸收。
    let amount = runtime.drain_plain_attack_after_dodge_into(
        EntityIdx(1),
        EntityIdx(0),
        false,
        640.0,
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );
    assert_eq!(amount, 10);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 890);
    assert_eq!(messages_of(&updates), ["[1]受到[2]点伤害"]);

    // 11 点奇数伤害被吸收并回血。
    updates.reset();
    let amount = runtime.drain_plain_attack_after_dodge_into(
        EntityIdx(1),
        EntityIdx(0),
        false,
        704.0,
        DamageKind::Default,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );
    assert_eq!(amount, -11);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 901);
    assert_eq!(messages_of(&updates), [get_lang(keys::SKL_IKARUGA_DEFEND), "[1]回复体力[2]点"]);
    assert_eq!(updates.updates[0].score, 11);
    assert_eq!(updates.updates[1].param, Some(11));
}

#[test]
fn boss_aokiji_defend_heal_caps_at_max_hp_but_message_keeps_absorbed_amount() {
    let mut runtime = boss_runtime("aokiji");
    let mut updates = RunUpdates::new();

    let amount = runtime.drain_plain_attack_after_dodge_into(
        EntityIdx(1),
        EntityIdx(0),
        true,
        6_400.0,
        DamageKind::Ice,
        None,
        PlainAttackOnDamage::None,
        &mut updates,
    );

    assert_eq!(amount, -100);
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 1000);
    assert_eq!(updates.updates[1].param, Some(100));
}

fn mage_aokiji_runtime() -> CombatRuntime {
    let (boss_kind, registry) = boss_test_registry();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "aokiji", boss_kind, 0, 1000, 3),
            PlayerTemplate::new(2, "mage", 1, 100, 3).with_magic(640),
        ],
        registry,
    ))
}

/// 推进 RNG，直到冰冻术的 get_at 与闪避掷骰都不触发闪避（命中下限 7）。
fn advance_until_ice_skill_not_dodged(runtime: &mut CombatRuntime) {
    loop {
        let mut probe = runtime.rng.clone();
        let _ = runtime.entities.get(EntityIdx(1)).unwrap().runtime.get_at(true, &mut probe);
        if probe.next_u8() >= 7 {
            break;
        }
        runtime.rng.next_u8();
    }
}

#[test]
fn boss_ice_skill_kind_reaches_aokiji_absorb() {
    let mut runtime = mage_aokiji_runtime();
    advance_until_ice_skill_not_dodged(&mut runtime);
    let mut probe = runtime.rng.clone();
    let atp = runtime.entities.get(EntityIdx(1)).unwrap().runtime.get_at(true, &mut probe) * 0.699999988079071;
    let expected_damage = (atp / 64.0).ceil() as u32;
    assert!(expected_damage > 0);

    let mut updates = RunUpdates::new();
    runtime.drain_plain_ice_skill_into(EntityIdx(1), EntityIdx(0), &mut updates);

    assert_eq!(
        messages_of(&updates),
        ["[0]使用[冰冻术]", get_lang(keys::SKL_AOKIJI_DEFEND), "[1]回复体力[2]点"]
    );
    assert_eq!(updates.updates[1].score, expected_damage);
    assert_eq!(updates.updates[2].param, Some(expected_damage));
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.hp, 1000);
}

fn boss_vs_aokiji_runtime(attacker: &str) -> CombatRuntime {
    let (boss_kind, registry) = boss_test_registry();
    CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, attacker, boss_kind, 0, 1000, 1000),
            PlayerTemplate::with_kind(2, "aokiji", boss_kind, 1, 1000, 3),
        ],
        registry,
    ))
}

/// 推进 RNG，直到普攻的 get_at 与闪避掷骰都不触发闪避（命中下限 7）。
fn advance_until_default_attack_not_dodged(runtime: &mut CombatRuntime) {
    loop {
        let mut probe = runtime.rng.clone();
        let _ = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(false, &mut probe);
        if probe.next_u8() >= 7 {
            break;
        }
        runtime.rng.next_u8();
    }
}

#[test]
fn boss_skill_attack_physical_kind_feeds_aokiji_absorb() {
    // 青雉的普攻走 `SklAttack.v` 物理分支，命中同为青雉的目标时被物理吸收清零。
    let mut runtime = boss_vs_aokiji_runtime("aokiji");
    advance_until_default_attack_not_dodged(&mut runtime);
    let mut updates = RunUpdates::new();
    runtime.drain_plain_default_attack_into(EntityIdx(0), EntityIdx(1), false, &mut updates);

    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp, 1000);
    assert_eq!(messages_of(&updates), ["[0]发起攻击", "[0]受到[2]点伤害[s_dmg0]"]);
    assert_eq!(updates.updates[1].param, Some(0));

    // 马里奥的普攻来自 `SklSimpleAttack`，使用默认伤害类型，不会被吸收。
    let mut runtime = boss_vs_aokiji_runtime("mario");
    advance_until_default_attack_not_dodged(&mut runtime);
    let mut updates = RunUpdates::new();
    runtime.drain_plain_default_attack_into(EntityIdx(0), EntityIdx(1), false, &mut updates);

    let hp = runtime.entities.get(EntityIdx(1)).unwrap().runtime.hp;
    assert!(hp < 1000, "default-kind boss attack should not be absorbed: hp={hp}");
    assert_eq!(messages_of(&updates)[1], "[1]受到[2]点伤害");
}

#[test]
fn boss_ikaruga_attack_uses_defense_conversion_for_every_target() {
    let mut runtime = boss_runtime("ikaruga");
    let targets = smallvec::smallvec![EntityIdx(1), EntityIdx(2)];
    let mut updates = RunUpdates::new();

    runtime.drain_plain_boss_ikaruga_attack_into(EntityIdx(0), targets, &mut updates);

    assert_eq!(updates.updates[0].message, get_lang(keys::SKL_IKARUGA_ATTACK));
    assert_eq!(updates.updates[0].score, 1);
    assert_eq!(updates.updates[0].target, 1);
    for target in [EntityIdx(1), EntityIdx(2)] {
        assert!(
            runtime.entities.get(target).unwrap().runtime.hp <= 0,
            "energy release should keep attacking after the first target falls: hp={} updates={:?}",
            runtime.entities.get(target).unwrap().runtime.hp,
            messages_of(&updates)
        );
    }
}

#[test]
fn boss_mario_reraise_consumes_life_and_resets_growth() {
    let mut runtime = boss_runtime("mario");
    let mut updates = RunUpdates::new();
    runtime.drain_plain_mario_get_into(EntityIdx(0), &mut updates);
    assert_eq!(mario_growth_value(&runtime), Some((1, 0)));
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.at_boost(), 1.5);
    assert!(
        runtime
            .entities
            .get_mut(EntityIdx(0))
            .unwrap()
            .states
            .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 3)),
        "ice state should be inserted"
    );
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
    updates.reset();

    assert!(runtime.drain_plain_mario_reraise_into(EntityIdx(0), &mut updates));

    let entity = runtime.entities.get(EntityIdx(0)).unwrap();
    assert_eq!(entity.runtime.hp, entity.template.max_hp);
    assert_eq!(entity.runtime.mario_reraise_lives, Some(2));
    // 复活会重置采菇成长与 1.5 倍攻击，但保留每次采菇叠加的速度。
    assert_eq!(entity.runtime.at_boost(), 1.0);
    assert_eq!(entity.runtime.move_state.speed_points, 2000);
    assert_eq!(
        entity.states.entry(PLAIN_MARIO_BOSS_STATE_KEY).and_then(StateEntry::mario_boss_value),
        None
    );
    assert!(entity.states.entry(PLAIN_ICE_STATE_KEY).is_none());
    assert_eq!(
        messages_of(&updates),
        [get_lang(keys::BOSS_MARIO_REVIVE), get_lang(keys::BOSS_MARIO_LIFE)]
    );
    assert_eq!(updates.updates[0].delay0, 3000);
    assert_eq!(updates.updates[1].param, Some(2));
}

#[test]
fn boss_mario_lethal_hit_reraise_keeps_entity_alive() {
    let mut runtime = boss_runtime("mario");
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 0;
    let mut updates = RunUpdates::new();

    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(0), &mut updates);

    let entity = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(entity.runtime.alive);
    assert_eq!(entity.runtime.hp, entity.template.max_hp);
    assert_eq!(entity.runtime.mario_reraise_lives, Some(2));
    assert_eq!(
        messages_of(&updates),
        [
            "\n",
            "[1]被击倒了",
            get_lang(keys::BOSS_MARIO_REVIVE),
            get_lang(keys::BOSS_MARIO_LIFE)
        ]
    );
}

#[test]
fn boss_mario_reraise_with_last_life_stays_dead() {
    let mut runtime = boss_runtime("mario");
    {
        let boss = &mut runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime;
        boss.hp = 0;
        boss.mario_reraise_lives = Some(1);
    }
    let mut updates = RunUpdates::new();

    runtime.drain_plain_lethal_damage_into(EntityIdx(1), EntityIdx(0), &mut updates);

    let entity = runtime.entities.get(EntityIdx(0)).unwrap();
    assert!(!entity.runtime.alive);
    assert_eq!(entity.runtime.hp, 0);
    assert_eq!(entity.runtime.mario_reraise_lives, Some(0));
    assert_eq!(messages_of(&updates), ["\n", "[1]被击倒了"]);
}

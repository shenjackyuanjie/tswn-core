use super::*;

use crate::namerena::BossActionSkill;
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

#[test]
fn boss_scan_saitama_only_burns_one_probe_byte() {
    let mut runtime = boss_runtime("saitama");
    let mut expected = runtime.rng.clone();
    expected.r127();

    assert!(runtime.scan_plain_boss_action_skills(EntityIdx(0), false).is_none());
    assert_rng_state_eq(&runtime.rng, &expected);
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

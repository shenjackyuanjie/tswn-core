//! 冰冻（ice）技能与调度：冻结优先序、haste 倍率刷新与解冻保留行动。
use super::*;

#[test]
fn plain_ice_freezes_before_hide_post_damage_rng() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let hide = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_HIDE_SKILL_EXPORT)
        .expect("default profile should register hide skill");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 1_000, 1_000_000).with_magic(1_000_000),
            PlayerTemplate::new(2, "target", 1, 1_000, 0)
                .with_def_res(0, 0)
                .with_skill_loadout(SkillLoadout::from_skill_levels([(hide, 64)])),
            PlayerTemplate::new(3, "ally", 1, 1_000, 0),
        ],
        config.registry,
    ));
    let mut expected_rng = runtime.rng.clone();
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng) * 0.699999988079071;
    let accuracy = runtime.entities.get(EntityIdx(0)).unwrap().runtime.magic_accuracy();
    let dodge = runtime.entities.get(EntityIdx(1)).unwrap().runtime.magic_dodge();
    assert!(!PlayerRuntime::dodge(accuracy, dodge, &mut expected_rng));
    assert!(atp > 0.0);

    let mut updates = RunUpdates::new();
    runtime.drain_plain_ice_skill_into(EntityIdx(0), EntityIdx(1), &mut updates);

    assert_rng_state_eq(&runtime.rng, &expected_rng);
    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.states.is_frozen());
    assert!(target.runtime.hide.is_none());
}

#[test]
fn plain_ice_first_application_refreshes_pending_haste_multiplier() {
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let haste = config
        .registry
        .state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT)
        .expect("default profile should register haste state");
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "caster", 0, 100, 3),
            PlayerTemplate::new(2, "target", 1, 100, 3).with_speed(100),
        ],
        config.registry,
    ));
    runtime
        .entities
        .get_mut(EntityIdx(1))
        .unwrap()
        .states
        .add_entry(StateEntry::haste_with_effective_faster(
            PLAIN_HASTE_STATE_KEY,
            haste,
            4,
            2,
            9,
            SkillPriority(210),
        ));
    assert_eq!(runtime.entities.get(EntityIdx(1)).unwrap().effective_speed(), 200);
    let mut updates = RunUpdates::new();

    runtime.apply_ice_on_damage(EntityIdx(0), EntityIdx(1), &mut updates);

    let target = runtime.entities.get(EntityIdx(1)).unwrap();
    assert!(target.states.is_frozen());
    assert_eq!(
        target.states.entry(PLAIN_HASTE_STATE_KEY).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(target.effective_speed(), 400);
}

#[test]
fn ice_pre_step_zero_step_does_not_release_at_threshold() {
    let mut states = StateStore::default();
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    let generation = states.generation();

    let (step, released) = states.apply_ice_pre_step(0, crate::runtime::MOVE_POINT_THRESHOLD + 1);

    assert_eq!(step, 0);
    assert!(!released);
    assert!(states.is_frozen());
    assert_eq!(states.generation(), generation);
}

#[test]
fn ice_pre_step_positive_step_releases_when_crossing_threshold() {
    let mut states = StateStore::default();
    states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));

    let (step, released) = states.apply_ice_pre_step(1, crate::runtime::MOVE_POINT_THRESHOLD);

    assert_eq!(step, 0);
    assert!(released);
    assert!(!states.is_frozen());
}

#[test]
fn natural_ice_release_commits_pending_charm_team_and_haste_multiplier() {
    let config = default_custom_runtime_import_config().unwrap();
    let haste = config.registry.state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT).unwrap();
    let charm = config.registry.state_id_by_export_name(DEFAULT_CORE_CHARM_STATE_EXPORT).unwrap();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "frozen", 0, 100, 3)
                .with_speed(100)
                .with_speed_points(crate::runtime::MOVE_POINT_THRESHOLD),
            PlayerTemplate::new(2, "old charmer", 1, 100, 3),
            PlayerTemplate::new(3, "new charmer", 2, 100, 3),
        ],
        config.registry,
    ));
    let frozen = EntityIdx(0);
    let owner = runtime.entities.get_mut(frozen).unwrap();
    owner.states.add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    // 重魅和叠加疾走只更新来源/倍率；自然解冻前仍应读取旧值。
    owner.states.add_entry(StateEntry::charm(
        crate::runtime::profile::PLAIN_CHARM_STATE_KEY,
        charm,
        2,
        Some(1),
        Some(2),
        Some(0),
        2,
        SkillPriority(210),
    ));
    owner.states.add_entry(StateEntry::haste_with_effective_faster(
        PLAIN_HASTE_STATE_KEY,
        haste,
        4,
        2,
        9,
        SkillPriority(210),
    ));
    assert_eq!(owner.effective_speed(), 200);
    assert_eq!(runtime.plain_effective_team(frozen), 1);
    while runtime.rng.clone().next_u8() & 3 == 0 {
        runtime.rng.next_u8();
    }

    let action = runtime
        .scheduler
        .select_action(&mut runtime.world, &mut runtime.entities, &mut runtime.rng);

    assert!(action.is_none());
    assert_eq!(runtime.scheduler.take_ice_release_events().as_slice(), &[frozen]);
    let owner = runtime.entities.get(frozen).unwrap();
    assert!(!owner.states.is_frozen());
    assert_eq!(owner.runtime.move_state.speed_points, crate::runtime::MOVE_POINT_THRESHOLD);
    assert_eq!(
        owner.states.entry(PLAIN_HASTE_STATE_KEY).and_then(StateEntry::haste_runtime_value),
        Some((4, 4, 9))
    );
    assert_eq!(owner.effective_speed(), 400);
    assert_eq!(runtime.plain_effective_team(frozen), 2);
}

#[test]
fn ice_release_keeps_action_when_saved_move_points_already_crossed_threshold() {
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::new(vec![
        PlayerTemplate::new(1, "frozen", 0, 100, 3)
            .with_speed(1)
            .with_speed_points(crate::runtime::MOVE_POINT_THRESHOLD + 1),
        PlayerTemplate::new(2, "target", 1, 100, 3).with_speed(1),
    ]));
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .states
        .add_entry(StateEntry::ice(PLAIN_ICE_STATE_KEY, 0));
    while {
        let mut probe = runtime.rng.clone();
        probe.next_u8() & 3 == 0
    } {
        runtime.rng.next_u8();
    }

    let action = runtime
        .scheduler
        .select_action(&mut runtime.world, &mut runtime.entities, &mut runtime.rng)
        .expect("ice release should keep the pending action");

    assert_eq!(action.actor, EntityIdx(0));
    assert_eq!(runtime.scheduler.take_ice_release_events().as_slice(), &[EntityIdx(0)]);
    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().states.is_frozen());
    assert_eq!(runtime.entities.get(EntityIdx(0)).unwrap().runtime.move_state.speed_points, 1);
}

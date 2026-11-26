//! 疾走（haste）目标选择与加速/减速调度的生效速度计算。
use super::*;

#[test]
fn plain_haste_samples_ally_alive_order_instead_of_roster_order() {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_state(
            "core",
            "haste",
            DEFAULT_CORE_HASTE_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("haste state should register");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::new(1, "actor", 0, 100, 3),
            PlayerTemplate::new(2, "slow-ally", 0, 100, 3).with_target_score_stats(10, 10, 1.0),
            PlayerTemplate::new(3, "best-ally", 0, 100, 3).with_target_score_stats(30, 30, 1.0),
            PlayerTemplate::new(4, "front-alive", 0, 100, 3).with_target_score_stats(20, 20, 1.0),
            PlayerTemplate::new(5, "enemy", 1, 100, 3),
        ],
        registry,
    ));
    runtime.world.sync_initial_views(
        &runtime.entities,
        vec![EntityIdx(0), EntityIdx(1), EntityIdx(2), EntityIdx(3), EntityIdx(4)],
        vec![vec![EntityIdx(0), EntityIdx(1), EntityIdx(2), EntityIdx(3)], vec![EntityIdx(4)]],
        vec![vec![EntityIdx(3), EntityIdx(0), EntityIdx(1), EntityIdx(2)], vec![EntityIdx(4)]],
        vec![EntityIdx(3), EntityIdx(0), EntityIdx(1), EntityIdx(2), EntityIdx(4)],
    );
    runtime.entities.get_mut(EntityIdx(0)).unwrap().runtime.hp = 16;
    runtime.entities.get_mut(EntityIdx(1)).unwrap().runtime.hp = 40;

    let candidates = [EntityIdx(3), EntityIdx(0), EntityIdx(1), EntityIdx(2)];
    let mut expected_rng = runtime.rng.clone();
    let mut expected_selected = Vec::new();
    let mut duplicate_count = 0usize;
    let mut invalid_count = -3i32;
    while duplicate_count <= 3 && invalid_count <= 3 {
        let Some(picked) = expected_rng.pick(&candidates) else {
            break;
        };
        let target = candidates[picked];
        let valid = matches!(target, EntityIdx(2) | EntityIdx(3));
        if !valid {
            invalid_count += 1;
            continue;
        }
        if expected_selected.contains(&target) {
            duplicate_count += 1;
            continue;
        }
        expected_selected.push(target);
        if expected_selected.len() >= 3 {
            break;
        }
    }
    expected_selected.sort_by(|lhs, rhs| {
        runtime
            .score_plain_haste_target(*rhs, true)
            .partial_cmp(&runtime.score_plain_haste_target(*lhs, true))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let selected = runtime.select_plain_haste_targets(EntityIdx(0), true);

    assert_eq!(selected.as_slice(), expected_selected.as_slice());
    assert_rng_state_eq(&runtime.rng, &expected_rng);
}

/// 测试用减速状态的数值 key。
const PLAIN_SLOW_STATE_KEY: u32 = 78;

/// 构造注册了 haste 状态的 ExtensionRegistry，用于加速/减速调度测试。
fn speed_state_registry() -> ExtensionRegistry {
    let mut builder = ExtensionRegistryBuilder::default();
    builder
        .register_state(
            "core",
            "haste",
            DEFAULT_CORE_HASTE_STATE_EXPORT,
            ProcMask::POST_ACTION,
            SkillPriority(210),
        )
        .expect("haste state should register");
    builder
        .register_state("core", "slow", "core.state.slow", ProcMask::POST_ACTION, SkillPriority(210))
        .expect("slow state should register");
    builder.build()
}

#[test]
fn haste_effective_speed_applies_upgrade_after_state_speed() {
    let registry = speed_state_registry();
    let haste_state = registry
        .state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT)
        .expect("haste state should register");
    let template = PlayerTemplate::new(1, "target", 0, 100, 40).with_speed(206);
    let mut entity = EntityRecord {
        runtime: PlayerRuntime::from_template(&template, &registry, EntityIdx(0), EntityIdx(0)),
        template,
        states: StateStore::default(),
        slots: EntitySlotStorage::default(),
    };
    entity
        .states
        .add_entry(StateEntry::haste(PLAIN_HASTE_STATE_KEY, haste_state, 2, 3, SkillPriority(210)));
    entity.runtime.upgrade_active = true;
    entity.refresh_runtime_stats_from_template();

    assert_eq!(entity.runtime.speed, 226);
    assert_eq!(entity.effective_speed(), 432);
}

#[test]
fn haste_effective_speed_preserves_slow_registration_order() {
    let registry = speed_state_registry();
    let haste_state = registry
        .state_id_by_export_name(DEFAULT_CORE_HASTE_STATE_EXPORT)
        .expect("haste state should register");
    let slow_state = registry.state_id_by_export_name("core.state.slow").expect("slow state should register");
    let template = PlayerTemplate::new(1, "target", 0, 100, 40).with_speed(81);
    let mut slow_then_haste = EntityRecord {
        runtime: PlayerRuntime::from_template(&template, &registry, EntityIdx(0), EntityIdx(0)),
        template: template.clone(),
        states: StateStore::default(),
        slots: EntitySlotStorage::default(),
    };
    slow_then_haste
        .states
        .add_entry(StateEntry::slow(PLAIN_SLOW_STATE_KEY, slow_state, 2, SkillPriority(210)));
    slow_then_haste
        .states
        .add_entry(StateEntry::haste(PLAIN_HASTE_STATE_KEY, haste_state, 2, 3, SkillPriority(210)));

    let mut haste_then_slow = EntityRecord {
        runtime: PlayerRuntime::from_template(&template, &registry, EntityIdx(0), EntityIdx(0)),
        template,
        states: StateStore::default(),
        slots: EntitySlotStorage::default(),
    };
    haste_then_slow
        .states
        .add_entry(StateEntry::haste(PLAIN_HASTE_STATE_KEY, haste_state, 2, 3, SkillPriority(210)));
    haste_then_slow
        .states
        .add_entry(StateEntry::slow(PLAIN_SLOW_STATE_KEY, slow_state, 2, SkillPriority(210)));

    assert_eq!(slow_then_haste.effective_speed(), 80);
    assert_eq!(haste_then_slow.effective_speed(), 81);
}

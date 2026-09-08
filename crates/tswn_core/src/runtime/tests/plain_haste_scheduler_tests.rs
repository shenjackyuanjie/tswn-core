use super::*;

const PLAIN_SLOW_STATE_KEY: u32 = 78;

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

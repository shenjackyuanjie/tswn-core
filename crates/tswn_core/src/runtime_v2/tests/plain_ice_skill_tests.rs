use super::*;

#[test]
fn plain_ice_freezes_before_hide_post_damage_rng() {
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
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
    let atp = runtime.entities.get(EntityIdx(0)).unwrap().runtime.get_at(true, &mut expected_rng)
        * crate::player::skill::act::ice::ICE_DAMAGE_MULTIPLIER;
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

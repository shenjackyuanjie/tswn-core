use super::*;

#[test]
fn case_d8c6_import_preserves_plain_pre_action_order_and_assassinate_levels() {
    let raw = "最光辉的时刻 #8ftphKKCk@Shabby_fish\n营救任务 #tmOaPuIoM@Shabby_fish";
    let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("legacy d8c6 runner should construct");
    let config = default_custom_runtime_v2_import_config().expect("default runtime v2 profile should build");
    let runner =
        RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config).expect("runtime v2 d8c6 runner should construct");

    for entity_index in 0..2 {
        let snapshot = legacy
            .storage
            .get_player(&entity_index)
            .expect("legacy d8c6 player should exist")
            .skill_loadout_snapshot();
        let entity = runner
            .runtime()
            .entities
            .get(EntityIdx(entity_index as u32))
            .expect("runtime v2 d8c6 player should exist");
        let imported_pre_action_keys = entity
            .template
            .skills
            .pre_action_order()
            .iter()
            .map(|lane| {
                entity
                    .template
                    .skills
                    .fixed_lane_key_at(*lane)
                    .expect("pre-action lane should have a legacy key")
            })
            .collect::<Vec<_>>();
        assert_eq!(imported_pre_action_keys, snapshot.pre_action_order);

        let assassinate_lane = entity
            .template
            .skills
            .skills()
            .iter()
            .enumerate()
            .position(|(lane, _)| {
                entity.template.skills.fixed_lane_key_at(lane) == Some(BuiltinActiveSkill::Assassinate.legacy_key())
            })
            .expect("runtime v2 d8c6 loadout should contain assassinate");
        let expected_level = snapshot
            .entries
            .iter()
            .find(|entry| entry.key == BuiltinActiveSkill::Assassinate.legacy_key())
            .map(|entry| entry.level)
            .expect("legacy d8c6 loadout should contain assassinate");
        assert_eq!(entity.template.skills.level_at(assassinate_lane), Some(expected_level));
    }
}

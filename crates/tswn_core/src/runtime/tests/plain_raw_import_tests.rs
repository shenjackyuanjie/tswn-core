use super::*;

#[test]
fn case_d8c6_import_preserves_plain_pre_action_order_and_assassinate_levels() {
    let raw = "最光辉的时刻 #8ftphKKCk@Shabby_fish\n营救任务 #tmOaPuIoM@Shabby_fish";
    let input = crate::namerena::NamerenaInput::parse(raw).unwrap();
    let prepared = crate::namerena::PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ).unwrap();
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let runner =
        RuntimeRunner::from_custom_mixed_namerena_raw(raw.to_owned(), config).expect("runtime d8c6 runner should construct");

    for entity_index in 0..2 {
        let snapshot = &prepared.players[entity_index].skills;
        let entity = runner
            .runtime()
            .entities
            .get(EntityIdx(entity_index as u32))
            .expect("runtime d8c6 player should exist");
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
            .expect("runtime d8c6 loadout should contain assassinate");
        let expected_level = snapshot
            .entries
            .iter()
            .find(|entry| entry.key == BuiltinActiveSkill::Assassinate.legacy_key())
            .map(|entry| entry.level)
            .expect("native d8c6 loadout should contain assassinate");
        assert_eq!(entity.template.skills.level_at(assassinate_lane), Some(expected_level));
    }
}

#[test]
fn case_large_67_import_builds_plain_summon_blueprint_with_static_child_skills() {
    let raw = "Stupefy #rkISERW8@Shabby_fish\n日落·日出 #Pd3J7shds@Shabby_fish";
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let runner =
        RuntimeRunner::from_custom_mixed_namerena_raw(raw.to_owned(), config).expect("runtime summon runner should build");
    let owner = runner.runtime.entities.get(EntityIdx(1)).expect("summon owner should exist");
    let blueprint_slot = runner
        .runtime
        .registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT)
        .expect("core summon blueprint slot should exist");
    let SlotValue::PlayerTemplate(blueprint) = owner.slots.get(blueprint_slot).expect("summon owner should carry a blueprint")
    else {
        panic!("core summon blueprint slot should contain a player template");
    };

    assert_eq!(blueprint.display_name, "使魔");
    let kind = runner
        .runtime
        .registry
        .player_kind(blueprint.kind)
        .expect("core summon kind should exist");
    assert!(kind.flags.contains(PlayerKindFlags::SUMMON));
    assert!(kind.flags.contains(PlayerKindFlags::MINION));
    let exports = blueprint
        .skills
        .skills()
        .iter()
        .map(|skill| runner.runtime.registry.skill(*skill).unwrap().export_name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        exports,
        vec![
            BuiltinActiveSkill::Fire.export_name(),
            BuiltinActiveSkill::Fire.export_name(),
            DEFAULT_CORE_SUMMON_EXPLODE_SKILL_EXPORT,
            DEFAULT_CORE_SUMMON_SHARE_DAMAGE_SKILL_EXPORT,
        ]
    );
    let mut active_skills = blueprint.skills.active_order()[..3].to_vec();
    active_skills.sort_unstable();
    assert_eq!(active_skills, vec![0, 1, 2]);
    assert_eq!(blueprint.skills.active_order()[3], 3);
    assert_eq!(
        blueprint.skills.fixed_lane_key_at(3),
        Some(crate::namerena::SUMMON_SHARE_DAMAGE_SKILL_KEY)
    );
}

#[test]
fn raw_import_minion_blueprints_keep_clone_build_metadata() {
    let raw = concat!(
        r#"owner@red+ol:{"attrs":[86,86,86,86,86,86,86,300],"skills":{"sklshadow":10,"sklsummon":10,"sklzombie":10}}"#,
        "\n",
        "target@blue",
    );
    let config = default_custom_runtime_import_config().expect("default runtime profile should build");
    let runner = RuntimeRunner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
        .expect("runtime minion blueprint runner should build");
    let owner = runner.runtime.entities.get(EntityIdx(0)).expect("minion owner should exist");
    assert!(owner.template.clone_build.is_some());

    for (slot_export, expected_name) in [
        (DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT, "幻影"),
        (DEFAULT_CORE_SUMMON_BLUEPRINT_ENTITY_EXPORT, "使魔"),
        (DEFAULT_CORE_ZOMBIE_BLUEPRINT_ENTITY_EXPORT, "丧尸"),
    ] {
        let slot = runner
            .runtime
            .registry
            .entity_slot_id_by_export_name(slot_export)
            .expect("core minion blueprint slot should exist");
        let SlotValue::PlayerTemplate(blueprint) = owner.slots.get(slot).expect("owner should carry minion blueprint") else {
            panic!("core minion blueprint slot should contain a player template");
        };

        assert_eq!(blueprint.display_name, expected_name);
        assert!(
            blueprint.clone_build.is_some(),
            "{expected_name} blueprint should keep clone build metadata"
        );
    }
}

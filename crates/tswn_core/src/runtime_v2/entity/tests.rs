use super::*;

#[test]
fn entity_records_start_with_empty_state_store() {
    let arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3)]);

    assert!(arena.get(EntityIdx(0)).unwrap().states.entries().is_empty());
    assert_eq!(arena.get(EntityIdx(0)).unwrap().states.hook_mask(), ProcMask::default());
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.kind, PlayerTemplate::DEFAULT_KIND);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.owner, EntityIdx(0));
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.root_owner, EntityIdx(0));
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.team, 0);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.flags, PlayerKindFlags::NONE);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.policies, PlayerKindPolicies::default());
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.move_state, MoveState::default());
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.attack, 3);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.magic, 0);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.magic_point, 0);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.wisdom, 0);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.agility, 0);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.attr_sum, 0);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.atk_sum, 3);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.attract(), 32768.0);
    assert_eq!(
        arena.get(EntityIdx(0)).unwrap().runtime.at_boost_millionths,
        DEFAULT_AT_BOOST_MILLIONTHS
    );
    assert!(arena.get(EntityIdx(0)).unwrap().template.skills.is_empty());
}

#[test]
fn arena_battle_reset_restores_hot_state_and_discards_spawned_entities() {
    let registry = ExtensionRegistry::default();
    let mut arena =
        EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 100, 30).with_magic(40)], &registry);
    let prepared = arena.clone();

    let entity = arena.get_mut(EntityIdx(0)).unwrap();
    entity.template.attack = 999;
    entity.template.team = 3;
    entity.runtime.hp = 1;
    entity.runtime.attack = 999;
    entity.states.add_legacy_key(42);
    arena.spawn_from_template(PlayerTemplate::new(2, "spawned", 1, 10, 1), &registry);

    arena.reset_battle_state_from(&prepared);

    assert_eq!(arena, prepared);
}

#[test]
fn compressed_legacy_states_reserve_registration_order_without_adding_hook_entries() {
    let mut states = StateStore::default();

    assert!(states.register_compressed_legacy_state(CompressedLegacyState::Shield));
    assert!(!states.register_compressed_legacy_state(CompressedLegacyState::Shield));
    assert_eq!(states.post_action_registration_cursor(), 1);
    assert!(states.entries().is_empty());

    assert!(states.add_entry(StateEntry::legacy(123)));
    assert_eq!(states.runtime_registration_order(123), Some(1));
    assert_eq!(states.post_action_registration_cursor(), 2);

    assert!(states.clear_compressed_legacy_state(CompressedLegacyState::Shield));
    assert!(states.register_compressed_legacy_state(CompressedLegacyState::Shield));
    assert_eq!(states.post_action_registration_cursor(), 3);
}

#[test]
fn player_template_carries_magic_point_into_runtime() {
    let arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_magic_point(96)]);

    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.magic_point, 96);
}

#[test]
fn player_template_carries_wisdom_into_runtime() {
    let arena = EntityArena::from_templates(vec![PlayerTemplate::new(1, "left", 0, 10, 3).with_wisdom(77)]);

    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.wisdom, 77);
}

#[test]
fn player_template_carries_target_score_stats_into_runtime() {
    let arena = EntityArena::from_templates(vec![
        PlayerTemplate::new(1, "left", 0, 10, 3).with_target_score_stats(42, 17, 1234.5),
    ]);
    let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;

    assert_eq!(runtime.attr_sum, 42);
    assert_eq!(runtime.atk_sum, 17);
    assert_eq!(runtime.attract(), 1234.5);
}

#[test]
fn player_runtime_get_at_matches_legacy_rng_formula_for_magic() {
    let arena = EntityArena::from_templates(vec![
        PlayerTemplate::new(1, "left", 0, 10, 3)
            .with_magic(80)
            .with_at_boost_millionths(1_500_000),
    ]);
    let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;
    let mut rng = RC4::default();
    let mut expected_rng = RC4::default();

    let a = {
        let mut temp = [
            expected_rng.r127() as i32,
            expected_rng.r127() as i32,
            expected_rng.r127() as i32,
            80 + 64,
            80,
        ];
        temp.sort_unstable();
        temp[2] as f64
    };
    let b = {
        let mut temp = [expected_rng.r63() as i32 + 64, expected_rng.r63() as i32 + 64, 80 + 64];
        temp.sort_unstable();
        temp[1] as f64
    };
    let expected = a * b * 1.5;

    assert_eq!(runtime.get_at(true, &mut rng), expected);
    assert_eq!(rng.i, expected_rng.i);
    assert_eq!(rng.j, expected_rng.j);
    assert_eq!(rng.main_val, expected_rng.main_val);
}

#[test]
fn player_runtime_dodge_matches_legacy_rng_formula() {
    let cases = [(64, 64), (200, 0), (0, 256), (80, 512)];

    for (accuracy, dodge_value) in cases {
        let mut runtime_rng = RC4::default();
        let mut legacy_rng = RC4::default();

        for _ in 0..8 {
            assert_eq!(
                PlayerRuntime::dodge(accuracy, dodge_value, &mut runtime_rng),
                crate::player::Player::dodge(accuracy, dodge_value, &mut legacy_rng)
            );
            assert_eq!(runtime_rng.i, legacy_rng.i);
            assert_eq!(runtime_rng.j, legacy_rng.j);
            assert_eq!(runtime_rng.main_val, legacy_rng.main_val);
        }
    }
}

#[test]
fn player_template_carries_move_state_into_runtime() {
    let template = PlayerTemplate::new(1, "left", 0, 10, 3).with_speed_points(2048);

    let arena = EntityArena::from_templates(vec![template.clone()]);

    assert_eq!(arena.get(EntityIdx(0)).unwrap().template.move_state, template.move_state);
    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.move_state, template.move_state);
}

#[test]
fn player_template_stores_registered_skill_loadout() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill(
            "custom",
            "fire",
            "custom.fire",
            crate::runtime_v2::TargetPolicy::Enemy,
            SkillPriority(3),
        )
        .expect("skill should register");
    let registry = builder.build();
    let template = PlayerTemplate::new(1, "left", 0, 10, 3).with_skills([skill]);

    let arena = EntityArena::from_templates_with_registry(vec![template], &registry);

    assert_eq!(arena.get(EntityIdx(0)).unwrap().template.skills.skills(), &[skill]);
}

#[test]
fn skill_loadout_tracks_fixed_lanes_and_active_order_separately() {
    let loadout = SkillLoadout::from_skills([SkillId(1), SkillId(2), SkillId(3)])
        .with_active_order([2, 0, 1])
        .with_post_damage_order([1, 2, 0]);

    assert_eq!(loadout.skills(), &[SkillId(1), SkillId(2), SkillId(3)]);
    assert_eq!(loadout.active_order(), &[2, 0, 1]);
    assert_eq!(loadout.post_damage_order(), &[1, 2, 0]);
}

#[test]
fn skill_loadout_caches_only_builtin_action_lanes_and_invalidates_with_order() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let custom = builder
        .register_skill(
            "custom",
            "action",
            "custom.action",
            crate::runtime_v2::TargetPolicy::Enemy,
            SkillPriority(0),
        )
        .expect("自定义技能应注册成功");
    let fire = builder
        .register_skill(
            "core",
            crate::runtime_v2::BuiltinActiveSkill::Fire.local_name(),
            crate::runtime_v2::BuiltinActiveSkill::Fire.export_name(),
            crate::runtime_v2::TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("内置技能应注册成功");
    let registry = builder.build();
    let mut loadout = SkillLoadout::from_skill_levels([(custom, 7), (fire, 9)]).with_active_order([0, 1]);

    loadout.prepare_hook_cache(&registry);
    let cached = loadout.cached_builtin_actions().expect("准备后应提供主动技能缓存");
    assert_eq!(cached.len(), 1);
    assert_eq!(usize::from(cached[0].fixed_lane), 1);
    assert_eq!(cached[0].skill, crate::runtime_v2::BuiltinActiveSkill::Fire);

    loadout.disable_action_lane(1);
    assert!(loadout.cached_builtin_actions().is_none());
    loadout.prepare_hook_cache(&registry);
    assert!(loadout.cached_builtin_actions().expect("重建后应恢复缓存").is_empty());
}

#[test]
fn skill_loadout_rebuilds_clone_levels_from_build_baseline_before_boosts() {
    let mut owner = SkillLoadout::from_skill_levels_and_boosts([
        (SkillId(1), 4, None),
        (SkillId(2), 96, None),
        (SkillId(3), 0, None),
        (SkillId(4), 92, Some(crate::player::skill::SkillBoost::LastBoost(46))),
        (
            SkillId(5),
            70,
            Some(crate::player::skill::SkillBoost::SlotBoost { base: 40, boost: 30 }),
        ),
    ]);
    assert!(owner.set_level_at(0, 10));
    assert!(owner.set_level_at(1, 100));
    assert!(owner.set_level_at(2, 12));
    assert!(owner.set_level_at(3, 40));
    assert!(owner.set_level_at(4, 20));

    let clone = owner.rebuilt_for_clone();

    assert_eq!(clone.levels(), &[4, 96, 0, 80, 40]);
    assert_eq!(clone.build_level_at(0), Some(4));
    assert_eq!(clone.build_level_at(3), Some(46));
    assert_eq!(clone.build_level_at(4), Some(40));
}

#[test]
fn skill_loadout_clone_rebuild_keeps_zero_build_lane_out_of_action_order_when_disabled() {
    let mut clone = SkillLoadout::from_skill_levels([(SkillId(1), 0), (SkillId(2), 4)]).with_active_order([0, 1]);

    assert!(clone.set_level_at(0, 3));
    clone.disable_action_lane(0);

    assert_eq!(clone.levels(), &[3, 4]);
    assert_eq!(clone.active_order(), &[1]);
    assert_eq!(clone.build_level_at(0), Some(0));
}

#[test]
fn skill_loadout_resets_each_dirty_field_group_from_battle_baseline() {
    let prepared = SkillLoadout::from_skill_levels([(SkillId(1), 5), (SkillId(2), 0), (SkillId(3), 3), (SkillId(4), 4)])
        .with_active_order([0, 1, 2, 3])
        .with_pre_action_order([0])
        .with_post_damage_order([0, 1, 2, 3])
        .with_post_action_after_states([(1, 0)]);
    let baseline_generation = prepared.hook_generation();

    let mut levels = prepared.clone();
    assert!(levels.set_level_at(0, 2));
    assert_ne!(levels.hook_generation(), baseline_generation);
    levels.reset_battle_fields_from(&prepared);
    assert_eq!(levels, prepared);
    assert_eq!(levels.hook_generation(), baseline_generation);

    let mut active_hooks = prepared.clone();
    active_hooks.disable_action_lane(3);
    active_hooks.reset_battle_fields_from(&prepared);
    assert_eq!(active_hooks, prepared);

    let mut deferred = prepared.clone();
    deferred.register_post_action_after_states(2, 3);
    deferred.reset_battle_fields_from(&prepared);
    assert_eq!(deferred, prepared);

    let mut pre_action = prepared.clone();
    pre_action.remove_pre_action_lane(0);
    pre_action.ensure_pre_action_lane(2);
    assert_eq!(pre_action.hook_generation(), baseline_generation);
    pre_action.reset_battle_fields_from(&prepared);
    assert_eq!(pre_action, prepared);

    let mut boosts = prepared.clone();
    assert!(boosts.boost_last_active_except_key(usize::MAX));
    boosts.reset_battle_fields_from(&prepared);
    assert_eq!(boosts, prepared);

    let source = SkillLoadout::from_skill_levels([(SkillId(5), 5), (SkillId(6), 7), (SkillId(7), 3), (SkillId(8), 4)]);
    let mut merged = prepared.clone();
    assert!(merged.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));
    merged.reset_battle_fields_from(&prepared);
    assert_eq!(merged, prepared);
}

#[test]
fn skill_loadout_merges_levels_by_fixed_lane_without_replacing_skill_ids() {
    let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 0), (SkillId(2), 4)])
        .with_fixed_lane_keys([10, 20])
        .with_active_order([1]);
    let source =
        SkillLoadout::from_skill_levels([(SkillId(3), 9), (SkillId(4), 7), (SkillId(5), 11)]).with_fixed_lane_keys([0, 2, 4]);

    assert!(target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));

    assert_eq!(target.skills(), &[SkillId(1), SkillId(2)]);
    assert_eq!(target.levels(), &[9, 7]);
    assert_eq!(target.active_order(), &[1, 0]);
}

#[test]
fn skill_loadout_fixed_lane_merge_ignores_source_skills_outside_legacy_slots() {
    let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 6), (SkillId(2), 0), (SkillId(3), 0), (SkillId(4), 0)]);
    let source = SkillLoadout::from_skill_levels([(SkillId(5), 6), (SkillId(6), 0), (SkillId(7), 0), (SkillId(8), 1)])
        .with_merge_lane_order([0, 1, 2]);

    assert!(!target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));
    assert_eq!(target.levels(), &[6, 0, 0, 0]);
}

#[test]
fn skill_loadout_ignores_unmapped_source_lanes() {
    let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 1), (SkillId(2), 2)]).with_fixed_lane_keys([0, 2]);
    let source = SkillLoadout::from_skill_levels([(SkillId(3), 9), (SkillId(4), 8)]).with_fixed_lane_keys([0, 7]);

    assert!(target.merge_fixed_lanes_from(&source, MergePolicy::DropUnmappedSkills));

    assert_eq!(target.skills(), &[SkillId(1), SkillId(2)]);
    assert_eq!(target.levels(), &[9, 2]);
}

#[test]
fn skill_loadout_moves_newly_enabled_lanes_to_action_order_tail() {
    let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 4), (SkillId(2), 0), (SkillId(3), 7), (SkillId(4), 0)])
        .with_active_order([1, 0, 3, 2]);
    let source = SkillLoadout::from_skill_levels([(SkillId(5), 1), (SkillId(6), 8), (SkillId(7), 2), (SkillId(8), 9)]);

    assert!(target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));

    assert_eq!(target.levels(), &[4, 8, 7, 9]);
    assert_eq!(target.active_order(), &[0, 2, 1, 3]);
    assert_eq!(target.post_damage_order(), &[0, 2, 1, 3]);
}

#[test]
fn skill_loadout_merge_reports_no_change_when_source_level_is_not_higher() {
    let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 5)]);
    let source = SkillLoadout::from_skill_levels([(SkillId(2), 5)]);

    assert!(!target.merge_fixed_lanes_from(&source, MergePolicy::FixedLane));

    assert_eq!(target.skills(), &[SkillId(1)]);
    assert_eq!(target.levels(), &[5]);
}

#[test]
fn skill_loadout_none_merge_policy_keeps_levels_unchanged() {
    let mut target = SkillLoadout::from_skill_levels([(SkillId(1), 1)]);
    let source = SkillLoadout::from_skill_levels([(SkillId(2), 9)]);

    assert!(!target.merge_fixed_lanes_from(&source, MergePolicy::None));
    assert_eq!(target.skills(), &[SkillId(1)]);
    assert_eq!(target.levels(), &[1]);
}

#[test]
fn entity_arena_preserves_skill_loadout_when_spawning() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let skill = builder
        .register_skill(
            "custom",
            "summon-skill",
            "custom.summon_skill",
            crate::runtime_v2::TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("skill should register");
    let registry = builder.build();
    let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);

    let spawned = arena.spawn_from_template(PlayerTemplate::new(2, "spawned", 1, 7, 2).with_skills([skill]), &registry);

    assert_eq!(arena.get(spawned).unwrap().template.skills.skills(), &[skill]);
    assert_eq!(arena.get(spawned).unwrap().runtime.owner, spawned);
    assert_eq!(arena.get(spawned).unwrap().runtime.root_owner, spawned);
}

#[test]
fn entity_arena_preserves_move_state_when_spawning() {
    let registry = ExtensionRegistry::default();
    let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);
    let payload = PlayerTemplate::new(2, "spawned", 1, 7, 2).with_speed_points(-2048);

    let spawned = arena.spawn_from_template(payload.clone(), &registry);

    assert_eq!(arena.get(spawned).unwrap().template.move_state, payload.move_state);
    assert_eq!(arena.get(spawned).unwrap().runtime.move_state, payload.move_state);
}

#[test]
fn entity_records_copy_registered_player_kind_flags_into_runtime() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            crate::runtime_v2::PlayerKindPolicies::default(),
        )
        .expect("kind should register");
    let registry = builder.build();

    let arena =
        EntityArena::from_templates_with_registry(vec![PlayerTemplate::with_kind(1, "summon", kind, 0, 10, 3)], &registry);

    let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;
    assert_eq!(runtime.kind, kind);
    assert_eq!(runtime.team, 0);
    assert!(runtime.flags.contains(PlayerKindFlags::SUMMON));
    assert!(runtime.flags.contains(PlayerKindFlags::MINION));
    assert_eq!(runtime.policies, crate::runtime_v2::PlayerKindPolicies::default());
}

#[test]
fn entity_records_copy_registered_player_kind_policies_into_runtime() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let policies = crate::runtime_v2::PlayerKindPolicies {
        owner_resolution: crate::runtime_v2::OwnerResolutionPolicy::RootOwner,
        damage_share: crate::runtime_v2::DamageSharePolicy::ShareToOwner,
        merge: crate::runtime_v2::MergePolicy::FixedLane,
        inherit_owner_def_res: false,
    };
    let kind = builder
        .register_player_kind_with_policies("custom", "summon", "custom.summon", PlayerKindFlags::SUMMON, policies)
        .expect("kind should register");
    let registry = builder.build();

    let arena =
        EntityArena::from_templates_with_registry(vec![PlayerTemplate::with_kind(1, "summon", kind, 0, 10, 3)], &registry);

    assert_eq!(arena.get(EntityIdx(0)).unwrap().runtime.policies, policies);
}

#[test]
fn player_template_policy_overrides_update_runtime_policy_fields() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let policies = crate::runtime_v2::PlayerKindPolicies {
        owner_resolution: crate::runtime_v2::OwnerResolutionPolicy::RootOwner,
        damage_share: crate::runtime_v2::DamageSharePolicy::ShareToOwner,
        merge: crate::runtime_v2::MergePolicy::FixedLane,
        inherit_owner_def_res: true,
    };
    let kind = builder
        .register_player_kind_with_policies("custom", "summon", "custom.summon", PlayerKindFlags::SUMMON, policies)
        .expect("kind should register");
    let registry = builder.build();

    let arena = EntityArena::from_templates_with_registry(
        vec![
            PlayerTemplate::with_kind(1, "summon", kind, 0, 10, 3)
                .with_damage_share_policy(crate::runtime_v2::DamageSharePolicy::None),
        ],
        &registry,
    );

    let runtime = &arena.get(EntityIdx(0)).unwrap().runtime;
    assert_eq!(
        runtime.policies.owner_resolution,
        crate::runtime_v2::OwnerResolutionPolicy::RootOwner
    );
    assert_eq!(runtime.policies.damage_share, crate::runtime_v2::DamageSharePolicy::None);
    assert_eq!(runtime.policies.merge, crate::runtime_v2::MergePolicy::FixedLane);
    assert!(runtime.policies.inherit_owner_def_res);
}

#[test]
fn entity_arena_uses_policy_overrides_when_spawning() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let kind = builder
        .register_player_kind_with_policies(
            "custom",
            "summon",
            "custom.summon",
            PlayerKindFlags::SUMMON,
            crate::runtime_v2::PlayerKindPolicies {
                owner_resolution: crate::runtime_v2::OwnerResolutionPolicy::SelfEntity,
                damage_share: crate::runtime_v2::DamageSharePolicy::None,
                merge: crate::runtime_v2::MergePolicy::None,
                inherit_owner_def_res: false,
            },
        )
        .expect("kind should register");
    let registry = builder.build();
    let mut arena = EntityArena::from_templates_with_registry(
        vec![PlayerTemplate::new(1, "owner", 0, 10, 3).with_def_res(77, 88)],
        &registry,
    );

    let spawned = arena.spawn_from_template_with_owner(
        PlayerTemplate::with_kind(2, "summon", kind, 0, 7, 2)
            .with_policy_overrides(PlayerPolicyOverrides::default().with_inherit_owner_def_res(true)),
        &registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );

    assert_eq!(arena.get(spawned).unwrap().template.defense, 77);
    assert_eq!(arena.get(spawned).unwrap().template.resistance, 88);
    assert!(arena.get(spawned).unwrap().runtime.policies.inherit_owner_def_res);
}

#[test]
fn entity_arena_spawns_with_owner_and_root_owner_metadata() {
    let registry = ExtensionRegistry::default();
    let mut arena = EntityArena::from_templates_with_registry(
        vec![PlayerTemplate::new(1, "owner", 0, 10, 3).with_identity_names("owner@red", "red")],
        &registry,
    );

    let spawned = arena.spawn_from_template_with_owner(
        PlayerTemplate::new(2, "owner?0", 1, 7, 2),
        &registry,
        Some(EntityIdx(0)),
        Some(EntityIdx(0)),
    );

    let runtime = &arena.get(spawned).unwrap().runtime;
    assert_eq!(runtime.owner, EntityIdx(0));
    assert_eq!(runtime.root_owner, EntityIdx(0));
    assert_eq!(runtime.team, 0);
    assert_eq!(arena.get(spawned).unwrap().template.team, 0);
    assert_eq!(arena.get(spawned).unwrap().template.clan_name, "red");
    assert_eq!(arena.get(spawned).unwrap().template.id_key_name, "owner?0@red");
}

#[test]
fn entity_records_reserve_registered_entity_slots() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let slot = builder
        .reserve_entity_slot("custom", "flag", "custom.flag")
        .expect("entity slot should reserve");
    let registry = builder.build();
    let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);

    arena
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(slot, crate::runtime_v2::SlotValue::Bool(true))
        .unwrap();

    assert_eq!(
        arena.get(EntityIdx(0)).unwrap().slots.get(slot),
        Some(&crate::runtime_v2::SlotValue::Bool(true))
    );
}

#[test]
fn entity_arena_spawns_new_entity_without_reusing_indices() {
    let mut builder = crate::runtime_v2::ExtensionRegistryBuilder::default();
    let slot = builder
        .reserve_entity_slot("custom", "flag", "custom.flag")
        .expect("entity slot should reserve");
    let registry = builder.build();
    let mut arena = EntityArena::from_templates_with_registry(vec![PlayerTemplate::new(1, "left", 0, 10, 3)], &registry);

    let spawned = arena.spawn_from_template(PlayerTemplate::new(2, "spawned", 1, 7, 2), &registry);

    assert_eq!(spawned, EntityIdx(1));
    assert_eq!(arena.len(), 2);
    assert_eq!(arena.get(spawned).unwrap().template.name, "spawned");
    assert_eq!(arena.get(spawned).unwrap().runtime.hp, 7);
    assert_eq!(arena.get(spawned).unwrap().slots.get(slot), None);
}

#[test]
fn state_store_updates_legacy_keys_and_generation() {
    let mut store = StateStore::default();

    assert!(store.add_legacy_key(11));
    assert!(!store.add_legacy_key(11));
    assert_eq!(store.generation(), 1);
    assert_eq!(store.entries(), &[StateEntry::legacy(11)]);
    assert_eq!(store.entry(11), Some(&StateEntry::legacy(11)));

    assert!(store.clear_legacy_key(11));
    assert!(!store.clear_legacy_key(11));
    assert_eq!(store.generation(), 2);
    assert!(store.entries().is_empty());
    assert_eq!(store.entry(11), None);
}

#[test]
fn state_store_rebuilds_dense_index_after_clear() {
    let mut store = StateStore::default();
    store.add_legacy_key(11);
    store.add_legacy_key(22);
    store.add_legacy_key(33);

    assert!(store.clear_legacy_key(22));

    assert_eq!(store.entry(11), Some(&StateEntry::legacy(11)));
    assert_eq!(store.entry(22), None);
    assert_eq!(store.entry(33), Some(&StateEntry::legacy(33)));
}

#[test]
fn state_store_tracks_v2_state_entry_metadata_and_hook_mask() {
    let mut store = StateStore::default();
    let entry = StateEntry {
        legacy_order_key: 42,
        extension_state_id: Some(StateId(3)),
        hook_mask: ProcMask::PRE_ACTION | ProcMask::POST_DAMAGE,
        priority: SkillPriority(9),
        registration_order: RegistrationOrder(4),
        payload: StatePayload::None,
    };

    assert!(store.add_entry(entry.clone()));
    assert!(!store.add_entry(entry.clone()));
    assert_eq!(store.entries(), &[entry.clone()]);
    assert_eq!(store.entry(42), Some(&entry));
    assert_eq!(store.hook_mask(), ProcMask::PRE_ACTION | ProcMask::POST_DAMAGE);

    assert!(store.clear_legacy_key(42));
    assert!(store.entries().is_empty());
    assert_eq!(store.hook_mask(), ProcMask::default());
}

#[test]
fn state_store_orders_entries_by_priority_then_registration() {
    let mut store = StateStore::default();
    let late = StateEntry {
        legacy_order_key: 11,
        extension_state_id: Some(StateId(1)),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(10),
        registration_order: RegistrationOrder(1),
        payload: StatePayload::None,
    };
    let early = StateEntry {
        legacy_order_key: 22,
        extension_state_id: Some(StateId(2)),
        hook_mask: ProcMask::PRE_ACTION,
        priority: SkillPriority(1),
        registration_order: RegistrationOrder(2),
        payload: StatePayload::None,
    };
    let tie = StateEntry {
        legacy_order_key: 33,
        extension_state_id: Some(StateId(3)),
        hook_mask: ProcMask::POST_DAMAGE,
        priority: SkillPriority(10),
        registration_order: RegistrationOrder(3),
        payload: StatePayload::None,
    };

    store.add_entry(late);
    store.add_entry(early);
    store.add_entry(tie);

    assert_eq!(
        store
            .entries_in_hook_order()
            .into_iter()
            .map(|entry| entry.legacy_order_key)
            .collect::<Vec<_>>(),
        vec![22, 11, 33]
    );
    assert_eq!(
        store
            .entries_in_hook_order_for(ProcMask::POST_ACTION)
            .into_iter()
            .map(|entry| entry.legacy_order_key)
            .collect::<Vec<_>>(),
        vec![11]
    );
    assert_eq!(
        store.hook_mask(),
        ProcMask::PRE_ACTION | ProcMask::POST_ACTION | ProcMask::POST_DAMAGE
    );
}

#[test]
fn state_store_uses_hook_specific_priority_for_iron() {
    let mut store = StateStore::default();
    store.add_entry(StateEntry::iron(11, StateId(1), 500, 3, SkillPriority(10)));
    store.add_entry(StateEntry {
        legacy_order_key: 22,
        extension_state_id: Some(StateId(2)),
        hook_mask: ProcMask::POST_ACTION,
        priority: SkillPriority(100),
        registration_order: RegistrationOrder(1),
        payload: StatePayload::None,
    });

    assert_eq!(
        store
            .entries_in_hook_order_for(ProcMask::POST_DEFEND)
            .into_iter()
            .map(|entry| (entry.legacy_order_key, entry.priority_for_hook(ProcMask::POST_DEFEND)))
            .collect::<Vec<_>>(),
        [(11, SkillPriority(10))].into_iter().collect::<Vec<_>>()
    );
    assert_eq!(
        store
            .entries_in_hook_order_for(ProcMask::POST_ACTION)
            .into_iter()
            .map(|entry| (entry.legacy_order_key, entry.priority_for_hook(ProcMask::POST_ACTION)))
            .collect::<Vec<_>>(),
        vec![(22, SkillPriority(100)), (11, SkillPriority(210))]
    );
}

#[test]
fn state_store_tracks_fire_mag_payload_as_half_steps() {
    let mut store = StateStore::default();

    assert_eq!(store.fire_mag(91), 0.0);
    assert!(store.add_entry(StateEntry::fire_mag(91, 3)));

    assert_eq!(store.entry(91).and_then(StateEntry::fire_mag_value), Some(1.5));
    assert_eq!(store.fire_mag(91), 1.5);
}

#[test]
fn state_store_adds_or_increments_fire_mag_half_steps() {
    let mut store = StateStore::default();

    store.add_fire_mag_half_step(91);
    assert_eq!(store.fire_mag(91), 0.5);
    assert_eq!(store.generation(), 1);

    store.add_fire_mag_half_step(91);
    assert_eq!(store.fire_mag(91), 1.0);
    assert_eq!(store.generation(), 2);

    assert!(store.add_legacy_key(22));
    store.add_fire_mag_half_step(22);
    assert_eq!(store.fire_mag(22), 0.5);
}

#[test]
fn state_store_scheduler_flags_follow_payload_changes_conservatively() {
    let mut store = StateStore::default();
    assert_eq!(store.effective_speed(81), 81);
    assert!(!store.is_frozen());

    assert!(store.add_entry(StateEntry::haste(11, StateId(1), 2, 3, SkillPriority(10))));
    assert_eq!(store.effective_speed(81), 162);
    let logical_copy = store.clone();
    let _ = store.entry_mut(11).unwrap();
    assert_eq!(store, logical_copy);
    assert!(store.set_payload(11, StatePayload::FireMagHalfSteps(1)));
    assert_eq!(store.effective_speed(81), 81);

    assert!(store.add_legacy_key(22));
    store.entry_mut(22).unwrap().payload = StatePayload::Ice { frozen_step: 3 };
    assert!(store.is_frozen());
    assert_eq!(store.apply_ice_pre_step(2, 0), (0, false));
    assert!(store.clear_legacy_key(22));
    assert!(!store.is_frozen());
}

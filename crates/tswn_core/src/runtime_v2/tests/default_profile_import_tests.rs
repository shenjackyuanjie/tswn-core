use super::*;

#[test]
fn runtime_v2_custom_import_profile_builds_mixed_raw_with_minion_overlays() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let fire = builder
        .register_skill(
            "custom",
            "summon-fire",
            "custom.summon.fire",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("summon fire skill should register");
    let explode = builder
        .register_skill(
            "custom",
            "summon-explode",
            "custom.summon.explode",
            TargetPolicy::Enemy,
            SkillPriority(2),
        )
        .expect("summon explode skill should register");
    let possess = builder
        .register_skill(
            "custom",
            "possess",
            "custom.minion.possess",
            TargetPolicy::Enemy,
            SkillPriority(3),
        )
        .expect("possess skill should register");
    let zombie_heal = builder
        .register_skill(
            "custom",
            "zombie-heal",
            "custom.minion.heal",
            TargetPolicy::Ally,
            SkillPriority(4),
        )
        .expect("zombie heal skill should register");
    let summon_template_slot = builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
    let shadow_template_slot = builder
        .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
        .expect("bed2 shadow template slot should reserve");
    let zombie_template_slot = builder
        .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
        .expect("bed2 zombie template slot should reserve");
    let bed2 = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2",
            "custom.bed2",
            PlayerKindFlags::BED2,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 kind should register");
    let summon_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2-summon",
            "custom.bed2.summon",
            PlayerKindFlags::SUMMON | PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: true,
            },
        )
        .expect("bed2 summon kind should register");
    let shadow_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2-shadow",
            "custom.bed2.shadow",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 shadow kind should register");
    let zombie_kind = builder
        .register_player_kind_with_policies(
            "custom",
            "bed2-zombie",
            "custom.bed2.zombie",
            PlayerKindFlags::MINION,
            PlayerKindPolicies {
                owner_resolution: OwnerResolutionPolicy::RootOwner,
                damage_share: DamageSharePolicy::ShareToOwner,
                merge: MergePolicy::FixedLane,
                inherit_owner_def_res: false,
            },
        )
        .expect("bed2 zombie kind should register");
    let registry = builder.build();
    let config = CustomRuntimeV2ImportConfig::new(registry, bed2, summon)
        .with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
            summon: CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: "custom.summon.fire",
                explode_skill_export_name: "custom.summon.explode",
            },
            shadow: CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: "custom.minion.possess",
            },
            zombie: CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: "custom.minion",
            },
        })
        .with_skill_handler(summon, skill_noop)
        .with_skill_handler(fire, skill_noop)
        .with_skill_handler(explode, skill_noop)
        .with_skill_handler(possess, skill_noop)
        .with_skill_handler(zombie_heal, skill_noop);
    let raw_input = "plain@red\n\
alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{\"sklheal\":3}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";

    let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw_input.to_owned(), config)
        .expect("custom import profile should construct mixed runner");
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_eq!(
        runner.runtime().entities.get(EntityIdx(0)).unwrap().template.kind,
        PlayerTemplate::DEFAULT_KIND
    );
    assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
    let SlotValue::PlayerTemplate(summon_template) = runner
        .runtime()
        .template_slots
        .get(summon_template_slot)
        .expect("profile import should populate summon template slot")
    else {
        panic!("profile summon overlay slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[1, 0]);

    let SlotValue::PlayerTemplate(shadow_template) = runner
        .runtime()
        .template_slots
        .get(shadow_template_slot)
        .expect("profile import should populate shadow template slot")
    else {
        panic!("profile shadow overlay slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.kind, shadow_kind);
    assert_eq!(shadow_template.skills.skills(), &[possess]);

    let SlotValue::PlayerTemplate(zombie_template) = runner
        .runtime()
        .template_slots
        .get(zombie_template_slot)
        .expect("profile import should populate zombie template slot")
    else {
        panic!("profile zombie overlay slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.kind, zombie_kind);
    assert_eq!(zombie_template.skills.skills(), &[zombie_heal]);
}

#[test]
fn runtime_v2_custom_import_profile_wraps_missing_overlay_skill_errors() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    builder
        .register_skill(
            "custom",
            "summon-fire",
            "custom.summon.fire",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("summon fire skill should register");
    builder
        .register_skill(
            "custom",
            "summon-explode",
            "custom.summon.explode",
            TargetPolicy::Enemy,
            SkillPriority(2),
        )
        .expect("summon explode skill should register");
    let summon_template_slot = builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
    let shadow_template_slot = builder
        .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
        .expect("bed2 shadow template slot should reserve");
    let zombie_template_slot = builder
        .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
        .expect("bed2 zombie template slot should reserve");
    let bed2 = builder
        .register_player_kind("custom", "bed2", "custom.bed2")
        .expect("bed2 kind should register");
    let summon_kind = builder
        .register_player_kind("custom", "bed2-summon", "custom.bed2.summon")
        .expect("bed2 summon kind should register");
    let shadow_kind = builder
        .register_player_kind("custom", "bed2-shadow", "custom.bed2.shadow")
        .expect("bed2 shadow kind should register");
    let zombie_kind = builder
        .register_player_kind("custom", "bed2-zombie", "custom.bed2.zombie")
        .expect("bed2 zombie kind should register");
    let registry = builder.build();
    let config =
        CustomRuntimeV2ImportConfig::new(registry, bed2, summon).with_bed2_minion_overlays(CustomBed2MinionOverlayConfig {
            summon: CustomBed2SummonTemplateConfig {
                template_slot: summon_template_slot,
                summon_kind,
                fire_skill_export_name: "custom.summon.fire",
                explode_skill_export_name: "custom.summon.explode",
            },
            shadow: CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: "custom.minion.possess",
            },
            zombie: CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: "custom.minion",
            },
        });

    let err = RuntimeV2Runner::from_custom_mixed_namerena_raw(
        r#"alpha@red@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"sklpossess":5}}}"#.to_owned(),
        config,
    )
    .expect_err("custom profile should wrap missing shadow possess export");

    assert_eq!(
        err,
        CustomRuntimeV2ImportError::Bed2MinionOverlay(CustomBed2MinionOverlayImportError::Shadow(
            CustomBed2ShadowTemplateImportError::MissingSkillExportName {
                export_name: "custom.minion.possess".to_owned(),
            }
        ))
    );
}

#[test]
fn default_custom_runtime_v2_profile_builds_mixed_raw_runner() {
    let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
    let bed2 = config.bed2_kind;
    let summon = config.bed2_summon_skill;
    let overlays = config
        .bed2_minion_overlays
        .expect("default custom profile should install bed2 minion overlay import");
    assert_eq!(
        config
            .registry
            .skill_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_SKILL_EXPORT)
            .map(|spec| spec.id),
        Some(summon)
    );
    let fire = config
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_FIRE_SKILL_EXPORT)
        .expect("default profile should register summon fire export");
    let explode = config
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_BED2_SUMMON_EXPLODE_SKILL_EXPORT)
        .expect("default profile should register summon explode export");
    let possess = config
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT)
        .expect("default profile should register minion possess export");
    assert_eq!(config.registry.skill(possess).unwrap().hook_mask, ProcMask::NONE);
    assert_eq!(config.registry.player_kind(bed2).unwrap().export_name, "custom.bed2");
    assert_eq!(overlays.summon.template_slot, TemplateSlotId(0));
    assert_eq!(
        config.registry.player_kind(overlays.summon.summon_kind).unwrap().export_name,
        DEFAULT_CUSTOM_BED2_SUMMON_KIND_EXPORT
    );
    assert_eq!(
        config.registry.player_kind(overlays.shadow.shadow_kind).unwrap().export_name,
        DEFAULT_CUSTOM_BED2_SHADOW_KIND_EXPORT
    );
    assert_eq!(
        config.registry.player_kind(overlays.zombie.zombie_kind).unwrap().export_name,
        DEFAULT_CUSTOM_BED2_ZOMBIE_KIND_EXPORT
    );

    let raw_input = "plain@red\n\
alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";
    let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw_input.to_owned(), config)
        .expect("default custom profile should construct mixed runner");
    let legacy = crate::Runner::new_from_namerena_raw(raw_input.to_owned()).expect("legacy runner should construct");

    assert_runtime_world_matches_legacy_raw_world(runner.runtime(), &legacy.world);
    assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.kind, bed2);
    assert_eq!(
        runner.runtime().entities.get(EntityIdx(1)).unwrap().template.skills.skills(),
        &[summon]
    );
    let SlotValue::PlayerTemplate(summon_template) = runner
        .runtime()
        .template_slots
        .get(overlays.summon.template_slot)
        .expect("default profile should populate summon template slot")
    else {
        panic!("default profile summon overlay slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.kind, overlays.summon.summon_kind);
    assert_eq!(summon_template.max_hp, 123);
    assert_eq!(summon_template.policy_overrides.inherit_owner_def_res, Some(true));
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[1, 0]);

    let SlotValue::PlayerTemplate(shadow_template) = runner
        .runtime()
        .template_slots
        .get(overlays.shadow.template_slot)
        .expect("default profile should populate shadow template slot")
    else {
        panic!("default profile shadow overlay slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.kind, overlays.shadow.shadow_kind);
    assert_eq!(shadow_template.max_hp, 88);
    assert_eq!(shadow_template.skills.skills(), &[possess]);

    let SlotValue::PlayerTemplate(zombie_template) = runner
        .runtime()
        .template_slots
        .get(overlays.zombie.template_slot)
        .expect("default profile should populate zombie template slot")
    else {
        panic!("default profile zombie overlay slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.kind, overlays.zombie.zombie_kind);
    assert_eq!(zombie_template.max_hp, 77);
    assert!(zombie_template.skills.skills().is_empty());
    let zombie_heal = runner
        .runtime()
        .registry
        .skill_id_by_export_name("custom.minion.heal")
        .expect("default profile should register minion heal export");
    assert!(runner.runtime().skill_handlers.get(zombie_heal).is_none());
}

#[test]
fn default_profile_imports_plain_defend_skill_level_from_legacy_loadout() {
    let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
    let defend = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_DEFEND_SKILL_EXPORT)
        .expect("default profile should register core defend skill");
    let raw = "left@red\n\nright@blue\n";
    let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
        .expect("plain raw should construct runtime v2 runner");
    let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("plain raw should construct legacy runner");
    let snapshot = legacy
        .storage
        .get_player(&1)
        .expect("right legacy player should exist")
        .skill_loadout_snapshot();
    let defend_kind = std::any::type_name::<crate::player::skill::defend::DefendSkill>();
    let expected_level = snapshot
        .entries
        .iter()
        .find(|entry| entry.runtime_kind == defend_kind)
        .map(|entry| entry.level)
        .expect("right legacy player should have DefendSkill");
    let right = runner.runtime().entities.get(EntityIdx(1)).expect("right runtime v2 player should exist");

    let defend_lane = right
        .template
        .skills
        .skills()
        .iter()
        .position(|skill| *skill == defend)
        .unwrap_or_else(|| panic!("runtime v2 loadout should contain DefendSkill; legacy snapshot: {snapshot:?}"));
    assert_eq!(right.template.skills.level_at(defend_lane), Some(expected_level));
}

#[test]
fn default_profile_imports_plain_merge_kill_hook_from_legacy_loadout() {
    let raw = "我力 7#W2ib8D@仙蛊屋+123\n\
                   万我 68#huMG43@仙蛊屋+123\n\n\
                   Dianmu YKFMWRPXIMCQ@nan+234\n\
                   Freddy FVNXBNVTWJEA@nan+234\n\n\
                   seed:第十八届武术大赛小组赛第8组:307-3@!\n";
    let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("large_51 raw should construct legacy runner");
    let snapshot = legacy
        .storage
        .get_player(&0)
        .expect("large_51 merge owner should exist")
        .skill_loadout_snapshot();
    let merge_kind = std::any::type_name::<crate::player::skill::merge::MergeSkill>();
    let expected_level = snapshot
        .entries
        .iter()
        .find(|entry| entry.runtime_kind == merge_kind)
        .map(|entry| entry.level)
        .expect("large_51 owner should have MergeSkill");

    let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
    let merge = config
        .registry
        .skill_id_by_export_name(DEFAULT_CORE_MERGE_SKILL_EXPORT)
        .expect("default profile should register core merge skill");
    assert_eq!(config.registry.skill(merge).unwrap().hook_mask, ProcMask::KILL);
    let runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
        .expect("large_51 raw should construct runtime v2 runner");
    let owner = runner
        .runtime()
        .entities
        .get(EntityIdx(0))
        .expect("large_51 runtime v2 owner should exist");
    let merge_lane = owner
        .template
        .skills
        .skills()
        .iter()
        .position(|skill| *skill == merge)
        .unwrap_or_else(|| panic!("runtime v2 loadout should contain MergeSkill; legacy snapshot: {snapshot:?}"));
    assert_eq!(owner.template.skills.level_at(merge_lane), Some(expected_level));
    assert!(runner.runtime().skill_handlers.get(merge).is_some());

    let plan = runner.runtime().scheduler.skill_hook_plan(
        &runner.runtime().entities,
        &runner.runtime().registry,
        EntityIdx(0),
        ProcMask::KILL,
    );
    assert!(plan.entries.iter().any(|entry| entry.skill_id == merge && entry.fixed_lane == merge_lane));
}

#[test]
fn builtin_active_skill_semantic_exports_round_trip_legacy_keys() {
    let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");

    for skill in BuiltinActiveSkill::ALL {
        assert_eq!(BuiltinActiveSkill::from_legacy_key(skill.legacy_key()), Some(skill));
        assert_eq!(BuiltinActiveSkill::from_export_name(skill.export_name()), Some(skill));
        let registered = config
            .registry
            .skill_id_by_export_name(skill.export_name())
            .unwrap_or_else(|| panic!("default profile should register {}", skill.export_name()));
        assert_eq!(config.registry.skill(registered).unwrap().export_name, skill.export_name());
    }

    assert_eq!(BuiltinActiveSkill::from_legacy_key(BuiltinActiveSkill::ALL.len()), None);
    assert_eq!(BuiltinActiveSkill::from_export_name("core.skill.24"), None);
}

#[test]
fn default_profile_imports_and_executes_plain_shadow_blueprint() {
    let raw = "我力 7#W2ib8D@仙蛊屋+123\n\
                   万我 68#huMG43@仙蛊屋+123\n\n\
                   Dianmu YKFMWRPXIMCQ@nan+234\n\
                   Freddy FVNXBNVTWJEA@nan+234\n\n\
                   seed:第十八届武术大赛小组赛第8组:307-3@!\n";
    let legacy = crate::Runner::new_from_namerena_raw(raw.to_owned()).expect("large_51 raw should construct legacy runner");
    let legacy_owner = legacy.storage.get_player(&0).expect("large_51 shadow owner should exist");
    let snapshot = legacy_owner.skill_loadout_snapshot();
    let shadow_kind = std::any::type_name::<crate::player::skill::act::shadow::ShadowSkill>();
    let expected_level = snapshot
        .entries
        .iter()
        .find(|entry| entry.runtime_kind == shadow_kind)
        .map(|entry| entry.level)
        .expect("large_51 owner should have ShadowSkill");
    let legacy_shadow = crate::player::skill::act::shadow::build_shadow_minion(0, &legacy.storage);
    let legacy_shadow_status = legacy_shadow.get_status();
    let legacy_shadow_snapshot = legacy_shadow.skill_loadout_snapshot();
    let possess_kind = std::any::type_name::<crate::player::skill::act::possess::PossessSkill>();
    let expected_possess_level = legacy_shadow_snapshot
        .entries
        .iter()
        .find(|entry| entry.runtime_kind == possess_kind)
        .map(|entry| entry.level)
        .expect("large_51 shadow should have PossessSkill");

    let config = default_custom_runtime_v2_import_config().expect("default custom runtime v2 profile should build");
    let shadow = config
        .registry
        .skill_id_by_export_name(BuiltinActiveSkill::Shadow.export_name())
        .expect("default profile should register core shadow skill");
    let possess = config
        .registry
        .skill_id_by_export_name(DEFAULT_CUSTOM_MINION_POSSESS_SKILL_EXPORT)
        .expect("default profile should register minion possess skill");
    assert_eq!(config.registry.skill(possess).unwrap().hook_mask, ProcMask::NONE);
    let blueprint_slot = config
        .registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_SHADOW_BLUEPRINT_ENTITY_EXPORT)
        .expect("default profile should register core shadow blueprint slot");
    let counter_slot = config
        .registry
        .entity_slot_id_by_export_name(DEFAULT_CORE_MINION_COUNTER_ENTITY_EXPORT)
        .expect("default profile should register core minion counter slot");
    let mut runner = RuntimeV2Runner::from_custom_mixed_namerena_raw(raw.to_owned(), config)
        .expect("large_51 raw should construct runtime v2 runner");
    let owner = runner.runtime().entities.get(EntityIdx(0)).expect("runtime v2 shadow owner should exist");
    let shadow_lane = owner
        .template
        .skills
        .skills()
        .iter()
        .position(|skill| *skill == shadow)
        .expect("runtime v2 owner should import ShadowSkill");
    assert_eq!(owner.template.skills.level_at(shadow_lane), Some(expected_level));
    let SlotValue::PlayerTemplate(blueprint) = owner
        .slots
        .get(blueprint_slot)
        .expect("runtime v2 owner should store a per-owner shadow blueprint")
    else {
        panic!("runtime v2 shadow blueprint slot should hold PlayerTemplate");
    };
    assert_eq!(blueprint.name, legacy_shadow.id_name());
    assert_eq!(blueprint.display_name, legacy_shadow.display_name());
    assert_eq!(blueprint.max_hp, legacy_shadow_status.max_hp);
    assert_eq!(blueprint.attack, legacy_shadow_status.attack);
    assert_eq!(blueprint.magic_point, legacy_shadow_status.magic_point);
    assert_eq!(blueprint.move_state.speed_points, legacy_shadow.move_point());
    let possess_lane = blueprint
        .skills
        .skills()
        .iter()
        .position(|skill| *skill == possess)
        .expect("runtime v2 shadow blueprint should import PossessSkill");
    assert_eq!(blueprint.skills.level_at(possess_lane), Some(expected_possess_level));
    assert!(blueprint.skills.active_order().contains(&possess_lane));
    let blueprint_skills = blueprint.skills.clone();

    let initial_entity_count = runner.runtime().entities.len();
    let owner_name = owner.template.name.clone();
    let round = runner.run_round_normalized();

    assert_eq!(
        round.frames.iter().map(|frame| frame.message.as_str()).collect::<Vec<_>>(),
        vec!["[0]使用[幻术]", "召唤出[1]", "\n"]
    );
    let owner = runner.runtime().entities.get(EntityIdx(0)).unwrap();
    assert_eq!(
        owner.template.skills.level_at(shadow_lane),
        Some(expected_level.saturating_mul(3).div_ceil(4).max(1))
    );
    assert_eq!(owner.slots.get(counter_slot), Some(&SlotValue::U64(1)));
    let spawned_idx = EntityIdx(initial_entity_count.try_into().unwrap());
    let spawned = runner
        .runtime()
        .entities
        .get(spawned_idx)
        .expect("ShadowSkill should spawn one shadow entity");
    assert_eq!(spawned.template.name, format!("{owner_name}?0"));
    assert_eq!(spawned.template.display_name, "幻影");
    assert_eq!(spawned.runtime.owner, EntityIdx(0));
    assert_eq!(spawned.runtime.root_owner, EntityIdx(0));
    assert_eq!(spawned.runtime.magic_point, legacy_shadow_status.magic_point);
    assert_eq!(spawned.template.skills.skills(), blueprint_skills.skills());
    assert_eq!(spawned.template.skills.level_at(possess_lane), Some(expected_possess_level));
    assert!(
        runner
            .runtime()
            .scheduler
            .skill_hook_plan(
                &runner.runtime().entities,
                &runner.runtime().registry,
                spawned_idx,
                ProcMask::PRE_ACTION,
            )
            .entries
            .is_empty()
    );
}

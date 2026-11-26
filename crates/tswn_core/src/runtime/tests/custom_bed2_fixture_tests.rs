//! 自定义 bed2 fixture 与混合 roster 导入：槽位映射、标记解析与桥接。
use super::*;

#[test]
fn custom_bed2_fixture_maps_kind_skill_and_marker_slots() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill_with_hooks(
            "custom",
            "summon",
            "custom.summon",
            ProcMask::PRE_ACTION,
            TargetPolicy::Enemy,
            SkillPriority(0),
        )
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
    let summon_template = builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
    let hp_marker = builder
        .reserve_entity_slot("custom", "hp-marker", "custom.hp_marker")
        .expect("hp marker slot should reserve");
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
    let registry = builder.build();
    let mut template = PreparedCombatTemplate::with_registry(
        vec![
            PlayerTemplate::with_kind(1, "bed2", bed2, 0, 3000, 0)
                .with_def_res(DEFAULT_BED2_DEFENSE, DEFAULT_BED2_RESISTANCE)
                .with_skills([summon]),
        ],
        registry,
    );
    let bed2_summon_template = PlayerTemplate::with_kind(2, "bed2?0", summon_kind, 0, 1000, 1)
        .with_def_res(99, 99)
        .with_skills([fire, explode]);
    template
        .slots
        .set(
            summon_template,
            SlotValue::PlayerTemplate(Box::new(bed2_summon_template.clone())),
        )
        .expect("bed2 summon template slot should write");

    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        summon,
        skill_bed2_template_slot_summon_handler,
        &[ExtensionCapability::ReadTemplateSlots],
    );
    runtime
        .entities
        .get_mut(EntityIdx(0))
        .unwrap()
        .slots
        .set(hp_marker, SlotValue::Bool(true))
        .expect("hp marker slot should write");
    let entity = runtime.entities.get(EntityIdx(0)).expect("bed2 entity should exist");

    assert_eq!(entity.template.max_hp, 3000);
    assert_eq!(entity.template.skills.skills(), &[summon]);
    assert!(entity.runtime.flags.contains(PlayerKindFlags::BED2));
    assert_eq!(entity.runtime.policies.owner_resolution, OwnerResolutionPolicy::RootOwner);
    assert_eq!(entity.runtime.policies.damage_share, DamageSharePolicy::ShareToOwner);
    assert_eq!(entity.runtime.policies.merge, MergePolicy::FixedLane);
    assert_eq!(entity.slots.get(hp_marker), Some(&SlotValue::Bool(true)));
    assert_eq!(
        runtime.template_slots.get(summon_template),
        Some(&SlotValue::PlayerTemplate(Box::new(bed2_summon_template.clone())))
    );
    let SlotValue::PlayerTemplate(stored_template) =
        runtime.template_slots.get(summon_template).expect("bed2 summon template should persist")
    else {
        panic!("bed2 summon template slot should hold a PlayerTemplate payload");
    };
    assert_eq!(stored_template.kind, summon_kind);
    assert_eq!(stored_template.max_hp, 1000);
    assert_eq!(stored_template.defense, DEFAULT_BED2_DEFENSE);
    assert_eq!(stored_template.resistance, DEFAULT_BED2_RESISTANCE);
    assert_eq!(stored_template.skills.skills(), &[fire, explode]);

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("bed2 summon handler should spawn template payload");

    assert_eq!(runtime.entities.len(), 2);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "出现一个新的[1]");
    assert_eq!(frame.updates.updates[0].target, 1);
    let summoned = runtime.entities.get(EntityIdx(1)).expect("bed2 summon should spawn from template slot");
    assert_eq!(summoned.template.kind, summon_kind);
    assert_eq!(summoned.template.max_hp, 1000);
    assert_eq!(summoned.template.skills.skills(), &[fire, explode]);
    assert_eq!(summoned.runtime.owner, EntityIdx(0));
    assert_eq!(summoned.runtime.root_owner, EntityIdx(0));
    assert_eq!(summoned.runtime.defense, DEFAULT_BED2_DEFENSE);
    assert_eq!(summoned.runtime.resistance, DEFAULT_BED2_RESISTANCE);
}

#[test]
fn push_summon_from_template_slot_reports_missing_template_payload() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill_with_hooks(
            "custom",
            "summon",
            "custom.summon",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon skill should register");
    builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
    let registry = builder.build();
    let mut runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(
        vec![PlayerTemplate::new(1, "bed2", 0, 3000, 0).with_skills([summon])],
        registry,
    ));
    runtime.set_skill_handler_with_capabilities(
        summon,
        skill_records_missing_template_slot_error,
        &[ExtensionCapability::ReadTemplateSlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("missing template payload should be recorded");

    assert_eq!(runtime.entities.len(), 1);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "missing summon template");
}

#[test]
fn push_summon_from_template_slot_can_emit_legacy_summon_message() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill_with_hooks(
            "custom",
            "summon",
            "custom.summon",
            ProcMask::PRE_ACTION,
            TargetPolicy::None,
            SkillPriority(0),
        )
        .expect("summon skill should register");
    let summon_template = builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
    let registry = builder.build();
    let mut template =
        PreparedCombatTemplate::with_registry(vec![PlayerTemplate::new(1, "bed2", 0, 3000, 0).with_skills([summon])], registry);
    let payload = PlayerTemplate::new(2, "bed2?0", 0, 1000, 1).with_skills([summon]);
    template
        .slots
        .set(summon_template, SlotValue::PlayerTemplate(Box::new(payload.clone())))
        .expect("bed2 summon template slot should write");
    let mut runtime = CombatRuntime::from_template(template);
    runtime.set_skill_handler_with_capabilities(
        summon,
        skill_bed2_template_slot_legacy_summon_handler,
        &[ExtensionCapability::ReadTemplateSlots],
    );

    let frame = runtime
        .run_skill_hooks(EntityIdx(0), ProcMask::PRE_ACTION)
        .expect("bed2 summon handler should spawn template payload");

    assert_eq!(runtime.entities.len(), 2);
    assert_eq!(frame.updates.updates.len(), 1);
    assert_eq!(frame.updates.updates[0].message, "召唤出[1]");
    assert_eq!(frame.updates.updates[0].target, 1);
    let summoned = runtime.entities.get(EntityIdx(1)).expect("summon should spawn");
    assert_eq!(summoned.template.name, payload.name);
    assert_eq!(summoned.template.skills.skills(), payload.skills.skills());
    assert_eq!(summoned.runtime.owner, EntityIdx(0));
    assert_eq!(summoned.runtime.root_owner, EntityIdx(0));
}

#[test]
fn custom_bed2_import_fixture_parses_markers_into_runtime_template() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
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
    let registry = builder.build();
    let plus = CustomBed2Import::parse("alpha@red+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}")
        .expect("bed2 plus marker should parse");
    let legacy_team = CustomBed2Import::parse("beta@blue@bed2").expect("legacy bed2 team marker should parse");
    let bare = CustomBed2Import::parse("gamma+bed2[2500]").expect("bare bed2 marker should parse");

    assert_eq!(plus.name, "alpha");
    assert_eq!(plus.team.as_deref(), Some("red"));
    assert_eq!(plus.hp, 4500);
    assert_eq!(legacy_team.name, "beta");
    assert_eq!(legacy_team.team.as_deref(), Some("blue"));
    assert_eq!(legacy_team.hp, DEFAULT_BED2_HP);
    assert_eq!(bare.name, "gamma");
    assert_eq!(bare.team, None);
    assert_eq!(bare.hp, 2500);
    assert_eq!(CustomBed2Import::parse("alpha@red+bed2[0]"), None);

    let facade_bridge =
        CustomBed2Import::parse_player_facade_raw("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}")
            .expect("bed2 raw should bridge through player facade id name");
    assert_eq!(
        crate::namerena::raw_namerena_to_id_name("alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}"),
        "alpha@red"
    );
    assert_eq!(facade_bridge.name, "alpha");
    assert_eq!(facade_bridge.team.as_deref(), Some("red"));
    assert_eq!(facade_bridge.hp, 4500);

    let same_team_bridge = CustomBed2Import::parse_player_facade_raw("same@same+bed2[1800]")
        .expect("same-team bed2 raw should bridge through normalized player facade id name");
    assert_eq!(crate::namerena::raw_namerena_to_id_name("same@same+bed2[1800]"), "same");
    assert_eq!(same_team_bridge.name, "same");
    assert_eq!(same_team_bridge.team, None);
    assert_eq!(same_team_bridge.hp, 1800);

    let template = plus.into_player_template(1, bed2, 0, summon);
    let runtime = CombatRuntime::from_template(PreparedCombatTemplate::with_registry(vec![template], registry));
    let entity = runtime.entities.get(EntityIdx(0)).expect("bed2 entity should import");

    assert_eq!(entity.template.name, "alpha");
    assert_eq!(entity.template.max_hp, 4500);
    assert_eq!(entity.template.attack, 0);
    assert_eq!(entity.template.defense, DEFAULT_BED2_DEFENSE);
    assert_eq!(entity.template.resistance, DEFAULT_BED2_RESISTANCE);
    assert_eq!(entity.template.skills.skills(), &[summon]);
    assert!(entity.runtime.flags.contains(PlayerKindFlags::BED2));
}

#[test]
fn custom_bed2_roster_import_rejects_non_bed2_players() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
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
    let registry = builder.build();
    let raw_groups = vec![vec!["alpha+bed2[4500]".to_owned()], vec!["plain".to_owned()]];

    let err = CustomBed2Import::roster_into_prepared_template(&raw_groups, registry, bed2, summon)
        .expect_err("non-bed2 raw players should be rejected by the bed2 roster importer");

    assert_eq!(
        err,
        CustomBed2RosterImportError {
            team_index: 1,
            player_index: 0,
            raw: "plain".to_owned(),
        }
    );
}

#[test]
fn custom_mixed_roster_import_bridges_bed2_and_legacy_player_templates() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
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
    let registry = builder.build();
    let raw_groups = vec![
        vec!["plain@red".to_owned(), "alpha@red+bed2[4500]".to_owned()],
        vec!["seed:custom-seed@!".to_owned(), "beta@blue@bed2".to_owned()],
    ];

    let template = CustomBed2Import::mixed_roster_into_prepared_template(&raw_groups, registry, bed2, summon)
        .expect("mixed native/bed2 raw roster should build a prepared template");
    let input = crate::namerena::NamerenaInput::parse("plain@red").unwrap();
    let plain = crate::namerena::PreparedRoster::build(&input, crate::namerena::eval_name::DEFAULT_EVAL_RQ)
        .unwrap()
        .players
        .remove(0);

    assert_eq!(template.players.len(), 3);
    assert_eq!(template.players[0].id, 1);
    assert_eq!(template.players[0].name, plain.name);
    assert_eq!(template.players[0].kind, PlayerTemplate::DEFAULT_KIND);
    assert_eq!(template.players[0].team, 0);
    assert_eq!(template.players[0].max_hp, plain.status.max_hp);
    assert_eq!(template.players[0].attack, plain.status.attack);
    assert_eq!(template.players[0].defense, plain.status.defense);
    assert_eq!(template.players[0].resistance, plain.status.resistance);
    assert_eq!(template.players[1].id, 2);
    assert_eq!(template.players[1].kind, bed2);
    assert_eq!(template.players[1].name, "alpha");
    assert_eq!(template.players[1].team, 0);
    assert_eq!(template.players[1].max_hp, 4500);
    assert_eq!(template.players[1].skills.skills(), &[summon]);
    assert_eq!(template.players[2].id, 3);
    assert_eq!(template.players[2].kind, bed2);
    assert_eq!(template.players[2].name, "beta");
    assert_eq!(template.players[2].team, 1);
    assert_eq!(template.players[2].max_hp, DEFAULT_BED2_HP);

    let runtime = CombatRuntime::from_template(template);
    assert!(!runtime.entities.get(EntityIdx(0)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
    assert!(runtime.entities.get(EntityIdx(1)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
    assert!(runtime.entities.get(EntityIdx(2)).unwrap().runtime.flags.contains(PlayerKindFlags::BED2));
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0), EntityIdx(1)].as_slice()));
    assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(2)].as_slice()));
}

#[test]
fn custom_mixed_roster_import_exports_bed2_ol_summon_overlay_to_template_slot() {
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
    let summon_template_slot = builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
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
    let registry = builder.build();
    let raw_groups = vec![
            vec![
                "plain@red".to_owned(),
                "alpha@red+bed2[4500]".to_owned(),
            ],
            vec![
                "seed:custom-seed@!".to_owned(),
                r#"beta@blue@bed2+ol:{"summon":{"attrs":[46,47,48,49,50,51,52,123],"skills":{"sklexplode":3,"sklfire1":5},"inherit_owner_def_res":true}}"#.to_owned(),
            ],
        ];

    let template = CustomBed2Import::mixed_roster_into_prepared_template_with_summon_overlay(
        &raw_groups,
        registry,
        bed2,
        summon,
        CustomBed2SummonTemplateConfig {
            template_slot: summon_template_slot,
            summon_kind,
            fire_skill_export_name: "custom.summon.fire",
            explode_skill_export_name: "custom.summon.explode",
        },
    )
    .expect("mixed roster should carry bed2 summon overlay into template slot");

    assert_eq!(template.players.len(), 3);
    assert_eq!(template.players[0].kind, PlayerTemplate::DEFAULT_KIND);
    assert_eq!(template.players[1].kind, bed2);
    assert_eq!(template.players[2].kind, bed2);
    let SlotValue::PlayerTemplate(summon_template) = template
        .slots
        .get(summon_template_slot)
        .expect("mixed roster summon overlay should populate template slot")
    else {
        panic!("mixed roster summon overlay slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.name, "beta?0");
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.team, 1);
    assert_eq!(summon_template.max_hp, 123);
    assert_eq!(summon_template.attack, 10);
    assert_eq!(summon_template.defense, 11);
    assert_eq!(summon_template.resistance, 15);
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[2, 0]);
    assert_eq!(summon_template.policy_overrides.inherit_owner_def_res, Some(true));
}

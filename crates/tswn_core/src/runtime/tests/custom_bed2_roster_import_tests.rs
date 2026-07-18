use super::*;

#[test]
fn custom_bed2_roster_import_builds_prepared_template_from_grouped_raw_players() {
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
        vec![
            "alpha@red+weapon+bed2[4500]+ol:{\"skills\":{\"sklsummon\":255}}".to_owned(),
            "seed:custom-seed@!".to_owned(),
        ],
        vec!["beta@blue@bed2".to_owned(), "same@same+bed2[1800]".to_owned()],
    ];

    let template = CustomBed2Import::roster_into_prepared_template(&raw_groups, registry, bed2, summon)
        .expect("grouped bed2 raw roster should build a prepared template");

    assert_eq!(template.players.len(), 3);
    assert_eq!(template.players[0].id, 1);
    assert_eq!(template.players[0].name, "alpha");
    assert_eq!(template.players[0].team, 0);
    assert_eq!(template.players[0].max_hp, 4500);
    assert_eq!(template.players[0].skills.skills(), &[summon]);
    assert_eq!(template.players[1].id, 2);
    assert_eq!(template.players[1].name, "beta");
    assert_eq!(template.players[1].team, 1);
    assert_eq!(template.players[1].max_hp, DEFAULT_BED2_HP);
    assert_eq!(template.players[2].id, 3);
    assert_eq!(template.players[2].name, "same");
    assert_eq!(template.players[2].team, 1);
    assert_eq!(template.players[2].max_hp, 1800);
    assert!(template.players.iter().all(|player| player.kind == bed2
        && player.attack == 0
        && player.defense == DEFAULT_BED2_DEFENSE
        && player.resistance == DEFAULT_BED2_RESISTANCE));

    let runtime = CombatRuntime::from_template(template);
    assert_eq!(runtime.world.team_alive(0), Some([EntityIdx(0)].as_slice()));
    assert_eq!(runtime.world.team_alive(1), Some([EntityIdx(1), EntityIdx(2)].as_slice()));
    assert!(
        runtime
            .entities
            .iter()
            .all(|(_, entity)| entity.runtime.flags.contains(PlayerKindFlags::BED2))
    );
}

#[test]
fn custom_bed2_roster_import_exports_ol_summon_overlay_to_template_slot() {
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
                "alpha@red+bed2[4500]".to_owned(),
                "seed:custom-seed@!".to_owned(),
            ],
            vec![r#"beta@blue@bed2+ol:{"summon":{"attrs":[36,86,56,55,36,89,88,89],"skills":{"sklfire2":4,"sklexplode":3,"sklfire1":"2*14"},"reuse_skills_on_recast":true,"inherit_owner_def_res":true}}"#.to_owned()],
        ];

    let template = CustomBed2Import::roster_into_prepared_template_with_summon_overlay(
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
    .expect("bed2 roster with summon overlay should build prepared template");

    assert_eq!(template.players.len(), 2);
    assert_eq!(template.players[0].name, "alpha");
    assert_eq!(template.players[0].skills.skills(), &[summon]);
    let SlotValue::PlayerTemplate(summon_template) = template
        .slots
        .get(summon_template_slot)
        .expect("summon overlay should populate template slot")
    else {
        panic!("summon overlay slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.name, "beta?0");
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.team, 1);
    assert_eq!(summon_template.max_hp, 89);
    assert_eq!(summon_template.attack, 0);
    assert_eq!(summon_template.defense, 50);
    assert_eq!(summon_template.resistance, 53);
    assert_eq!(summon_template.agility, 19);
    assert_eq!(summon_template.magic, 0);
    assert_eq!(summon_template.wisdom, 52);
    assert_eq!(summon_template.magic_point, 26);
    assert_eq!(summon_template.move_state.speed_points, 180);
    assert_eq!(summon_template.policy_overrides.inherit_owner_def_res, Some(true));
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[1, 2, 0]);
}

#[test]
fn custom_bed2_summon_overlay_import_rejects_missing_skill_export_name() {
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
    let summon_template_slot = builder
        .reserve_template_slot("custom", "bed2-summon-template", "custom.bed2.summon_template")
        .expect("bed2 summon template slot should reserve");
    let bed2 = builder
        .register_player_kind("custom", "bed2", "custom.bed2")
        .expect("bed2 kind should register");
    let summon_kind = builder
        .register_player_kind("custom", "bed2-summon", "custom.bed2.summon")
        .expect("bed2 summon kind should register");
    let registry = builder.build();
    let raw_groups = vec![vec![
        r#"beta@blue@bed2+ol:{"summon":{"attrs":[36,86,56,55,36,89,88,89],"skills":{"sklfire1":5}}}"#.to_owned(),
    ]];

    let err = CustomBed2Import::roster_into_prepared_template_with_summon_overlay(
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
    .expect_err("missing explode skill export should reject parser-facing import");

    assert_eq!(fire, SkillId(1));
    assert_eq!(
        err,
        CustomBed2SummonTemplateImportError::MissingSkillExportName {
            export_name: "custom.summon.explode".to_owned(),
        }
    );
}

#[test]
fn custom_bed2_roster_import_exports_ol_shadow_overlay_to_template_slot() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let possess = builder
        .register_skill(
            "custom",
            "possess",
            "custom.minion.possess",
            TargetPolicy::Enemy,
            SkillPriority(1),
        )
        .expect("possess skill should register");
    let shadow_template_slot = builder
        .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
        .expect("bed2 shadow template slot should reserve");
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
    let registry = builder.build();
    let raw_groups = vec![
        vec!["alpha@red+bed2[4500]".to_owned()],
        vec![r#"beta@blue@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"sklpossess":5}}}"#.to_owned()],
    ];

    let template = CustomBed2Import::roster_into_prepared_template_with_shadow_overlay(
        &raw_groups,
        registry,
        bed2,
        summon,
        CustomBed2ShadowTemplateConfig {
            template_slot: shadow_template_slot,
            shadow_kind,
            possess_skill_export_name: "custom.minion.possess",
        },
    )
    .expect("bed2 roster with shadow overlay should build prepared template");

    assert_eq!(template.players.len(), 2);
    assert_eq!(template.players[0].skills.skills(), &[summon]);
    let SlotValue::PlayerTemplate(shadow_template) = template
        .slots
        .get(shadow_template_slot)
        .expect("shadow overlay should populate template slot")
    else {
        panic!("shadow overlay slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.name, "beta?shadow");
    assert_eq!(shadow_template.kind, shadow_kind);
    assert_eq!(shadow_template.team, 1);
    assert_eq!(shadow_template.max_hp, 88);
    assert_eq!(shadow_template.attack, 11);
    assert_eq!(shadow_template.defense, 12);
    assert_eq!(shadow_template.resistance, 16);
    assert_eq!(shadow_template.agility, 14);
    assert_eq!(shadow_template.magic, 15);
    assert_eq!(shadow_template.wisdom, 17);
    assert_eq!(shadow_template.magic_point, 8);
    assert_eq!(shadow_template.move_state.speed_points, -2048);
    assert_eq!(shadow_template.skills.skills(), &[possess]);
    assert_eq!(shadow_template.skills.active_order(), &[0]);
}

#[test]
fn custom_bed2_shadow_overlay_import_rejects_missing_skill_export_name() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let shadow_template_slot = builder
        .reserve_template_slot("custom", "bed2-shadow-template", "custom.bed2.shadow_template")
        .expect("bed2 shadow template slot should reserve");
    let bed2 = builder
        .register_player_kind("custom", "bed2", "custom.bed2")
        .expect("bed2 kind should register");
    let shadow_kind = builder
        .register_player_kind("custom", "bed2-shadow", "custom.bed2.shadow")
        .expect("bed2 shadow kind should register");
    let registry = builder.build();
    let raw_groups = vec![vec![
        r#"beta@blue@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"sklpossess":5}}}"#.to_owned(),
    ]];

    let err = CustomBed2Import::roster_into_prepared_template_with_shadow_overlay(
        &raw_groups,
        registry,
        bed2,
        summon,
        CustomBed2ShadowTemplateConfig {
            template_slot: shadow_template_slot,
            shadow_kind,
            possess_skill_export_name: "custom.minion.possess",
        },
    )
    .expect_err("missing possess skill export should reject parser-facing import");

    assert_eq!(
        err,
        CustomBed2ShadowTemplateImportError::MissingSkillExportName {
            export_name: "custom.minion.possess".to_owned(),
        }
    );
}

#[test]
fn custom_bed2_roster_import_exports_ol_zombie_overlay_to_template_slot() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let zombie_template_slot = builder
        .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
        .expect("bed2 zombie template slot should reserve");
    let zombie_heal = builder
        .register_skill(
            "custom",
            "zombie-heal",
            "custom.minion.heal",
            TargetPolicy::Ally,
            SkillPriority(1),
        )
        .expect("zombie heal skill should register");
    let zombie_explode = builder
        .register_skill(
            "custom",
            "zombie-explode",
            "custom.minion.explode",
            TargetPolicy::Enemy,
            SkillPriority(2),
        )
        .expect("zombie explode skill should register");
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
    let raw_groups = vec![
        vec!["alpha@red+bed2[4500]".to_owned()],
        vec![
            r#"beta@blue@bed2+ol:{"zombie":{"attrs":[46,47,48,49,50,51,52,77],"skills":{"sklheal":3,"sklexplode":4}}}"#
                .to_owned(),
        ],
    ];

    let template = CustomBed2Import::roster_into_prepared_template_with_zombie_overlay(
        &raw_groups,
        registry,
        bed2,
        summon,
        CustomBed2ZombieTemplateConfig {
            template_slot: zombie_template_slot,
            zombie_kind,
            skill_export_name_prefix: "custom.minion",
        },
    )
    .expect("bed2 roster with zombie overlay should build prepared template");

    assert_eq!(template.players.len(), 2);
    assert_eq!(template.players[0].skills.skills(), &[summon]);
    let SlotValue::PlayerTemplate(zombie_template) = template
        .slots
        .get(zombie_template_slot)
        .expect("zombie overlay should populate template slot")
    else {
        panic!("zombie overlay slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.name, "beta?zombie");
    assert_eq!(zombie_template.kind, zombie_kind);
    assert_eq!(zombie_template.team, 1);
    assert_eq!(zombie_template.max_hp, 77);
    assert_eq!(zombie_template.attack, 10);
    assert_eq!(zombie_template.defense, 11);
    assert_eq!(zombie_template.resistance, 15);
    assert_eq!(zombie_template.agility, 13);
    assert_eq!(zombie_template.magic, 14);
    assert_eq!(zombie_template.wisdom, 16);
    assert_eq!(zombie_template.magic_point, 8);
    assert_eq!(zombie_template.move_state.speed_points, 0);
    assert_eq!(zombie_template.skills.skills(), &[zombie_heal, zombie_explode]);
    assert_eq!(zombie_template.skills.active_order(), &[0, 1]);
}

#[test]
fn custom_bed2_roster_import_exports_all_ol_minion_overlays_to_template_slots() {
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
    let raw_groups = vec![
            vec![
                r#"alpha@red@bed2+ol:{"summon":{"attrs":[46,47,48,49,50,51,52,123],"skills":{"sklfire2":4,"sklfire1":5},"inherit_owner_def_res":true}}"#.to_owned(),
                r#"beta@red@bed2+ol:{"shadow":{"attrs":[47,48,49,50,51,52,53,88],"skills":{"phantom:sklpossess":5}}}"#.to_owned(),
                r#"gamma@red@bed2+ol:{"zombie":{"attrs":[46,47,48,49,50,51,52,77],"skills":{"sklheal":3}}}"#.to_owned(),
            ],
            vec!["delta@blue+bed2[8]".to_owned()],
        ];

    let template = CustomBed2Import::roster_into_prepared_template_with_minion_overlays(
        &raw_groups,
        registry,
        bed2,
        summon,
        CustomBed2MinionOverlayConfig {
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
        },
    )
    .expect("combined minion overlay import should build prepared template");

    assert_eq!(template.players.len(), 4);
    let SlotValue::PlayerTemplate(summon_template) = template
        .slots
        .get(summon_template_slot)
        .expect("combined import should populate summon template slot")
    else {
        panic!("summon overlay slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.name, "alpha?0");
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.max_hp, 123);
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[1, 0]);

    let SlotValue::PlayerTemplate(shadow_template) = template
        .slots
        .get(shadow_template_slot)
        .expect("combined import should populate shadow template slot")
    else {
        panic!("shadow overlay slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.name, "beta?shadow");
    assert_eq!(shadow_template.kind, shadow_kind);
    assert_eq!(shadow_template.max_hp, 88);
    assert_eq!(shadow_template.move_state.speed_points, -2048);
    assert_eq!(shadow_template.skills.skills(), &[possess]);
    assert_eq!(shadow_template.skills.active_order(), &[0]);

    let SlotValue::PlayerTemplate(zombie_template) = template
        .slots
        .get(zombie_template_slot)
        .expect("combined import should populate zombie template slot")
    else {
        panic!("zombie overlay slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.name, "gamma?zombie");
    assert_eq!(zombie_template.kind, zombie_kind);
    assert_eq!(zombie_template.max_hp, 77);
    assert_eq!(zombie_template.move_state.speed_points, 0);
    assert_eq!(zombie_template.skills.skills(), &[zombie_heal]);
    assert_eq!(zombie_template.skills.active_order(), &[0]);
}

#[test]
fn custom_bed2_zombie_overlay_import_rejects_missing_skill_export_name() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
    let zombie_template_slot = builder
        .reserve_template_slot("custom", "bed2-zombie-template", "custom.bed2.zombie_template")
        .expect("bed2 zombie template slot should reserve");
    let bed2 = builder
        .register_player_kind("custom", "bed2", "custom.bed2")
        .expect("bed2 kind should register");
    let zombie_kind = builder
        .register_player_kind("custom", "bed2-zombie", "custom.bed2.zombie")
        .expect("bed2 zombie kind should register");
    let registry = builder.build();
    let raw_groups = vec![vec![
        r#"beta@blue@bed2+ol:{"zombie":{"attrs":[46,47,48,49,50,51,52,77],"skills":{"sklheal":3}}}"#.to_owned(),
    ]];

    let err = CustomBed2Import::roster_into_prepared_template_with_zombie_overlay(
        &raw_groups,
        registry,
        bed2,
        summon,
        CustomBed2ZombieTemplateConfig {
            template_slot: zombie_template_slot,
            zombie_kind,
            skill_export_name_prefix: "custom.minion",
        },
    )
    .expect_err("missing zombie skill export should reject parser-facing import");

    assert_eq!(
        err,
        CustomBed2ZombieTemplateImportError::MissingSkillExportName {
            export_name: "custom.minion.heal".to_owned(),
        }
    );
}

use super::*;

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

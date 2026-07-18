use super::*;

#[test]
fn runtime_runner_constructs_and_runs_mixed_roster() {
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
        vec!["plain@red".to_owned(), "alpha@red+bed2[9]".to_owned()],
        vec!["seed:custom-seed@!".to_owned(), "beta@blue@bed2".to_owned()],
    ];

    let mut runner = RuntimeRunner::from_mixed_roster(&raw_groups, registry, bed2, summon)
        .expect("mixed roster should construct a runtime runner");
    runner.runtime_mut().set_skill_handler(summon, skill_noop);

    assert_eq!(
        runner.runtime().world.team_alive(0),
        Some([EntityIdx(0), EntityIdx(1)].as_slice())
    );
    assert_eq!(runner.runtime().world.team_alive(1), Some([EntityIdx(2)].as_slice()));
    assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.max_hp, 9);
    assert!(
        runner
            .runtime()
            .entities
            .get(EntityIdx(1))
            .unwrap()
            .runtime
            .flags
            .contains(PlayerKindFlags::BED2)
    );

    let actor_attack = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.attack;
    let plain_hp = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.max_hp;
    let plain_mp = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.magic_point;
    let plain_defense = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.defense;
    let plain_resistance = runner.runtime().entities.get(EntityIdx(0)).unwrap().template.resistance;

    let actual = runner.run_round_normalized();
    let expected = NormalizedOutcome {
        winner_team: None,
        round: 1,
        total_score: actor_attack as u64,
        rng: crate::runtime::NormalizedRngCheckpoint::after_next_u8(1),
        entity_ids: vec![1, 2, 3],
        teams: vec![0, 0, 1],
        hp: vec![plain_hp, 9, DEFAULT_BED2_HP - actor_attack],
        magic_point: vec![plain_mp, 0, 0],
        defense: vec![plain_defense, 99, 99],
        resistance: vec![plain_resistance, 99, 99],
        alive: vec![true, true, true],
        round_order: vec![0, 1, 2],
        flat_alive: vec![0, 1, 2],
        team_alive: vec![vec![0, 1], vec![2]],
        alive_group_count: 2,
        actions: vec![crate::runtime::NormalizedActionBoundary {
            round: 1,
            actor: 0,
            target: 2,
            amount: actor_attack,
        }],
        frames: vec![NormalizedUpdateFrame {
            message: "[0]攻击[1]".to_owned(),
            caster: 0,
            target: 2,
            targets: Vec::new(),
            param: None,
            score: actor_attack as u32,
            delay0: crate::runtime::update::DEFAULT_DELAY0_MS,
            delay1: crate::runtime::update::DEFAULT_DELAY1_MS,
            update_type: crate::runtime::update::UpdateType::None,
        }],
    };

    assert_eq!(actual, expected);
}

#[test]
fn runtime_runner_constructs_from_bed2_namerena_raw_fixture_shape() {
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
    let raw_input = "alpha@red+bed2[5]\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

    let runner = prepared_init_tests::runtime_runner_from_raw(raw_input, |raw_groups| {
        RuntimeRunner::from_bed2_roster(raw_groups, registry, bed2, summon)
    });
    assert_eq!(runner.runtime().entities.len(), 2);
    assert_eq!(runner.runtime().entities.get(EntityIdx(0)).unwrap().template.max_hp, 5);
    assert_eq!(runner.runtime().entities.get(EntityIdx(1)).unwrap().template.max_hp, 8);
}

#[test]
fn runtime_runner_bed2_raw_can_import_ol_summon_overlay_template_slot() {
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
    let raw_input = "alpha@red+bed2[5]+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5}}}\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

    let runner = prepared_init_tests::runtime_runner_from_raw(raw_input, |raw_groups| {
        RuntimeRunner::from_bed2_roster_with_summon_overlay(
            raw_groups,
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
    });
    let SlotValue::PlayerTemplate(summon_template) = runner
        .runtime()
        .template_slots
        .get(summon_template_slot)
        .expect("runner should preserve imported summon template slot")
    else {
        panic!("runner summon template slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.name, "alpha?0");
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.max_hp, 123);
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[1, 0]);
}

#[test]
fn runtime_runner_bed2_raw_can_import_ol_shadow_overlay_template_slot() {
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
    let raw_input = "alpha@red+bed2[5]+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

    let runner = prepared_init_tests::runtime_runner_from_raw(raw_input, |raw_groups| {
        RuntimeRunner::from_bed2_roster_with_shadow_overlay(
            raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2ShadowTemplateConfig {
                template_slot: shadow_template_slot,
                shadow_kind,
                possess_skill_export_name: "custom.minion.possess",
            },
        )
    });
    let SlotValue::PlayerTemplate(shadow_template) = runner
        .runtime()
        .template_slots
        .get(shadow_template_slot)
        .expect("runner should preserve imported shadow template slot")
    else {
        panic!("runner shadow template slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.name, "alpha?shadow");
    assert_eq!(shadow_template.kind, shadow_kind);
    assert_eq!(shadow_template.max_hp, 88);
    assert_eq!(shadow_template.move_state.speed_points, -2048);
    assert_eq!(shadow_template.skills.skills(), &[possess]);
    assert_eq!(shadow_template.skills.active_order(), &[0]);
}

#[test]
fn runtime_runner_bed2_raw_can_import_ol_zombie_overlay_template_slot() {
    let mut builder = ExtensionRegistryBuilder::default();
    let summon = builder
        .register_skill("custom", "summon", "custom.summon", TargetPolicy::Enemy, SkillPriority(0))
        .expect("summon skill should register");
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
    let raw_input =
        "alpha@red+bed2[5]+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77]}}\n\nseed:custom-seed@!\n\nbeta@blue+bed2[8]\n";

    let runner = prepared_init_tests::runtime_runner_from_raw(raw_input, |raw_groups| {
        RuntimeRunner::from_bed2_roster_with_zombie_overlay(
            raw_groups,
            registry,
            bed2,
            summon,
            CustomBed2ZombieTemplateConfig {
                template_slot: zombie_template_slot,
                zombie_kind,
                skill_export_name_prefix: "custom.minion",
            },
        )
    });
    let SlotValue::PlayerTemplate(zombie_template) = runner
        .runtime()
        .template_slots
        .get(zombie_template_slot)
        .expect("runner should preserve imported zombie template slot")
    else {
        panic!("runner zombie template slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.name, "alpha?zombie");
    assert_eq!(zombie_template.kind, zombie_kind);
    assert_eq!(zombie_template.max_hp, 77);
    assert_eq!(zombie_template.move_state.speed_points, 0);
    assert!(zombie_template.skills.is_empty());
}

#[test]
fn runtime_runner_bed2_raw_can_import_all_ol_minion_overlay_template_slots() {
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
    let raw_input = "alpha@red@bed2+ol:{\"summon\":{\"attrs\":[46,47,48,49,50,51,52,123],\"skills\":{\"sklfire2\":4,\"sklfire1\":5},\"inherit_owner_def_res\":true}}\n\
beta@red@bed2+ol:{\"shadow\":{\"attrs\":[47,48,49,50,51,52,53,88],\"skills\":{\"phantom:sklpossess\":5}}}\n\
gamma@red@bed2+ol:{\"zombie\":{\"attrs\":[46,47,48,49,50,51,52,77],\"skills\":{\"sklheal\":3}}}\n\n\
seed:custom-seed@!\n\n\
delta@blue+bed2[8]\n";

    let runner = prepared_init_tests::runtime_runner_from_raw(raw_input, |raw_groups| {
        RuntimeRunner::from_bed2_roster_with_minion_overlays(
            raw_groups,
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
    });
    let SlotValue::PlayerTemplate(summon_template) = runner
        .runtime()
        .template_slots
        .get(summon_template_slot)
        .expect("runner should preserve imported summon template slot")
    else {
        panic!("runner summon template slot should hold PlayerTemplate");
    };
    assert_eq!(summon_template.name, "alpha?0");
    assert_eq!(summon_template.kind, summon_kind);
    assert_eq!(summon_template.max_hp, 123);
    assert_eq!(summon_template.skills.skills(), &[fire, fire, explode]);
    assert_eq!(summon_template.skills.active_order(), &[1, 0]);

    let SlotValue::PlayerTemplate(shadow_template) = runner
        .runtime()
        .template_slots
        .get(shadow_template_slot)
        .expect("runner should preserve imported shadow template slot")
    else {
        panic!("runner shadow template slot should hold PlayerTemplate");
    };
    assert_eq!(shadow_template.name, "beta?shadow");
    assert_eq!(shadow_template.kind, shadow_kind);
    assert_eq!(shadow_template.max_hp, 88);
    assert_eq!(shadow_template.skills.skills(), &[possess]);
    assert_eq!(shadow_template.skills.active_order(), &[0]);

    let SlotValue::PlayerTemplate(zombie_template) = runner
        .runtime()
        .template_slots
        .get(zombie_template_slot)
        .expect("runner should preserve imported zombie template slot")
    else {
        panic!("runner zombie template slot should hold PlayerTemplate");
    };
    assert_eq!(zombie_template.name, "gamma?zombie");
    assert_eq!(zombie_template.kind, zombie_kind);
    assert_eq!(zombie_template.max_hp, 77);
    assert_eq!(zombie_template.skills.skills(), &[zombie_heal]);
    assert_eq!(zombie_template.skills.active_order(), &[0]);
}
